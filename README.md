# helix-wasm

A wasm32 port of [Helix](https://github.com/helix-editor/helix) that consumes
**pristine upstream source at a release tag** — no fork, no vendored tree.
All wasm accommodation lives on this side of the fence, so updating to a new
Helix release is a tag bump, not a patch-set rebase.

## Layout

| Path | Purpose |
| --- | --- |
| `Cargo.toml` | Wrapper workspace: helix crates as git dependencies pinned to the `helix-patched` branch, plus `[patch.crates-io]` stub swaps |
| `stubs/` | Stand-ins for transitive dependencies with no wasm32 support (`home`, `which`, `libloading`, and `url` with a wasm cfg) |
| `sysroot/` | Stub libc headers and the `wasm-cc` clang shim that let tree-sitter's stock build script compile its C for wasm32 |
| `.cargo/config.toml` | Wires `wasm-cc` up as the C compiler for the wasm32 target |
| `SPIKE-NOTES.md` | The full build recipe, enumerated blockers, and known runtime traps |

## Building

```sh
rustup target add wasm32-unknown-unknown
cargo check -p helix-core --target wasm32-unknown-unknown
```

A clang that can emit wasm is required. Linux distro clang works; on macOS the
system clang cannot emit wasm, so install LLVM (`brew install llvm`) or point
`HELIX_WASM_CLANG` at a suitable clang.

## Branch map

- `v2` (this branch) — the zero-fork port; becomes `main` once the browser
  demo boots ([#33](https://github.com/Jecoms/helix-wasm/issues/33)).
- `helix-patched` — upstream Helix at the release tag plus the few
  not-yet-upstreamed fixes (currently the `faccess` fallback fix,
  [helix-editor/helix#16186](https://github.com/helix-editor/helix/pull/16186)).
  Retires as upstream PRs land.
- `main` — the previous in-tree port, to be archived as `legacy` at the swap.
