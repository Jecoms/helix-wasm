//! The exported JS surface: boot the editor, feed it input, keep its size
//! current. Unstable, internal to the host page (see crate docs).

use std::cell::{Cell, RefCell};
use std::future::{poll_fn, Future};
use std::io::Write;
use std::pin::pin;
use std::task::Poll;

use crate::{keys, mouse};
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
    /// The host page's exit callback, registered with [`on_exit`].
    static EXIT: RefCell<Option<Function>> = const { RefCell::new(None) };
    static STARTED: Cell<bool> = const { Cell::new(false) };
    /// Whether there is an editor left to receive input: `false` until
    /// [`start`] hands the app to [`drive`], and again the moment helix
    /// exits. The input exports queue events for the event loop to drain,
    /// so once the loop is gone every further event is one nothing will
    /// ever take back out of [`bridge`]'s queue — see [`forward`].
    static RUNNING: Cell<bool> = const { Cell::new(false) };
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

    // Themes must be in the vfs before the editor boots: `Application::new`
    // resolves the configured theme, and `:theme` completion lists whatever
    // the runtime themes directory holds.
    crate::themes::seed();

    let mut config = Config::load_default().unwrap_or_else(|_| Config::default());
    // helix detects true color from COLORTERM/terminfo, neither of which
    // exists on wasm32 — without this override it refuses every RGB theme
    // ("theme requires true color support"). xterm.js always renders 24-bit
    // color, so claim it unconditionally, like `true-color = true` in a
    // user's config.toml would.
    //
    // The override survives `:config-reload`: `Application::refresh_config`
    // propagates the `Config::load_default()` error — always Err on wasm32,
    // where config.toml is read through `std::fs` — before it stores the new
    // config or reloads the theme, so the running config, this override and
    // the active RGB theme are all left alone.
    config.editor.true_color = true;
    let mouse = config.editor.mouse;
    let lang_loader = helix_wasm::helix_core::config::default_lang_loader();

    // Seed the vendored tutorial text (see ../runtime/README.md) into the
    // vfs so `:tutor` finds it. `runtime_file("tutor")` resolves to a path
    // under the wasm32 config dir (absolute; nothing on wasm32 exists on the
    // real fs, so the fallback always wins), so the seeded key and the path
    // `Editor::open` resolves when the command runs are the same regardless
    // of the current `:cd` directory.
    helix_wasm::helix_stdx::vfs::write(
        helix_wasm::helix_loader::runtime_file("tutor"),
        include_str!("../runtime/tutor"),
    )
    .map_err(|err| JsValue::from_str(&format!("failed to seed the tutor file: {err}")))?;

    // Sample files, so the file picker and `:o` open on something worth
    // selecting (see samples.rs).
    crate::samples::seed();

    // Default args: no files, so helix opens a scratch buffer.
    let app = Application::new(Args::default(), config, lang_loader)
        .map_err(|err| JsValue::from_str(&format!("failed to initialize helix: {err}")))?;

    claim_terminal(mouse)
        .map_err(|err| JsValue::from_str(&format!("failed to claim the terminal: {err}")))?;

    // Unlike `Application::run`, no panic hook restoring the terminal is
    // installed (upstream chains a `force_restore` hook before its loop):
    // intentional — after a panic this instance is unusable anyway and a
    // page reload resets xterm.js, so the hook stays the default
    // console_error_panic_hook set above.

    // Belt and braces on top of set_size above: a resize event forces a
    // re-layout in case the embedder's size was stale at boot. It also
    // triggers the first render once the event loop starts polling.
    bridge::inject_event(Event::Resize(columns, rows));

    APP.with(|cell| *cell.borrow_mut() = Some(app));
    RUNNING.with(|running| running.set(true));
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

    // The loop that drains the bridge queue is done; stop the input exports
    // from filling it (see [`forward`]) before anything gets a chance to
    // run between here and the exit announcement.
    RUNNING.with(|running| running.set(false));

    // Take the app out before awaiting close(): the cell stays unborrowed
    // across those awaits, and inspection reports not-running from here on.
    let Some(mut app) = APP.with(|cell| cell.borrow_mut().take()) else {
        return;
    };

    let mut exit_code = app.editor.exit_code;
    // `Application::close` is unusable here: its third step,
    // `Editor::close_language_servers`, wraps the shutdown in a
    // `tokio::time::timeout`, and building that timer calls
    // `std::time::Instant::now()` — which traps on wasm32-unknown-unknown
    // ("time not implemented on this platform"), poisoning the module
    // mid-teardown, so `:q` took the page down instead of exiting. The two
    // steps worth keeping are covered: pending writes flush below, and
    // `Jobs::finish` only awaits jobs that asked to be waited on, which
    // upstream creates solely for format-on-write — unreachable without a
    // formatter or a language server (and `jobs` is private, so it could
    // not be drained on its own regardless).
    if let Err(err) = app.editor.flush_writes().await {
        log::error!("error flushing writes on exit: {err}");
        exit_code = 1;
    }
    let mouse = app.editor.config().mouse;
    drop(app);
    if let Err(err) = restore_terminal(mouse) {
        log::error!("failed to restore the terminal: {err}");
    }
    log::info!("helix exited with code {exit_code}");
    announce_exit(exit_code);
}

/// Tells the reader — and the embedder — that helix is gone.
///
/// `:q` is a dead end in a browser tab: there is no shell to return to, and
/// the editor cannot be restarted in place (`start` is once per page load).
/// Left alone, the tutorial's chapter 1.2 exercise drops the reader on a
/// blank terminal that silently swallows every keystroke — indistinguishable
/// from a frozen page. So the exit gets said out loud, on the restored main
/// screen, and handed to the host page through the [`on_exit`] callback.
/// `:q` itself is untouched: it really does quit.
fn announce_exit(exit_code: i32) {
    let mut out = bridge::Output::new();
    // Plain English first: a demo visitor should be able to act on this line
    // without knowing what an exit code is.
    let notice = format!(
        "\r\nHelix has exited. Refresh the page to start a new session. (exit code {exit_code})\r\n"
    );
    if let Err(err) = out.write_all(notice.as_bytes()).and_then(|()| out.flush()) {
        log::error!("failed to write the exit notice: {err}");
    }

    // Take the handler out of the cell before calling into JS, rather than
    // calling through a live borrow: a handler that re-enters `on_exit` (to
    // detach itself, say) would otherwise hit `borrow_mut()` on that borrow
    // and panic — and a wasm32 panic doesn't unwind, so the cell would stay
    // borrowed for the life of the page. `bridge::Output::flush` copies its
    // sink out of the mutex first for the same reason. Taking it also makes
    // this doc's "invoked once" structural instead of incidental.
    let handler = EXIT.with(|slot| slot.borrow_mut().take());
    if let Some(handler) = handler {
        let _ = handler.call1(&JsValue::NULL, &JsValue::from_f64(exit_code.into()));
    }
}

/// Registers a callback for helix exiting, invoked once with the exit code.
///
/// The editor cannot be restarted afterwards; a host page that wants a live
/// editor back has to reload. Register before [`start`] — a `:q` on the
/// first keystroke would otherwise beat the registration.
///
/// The input exports ([`key_event`], [`paste`], [`mouse_event`],
/// [`focus_event`], [`resize`]) stay callable once this fires but stop doing
/// anything, so a host page that keeps forwarding is inert rather than
/// harmful. Treating this callback as "stop forwarding" is still the right
/// thing to do: the page has a dead editor on screen and needs to say so.
#[wasm_bindgen]
pub fn on_exit(handler: Function) {
    EXIT.with(|slot| *slot.borrow_mut() = Some(handler));
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
/// private `restore_term`: un-hide the cursor (the renderer commonly leaves
/// it Hidden — helix draws its own block cursor) and reset its shape, then
/// undo the claim.
fn restore_terminal(mouse: bool) -> std::io::Result<()> {
    use crossterm::cursor::{SetCursorStyle, Show};
    use crossterm::event::{DisableBracketedPaste, DisableFocusChange, DisableMouseCapture};
    use crossterm::terminal::{self, LeaveAlternateScreen};

    let mut out = bridge::Output::new();
    // `restore_term`'s `show_cursor(CursorKind::Block)`: Show + SteadyBlock,
    // ahead of the backend's own reset sequence below.
    execute!(out, Show, SetCursorStyle::SteadyBlock)?;
    out.write_all(b"\x1B[0 q")?;
    if mouse {
        execute!(out, DisableMouseCapture)?;
    }
    execute!(out, DisableBracketedPaste)?;
    execute!(out, DisableFocusChange, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()
}

/// Queues one input event for the event loop, or drops it if helix is gone.
///
/// The exports below hand events to [`bridge`]'s queue, which only the event
/// loop drains — so after the exit every further event is one nothing will
/// ever take back out, and a host page that keeps forwarding (a `:q` leaves
/// the module perfectly callable now) would grow that queue without bound.
/// Dropping them here covers every embedder, rather than making a correct
/// liveness gate each host page's problem. Events sent before [`start`] are
/// dropped for the same reason: nothing has been booted to consume them.
fn forward(event: Event) {
    if RUNNING.with(Cell::get) {
        bridge::inject_event(event);
    }
}

/// Feeds one keyboard event, as the fields of a DOM `KeyboardEvent`.
///
/// A no-op once helix has exited (see [`on_exit`]).
#[wasm_bindgen]
pub fn key_event(key: &str, ctrl: bool, alt: bool, shift: bool, meta: bool) {
    if let Some(event) = keys::convert(key, ctrl, alt, shift, meta) {
        forward(Event::Key(event));
    }
}

/// Feeds one mouse event, as the fields of an SGR mouse report from the
/// terminal emulator: the button/modifier code, the 1-based column and row,
/// and whether the final byte was `M` (press) rather than `m` (release).
///
/// A no-op once helix has exited (see [`on_exit`]).
#[wasm_bindgen]
pub fn mouse_event(code: u16, column: u16, row: u16, pressed: bool) {
    if let Some(event) = mouse::convert(code, column, row, pressed) {
        forward(Event::Mouse(event));
    }
}

/// Feeds a terminal focus change (from the emulator's focus reports —
/// helix enables focus reporting at boot).
///
/// A no-op once helix has exited (see [`on_exit`]).
#[wasm_bindgen]
pub fn focus_event(gained: bool) {
    forward(if gained {
        Event::FocusGained
    } else {
        Event::FocusLost
    });
}

/// Feeds pasted text (from the terminal emulator's paste handling).
///
/// A no-op once helix has exited (see [`on_exit`]).
#[wasm_bindgen]
pub fn paste(text: &str) {
    forward(Event::Paste(text.to_owned()));
}

/// Reports new terminal dimensions and triggers a re-layout.
///
/// A no-op once helix has exited (see [`on_exit`]).
#[wasm_bindgen]
pub fn resize(columns: u16, rows: u16) {
    if RUNNING.with(Cell::get) {
        bridge::set_size(columns, rows);
    }
    forward(Event::Resize(columns, rows));
}
