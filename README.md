# helix-wasm

A wasm32 port of [Helix](https://github.com/helix-editor/helix) that consumes
**pristine upstream source at a release tag** — no fork, no vendored tree.
All wasm accommodation lives on this side of the fence, so updating to a new
Helix release is a tag bump, not a patch-set rebase.

## Layout

| Path | Purpose |
| --- | --- |
| `Cargo.toml` | Wrapper workspace: helix crates as git dependencies pinned to a frozen `helix/<version>` snapshot ref, plus `[patch.crates-io]` stub swaps |
| `stubs/` | Stand-ins for dependencies with no wasm32 support: transitive crates (`home`, `which`, `libloading`, and `url` with a wasm cfg), vendored copies of `helix-lsp`/`helix-dap` with the server-subprocess machinery removed, a vendored `crossterm` whose OS terminal layer is replaced by a browser bridge, and a vendored `nucleo` that runs picker matching inline instead of on a threadpool |
| `sysroot/` | Stub libc headers, the `wasm-cc` clang shim that lets tree-sitter's stock build script compile its C for wasm32, and the libc shim implementations (`shims.c`, `wctype.c`) the final wasm link needs |
| `web/` | The browser frontend: a wasm-bindgen cdylib that boots helix-term against the crossterm bridge, plus the xterm.js host page in `web/www/` |
| `.cargo/config.toml` | Wires `wasm-cc` up as the C compiler for the wasm32 target |
| `scripts/` | Release tooling: `snapshot-helix.sh` cuts the append-only `helix/<version>` snapshot refs that `Cargo.toml` pins, plus their signed `helix-<version>` attestation tags |

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
javascript, python, regex, rust, toml — try `:set-language rust`).
Notable limitations: no LSP or DAP (the browser has no subprocesses), and
documents live in an in-memory virtual file system.
The grammar build fetches pinned parser sources at
build time, so it needs network access and `git`. Set `HELIX_WEB_GRAMMARS`
to a comma-separated subset (e.g. `HELIX_WEB_GRAMMARS=rust,toml wasm-pack
build web --target web`) to slim the bundle; to add a grammar to the
catalog, see `GRAMMARS` in `web/build.rs` and `web/queries/README.md`.

### Virtual file system

Documents live in an in-memory virtual file system (`helix_stdx::vfs`, part
of the wasm patch set): `:w /notes.txt` saves there, `:o` and the space-f
file picker (with preview) read from it, and `:reload` picks up outside
changes. Nothing survives a page reload. The wasm module exports
`vfs_write` / `vfs_read` / `vfs_list` so an embedding page can inject and
extract files; the demo page exposes them as `window.helixVfs` — try
`helixVfs.write("hello.rs", "fn main() {}")` in the devtools console, then
`:o hello.rs` in the editor. Persistent backends (localStorage/OPFS) can be
layered on those hooks by the host page.

### Editor state inspection

The wasm module also exports a read-only inspection surface
([#18](https://github.com/Jecoms/helix-wasm/issues/18)) so embedding pages
(interactive docs, tutorials, test harnesses) can poll editor state instead
of scraping the rendered terminal: `editor_state()` returns
`{ mode, path, cursor: { row, col }, selections: [{ anchor, head }] }` for
the focused view, and `editor_text()` returns the live buffer text —
unsaved edits included, unlike `vfs_read`, which sees what was last saved.
The demo page exposes them as `window.helixState` — try
`helixState.state()` in the devtools console while switching modes. Both
return `undefined` when helix is not running; see `web/src/inspect.rs` for
the coordinate semantics (0-based rows, grapheme-cluster cols, char-index
anchors/heads).

### Browser smoke tests

A Playwright suite (`web/www/tests/`) boots the built bundle in headless
Chromium and asserts on editor behavior through `helixState` / `helixVfs`
and the terminal buffer — the same checks CI runs in the `wasm32 check`
workflow. Run it against a fresh build:

```sh
wasm-pack build web --target web
cd web/www
npm install
npm run build                      # tests run against dist/, not the dev server
npx playwright install chromium    # first run only
npm test
```

## Live demo

The demo deploys to <https://jecoms.github.io/helix-wasm/> via the
`Deploy web demo` workflow (`.github/workflows/web_demo.yml`), which builds
the full-catalog bundle and publishes it with `actions/deploy-pages`.

Every push to `main` deploys automatically; a manual
`gh workflow run web_demo.yml` works too. Deploys are gated by the
`github-pages` environment's deployment branch policy — only `main` is on
the allowed list.

## Embedding the editor

Each `web-v<semver>` tag publishes the `web/pkg` wasm-pack output — the same
unit the demo consumes as `file:../pkg` — as a GitHub release with a
`helix-web-<version>.tar.gz` attached (the `Publish web bundle` workflow,
`.github/workflows/web_release.yml`). Embedders should pin one of those
instead of linking into the deployed demo, whose asset names are
content-hashed and replaced on every push to `main`:

```sh
curl -LO https://github.com/Jecoms/helix-wasm/releases/download/web-v0.1.0/helix-web-0.1.0.tar.gz
tar xzf helix-web-0.1.0.tar.gz    # extracts helix-web-0.1.0/
```

The extracted directory is a standard wasm-pack `--target web` package (ES
module + `.wasm` + `.d.ts`, plus `NOTICE.md` with the license notices for
the statically linked grammar parsers). Consume it the way the demo's
`web/www/package.json` does:

```json
"dependencies": { "helix-web": "file:../helix-web-0.1.0" }
```

`web/www/main.js` is the reference host wiring to replicate: call `init()`
(fetches and instantiates the wasm module), then `start(writeBytes, cols,
rows)` with a callback that feeds editor output bytes to an xterm.js
`Terminal`, and forward input with `key_event(...)`, `paste(...)`, and
`resize(cols, rows)`. Beyond the terminal loop, the module exports the
file-injection hooks (`vfs_write` / `vfs_read` / `vfs_list`, see "Virtual
file system" above) and the read-only inspection surface (`editor_state()`
/ `editor_text()`, see "Editor state inspection") — the intended surface
for tutorial-style embedders that drive and assert on the editor rather
than scrape the rendered terminal. The JS surface is unstable by design
(`web/src/session.rs`, `web/src/vfs.rs`), with one exception: the
read-only inspection surface (`web/src/inspect.rs`,
[#18](https://github.com/Jecoms/helix-wasm/issues/18)) is meant to be
kept stable. Either way, pin a tagged tarball and check its `.d.ts` when
upgrading.

To cut a release: bump `version` in `web/Cargo.toml`, merge, then tag that
commit `web-v<version>` and push the tag. The workflow verifies the tag
against the crate version, rebuilds the bundle with `--locked`, and
attaches the tarball to a release on the tag.

## Branch and tag map

- `main` (this branch) — the zero-fork port, produced by the
  [#33](https://github.com/Jecoms/helix-wasm/issues/33) restructure.
- `helix-patched` — the moving workbench: upstream Helix at the release tag
  plus the few not-yet-upstreamed fixes (the `faccess` fallback fix,
  [helix-editor/helix#16186](https://github.com/helix-editor/helix/pull/16186);
  wasm32 trims of the subprocess and signal machinery in helix-view and
  helix-term; repairs to helix-view's bit-rotted wasm32 clipboard/terminal
  fallbacks; the web-time clock swap; browser-timeout editor timers; the
  bridge render target; wasm32 fallbacks for the working directory and
  loader paths; the wasm32 grammar/query registration API in
  helix-loader; and the `helix_stdx::vfs` virtual file system with the
  wasm32 document-IO and picker arms that use it).
  Rebased onto each new upstream release — a moving target, so it is not a
  build input. Retires as upstream PRs land.
- `helix/<version>` (e.g. `helix/25.07.1`) — append-only snapshot refs, the
  actual build inputs: each is a single parentless commit of the workbench's
  tree at that release, cut by `scripts/snapshot-helix.sh`. Pinning these
  keeps a fresh `cargo fetch` to one commit + one tree (no upstream history)
  and keeps old lockfiles buildable when the workbench rebases. A snapshot
  is never regenerated once pinned; a changed patch set against the same
  upstream base gets a revision suffix (`helix/25.07.1-r2`, ...). Note: the
  repo also carries upstream's pristine `25.07.1` tag — branch
  `helix/25.07.1` intentionally does **not** match it (patches applied).
  "Pristine" in the opening above means no helix source is vendored or
  rewritten in this repo, not byte-identity with the tag: the snapshot is
  upstream's tree plus the transiting patch set, which retires as its PRs
  land. Snapshot commits are unsigned by design — a signature would tie
  the reproducible SHA to a signing key — so each carries a signed
  annotated tag `helix-<version>` (dash, not slash, which would make the
  refname ambiguous with the branch) as its attestation. Both namespaces
  are frozen by creation-only rulesets (`snapshot-branches-frozen` /
  `snapshot-tags-frozen`, no bypass actors): new snapshot refs can be
  pushed, existing ones can never be moved or deleted.
- `web-v<semver>` (e.g. `web-v0.1.0`) — release tags for the embeddable web
  bundle. Pushing one runs the `Publish web bundle` workflow
  (`.github/workflows/web_release.yml`), which checks the tag against
  `web/Cargo.toml`'s `version`, rebuilds the full-catalog `web/pkg`
  wasm-pack output, and attaches it to a GitHub release as
  `helix-web-<version>.tar.gz` — the artifact "Embedding the editor" above
  pins.
- `legacy` — the previous in-tree port, archived at the swap.
