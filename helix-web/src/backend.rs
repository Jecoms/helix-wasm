use std::cell::RefCell;
use std::io;

use helix_tui::backend::{ansi, Backend, Buffer};
use helix_tui::buffer::Cell;
use helix_tui::terminal::Config;
use helix_view::graphics::{CursorKind, Rect};
use rs_xterm_js::{
    addons::{fit::FitAddon, webgl::WebglAddon},
    Terminal, TerminalOptions, Theme,
};
use wasm_bindgen::JsCast;

/// Resets colors/attributes, restores the default cursor shape, and shows the
/// cursor. Shared by [`Backend::restore`] and [`Backend::force_restore`] so
/// the two can't drift apart.
const RESET_SEQ: &str = "\x1b[0m\x1b[0 q\x1b[?25h";

thread_local! {
    /// Handle to the xterm.js terminal created by [`spawn_terminal`], so the
    /// receiver-less [`Backend::force_restore`] (called from the panic hook)
    /// can reach the terminal without going through a lifetime-bound writer.
    /// wasm32 is single-threaded, so a thread-local is effectively global.
    static TERMINAL: RefCell<Option<Terminal>> = const { RefCell::new(None) };
}

pub fn spawn_terminal() -> Terminal {
    let theme = Theme::new();
    theme.set_background("#282a36");

    let term_opts = TerminalOptions::new();
    term_opts.set_font_family("Fira Code, monospace");
    term_opts.set_font_size(20);
    term_opts.set_scrollback(0);
    term_opts.set_theme(&theme);
    let terminal = Terminal::new(&term_opts);
    let elem = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .get_element_by_id("terminal")
        .unwrap();
    terminal.open(elem.dyn_into().unwrap());
    let addon = FitAddon::new();
    terminal.load_addon(addon.clone().dyn_into::<FitAddon>().unwrap().into());
    addon.fit();
    let addon = WebglAddon::new(None);
    terminal.load_addon(addon.clone().dyn_into::<WebglAddon>().unwrap().into());
    terminal.focus();
    TERMINAL.with(|cell| {
        if let Ok(mut slot) = cell.try_borrow_mut() {
            // `Terminal` has no direct `Clone` impl (method resolution derefs
            // to `Disposable`), so clone the underlying JS handle and cast.
            *slot = Some(wasm_bindgen::JsValue::from(&terminal).unchecked_into());
        }
    });
    terminal
}

/// A [`Backend`] that renders through any wasm32 [`Buffer`] writer (in
/// practice [`crate::xtct::XtermJsCrosstermBackend`]) by emitting ANSI escape
/// sequences directly.
///
/// crossterm doesn't compile for wasm32-unknown-unknown, so the escape
/// sequences the native `CrosstermBackend` delegates to crossterm are written
/// out by hand, via the host-testable helpers in [`helix_tui::backend::ansi`].
/// xterm.js interprets them on the other side of the writer.
/// Terminal-claiming, raw mode, and mouse capture have no wasm equivalent and
/// are no-ops.
pub struct WebBackend<W: Buffer> {
    buffer: W,
}

impl<W: Buffer> WebBackend<W> {
    pub fn new(buffer: W) -> Self {
        Self { buffer }
    }
}

impl<W: Buffer> Backend for WebBackend<W> {
    fn claim(&mut self, _config: Config) -> io::Result<()> {
        // TODO(wasm32): the terminal Config (mouse capture, keyboard
        // enhancement, …) is ignored; xterm.js is already a dedicated surface
        // with no raw mode or alternate screen. Just start from a clean screen.
        self.clear()?;
        self.buffer.flush()
    }

    fn reconfigure(&mut self, _config: Config) -> io::Result<()> {
        // TODO(wasm32): no-op for the same reason as `claim` — config changes
        // that map to terminal modes have nothing to apply to here.
        Ok(())
    }

    fn restore(&mut self, _config: Config) -> io::Result<()> {
        // Reset colors/attributes and bring the cursor back.
        write!(self.buffer, "{}", RESET_SEQ)?;
        self.buffer.flush()
    }

    fn force_restore() -> io::Result<()> {
        // Runs from the panic hook, so nothing here may panic: tolerate an
        // inaccessible thread-local or a held borrow by doing nothing.
        let _ = TERMINAL.try_with(|cell| {
            if let Ok(slot) = cell.try_borrow() {
                if let Some(terminal) = slot.as_ref() {
                    terminal.write(RESET_SEQ);
                }
            }
        });
        Ok(())
    }

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        ansi::draw(&mut self.buffer, content)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        write!(self.buffer, "\x1b[?25l")?;
        self.buffer.flush()
    }

    fn show_cursor(&mut self, kind: CursorKind) -> io::Result<()> {
        let shape = match kind {
            CursorKind::Block => 2,     // steady block
            CursorKind::Underline => 4, // steady underscore
            CursorKind::Bar => 6,       // steady bar
            CursorKind::Hidden => unreachable!(),
        };
        write!(self.buffer, "\x1b[?25h\x1b[{} q", shape)?;
        self.buffer.flush()
    }

    fn get_cursor(&mut self) -> io::Result<(u16, u16)> {
        Ok((self.buffer.cursor_x(), self.buffer.cursor_y()))
    }

    fn set_cursor(&mut self, x: u16, y: u16) -> io::Result<()> {
        ansi::move_to(&mut self.buffer, x, y)?;
        self.buffer.flush()
    }

    fn clear(&mut self) -> io::Result<()> {
        write!(self.buffer, "\x1b[2J")?;
        self.buffer.flush()
    }

    fn size(&self) -> io::Result<Rect> {
        self.buffer.size()
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buffer.flush()
    }
}
