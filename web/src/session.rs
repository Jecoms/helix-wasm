//! The exported JS surface: boot the editor, feed it input, keep its size
//! current. Unstable, internal to the host page (see crate docs).

use std::cell::{Cell, RefCell};
use std::future::{poll_fn, Future};
use std::io::Write;
use std::pin::pin;
use std::task::Poll;

use crate::keys;
use crossterm::bridge;
use crossterm::event::{Event, EventStream};
use crossterm::execute;
use helix_wasm::helix_term::application::Application;
use helix_wasm::helix_term::args::Args;
use helix_wasm::helix_term::config::Config;
use js_sys::{Function, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

thread_local! {
    /// The host page's output callback. Lives in a thread-local because the
    /// bridge's sink is a plain `fn` (it can't capture JS values); wasm32 is
    /// single-threaded, so this is effectively a global.
    static OUTPUT: RefCell<Option<Function>> = const { RefCell::new(None) };
    static STARTED: Cell<bool> = const { Cell::new(false) };
    /// The running editor: `None` before [`start`] and again once helix
    /// exits. Owning the `Application` here — instead of moving it into an
    /// `app.run()` future — is what makes inspection possible: [`drive`]
    /// borrows the cell only inside each poll of the event loop, so whenever
    /// JS runs the cell is unborrowed and [`with_app`] can read editor state
    /// synchronously.
    static APP: RefCell<Option<Application>> = const { RefCell::new(None) };
}

/// Inspection is impossible right now because the editor is mid-poll: the
/// caller is JS invoked from inside the event loop (the `output` callback
/// during a render). Such callers must defer to a microtask.
pub(crate) struct AppBusy;

/// Runs `f` against the live editor. `Ok(None)` means helix is not running
/// (never started, or already exited).
pub(crate) fn with_app<R>(f: impl FnOnce(&Application) -> R) -> Result<Option<R>, AppBusy> {
    APP.with(|cell| match cell.try_borrow() {
        Ok(guard) => Ok(guard.as_ref().map(f)),
        Err(_) => Err(AppBusy),
    })
}

/// The `fn` registered with [`bridge::set_output`]; forwards each flushed
/// ANSI chunk to the host page as a `Uint8Array`.
fn forward_output(bytes: &[u8]) {
    OUTPUT.with(|slot| {
        if let Some(sink) = slot.borrow().as_ref() {
            let chunk = Uint8Array::from(bytes);
            let _ = sink.call1(&JsValue::NULL, &chunk.into());
        }
    });
}

/// Boots the editor. `output` receives `Uint8Array` chunks of ANSI to write
/// to the terminal emulator; `columns`/`rows` are its current dimensions.
///
/// Must be called exactly once per page load, before any other export, and
/// with the real terminal size — the bridge otherwise reports a placeholder
/// 80x24 and helix would lay out against it.
#[wasm_bindgen]
pub fn start(output: Function, columns: u16, rows: u16) -> Result<(), JsValue> {
    if STARTED.with(|started| started.replace(true)) {
        return Err(JsValue::from_str("helix is already running on this page"));
    }

    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);

    OUTPUT.with(|slot| *slot.borrow_mut() = Some(output));
    bridge::set_output(forward_output);
    bridge::set_size(columns, rows);

    // Config and log paths resolve under the stubbed home directory; the
    // reads fail cleanly (no fs on wasm32) and defaults apply. Logs actually
    // land on the JS console via console_log.
    helix_wasm::helix_loader::initialize_config_file(None);
    helix_wasm::helix_loader::initialize_log_file(None);

    // Each language's syntax config is compiled lazily, once, on first use;
    // register the static grammar set before anything can trigger that. A
    // language with no registered grammar degrades to plain text.
    crate::grammars::register();

    let config = Config::load_default().unwrap_or_else(|_| Config::default());
    let mouse = config.editor.mouse;
    let lang_loader = helix_wasm::helix_core::config::default_lang_loader();

    // Default args: no files, so helix opens a scratch buffer. (--tutor needs
    // a runtime-directory read that has no backing storage here yet.)
    let app = Application::new(Args::default(), config, lang_loader)
        .map_err(|err| JsValue::from_str(&format!("failed to initialize helix: {err}")))?;

    claim_terminal(mouse)
        .map_err(|err| JsValue::from_str(&format!("failed to claim the terminal: {err}")))?;

    // Belt and braces on top of set_size above: a resize event forces a
    // re-layout in case the embedder's size was stale at boot. It also
    // triggers the first render once the event loop starts polling.
    bridge::inject_event(Event::Resize(columns, rows));

    APP.with(|cell| *cell.borrow_mut() = Some(app));
    spawn_local(drive());

    Ok(())
}

/// Drives the editor's event loop until it exits, then tears it down.
///
/// `app.run()` would own the `Application` for the whole session; instead
/// the app stays in [`APP`] and each poll borrows it just long enough to
/// poll a freshly created `event_loop_until_idle` future. Recreating the
/// future every poll is sound at the pinned helix rev because no state is
/// lost when one is dropped mid-pend:
///
/// - every `tokio::select!` source is cancel-safe: the shim `EventStream` is
///   a stateless poll of the bridge queue, the jobs channels are tokio mpsc,
///   redraw requests go through `Notify::notify_one` (the permit survives a
///   dropped future), and the idle/redraw timers live in `Editor`;
/// - once an arm fires, its handler body runs to completion within that same
///   poll — on wasm the handlers never actually pend (`render`'s body is
///   synchronous, the LSP/DAP arms are stubbed, `signals` is
///   `stream::empty()` on non-unix) — so a drop can't abandon a half-handled
///   event.
///
/// INVARIANT: "handlers never pend on wasm" is rev-specific. Recheck it on
/// every upstream bump — an event handler that gains a real `.await` would
/// silently break this driver (dropping the future would abandon the
/// half-run handler and the event it consumed).
async fn drive() {
    let mut events = EventStream::new();

    poll_fn(|cx| {
        APP.with(|cell| {
            let mut guard = cell.borrow_mut();
            let Some(app) = guard.as_mut() else {
                // Can't happen (nothing else takes the app while we run),
                // but treat a missing app as an exit rather than panicking.
                return Poll::Ready(());
            };
            let fut = pin!(app.event_loop_until_idle(&mut events));
            // Outside the integration-test build the loop only ever returns
            // `false` ("the editor wants to close"), so any Ready is an exit.
            match fut.poll(cx) {
                Poll::Ready(_) => Poll::Ready(()),
                Poll::Pending => Poll::Pending,
            }
        })
    })
    .await;

    // Take the app out before awaiting close(): the cell stays unborrowed
    // across those awaits, and inspection reports not-running from here on.
    let Some(mut app) = APP.with(|cell| cell.borrow_mut().take()) else {
        return;
    };

    let mut exit_code = app.editor.exit_code;
    for err in app.close().await {
        log::error!("error on close: {err}");
        exit_code = 1;
    }
    let mouse = app.editor.config().mouse;
    drop(app);
    if let Err(err) = restore_terminal(mouse) {
        log::error!("failed to restore the terminal: {err}");
    }
    log::info!("helix exited with code {exit_code}");
}

/// What `Application::run` does before entering its event loop (`claim_term`
/// → `Terminal::claim` → `CrosstermBackend::claim`) — private upstream, so
/// replicated here against a fresh [`bridge::Output`], which flushes to the
/// same sink the renderer writes through. Raw mode, alternate screen, focus
/// reporting, bracketed paste (the host page's paste handling relies on it),
/// a full clear, and mouse capture when configured. The backend would also
/// push keyboard-enhancement flags, but only where
/// `terminal::supports_keyboard_enhancement` reports support, which the
/// bridge never does.
fn claim_terminal(mouse: bool) -> std::io::Result<()> {
    use crossterm::event::{EnableBracketedPaste, EnableFocusChange, EnableMouseCapture};
    use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen};

    terminal::enable_raw_mode()?;
    let mut out = bridge::Output::new();
    execute!(out, EnterAlternateScreen, EnableFocusChange)?;
    execute!(out, EnableBracketedPaste)?;
    execute!(out, Clear(ClearType::All))?;
    if mouse {
        execute!(out, EnableMouseCapture)?;
    }
    Ok(())
}

/// The counterpart of [`claim_terminal`], mirroring `Application::run`'s
/// private `restore_term`: reset the cursor shape, then undo the claim.
fn restore_terminal(mouse: bool) -> std::io::Result<()> {
    use crossterm::event::{DisableBracketedPaste, DisableFocusChange, DisableMouseCapture};
    use crossterm::terminal::{self, LeaveAlternateScreen};

    let mut out = bridge::Output::new();
    out.write_all(b"\x1B[0 q")?;
    if mouse {
        execute!(out, DisableMouseCapture)?;
    }
    execute!(out, DisableBracketedPaste)?;
    execute!(out, DisableFocusChange, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()
}

/// Feeds one keyboard event, as the fields of a DOM `KeyboardEvent`.
#[wasm_bindgen]
pub fn key_event(key: &str, ctrl: bool, alt: bool, shift: bool, meta: bool) {
    if let Some(event) = keys::convert(key, ctrl, alt, shift, meta) {
        bridge::inject_event(Event::Key(event));
    }
}

/// Feeds pasted text (from the terminal emulator's paste handling).
#[wasm_bindgen]
pub fn paste(text: &str) {
    bridge::inject_event(Event::Paste(text.to_owned()));
}

/// Reports new terminal dimensions and triggers a re-layout.
#[wasm_bindgen]
pub fn resize(columns: u16, rows: u16) {
    bridge::set_size(columns, rows);
    bridge::inject_event(Event::Resize(columns, rows));
}
