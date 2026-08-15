# v2 spike notes — zero-fork wasm32 port

Seed artifacts from the 2026-08-15 spike that proved pristine helix 25.07.1
compiles for wasm32 with dependency-level substitution only. Plan and phases:
see issue #33. Upstream bug-fix PR: helix-editor/helix#16186.

## Proven result

`cargo check -p helix-core --target wasm32-unknown-unknown` passes against
**unmodified** helix source (plus the 3-line `faccess.rs` fix now in PR
#16186), using only the pieces in this branch.

## The working build recipe

Consumer workspace `Cargo.toml`:

```toml
[patch.crates-io]
home = { path = "stubs/home" }             # 0.5.9 — no wasm support upstream
which = { path = "stubs/which" }           # 8.0.0 — no wasm support upstream
libloading = { path = "stubs/libloading" } # 0.8.7 — dlopen; wasm links grammars statically
url = { path = "stubs/url" }               # 2.5.4 + wasm cfg for from/to_file_path
```

Environment for the wasm target (belongs in `.cargo/config.toml` `[env]`):

```sh
CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang   # Apple clang PARSES wasm but cannot EMIT it ("unable to create target"); brew llvm required
CFLAGS_wasm32_unknown_unknown="-I<repo>/sysroot -D__linux__ -ffreestanding"
```

Why each flag:
- `-I sysroot` — ~10 stub libc headers; enough for tree-house-bindings'
  **stock build script** (no vendoring, no fork).
- `-D__linux__` — routes tree-sitter's `portable/endian.h` (which has no
  wasm case, `#error platform not supported`) to `#include <endian.h>`,
  which our sysroot provides (wasm is little-endian; identity macros).
- `-ffreestanding` — stops clang's builtin headers forwarding to a hosted
  libc via `include_next`. `inttypes.h` forwards regardless, hence our own
  `sysroot/inttypes.h` shadowing it.

`sysroot/wctype.h`/`wctype.c` come from the legacy port (unicode tables for
tree-sitter's lexer); `wctype.c` must be compiled+linked by the wrapper's
build.rs (see legacy `helix-web/build.rs`).

## Enumerated blockers for helix-view / helix-term (Phase 2)

- Single failing crate in helix-view's whole tree: **mio**, pulled by
  tokio's `process` feature, requested in Cargo.toml by helix-view,
  helix-term, helix-lsp, helix-dap. Trimming `process` = the only
  Cargo.toml lines needed on the `helix-patched` branch.
- Direct `tokio::process`/`net` use in view+term source: only ~3 sites.
- Stub surface: helix-lsp ~26 distinct symbols used by view+term (grep
  undercounts `Client` method calls — expect more), helix-dap ~5,
  helix-vcs ~1 (may compile as-is once tokio unblocks; git2 stays off).
- crossterm: `[patch]` with a browser shim; port legacy
  `helix-web/src/crossterm/`.
- helix-term's `signal-hook-tokio` dep needs a `cfg(unix)` target gate.

## Known runtime traps (Phase 3, compile ≠ run)

- `std::time::Instant::now()` panics on wasm32-unknown-unknown → upstream a
  `web-time` swap (free on native) or hold on `helix-patched`.
- `faccess::readonly()` maps fs errors to `true` → every buffer would open
  read-only in the browser; virtual storage must intercept fs paths (see
  legacy `helix-core/src/storage.rs`).
- No `block_on` on the browser main thread; event loop must be driven async
  (see legacy `helix-term/src/application.rs` genericized backend).
- tokio `time`/`fs` features compile but their runtime behavior on wasm is
  unverified.

## Branch map

- `v2` (this branch, orphan) — becomes the new `main` at Phase 4 swap.
- `helix-patched` (to create) — helix at tag + faccess fix + tokio feature
  trims; Cargo git-deps point here until upstream PRs land.
- `legacy` (to create from current `main`) — the old in-tree port;
  reference for crossterm shim, storage, backend wiring, wasm-sysroot.
