//! Shim: upstream inspects file descriptors (unix) or console handles
//! (windows). The browser has neither, so nothing is ever a tty.

/// Adds the `is_tty` method to types that might represent a terminal.
pub trait IsTty {
    /// Returns true if this is a tty.
    fn is_tty(&self) -> bool;
}

// Wider than upstream's `impl<S: AsRawFd> IsTty for S`: `AsRawFd` does not
// exist on wasm32-unknown-unknown, so the bound is dropped and `.is_tty()`
// exists (and is false) on types upstream would reject at compile time.
impl<S> IsTty for S {
    fn is_tty(&self) -> bool {
        false
    }
}
