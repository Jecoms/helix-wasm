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

The demo boots helix on a scratch buffer, with syntax highlighting for a
static grammar set and a curated set of bundled color schemes
(`THEME_CATALOG` in `web/build.rs` — try `:theme gruvbox`). `:tutor` works,
with a handful of steps the browser cannot honor as written;
`web/runtime/README.md` lists those under "Known gaps in the browser".

A build links the **default grammar set** (`DEFAULT_GRAMMARS` in
`web/build.rs`): bash, c, css, go, html, java, javascript, json, markdown,
markdown_inline, python, regex, rust, toml, tsx, typescript. That is what the
demo runs and what the release's `helix-web-<version>.tar.gz` ships. The
**catalog** (`GRAMMARS`, pinned to the same revisions as helix's own
`languages.toml`) is larger — it adds c-sharp, clojure, cpp, diff,
dockerfile, elixir, git-config, git-rebase, gitattributes, gitignore,
haskell, hcl, heex, ini, kotlin, lua, make, nix, ocaml, scala, scss, sql,
swift, xml, zig — and everything beyond the default set is opt-in, because a
grammar's parser is anywhere from a few KB to 5 MB of wasm and the seven
largest alone would triple the bundle. A grammar serves every helix language
that uses it — `json` also covers `jsonc`, `bash` covers `env`, `hcl` covers
`tfvars`, `markdown_inline` is the injection target `markdown` needs — and
the build embeds the queries for all of them.

`HELIX_WEB_GRAMMARS` picks the set. It is a comma-separated list of catalog
names and two aliases, `default` and `full`, and the build links their
union:

```sh
HELIX_WEB_GRAMMARS=rust,toml wasm-pack build web --target web        # two grammars
HELIX_WEB_GRAMMARS=default,kotlin,cpp wasm-pack build web --target web  # the default set plus two
HELIX_WEB_GRAMMARS=full wasm-pack build web --target web             # the whole catalog
```

The release attaches the last of those too, as
`helix-web-<version>-full.tar.gz`, for embedders who want every grammar
without building (see [Embedding the editor](embedding.md)). To add a grammar
to the catalog, add a row to `GRAMMARS`. Only grammars whose external scanner
is C can be linked — the wasm C toolchain here has no C++ sysroot — so a
grammar that still ships a `scanner.cc` at helix's pin (php, ruby, yaml,
cmake at 25.07.1) fails the build with a message saying so; `gitcommit` is
left out for a different reason (its generated `parser.c` takes clang
twenty minutes and nine gigabytes to compile for wasm). The queries and
themes the bundle embeds are read out of the in-tree port at
`helix/runtime/`, not copied into `web/` — see `web/queries/README.md` and
`web/themes/README.md` for how the build picks which of them to embed.
