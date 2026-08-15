# Vendored tree-sitter queries

Pristine copies of `runtime/queries/<lang>/` from this repo's
`helix-patched` branch (helix's own query files), for the languages in the
static grammar set (`GRAMMARS` in `../build.rs`). The build script embeds
every `.scm` file found here and the frontend registers them with
`helix_loader` at startup.

Do not edit these files; re-copy them from the pinned `helix-patched`
revision when bumping the helix tag, and add a directory here when adding a
grammar to the set.
