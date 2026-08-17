//! The host page's half of `:download` (issue #67): the editor hands a file
//! over, the page saves it.
//!
//! Everything a session writes lives in the virtual file system and dies
//! with the page, and the module cannot get a file out on its own — saving
//! to the reader's machine means a `Blob` and an object URL, or the File
//! System Access API, all of which are the page's to reach. So the split is
//! the same one the terminal output takes: helix produces the bytes, the
//! host decides where they go. `web/www/main.js` is the reference
//! implementation; an embedder that wants a different one — a POST to a
//! server, a File System Access handle, a save that first asks — registers
//! its own and gets the same command.
//!
//! Registering is what turns `:download` on: with no handler the command
//! fails with "this host cannot save files" rather than silently doing
//! nothing, so a page that has not wired this up says so on the statusline.

use std::cell::RefCell;
use std::io;

use helix_wasm::helix_stdx::download;
use js_sys::{Function, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

thread_local! {
    /// The host page's download handler. A thread-local for the same reason
    /// the output callback is one: the seam in `helix_stdx::download` takes
    /// a plain `fn`, which cannot capture a JS value, and wasm32 is
    /// single-threaded so this is effectively a global.
    static HANDLER: RefCell<Option<Function>> = const { RefCell::new(None) };
}

/// Registers the page's download handler, replacing any previous one.
///
/// It is called as `handler(name, bytes)` with the file name to save under —
/// a bare name, never a path, since a download lands wherever the browser
/// puts downloads — and a `Uint8Array` of the file's contents (its own copy;
/// it does not alias wasm memory). Throwing from it refuses the save, and
/// the error's message is what the editor shows the user, so say why.
///
/// The handler runs inside the editor's event loop, so the inspection
/// exports ([`crate::editor_state`], [`crate::editor_text`]) throw if called
/// from it; a handler that wants editor state must defer to a microtask.
#[wasm_bindgen]
pub fn on_download(handler: Function) {
    HANDLER.with(|slot| *slot.borrow_mut() = Some(handler));
    download::set_handler(forward_download);
}

/// The `fn` registered with [`download::set_handler`]; hands one file to the
/// page's callback and reports back what it said.
fn forward_download(name: &str, contents: &[u8]) -> io::Result<()> {
    // Clone the handler out of the cell rather than calling through a live
    // borrow: a handler that re-entered `on_download` (to swap itself out,
    // say) would otherwise hit `borrow_mut()` on that borrow and panic — and
    // a wasm32 panic does not unwind, so the cell would stay borrowed for
    // the life of the page.
    let handler = HANDLER.with(|slot| slot.borrow().clone());
    let Some(handler) = handler else {
        // `on_download` registers this `fn` and the callback together, so
        // this is unreachable; treat it as a refusal rather than panicking.
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this page registered no download handler",
        ));
    };

    handler
        .call2(
            &JsValue::NULL,
            &JsValue::from_str(name),
            &Uint8Array::from(contents).into(),
        )
        .map(|_| ())
        .map_err(|err| io::Error::other(describe(err)))
}

/// The message out of whatever the handler threw. JS can throw anything, so
/// take an `Error`'s message, then a bare string, and fall back to saying
/// which side failed.
fn describe(err: JsValue) -> String {
    err.dyn_ref::<js_sys::Error>()
        .map(|err| String::from(err.message()))
        .or_else(|| err.as_string())
        .unwrap_or_else(|| "the page's download handler failed".to_string())
}
