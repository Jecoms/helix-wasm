//! Extensions to the standard library. A collection of helper functions
//! used throughout helix.

pub mod env;
pub mod faccess;
pub mod path;
pub mod range;
pub mod rope;
// Only wasm32 code paths consult the virtual file system; `test` keeps its
// unit tests runnable on the host without shipping the module (and its
// process-wide store) in native builds.
#[cfg(any(target_arch = "wasm32", test))]
pub mod vfs;

pub use range::Range;
