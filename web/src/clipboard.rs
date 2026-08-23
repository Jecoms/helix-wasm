//! The browser half of the `+`/`*` register bridge (issue #140).
//!
//! helix reads and writes the system clipboard synchronously through
//! `ClipboardProvider`; the browser only offers promises. The wasm32
//! provider in helix-view is therefore a mirror of the clipboard, and this
//! module keeps that mirror honest in both directions: writes fan out to
//! `navigator.clipboard.writeText` as they happen, and a read is
//! *prefetched* — `readText` is started by [`crate::session::key_event`]
//! right before a keystroke that will read the register, with input held
//! back until it settles, so that by the time helix asks the mirror holds
//! what the browser had.
//!
//! What the browser does with those calls varies: writes go through on a
//! keystroke everywhere; a read is a one-time permission in Chromium and a
//! per-read "Paste" affordance in Safari and Firefox (which skips it when
//! the clipboard holds the page's own copy). A refused or ignored read
//! leaves the mirror alone, so the register pastes what it holds — the
//! last in-page yank — rather than failing.

use helix_wasm::helix_view::clipboard::{self, ClipboardType};
use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

/// Points helix-view's provider at the browser. Call before the editor is
/// built, so the first yank already reaches the clipboard.
pub(crate) fn install() {
    clipboard::set_write_hook(write);
}

/// `navigator.clipboard`, or `None` where the page has none — an insecure
/// origin that is not localhost, or a browser without the async API. The
/// mirror keeps the registers editor-local there, as they were.
fn navigator_clipboard() -> Option<web_sys::Clipboard> {
    let navigator = web_sys::window()?.navigator();
    let clipboard = Reflect::get(navigator.as_ref(), &JsValue::from_str("clipboard")).ok()?;
    (!clipboard.is_undefined()).then(|| clipboard.unchecked_into())
}

/// The `fn` registered with [`clipboard::set_write_hook`]: hands a yank to
/// the browser, fire-and-forget. A rejection (no permission, the page not
/// focused) is logged, not surfaced: the mirror already holds the value, so
/// the in-page side of the register is fine either way.
fn write(contents: &str, _kind: ClipboardType) {
    let Some(clipboard) = navigator_clipboard() else {
        return;
    };
    let promise = clipboard.write_text(contents);
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(err) = JsFuture::from(promise).await {
            log::warn!(
                "clipboard write refused: {}",
                crate::download::describe(err, "writeText failed")
            );
        }
    });
}

/// Starts a browser read, synchronously — the promise has to be created in
/// the keystroke's own turn for the browser to count it as user-initiated —
/// and returns the future that resolves to its text. `None` where there is
/// no clipboard to read (see [`navigator_clipboard`]).
pub(crate) fn read() -> Option<impl std::future::Future<Output = Result<Option<String>, JsValue>>> {
    let clipboard = navigator_clipboard()?;
    let promise = clipboard.read_text();
    Some(async move { JsFuture::from(promise).await.map(|text| text.as_string()) })
}
