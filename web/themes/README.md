# Vendored themes

Pristine copies of `runtime/themes/<name>.toml` from this repo's pinned
`helix/<version>` snapshot (helix's own theme files): a curated set chosen
to cover distinct palettes, including two light themes (`catppuccin_latte`,
`onelight`). The build script embeds every `.toml` file found here and the
frontend seeds them into the virtual file system's runtime themes
directory at startup, where helix's theme loader finds them (`:theme`).

A theme that `inherits` from another must have its parent vendored here
too — the build script asserts this closure, like it does for query
`; inherits:` directives (`catppuccin_latte` inherits `catppuccin_mocha`;
the built-in `default` and `base16_default` parents are always available).

Do not edit these files; re-copy them from the pinned `helix/<version>`
revision when bumping the helix pin. To add a theme, drop its `.toml`
(plus any `inherits` parent) here.

The themes are helix's own files (MPL-2.0, like helix itself); the notice
lives in `../NOTICE.md`.
