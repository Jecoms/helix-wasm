//! Telling the host a file is leaving the store.
//!
//! The deleting half of [`crate::download`]'s seam: the store keeps a wasm32
//! session's files, [`crate::vfs::remove`] takes one out, and this is how
//! the host finds out — and how it gets a say. A page that mirrors the
//! store (to `localStorage`, to a server) needs the deletion to mirror it,
//! and a page that must *not* offer deletion — a read-only lesson, say —
//! needs a way to say so. So registering a handler is what turns `:remove`
//! on: until a host does, [`send`] fails with `Unsupported` and the command
//! refuses on the statusline, exactly as `:download` does without a
//! download handler.
//!
//! Same shape as [`crate::download`], for the same reasons: a plain `fn`
//! keeps this crate free of frontend types, and the wasm frontend
//! registers one that reads its JS callback out of a thread-local.
//!
//! Gated to wasm32 and `test` as [`crate::vfs`] is.

use std::io;
use std::sync::Mutex;

/// A host's removal handler: the store is about to drop the file at `path`.
///
/// `path` is the store key — absolute, as [`crate::vfs::list`] reports it —
/// so a host that mirrors the store can prune the same entry. The handler
/// runs *before* the key is dropped, and returning an error refuses the
/// removal with the store untouched; the message reaches the user, so it
/// should say why. The editor only calls this for a key the store actually
/// holds: a buffer whose path has no key (never saved, or deleted by the
/// host since) just closes, and that is not a removal the host was ever
/// told the other half of.
pub type Handler = fn(path: &str) -> io::Result<()>;

static HANDLER: Mutex<Option<Handler>> = Mutex::new(None);

/// Registers the handler [`send`] consults, replacing any previous one.
/// Until a host registers one, [`send`] fails with `Unsupported` and
/// [`is_registered`] is false.
pub fn set_handler(handler: Handler) {
    *HANDLER.lock().unwrap() = Some(handler);
}

/// Whether a host has registered a [`Handler`] — the `:remove` gate, asked
/// up front so an unregistered host refuses before anything is closed,
/// including the never-saved buffer [`send`] is not called for.
pub fn is_registered() -> bool {
    HANDLER.lock().unwrap().is_some()
}

/// Tells the host the file at `path` is about to leave the store, or fails
/// with `Unsupported` if no host has registered a [`Handler`].
///
/// `Ok` is the host's consent: the caller drops the key after it, never
/// before, so a refusing host sees a store it can still read the file out
/// of.
pub fn send(path: &str) -> io::Result<()> {
    // Copy the `fn` out so the lock is released before calling it: the
    // handler crosses into frontend code (JS on wasm32) and
    // `std::sync::Mutex` is non-reentrant, so a handler that re-entered this
    // module while the lock was held would deadlock the single wasm thread.
    let handler = *HANDLER.lock().unwrap();
    match handler {
        Some(handler) => handler(path),
        None => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this host cannot remove files",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static DELIVERED: Mutex<Vec<String>> = Mutex::new(Vec::new());

    fn record(path: &str) -> io::Result<()> {
        DELIVERED.lock().unwrap().push(path.to_string());
        Ok(())
    }

    fn refuse(_path: &str) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "no"))
    }

    // One test for the whole lifecycle, for the reason `download`'s is: the
    // handler is a process-wide static and tests run in parallel.
    //
    // Local-only coverage, and the only coverage of the unregistered arm
    // anywhere: CI never runs helix's host unit tests, and the browser
    // suite cannot reach this state because the demo page registers a
    // handler at boot.
    #[test]
    fn consults_the_registered_handler() {
        assert!(!is_registered());
        let err = send("/a.txt").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);

        set_handler(record);
        assert!(is_registered());
        send("/a.txt").unwrap();
        assert_eq!(DELIVERED.lock().unwrap().as_slice(), ["/a.txt".to_string()]);

        // A handler that refuses reports why, rather than the refusal
        // passing for consent.
        set_handler(refuse);
        let err = send("/a.txt").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(DELIVERED.lock().unwrap().len(), 1);
    }
}
