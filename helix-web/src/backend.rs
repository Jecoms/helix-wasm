use std::io::{self, Write};

use helix_tui::backend::{Backend, Buffer};
use helix_tui::buffer::Cell;
use helix_tui::terminal::Config;
use helix_view::graphics::{Color, CursorKind, Modifier, Rect, UnderlineStyle};
use rs_xterm_js::{
    addons::{fit::FitAddon, webgl::WebglAddon},
    Terminal, TerminalOptions, Theme,
};
use wasm_bindgen::JsCast;

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
    terminal
}

/// A [`Backend`] that renders through any wasm32 [`Buffer`] writer (in
/// practice [`crate::xtct::XtermJsCrosstermBackend`]) by emitting ANSI escape
/// sequences directly.
///
/// crossterm doesn't compile for wasm32-unknown-unknown, so the escape
/// sequences the native `CrosstermBackend` delegates to crossterm are written
/// out by hand here. xterm.js interprets them on the other side of the writer.
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

/// Writes the SGR parameters (no `\x1b[` prefix, no `m` suffix) selecting the
/// given color. `base` is 38 for foreground, 48 for background; the classic
/// palette is offset relative to 30/40.
fn write_color(out: &mut impl Write, color: Color, base: u8) -> io::Result<()> {
    let classic = base - 8; // 30 for fg, 40 for bg
    match color {
        Color::Reset => write!(out, "{}", classic + 9),
        Color::Black => write!(out, "{}", classic),
        Color::Red => write!(out, "{}", classic + 1),
        Color::Green => write!(out, "{}", classic + 2),
        Color::Yellow => write!(out, "{}", classic + 3),
        Color::Blue => write!(out, "{}", classic + 4),
        Color::Magenta => write!(out, "{}", classic + 5),
        Color::Cyan => write!(out, "{}", classic + 6),
        Color::LightGray => write!(out, "{}", classic + 7),
        Color::Gray => write!(out, "{}", classic + 60),
        Color::LightRed => write!(out, "{}", classic + 61),
        Color::LightGreen => write!(out, "{}", classic + 62),
        Color::LightYellow => write!(out, "{}", classic + 63),
        Color::LightBlue => write!(out, "{}", classic + 64),
        Color::LightMagenta => write!(out, "{}", classic + 65),
        Color::LightCyan => write!(out, "{}", classic + 66),
        Color::White => write!(out, "{}", classic + 67),
        Color::Indexed(i) => write!(out, "{};5;{}", base, i),
        Color::Rgb(r, g, b) => write!(out, "{};2;{};{};{}", base, r, g, b),
    }
}

fn set_colors(out: &mut impl Write, fg: Color, bg: Color) -> io::Result<()> {
    write!(out, "\x1b[")?;
    write_color(out, fg, 38)?;
    write!(out, ";")?;
    write_color(out, bg, 48)?;
    write!(out, "m")
}

/// `\x1b[58…m` underline color, colon-separated (see the native backend's
/// `SetUnderlineColor` for why colons rather than semicolons).
fn set_underline_color(out: &mut impl Write, color: Color) -> io::Result<()> {
    match color {
        Color::Reset => write!(out, "\x1b[59m"),
        Color::Rgb(r, g, b) => write!(out, "\x1b[58:2::{}:{}:{}m", r, g, b),
        Color::Indexed(i) => write!(out, "\x1b[58:5:{}m", i),
        classic => {
            // Map the named colors onto the first 16 palette entries.
            let index = match classic {
                Color::Black => 0,
                Color::Red => 1,
                Color::Green => 2,
                Color::Yellow => 3,
                Color::Blue => 4,
                Color::Magenta => 5,
                Color::Cyan => 6,
                Color::LightGray => 7,
                Color::Gray => 8,
                Color::LightRed => 9,
                Color::LightGreen => 10,
                Color::LightYellow => 11,
                Color::LightBlue => 12,
                Color::LightMagenta => 13,
                Color::LightCyan => 14,
                _ => 15, // Color::White
            };
            write!(out, "\x1b[58:5:{}m", index)
        }
    }
}

fn underline_style_param(style: UnderlineStyle) -> &'static str {
    match style {
        UnderlineStyle::Reset => "24",
        UnderlineStyle::Line => "4",
        UnderlineStyle::Curl => "4:3",
        UnderlineStyle::Dotted => "4:4",
        UnderlineStyle::Dashed => "4:5",
        UnderlineStyle::DoubleLine => "4:2",
    }
}

fn move_to(out: &mut impl Write, x: u16, y: u16) -> io::Result<()> {
    // ANSI cursor positions are 1-based.
    write!(out, "\x1b[{};{}H", y + 1, x + 1)
}

fn write_modifier_diff(out: &mut impl Write, from: Modifier, to: Modifier) -> io::Result<()> {
    let removed = from - to;
    if removed.contains(Modifier::REVERSED) {
        write!(out, "\x1b[27m")?;
    }
    if removed.contains(Modifier::BOLD) {
        write!(out, "\x1b[22m")?;
        if to.contains(Modifier::DIM) {
            write!(out, "\x1b[2m")?;
        }
    }
    if removed.contains(Modifier::ITALIC) {
        write!(out, "\x1b[23m")?;
    }
    if removed.contains(Modifier::DIM) {
        write!(out, "\x1b[22m")?;
    }
    if removed.contains(Modifier::CROSSED_OUT) {
        write!(out, "\x1b[29m")?;
    }
    if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) {
        write!(out, "\x1b[25m")?;
    }
    if removed.contains(Modifier::HIDDEN) {
        write!(out, "\x1b[28m")?;
    }

    let added = to - from;
    if added.contains(Modifier::REVERSED) {
        write!(out, "\x1b[7m")?;
    }
    if added.contains(Modifier::BOLD) {
        write!(out, "\x1b[1m")?;
    }
    if added.contains(Modifier::ITALIC) {
        write!(out, "\x1b[3m")?;
    }
    if added.contains(Modifier::DIM) {
        write!(out, "\x1b[2m")?;
    }
    if added.contains(Modifier::CROSSED_OUT) {
        write!(out, "\x1b[9m")?;
    }
    if added.contains(Modifier::SLOW_BLINK) {
        write!(out, "\x1b[5m")?;
    }
    if added.contains(Modifier::RAPID_BLINK) {
        write!(out, "\x1b[6m")?;
    }
    if added.contains(Modifier::HIDDEN) {
        write!(out, "\x1b[8m")?;
    }

    Ok(())
}

impl<W: Buffer> Backend for WebBackend<W> {
    fn claim(&mut self, _config: Config) -> io::Result<()> {
        // No raw mode, alternate screen, or mouse capture on the web: xterm.js
        // is already a dedicated surface. Just start from a clean screen.
        self.clear()?;
        self.buffer.flush()
    }

    fn reconfigure(&mut self, _config: Config) -> io::Result<()> {
        Ok(())
    }

    fn restore(&mut self, _config: Config) -> io::Result<()> {
        // Reset colors/attributes and bring the cursor back.
        write!(self.buffer, "\x1b[0m\x1b[0 q\x1b[?25h")?;
        self.buffer.flush()
    }

    fn force_restore() -> io::Result<()> {
        // There is no global handle on the xterm.js terminal to write to.
        Ok(())
    }

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut fg = Color::Reset;
        let mut bg = Color::Reset;
        let mut underline_color = Color::Reset;
        let mut underline_style = UnderlineStyle::Reset;
        let mut modifier = Modifier::empty();
        let mut last_pos: Option<(u16, u16)> = None;
        for (x, y, cell) in content {
            // Move the cursor if the previous location was not (x - 1, y)
            if !matches!(last_pos, Some(p) if x == p.0 + 1 && y == p.1) {
                move_to(&mut self.buffer, x, y)?;
            }
            last_pos = Some((x, y));
            if cell.modifier != modifier {
                write_modifier_diff(&mut self.buffer, modifier, cell.modifier)?;
                modifier = cell.modifier;
            }
            if cell.fg != fg || cell.bg != bg {
                set_colors(&mut self.buffer, cell.fg, cell.bg)?;
                fg = cell.fg;
                bg = cell.bg;
            }

            if cell.underline_color != underline_color {
                set_underline_color(&mut self.buffer, cell.underline_color)?;
                underline_color = cell.underline_color;
            }

            if cell.underline_style != underline_style {
                write!(
                    self.buffer,
                    "\x1b[{}m",
                    underline_style_param(cell.underline_style)
                )?;
                underline_style = cell.underline_style;
            }

            self.buffer.write_all(cell.symbol.as_bytes())?;
        }

        set_underline_color(&mut self.buffer, Color::Reset)?;
        write!(self.buffer, "\x1b[39m\x1b[49m\x1b[0m")
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
        move_to(&mut self.buffer, x, y)?;
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
