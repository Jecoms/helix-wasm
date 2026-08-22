//! The host page's half of `:remove` (issue #132): the editor is about to
//! drop a file out of the virtual file system, and the page gets told — and
//! gets a veto.
//!
//! Deletion is a host capability here the way saving is for `:download`
//! (see [`crate::on_download`]), though for the opposite reason: the module
//! *can* drop a key on its own, but a page that mirrors the store somewhere
//! durable (a `localStorage` scratchpad, a server) has to prune its mirror
//! in the same act, and a page that must not offer deletion at all — a
//! read-only lesson — needs a way to say so. Registering is what turns
//! `:remove` on: with no handler the command fails with "this host cannot
//! remove files" rather than silently deleting, so a page that has not
//! wired this up says so on the statusline.

use std::cell::RefCell;
use std::io;

use helix_wasm::helix_stdx::remove;
use js_sys::Function;
use wasm_bindgen::prelude::*;

thread_local! {
    /// The host page's removal handler; a thread-local for the same reason
    /// the download handler is one (a plain `fn` seam, single-threaded
    /// wasm32).
    static HANDLER: RefCell<Option<Function>> = const { RefCell::new(None) };
}

/// Registers the page's removal handler, replacing any previous one — and
/// in doing so enables `:remove` / `:rm`, which refuse until a page
/// registers one.
///
/// It is called as `handler(path)` with the store key about to be dropped
/// — absolute, the same string `vfs_list` reports — *before* the key goes
/// and before the buffer on it closes. Throwing from it refuses the
/// removal with the store untouched, and the error's message is what the
/// editor shows the user, so say why. It is not called for a buffer that
/// was never saved (no key, nothing to mirror); that buffer just closes.
/// Nor is it called by [`crate::vfs_delete`] — a page calling that is
/// already the one doing the deleting.
///
/// The handler runs inside the editor's event loop, so the inspection
/// exports ([`crate::editor_state`], [`crate::editor_text`]) throw if called
/// from it; a handler that wants editor state must defer to a microtask.
#[wasm_bindgen]
pub fn on_remove(handler: Function) {
    HANDLER.with(|slot| *slot.borrow_mut() = Some(handler));
    remove::set_handler(forward_remove);
}

/// The `fn` registered with [`remove::set_handler`]; asks the page's
/// callback about one key and reports back what it said.
fn forward_remove(path: &str) -> io::Result<()> {
    // Cloned out of the cell rather than called through a live borrow, as
    // in `forward_download`: a handler that re-entered `on_remove` would
    // otherwise panic on `borrow_mut()`, and a wasm32 panic does not unwind.
    let handler = HANDLER.with(|slot| slot.borrow().clone());
    let Some(handler) = handler else {
        // `on_remove` registers this `fn` and the callback together, so
        // this is unreachable; treat it as a refusal rather than panicking.
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this page registered no removal handler",
        ));
    };

    handler
        .call1(&JsValue::NULL, &JsValue::from_str(path))
        .map(|_| ())
        .map_err(|err| {
            io::Error::other(crate::download::describe(
                err,
                "the page's removal handler failed",
            ))
        })
}
