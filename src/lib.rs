//! wasm32 wrapper around pristine upstream Helix.
//!
//! The heavy lifting happens in the manifests: helix crates come in as git
//! dependencies pinned to the `helix-patched` branch (the upstream release
//! tag plus not-yet-upstreamed fixes), and two `[patch]` tables swap in the
//! stubs from `stubs/`: `[patch.crates-io]` replaces the transitive
//! crates-io dependencies with no wasm32 support, and
//! `[patch."<this repo's git URL>"]` replaces the vendored helix-lsp /
//! helix-dap crates that helix-view depends on directly. See SPIKE-NOTES.md
//! for the full recipe and phase plan.

pub use helix_core;
pub use helix_loader;
pub use helix_term;
pub use helix_view;
