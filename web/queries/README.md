# Vendored tree-sitter queries

Pristine copies of `runtime/queries/<lang>/` from this repo's pinned
`helix/<version>` snapshot (helix's own query files), for the languages in the
static grammar set (`GRAMMARS` in `../build.rs`). The build script embeds
every `.scm` file found here and the frontend registers them with
`helix_loader` at startup.

Some directories are query-only bases with no grammar of their own,
pulled in by `; inherits:` directives (javascript inherits from `ecma` and
`_javascript`); every directory here is registered, grammar or not.

Do not edit these files; re-copy them from the new `helix/<version>`
snapshot when bumping the helix tag. When adding a grammar to the set, add
its directory here plus any directory its queries `; inherits:` from.

License and attribution notices for the grammar C sources these queries
pair with (statically linked into the shipped wasm) live in `../NOTICE.md`.
