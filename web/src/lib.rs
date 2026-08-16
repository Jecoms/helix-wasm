//! Browser frontend for the helix wasm port.
//!
//! Boots pristine helix-term against the browser crossterm shim: rendered
//! ANSI flows out through `crossterm::bridge`'s output sink to a JS callback,
//! and input/resize events flow in through `bridge::inject_event`. The host
//! page (www/) owns the terminal emulator (xterm.js) and drives the exported
//! functions in [`session`].
//!
//! The exported JS surface is an internal contract between this crate and
//! the host page — unstable by design. A supported state-inspection /
//! embedding API is issue #18, sequenced after the port lands.

#[cfg(target_family = "wasm")]
mod c_alloc;
#[cfg(target_family = "wasm")]
mod grammars;
#[cfg(target_family = "wasm")]
mod keys;
#[cfg(target_family = "wasm")]
mod session;
#[cfg(target_family = "wasm")]
mod vfs;

#[cfg(target_family = "wasm")]
pub use session::*;
#[cfg(target_family = "wasm")]
pub use vfs::*;
