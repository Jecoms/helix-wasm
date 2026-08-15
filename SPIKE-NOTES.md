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

# helix-view and up: swap the subprocess-bound helix crates for vendored
# stubs (patching the git source replaces the in-repo path deps too)
[patch."https://github.com/Jecoms/helix-wasm.git"]
helix-lsp = { path = "stubs/helix-lsp" }
helix-dap = { path = "stubs/helix-dap" }
```

Plus, for helix-view and up, `--cfg tokio_unstable` for the wasm32 target
(`.cargo/config.toml` `[target.wasm32-unknown-unknown] rustflags`): tokio's
feature guard otherwise rejects `fs`/`io-std`/`rt-multi-thread` on wasm.
Upstream helix sets the same cfg in its own `.cargo/config.toml`.

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

Resolved — `cargo check -p helix-view --target wasm32-unknown-unknown` is
green and CI-gated:

- **mio** via tokio's `process` feature: trimmed on `helix-patched` for
  helix-view (feature moved to a `cfg(not(wasm32))` target dep; the one
  `tokio::process` use site, external formatters in `Document::format`, is
  compiled out for wasm32). helix-lsp/helix-dap's `process`+`net` requests
  went away with the stubs below.
- The spike's "mio is the only failing crate" claim was incomplete: tokio
  also has a *feature guard* that rejects `fs`/`io-std`/`rt-multi-thread`
  on wasm unless `--cfg tokio_unstable` is set. Upstream helix already
  builds with tokio_unstable (its own `.cargo/config.toml`), so ours sets
  it for the wasm32 target too — no upstream edit needed.
- helix-lsp / helix-dap: stubbed via `[patch."<this repo's git URL>"]` →
  `stubs/helix-lsp`, `stubs/helix-dap`. Not hand-written facades: vendored
  copies of the upstream crates with only the subprocess/TCP machinery
  removed (spawn paths return errors), so Registry/util/jsonrpc/protocol
  types behave identically and the surface automatically covers whatever
  view+term use. Re-vendor on tag bumps; the delta is documented in each
  stub's Cargo.toml header.
- helix-view's own wasm32 fallbacks had bit-rotted upstream (noop clipboard
  provider signatures/derives, missing `get_terminal_provider`); fixed on
  `helix-patched` — upstreamable, same category as the faccess fix.
- helix-vcs: compiles as-is once tokio unblocks — without gix, which is
  optional behind helix-vcs's `git` feature and enabled by nothing in the
  helix-view build, so the wasm build has no git integration. Confirmed.

Resolved — `cargo check -p helix-term --target wasm32-unknown-unknown` is
green and CI-gated:

- crossterm: `[patch.crates-io]` → `stubs/crossterm`, a vendored 0.28.1
  with the OS terminal layer replaced by a browser bridge. The ANSI
  command API, the style/event types, and the `execute!`/`queue!` macros
  are pristine upstream code; terminal size, raw mode, and `EventStream`
  read from the new `crossterm::bridge` module instead of the tty, and
  the Phase-3 browser backend feeds it (`bridge::inject_event`,
  `bridge::set_size`). The legacy port solved this by editing helix-tui
  in-tree; the Cargo-level swap keeps upstream helix-tui/helix-term
  untouched. The shim is pure Rust, so it also compiles natively and the
  native `cargo check` type-checks helix-term against it.
- helix-term's subprocess/signal surface: trimmed on `helix-patched`.
  tokio's `process` feature and the `open` crate (no wasm32 support at
  all) moved to `cfg(not(target_arch = "wasm32"))` target deps, their two
  use sites (shell commands, external URL opening) return errors on
  wasm32; signal handling (signal-hook/signal-hook-tokio/libc) narrowed
  from `cfg(not(windows))` to `cfg(unix)`, so wasm32 takes the same
  no-signals path Windows already did.
- helix-term's build script fetches + dlopen-builds grammars by default;
  `.cargo/config.toml` sets `HELIX_DISABLE_AUTO_GRAMMAR_BUILD=1` since
  the wasm build links its grammar set statically (Phase 3).
- The rest of term's new dep tree (helix-tui, termini, grep-searcher,
  ignore, fern, chrono, pulldown-cmark, nucleo, ...) compiled for wasm32
  as-is.
- Adding helix-term reunified the cargo feature graph: helix-tui requires
  helix-view's `term` feature, and since the wrapper is a single package
  Cargo resolves one feature graph regardless of `-p`, so
  `cargo check -p helix-view` now runs with `term`/crossterm active. The
  deliberate isolation the wrapper had before this step (no `term`
  feature, no crossterm in the graph) is gone; the per-crate CI jobs are
  progressive-narrowing diagnostics for where a breakage enters the
  stack, not proofs that each crate builds in isolation.

## Phase 3 status: the demo boots

`web/` (crate `helix-web`) + `web/www/` boot pristine helix-term in the
browser: scratch buffer, modal editing, command palette, resize — verified
in a headless-Chromium smoke run. The runtime traps below are resolved
except where noted. Still open for the rest of Phase 3:

- **Static grammar set** — no grammars are linked yet, so no syntax
  highlighting; grammar loads fail cleanly through the libloading stub. The
  plan: compile a small grammar list's parser C in `web/build.rs` (the libc
  side — `sysroot/shims.c`, `wctype.c`, the `c_alloc` allocator — already
  links) and resolve symbols through a registration path. Watch for
  tree-house's parse-timeout `Instant` use when the first grammar lands.
- **GH Pages deploy** — the CI `web-bundle` job builds the bundle; the
  deploy workflow (port of legacy `web_demo.yml`) is deliberately not
  enabled yet.
- **Persistence / virtual storage** — nothing persists; `:w` on a path hits
  unsupported fs APIs and reports an error. `faccess::readonly()` maps fs
  errors to `true`, so any buffer opened *from a path* would be read-only;
  the scratch-buffer demo doesn't hit it. Legacy solved both with
  `helix-core/src/storage.rs` (localStorage-backed); v2 needs an equivalent
  that doesn't fork helix-core.
- **No tokio runtime is entered** — `AsyncHook` workers check
  `Handle::try_current()` and silently don't spawn (completion debounce,
  signature help, auto-save stay inert; harmless without LSP). Code paths
  that `tokio::spawn` unconditionally (`Jobs::add` for non-wait jobs, saves)
  panic if reached; same exposure the legacy port shipped with.

## Known runtime traps (Phase 3, compile ≠ run)

- ~~`std::time::Instant::now()` panics on wasm32-unknown-unknown~~ →
  resolved on `helix-patched` by the `web-time` swap (free on native) plus
  gloo-timers-backed editor idle/redraw timers. Upstreamable. Two caveats:
  - **helix-vcs is NOT covered**: `helix-vcs/src/diff.rs` still uses
    `tokio::time::Instant` (`Instant::now()` feeds
    `tokio::time::timeout_at` in the diff worker), which web-time cannot
    replace — `timeout_at` demands tokio's own `Instant`. Unreachable in
    the demo (helix-term builds with `default-features = false`, no git
    provider), but making helix-vcs reachable on wasm32 needs
    browser-timer work in the diff worker (the 547247fb approach), not a
    clock swap.
  - web-time sits in the plain `[dependencies]` of the patched crates
    (helix-core/term/tui/view), not a `target.'cfg(...)'` table like
    gloo-timers: on native it is a pure `std::time` re-export, and gating
    it would force cfg-gated `use` statements through shared code. The
    deliberate cost is that native dependency graphs gain the (inert)
    web-time crate — the one softening of the "native untouched" framing.
- `faccess::readonly()` maps fs errors to `true` → every buffer opened from
  a path would be read-only in the browser; virtual storage must intercept
  fs paths (see legacy `helix-core/src/storage.rs`). Not hit by the
  scratch-buffer demo.
- ~~`crossterm::bridge` starts at a static 80x24~~ → the web frontend's
  `start()` calls `bridge::set_size()` and injects `Event::Resize` before
  the first render. The trap remains for other embedders: size the bridge
  before booting the app.
- No `block_on` on the browser main thread; the event loop runs as a
  wasm-bindgen future (`spawn_local(app.run(...))`) — works with pristine
  helix-term because the crossterm stub's `EventStream` and the
  gloo-timers `Sleep` are plain futures needing no runtime.
- tokio `time`/`fs` features compile but panic at runtime if actually
  driven (no runtime is entered; see Phase 3 status above).
- `std::env::current_dir`/`current_exe` and etcetera's `$HOME` lookup are
  unsupported → fixed working directory `/` and fixed loader paths on
  `helix-patched`.
- tree-house-bindings ≤0.2.4 (and upstream master as of 2026-08) declares
  `ts_query_cursor_set_byte_range` without the C function's `bool` return —
  harmless on native ABIs, but wasm32 traps with `signature_mismatch` the
  moment a query cursor runs (`InactiveQueryCursor::new` calls it
  unconditionally; helix-core's syntax/indent paths hit it). `cargo check`
  can't catch it. Neutralized here by `[patch.crates-io]` → the
  `Jecoms/tree-house` fork, branch `bindings-v0.2.4-patched` (pristine
  0.2.4 release commit + the one-line fix from legacy PR #22). Upstream
  PR helix-editor/tree-house#40 carries the same fix; retire the patch
  once it lands in a release helix depends on.

## Branch map

- `v2` (this branch, orphan) — becomes the new `main` at Phase 4 swap.
- `helix-patched` — helix at tag + the not-yet-upstreamed fixes (faccess,
  tokio feature trims, web-time clocks, browser timers, bridge render
  target, loader/env fallbacks); Cargo git-deps point here until upstream
  PRs land.
- `legacy` (to create from current `main`) — the old in-tree port;
  reference for crossterm shim, storage, backend wiring, wasm-sysroot.
