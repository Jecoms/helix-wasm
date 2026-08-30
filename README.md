# helix-wasm

[Helix](https://github.com/helix-editor/helix) compiled to `wasm32`, running in
a browser tab. The editor drives an xterm.js terminal through a wasm-bindgen
module, with modal editing, tree-sitter highlighting, themes, pickers and
`:tutor` all present. What the browser cannot give it — a subprocess, a real
filesystem, a thread — is either adapted or missing; the
[limitations catalog](docs/limitations.md) lists every one of those.

**Live demo: <https://jecoms.github.io/helix-wasm/>**

**Helix tutorial site: <https://helix.manycoolprojects.com/>**

## Quick start

Run the demo locally (needs a Rust toolchain, [wasm-pack](https://rustwasm.github.io/wasm-pack/),
Node, and a clang that can emit wasm — on macOS, `brew install llvm`):

```sh
rustup target add wasm32-unknown-unknown
wasm-pack build web --target web
cd web/www
npm install
npm run dev      # serves the demo on a local vite dev server
```

Or embed a released bundle in your own page:

```sh
curl -LO https://github.com/Jecoms/helix-wasm/releases/download/web-v0.0.5/helix-web-0.0.5.tar.gz
tar xzf helix-web-0.0.5.tar.gz    # a standard wasm-pack --target web package
```

Call `init()`, then `start(writeBytes, cols, rows, config, languages)` with a
callback that feeds output bytes to an xterm.js `Terminal`, and forward input
with `key_event(...)`, `paste(...)` and `resize(cols, rows)`. `web/www/main.js`
is the reference host wiring; [Embedding the editor](docs/embedding.md) covers
the rest (download/remove callbacks, language servers over a Web Worker, the
VFS and inspection exports, cutting a release).

## What you get

The demo boots on a scratch buffer with syntax highlighting for a static
grammar set of sixteen grammars (try `:set-language rust`; a catalog of
forty-one is there to opt into), a curated
set of bundled themes (try `:theme gruvbox`), and a working `:tutor`. Files
live in an in-memory virtual file system — `:w`, `:o`, the file picker and
global search all use it — and `:download` / `:download-all` get them back
out. Nothing survives a page reload.

## Documentation

- [Building from source](docs/building.md) — toolchain details, reproducible
  `wasm-opt` bytes, trimming or extending the grammar and theme catalog
- [Virtual file system and state inspection](docs/vfs.md) — the VFS,
  `:download`, `:download-all`, `:remove`, and the `editor_state()` /
  `editor_text()` inspection surface
- [Limitations and behavioral differences](docs/limitations.md) — everything
  the browser changes: no subprocesses, the VFS, configuration, no background
  work, terminal and browser differences, bundled content only
- [Embedding the editor](docs/embedding.md) — pinning a release, host wiring,
  callbacks, language servers, the JS surface's stability, cutting a release
- [Working on the port](docs/development.md) — repository layout, checking
  the crates, patching helix, browser smoke tests, deploying the demo, taking
  a new helix release, the branch and tag map
- [CHANGELOG](CHANGELOG.md) — what changed between two bundle releases

## Credits and license

- [helix-editor/helix](https://github.com/helix-editor/helix) — the editor
  itself, MPL-2.0. Its [documentation](https://docs.helix-editor.com/) is the
  place to learn helix; all of it applies here except what the
  [limitations catalog](docs/limitations.md) carves out.
- [makemeunsee/helix](https://github.com/makemeunsee/helix), branch `wasm32` —
  the browser port this one grew out of.

The port keeps upstream's MPL-2.0 ([`LICENSE`](LICENSE), byte-identical to
`helix/LICENSE`). Vendored dependencies under `stubs/` keep their own
licenses, and `web/NOTICE.md` carries the notices for everything the bundle
ships — see [License notes](docs/license.md) for the details.
