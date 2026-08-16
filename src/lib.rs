//! wasm32 wrapper around pristine upstream Helix.
//!
//! The heavy lifting happens in the manifests: helix crates come in as git
//! dependencies pinned to an append-only `helix/<version>` snapshot ref
//! (currently `helix/25.07.1-r3`), a frozen single-commit capture of the
//! `helix-patched` workbench — the upstream release tag plus
//! not-yet-upstreamed fixes — and two `[patch]` tables swap in the
//! stubs from `stubs/`: `[patch.crates-io]` replaces the transitive
//! crates-io dependencies with no wasm32 support, and
//! `[patch."<this repo's git URL>"]` replaces the vendored helix-lsp /
//! helix-dap crates that helix-view depends on directly. The build recipe
//! lives in README.md; the porting history in issue #33.

pub use helix_core;
pub use helix_loader;
pub use helix_stdx;
pub use helix_term;
pub use helix_view;
