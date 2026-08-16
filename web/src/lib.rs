//! Browser frontend for the helix wasm port.
//!
//! Boots pristine helix-term against the browser crossterm shim: rendered
//! ANSI flows out through `crossterm::bridge`'s output sink to a JS callback,
//! and input/resize events flow in through `bridge::inject_event`. The host
//! page (www/) owns the terminal emulator (xterm.js) and drives the exported
//! functions in [`session`].
//!
//! The exported JS surface is an internal contract between this crate and
//! the host page — unstable by design, except [`inspect`]: the read-only
//! state-inspection surface for embedders (issue #18), which is meant to be
//! kept stable.

#[cfg(target_family = "wasm")]
mod c_alloc;
#[cfg(target_family = "wasm")]
mod grammars;
#[cfg(target_family = "wasm")]
mod inspect;
#[cfg(target_family = "wasm")]
mod keys;
// Not wasm-gated: pure decode over the (vendored) crossterm event types, so
// compiling it everywhere lets its unit tests run under a native cargo test.
mod mouse;
#[cfg(target_family = "wasm")]
mod session;
#[cfg(target_family = "wasm")]
mod themes;
#[cfg(target_family = "wasm")]
mod vfs;

#[cfg(target_family = "wasm")]
pub use inspect::*;
#[cfg(target_family = "wasm")]
pub use session::*;
#[cfg(target_family = "wasm")]
pub use vfs::*;
