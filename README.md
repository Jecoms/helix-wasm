<div align="center">

<h1>
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="logo_dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="logo_light.svg">
  <img alt="Helix" height="128" src="logo_light.svg">
</picture>
</h1>

</div>

# helix-wasm

The [Helix](https://github.com/helix-editor/helix) editor compiled to
`wasm32`, running entirely in the browser on top of
[xterm.js](https://xtermjs.org/) — no server, no install.

**➡️ Live demo: <https://jecoms.github.io/helix-wasm/demo/>**

## Status

This is an experimental port. What works:

- Helix compiled to `wasm32-unknown-unknown` (with a reduced feature set)
- Rendering and input through xterm.js in the browser
- Modal editing, multiple selections, and most core editing features
- Bundled as a static web app (see the live demo above)

What is disabled under `wasm32` (it would require native-only tooling or
significant design changes upstream):

- Language server support and debugging (DAP)
- Shell command execution and piping
- VCS features (git info, diffs)
- Cloning/compiling tree-sitter grammars at runtime
- Most filesystem operations — reading/writing "files" is crudely emulated
  via the Web Storage API

See [`helix-web/README.md`](./helix-web/README.md) for the full list of
limitations and known issues.

## Building and running

The wasm build lives in the [`helix-web/`](./helix-web) crate; the browser
demo app lives in `helix-web/www/`. In short:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

cd helix-web/
npm install
wasm-pack build --target web
# if wasm-bindgen-cli install fails, see helix-web/README.md

cd www/
npm install
npm run dev       # visit http://localhost:5173
```

Full setup, build, deploy, and troubleshooting notes (including platform
toolchain caveats) are in [`helix-web/README.md`](./helix-web/README.md).

Native builds of the editor itself should still work — see the upstream
[installation documentation](https://docs.helix-editor.com/install.html).

## Using the editor

Helix is a [Kakoune](https://github.com/mawww/kakoune) /
[Neovim](https://github.com/neovim/neovim) inspired modal editor written in
Rust. For editor usage, see the upstream
[documentation](https://docs.helix-editor.com/) and
[keymap reference](https://docs.helix-editor.com/keymap.html) — the port
follows upstream behavior wherever the feature isn't disabled under wasm.

## Credits & upstream

- [helix-editor/helix](https://github.com/helix-editor/helix) — the Helix
  editor itself, of which this repo is a fork (MPL-2.0).
- [makemeunsee/helix](https://github.com/makemeunsee/helix), branch
  [`wasm32`](https://github.com/makemeunsee/helix/tree/wasm32) — the original
  wasm port this repo branched from, which in turn builds on
  [rrbutani](https://github.com/rrbutani)'s
  [crossterm/xterm.js integration](https://github.com/rrbutani/crossterm/tree/xtermjs).

## License

[MPL-2.0](./LICENSE), unchanged from upstream Helix.
