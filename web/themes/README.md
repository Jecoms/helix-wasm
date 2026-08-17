# The bundled themes

No theme files live in this directory. The ones the wasm bundle embeds are
helix's own, read straight out of the in-tree port at
`../../helix/runtime/themes/<name>.toml`: `../build.rs` embeds each one and
the frontend seeds them into the virtual file system's runtime themes
directory at startup, where helix's theme loader finds them (`:theme` — see
`../src/themes.rs`).

## Which themes get embedded

The selection is the `THEMES` catalog in `../build.rs`. Unlike the query
set — derived from the grammar catalog's `; inherits:` closure — this one is
a judgement call and so is written out: helix ships far more themes than a
browser bundle wants to carry, and the ten listed are chosen to cover
distinct palettes, including two light themes (`catppuccin_latte`,
`onelight`).

To add a theme, add its file stem to `THEMES`. A theme that `inherits` from
another needs its parent in the catalog too — `catppuccin_latte` inherits
`catppuccin_mocha` — and the build asserts that closure, because an
unresolved parent surfaces at runtime only as a theme that silently refuses
to load. The built-in `default` and `base16_default` parents are always
available and need no entry.

The themes are helix's own files (MPL-2.0, like helix itself); the notice
lives in `../NOTICE.md`.
