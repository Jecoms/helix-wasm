# Building from source

```sh
rustup target add wasm32-unknown-unknown
wasm-pack build web --target web
cd web/www
npm install
npm run dev      # serves the demo on a local vite dev server
```

That needs a Rust toolchain, [wasm-pack](https://rustwasm.github.io/wasm-pack/)
and Node. The grammar build compiles C for wasm32 as well, so a clang that can
emit wasm is required: Linux distro clang works, but on macOS the system clang
cannot emit wasm, so install LLVM (`brew install llvm`) or point
`HELIX_WASM_CLANG` at a suitable clang. It also fetches pinned parser sources
at build time, so it needs network access and `git`.

The `web/pkg` a local build produces will not be byte-identical to the
published one. `wasm-pack` shrinks its output with `wasm-opt`, preferring one
already on `$PATH` and otherwise downloading binaryen itself, and `wasm-opt`'s
output depends on its version. CI, the Pages deploy and the release tarball all
pin **binaryen 132** (`.github/actions/wasm-opt`); put that version's `wasm-opt`
on `$PATH` to reproduce their bytes locally.

## What the bundle ships with

The demo boots helix on a scratch buffer, with syntax highlighting for a small
static grammar set (c, go, java, javascript, python, regex, rust, toml — try
`:set-language rust`) and a curated set of bundled color schemes
(`THEME_CATALOG` in `web/build.rs` — try `:theme gruvbox`). `:tutor` works,
with a handful of steps the browser cannot honor as written;
`web/runtime/README.md` lists those under "Known gaps in the browser".

Set `HELIX_WEB_GRAMMARS` to a comma-separated subset (e.g.
`HELIX_WEB_GRAMMARS=rust,toml wasm-pack build web --target web`) to slim the
bundle; to add a grammar to the catalog, see `GRAMMARS` in `web/build.rs`. The
queries and themes the bundle embeds are read out of the in-tree port at
`helix/runtime/`, not copied into `web/` — see `web/queries/README.md` and
`web/themes/README.md` for how the build picks which of them to embed.
