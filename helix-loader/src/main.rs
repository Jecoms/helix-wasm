// This binary is used in the Release CI as an optimization to cut down on
// compilation time. This is not meant to be run manually.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    helix_loader::grammar::fetch_grammars(None)
}

/// There is nothing to fetch on wasm32: grammars are statically linked.
#[cfg(target_arch = "wasm32")]
fn main() {}
