# helix-wasm

A wasm32 port of [Helix](https://github.com/helix-editor/helix) that consumes
**pristine upstream source at a release tag** — no fork, no vendored tree.
All wasm accommodation lives on this side of the fence, so updating to a new
Helix release is a tag bump, not a patch-set rebase.

## Layout

| Path | Purpose |
| --- | --- |
| `Cargo.toml` | Wrapper workspace: helix crates as git dependencies pinned to the `helix-patched` branch, plus `[patch.crates-io]` stub swaps |
| `stubs/` | Stand-ins for dependencies with no wasm32 support: transitive crates (`home`, `which`, `libloading`, and `url` with a wasm cfg), vendored copies of `helix-lsp`/`helix-dap` with the server-subprocess machinery removed, and a vendored `crossterm` whose OS terminal layer is replaced by a browser bridge |
| `sysroot/` | Stub libc headers, the `wasm-cc` clang shim that lets tree-sitter's stock build script compile its C for wasm32, and the libc shim implementations (`shims.c`, `wctype.c`) the final wasm link needs |
| `web/` | The browser frontend: a wasm-bindgen cdylib that boots helix-term against the crossterm bridge, plus the xterm.js host page in `web/www/` |
| `.cargo/config.toml` | Wires `wasm-cc` up as the C compiler for the wasm32 target |
| `SPIKE-NOTES.md` | The full build recipe, enumerated blockers, and known runtime traps |

## Building

```sh
rustup target add wasm32-unknown-unknown
cargo check -p helix-core --target wasm32-unknown-unknown
cargo check -p helix-view --target wasm32-unknown-unknown
cargo check -p helix-term --target wasm32-unknown-unknown
```

A clang that can emit wasm is required. Linux distro clang works; on macOS the
system clang cannot emit wasm, so install LLVM (`brew install llvm`) or point
`HELIX_WASM_CLANG` at a suitable clang.

## Running the browser demo

```sh
wasm-pack build web --target web
cd web/www
npm install
npm run dev      # serves the demo on a local vite dev server
```

The demo boots helix into an xterm.js terminal with a scratch buffer, with
syntax highlighting for a small static grammar set (c, go, java,
javascript, python, regex, rust, toml — try `:set-language rust`). Nothing persists — see SPIKE-NOTES.md for the
current limitations. The grammar build fetches pinned parser sources at
build time, so it needs network access and `git`. Set `HELIX_WEB_GRAMMARS`
to a comma-separated subset (e.g. `HELIX_WEB_GRAMMARS=rust,toml wasm-pack
build web --target web`) to slim the bundle; to add a grammar to the
catalog, see `GRAMMARS` in `web/build.rs` and `web/queries/README.md`.

## Live demo

The demo deploys to <https://jecoms.github.io/helix-wasm/> via the
`Deploy web demo` workflow (`.github/workflows/web_demo.yml`), which builds
the full-catalog bundle and publishes it with `actions/deploy-pages`.

> **Pending the first deploy from `v2`** — until that happens the URL still
> serves the legacy port's demo. Deploys are gated by the `github-pages`
> environment's deployment branch policy: a run only deploys if its branch
> is on the allowed list, so allowing `v2` (repo Settings → Environments →
> github-pages, or one `gh api` call) is the switch that turns the workflow
> on.

## Branch map

- `v2` (this branch) — the zero-fork port; becomes `main` once the browser
  demo boots ([#33](https://github.com/Jecoms/helix-wasm/issues/33)).
- `helix-patched` — upstream Helix at the release tag plus the few
  not-yet-upstreamed fixes (the `faccess` fallback fix,
  [helix-editor/helix#16186](https://github.com/helix-editor/helix/pull/16186);
  wasm32 trims of the subprocess and signal machinery in helix-view and
  helix-term; repairs to helix-view's bit-rotted wasm32 clipboard/terminal
  fallbacks; the web-time clock swap; browser-timeout editor timers; the
  bridge render target; wasm32 fallbacks for the working directory and
  loader paths; and the wasm32 grammar/query registration API in
  helix-loader).
  Retires as upstream PRs land.
- `main` — the previous in-tree port, to be archived as `legacy` at the swap.
