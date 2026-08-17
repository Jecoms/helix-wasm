//! Handing a file to the host to save.
//!
//! The counterpart of [`crate::vfs`]: the store keeps a wasm32 session's
//! files alive, and this gets one back out. Nothing in the store outlives
//! the page, and on `wasm32-unknown-unknown` there is no file system to
//! write a copy to and no reach into the user's — saving to the machine the
//! browser runs on is a host capability (a `Blob` and an object URL, or the
//! File System Access API), not an editor one. So this module is only the
//! seam: the frontend registers a handler with [`set_handler`], and the
//! editor calls [`send`] with the bytes and the name to save them under.
//!
//! Same shape, and for the same reason, as the output sink the vendored
//! `crossterm`'s bridge module registers: a plain `fn` keeps this crate free
//! of frontend types, and a wasm frontend registers one that reads its JS
//! callback out of a thread-local (wasm32 is single-threaded, so that is
//! sound).
//!
//! Gated to wasm32 and `test` exactly as [`crate::vfs`] is: only wasm32 code
//! paths have anything to hand out, and the `test` arm is there so the unit
//! tests below run on the host.

use std::io;
use std::sync::Mutex;

/// A host's download implementation: save `contents` under `name`.
///
/// `name` is a file name, never a path — a download lands wherever the host
/// puts downloads, and there is no directory to name. Returning an error
/// means the host refused (or could not manage) the save; the message
/// reaches the user, so it should say which.
pub type Handler = fn(name: &str, contents: &[u8]) -> io::Result<()>;

static HANDLER: Mutex<Option<Handler>> = Mutex::new(None);

/// Registers the handler [`send`] hands files to, replacing any previous
/// one. Until a host registers one, [`send`] fails with `Unsupported`.
pub fn set_handler(handler: Handler) {
    *HANDLER.lock().unwrap() = Some(handler);
}

/// Hands `contents` to the host to save under `name`, or fails with
/// `Unsupported` if no host has registered a [`Handler`].
///
/// Whether the save actually happens is the host's business: a browser may
/// route it through a save dialog the user can still cancel, so an `Ok` here
/// means the file was handed over, not that it landed.
pub fn send(name: &str, contents: &[u8]) -> io::Result<()> {
    // Copy the `fn` out so the lock is released before calling it: the
    // handler crosses into frontend code (JS on wasm32) and
    // `std::sync::Mutex` is non-reentrant, so a handler that re-entered this
    // module while the lock was held would deadlock the single wasm thread.
    let handler = *HANDLER.lock().unwrap();
    match handler {
        Some(handler) => handler(name, contents),
        None => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this host cannot save files",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static DELIVERED: Mutex<Vec<(String, Vec<u8>)>> = Mutex::new(Vec::new());

    fn record(name: &str, contents: &[u8]) -> io::Result<()> {
        DELIVERED
            .lock()
            .unwrap()
            .push((name.to_string(), contents.to_vec()));
        Ok(())
    }

    fn refuse(_name: &str, _contents: &[u8]) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "no"))
    }

    // The handler is a process-wide static and the harness runs tests in
    // parallel, so the whole lifecycle is one test: a second test observing
    // the unregistered state would race this one's registration.
    #[test]
    fn sends_to_the_registered_handler() {
        let err = send("a.txt", b"contents").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);

        set_handler(record);
        send("a.txt", b"contents").unwrap();
        assert_eq!(
            DELIVERED.lock().unwrap().as_slice(),
            [("a.txt".to_string(), b"contents".to_vec())]
        );

        // A handler that refuses reports why, rather than the refusal
        // passing for a save.
        set_handler(refuse);
        let err = send("a.txt", b"contents").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(DELIVERED.lock().unwrap().len(), 1);
    }
}
