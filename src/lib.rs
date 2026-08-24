//! wasm32 wrapper around the patched Helix source in `helix/`.
//!
//! The heavy lifting happens in the manifests: the helix crates come in as
//! path dependencies on `helix/`, which holds upstream's 25.07.1 release
//! tree plus the wasm patch set as ordinary commits on top of
//! `upstream/25.07.1`, so a helix edit is an edit in this workspace. One
//! `[patch.crates-io]` table swaps in the stubs from `stubs/` for the
//! transitive crates-io dependencies with no wasm32 support. The build
//! recipe lives in README.md; the porting history in issue #33, the move
//! in-tree in #82.

pub use helix_core;
pub use helix_event;
pub use helix_loader;
pub use helix_lsp;
pub use helix_stdx;
pub use helix_term;
pub use helix_view;
