//! The exported JS surface: boot the editor, feed it input, keep its size
//! current. Unstable, internal to the host page (see crate docs).

use std::cell::{Cell, RefCell};

use crate::keys;
use crossterm::bridge;
use crossterm::event::{Event, EventStream};
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

    let config = Config::load_default().unwrap_or_else(|_| Config::default());
    let lang_loader = helix_wasm::helix_core::config::default_lang_loader();

    // Default args: no files, so helix opens a scratch buffer. (--tutor needs
    // a runtime-directory read that has no backing storage here yet.)
    let mut app = Application::new(Args::default(), config, lang_loader)
        .map_err(|err| JsValue::from_str(&format!("failed to initialize helix: {err}")))?;

    // Belt and braces on top of set_size above: a resize event forces a
    // re-layout in case the embedder's size was stale at boot.
    bridge::inject_event(Event::Resize(columns, rows));

    spawn_local(async move {
        let mut events = EventStream::new();
        match app.run(&mut events).await {
            Ok(code) => log::info!("helix exited with code {code}"),
            Err(err) => log::error!("helix exited with an error: {err}"),
        }
    });

    Ok(())
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
