# The bundled tree-sitter queries

No query files live in this directory. The ones the wasm bundle embeds are
helix's own, read straight out of the in-tree port at
`../../helix/runtime/queries/<lang>/`: `../build.rs` embeds every `.scm`
file it finds there for the selected languages, and the frontend registers
them with `helix_loader` at startup (`../src/grammars.rs`).

## Which languages get embedded

Upstream ships queries for hundreds of languages; the bundle carries a
subset, and that subset is **derived, not listed**. It is the `; inherits:`
closure over the grammars the build links — `GRAMMARS` in `../build.rs`, or
the subset `HELIX_WEB_GRAMMARS` narrows that to:

- each selected grammar's own directory, plus
- every language its query files `; inherits:` from, transitively.

The closure matters because a directive can name a query-only base language
with no grammar of its own — `javascript` inherits from `ecma` and
`_javascript` — and a missing base is invisible at runtime:
`load_runtime_file` feeds `unwrap_or_default()`, so the inherited part of
the query comes back silently empty and highlighting quietly degrades. A
language the port has no queries for fails the build instead
(`query_languages` in `../build.rs`).

So adding a grammar is a one-line edit to `GRAMMARS`: its queries and their
bases are found in the port automatically, and a helix version bump carries
them along with the tree instead of leaving a stale copy behind.

License and attribution notices for the grammar C sources these queries
pair with (statically linked into the shipped wasm) live in `../NOTICE.md`.
