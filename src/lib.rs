//! wasm32 wrapper around pristine upstream Helix.
//!
//! The heavy lifting happens in the manifests: helix crates come in as git
//! dependencies pinned to the `helix-patched` branch (the upstream release
//! tag plus not-yet-upstreamed fixes), and `[patch.crates-io]` swaps the
//! handful of transitive dependencies with no wasm32 support for the stubs
//! in `stubs/`. See SPIKE-NOTES.md for the full recipe and phase plan.

pub use helix_core;
