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
  ignore, fern, chrono, pulldown-cmark, ...) compiled for wasm32 as-is —
  except nucleo, which *compiled* as-is but panics at runtime building its
  threadpool, and is now `[patch.crates-io]`-swapped for `stubs/nucleo`
  (see the runtime trap list).
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

- ~~**Static grammar set**~~ — done: c, go, java, javascript, python,
  regex, rust, toml are statically linked and highlighting renders (headless-Chromium-verified). The pieces:
  - `web/build.rs` `GRAMMARS` is the single source of truth: it
    shallow-fetches each grammar's C source pinned by rev (the same pins as
    helix's `languages.toml`) into OUT_DIR, compiles parser.c/scanner.c via
    the wasm-cc sysroot shim, and generates the `register()` glue.
  - `helix-patched` gained a wasm32 embedder-registration API in
    `helix-loader::grammar` (`register_grammar`, `register_runtime_file`;
    `get_language`/`load_runtime_file` read those registries) — replaces
    upstream's `unimplemented!()` wasm arm; upstreamable. The libloading
    stub stays a pure failure stub; symbols resolve through registration,
    not fake dlopen.
  - Queries are helix's own files, vendored under `web/queries/<lang>/`
    (see the README there; re-copy on tag bump), embedded and registered at
    boot in `web/src/session.rs::start`.
  - To add a grammar: one row in `GRAMMARS` + vendor its queries (plus any
    query-only base dirs they `; inherits:` from, like javascript's `ecma`). Languages
    without a registered grammar degrade to plain text (`get_language`
    returns `Ok(None)`), so the pristine full `languages.toml` ships
    untrimmed.
  - The feared tree-house parse-timeout trap is a non-trap: the timeout
    goes through `ts_parser_set_timeout_micros` into tree-sitter's C clock
    (`clock_gettime`), which `sysroot/shims.c` freezes at zero — elapsed
    time is always zero, timeouts never fire, parses run to completion. No
    Rust `Instant` involved.
- ~~**GH Pages deploy**~~ — done: `.github/workflows/web_demo.yml` builds
  the full-catalog bundle (same steps as the CI `web-bundle` job) and
  publishes via `upload-pages-artifact`/`deploy-pages`, serialized under
  `concurrency: pages`. Design notes:
  - The file keeps the legacy workflow's exact path: `workflow_dispatch`
    resolves workflows from the default branch, so sharing `main`'s
    registered filename is what lets
    `gh workflow run web_demo.yml --ref v2` run this branch's version
    before the v2 → main swap (proven by the legacy deploys dispatched
    from the non-default `15-deploy-web-demo-pages` branch).
  - Pages is already enabled for the repo (`build_type: workflow`); the
    actual gate is the `github-pages` environment's **deployment branch
    policy** (allowed as of 2026-08-15: `main`, `wasm32`,
    `15-deploy-web-demo-pages`). A run from `v2` fails at the environment
    until `v2` is added — one `gh api` call, deliberately left to the
    user. No `configure-pages` step in the workflow: enablement is a repo
    setting the workflow shouldn't flip.
  - vite's `base: "./"` (relative asset URLs) makes the same dist work at
    the site root and under the `/helix-wasm/` project path — verified by
    running the Chromium smoke suite against the staged `_site` behind a
    local `/helix-wasm/`-prefixed server (full pass, all grammars).
  - v2 deploys to the site root (the legacy demo lived at `/demo/` with a
    root redirect); the staged site keeps a `/demo/` → root redirect so
    old links don't 404. The first v2 deploy **replaces** the legacy demo.
  - `web/NOTICE.md` ships with the deployed assets as a static file: a
    vite plugin emits it into dist as `NOTICE.txt`, so it lives at
    <https://jecoms.github.io/helix-wasm/NOTICE.txt> once deployed
    (text/plain displays inline; Pages would serve `.md` as
    text/markdown, which some browsers download). Deliberately no page UI
    for attribution — MIT's notice condition is satisfied by the notice
    accompanying the distributed assets.
- ~~**Persistence / virtual storage**~~ — done: document IO goes through
  `helix_stdx::vfs`, an in-memory virtual file system (Chromium-verified:
  `:w`, `:o`, `:reload`, space-f picker with preview, JS round-trips). The
  pieces:
  - `helix-patched` gained `helix_stdx::vfs` (path-keyed byte store,
    normalized keys — paths that name no file, like `""` or `/`, are
    rejected at the boundary — atomic commit-on-flush writer; gated to
    `cfg(any(wasm32, test))` so it stays host-tested without shipping in
    native builds) plus wasm32 arms that route through it:
    `Document::open`/`reload`/`save_impl` in helix-view (saving via
    `to_writer_sync`, a synchronous sibling of `to_writer` kept
    byte-identical by the encoding tests, BOM branch included — no tokio
    fs/runtime on the save path), a vfs-backed `faccess` imp (so `readonly()` stops
    reporting every path read-only), the file picker listing the vfs
    instead of walking directories, the picker preview reading vfs
    documents, and the pickers' `Path::exists` workspace guards skipped
    (the virtual root exists; real fs can't see it). Legacy's equivalent
    forked helix-core (`storage.rs`, localStorage-backed); this one lives
    behind upstreamable wasm32 cfg arms.
  - The web crate exports `vfs_write` / `vfs_read` / `vfs_list` (kept to
    the obvious three; #18 owns any richer surface), and the demo page
    exposes them as `window.helixVfs` for the devtools console and as an
    assertion surface for browser automation (no harness is committed
    yet).
  - Deliberately **in-memory only**: nothing survives a reload. The JS
    hooks are the extension point — an embedder (or a later slice) can
    persist to localStorage/OPFS by syncing through them, without another
    helix-patched change.
  - Not covered: `:w` skips the native mtime "modified by an external
    process" check (vfs entries carry no mtimes — a JS write between open
    and save wins silently); config loading still reads the real fs on
    every target (`Config::load_default`), so `:config-open` + `:w` saves
    into the vfs but the result is never loaded (legacy routed this
    through its storage layer); `:read` always fails (typed.rs guards on
    a real-fs `path.exists()`); `:mv` silently skips the rename
    (`Editor::move_path` guards on `old_path.exists()`, so the document
    repoints while the vfs keeps the old key — the sharpest of these);
    the file *explorer* (space-e) and global search still walk the real
    fs and degrade to errors/empty; command-line path completion
    completes nothing (real fs readdir).
- **No tokio runtime is entered** — `AsyncHook` workers check
  `Handle::try_current()` and silently don't spawn (completion debounce,
  signature help, auto-save stay inert; harmless without LSP). Code paths
  that `tokio::spawn` unconditionally (`Jobs::add` for non-wait jobs)
  panic if reached; same exposure the legacy port shipped with. Saves are
  no longer in this bucket: the save future runs on the main task via the
  editor's `save_queue` and its wasm32 arm is synchronous vfs IO, so `:w`
  needs no runtime.

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
- ~~`faccess::readonly()` maps fs errors to `true`~~ → resolved on
  `helix-patched`: a wasm32 `faccess` imp answers from `helix_stdx::vfs`
  (always writable, exists = in the store).
- std path/fs queries lie on wasm32-unknown-unknown — anything gating on
  them silently takes the wrong branch. Two distinct bites:
  `Path::is_absolute()` is **always false** (`has_root() &&
  prefix().is_some()`, and only Windows has prefixes), so the url stub's
  `from_file_path` rejected every path (`Document::identifier()` then
  unwrapped None and panicked on the second `:w`); and `Path::exists()`
  is **always false** (std::fs is unsupported), so the pickers' workspace
  guards never passed. Fixed with `has_root()` in the url stub and
  cfg-gated guards on `helix-patched`; audit any new
  `is_absolute`/`exists` call sites that wasm code can reach.
- ~~nucleo's matcher threadpool~~ → `Nucleo::new` builds a dedicated rayon
  pool, and rayon can build **no custom pool** on wasm32 (thread spawn
  unsupported) — every Picker construction panicked. Resolved by
  `[patch.crates-io]` → `stubs/nucleo`, a vendored 0.5.0 whose pool
  wrapper runs match jobs inline on wasm32 (the par_iters inside then use
  rayon's documented single-threaded global fallback). Note rayon's
  fallback only covers the *global* pool — `ThreadPoolBuilder::build()`
  always errors on wasm.
- ~~`crossterm::bridge` starts at a static 80x24~~ → the web frontend's
  `start()` calls `bridge::set_size()` and injects `Event::Resize` before
  the first render. The trap remains for other embedders: size the bridge
  before booting the app.
- No `block_on` on the browser main thread; the event loop runs as a
  wasm-bindgen future — a `spawn_local`'d poll-driver (`drive()` in
  `web/src/session.rs`, PR #43) that recreates `app.event_loop_until_idle`
  each poll so the `Application` stays inspectable from JS between polls
  (previously `spawn_local(app.run(...))`). Works with pristine helix-term
  because the crossterm stub's `EventStream` and the gloo-timers `Sleep`
  are plain futures needing no runtime.
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
  target, loader/env fallbacks, wasm32 grammar/query registration in
  helix-loader, the `helix_stdx::vfs` virtual file system and the wasm32
  document-IO/picker arms that use it); Cargo git-deps point here until
  upstream PRs land.
- `legacy` (to create from current `main`) — the old in-tree port;
  reference for crossterm shim, storage, backend wiring, wasm-sysroot.
