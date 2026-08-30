# The bundled tree-sitter queries

No query files live in this directory. The ones the wasm bundle embeds are
helix's own, read straight out of the in-tree port at
`../../helix/runtime/queries/<lang>/`: `../build.rs` embeds every `.scm`
file it finds there for the selected languages, and the frontend registers
them with `helix_loader` at startup (`../src/grammars.rs`).

## Which languages get embedded

Upstream ships queries for hundreds of languages; the bundle carries a
subset, and that subset is **derived, not listed**. Starting from the
grammars the build links — `DEFAULT_GRAMMARS` in `../build.rs` when
`HELIX_WEB_GRAMMARS` is unset, otherwise the union that variable names
(catalog names plus the `default` and `full` aliases, so it widens as well
as narrows) — it is:

- every language in `../../helix/languages.toml` that uses a selected
  grammar (its `grammar = "..."` key, or its name when it has none), plus
- every language those languages' query files `; inherits:` from,
  transitively.

The first step matters because languages and grammars are not one-to-one:
queries are keyed by language, and `markdown.inline` uses the
`markdown_inline` grammar, `jsonc` and `json` share one, `jsx` rides on
`javascript`. helix reads queries by language name, so a language whose
grammar is linked but whose queries were never embedded opens with no
highlighting. A language upstream has no queries for is skipped (helix
reads empty queries for it natively too).

The closure matters because a directive can name a query-only base language
with no grammar of its own — `javascript` inherits from `ecma` and
`_javascript` — and a missing base is invisible at runtime:
`load_runtime_file` feeds `unwrap_or_default()`, so the inherited part of
the query comes back silently empty and highlighting quietly degrades. A
base the port has no queries for fails the build instead
(`query_languages` in `../build.rs`).

So adding a grammar is a one-line edit to `GRAMMARS`: the languages it
serves, their queries and their bases are found in the port automatically,
and a helix version bump carries them along with the tree instead of leaving
a stale copy behind.

License and attribution notices for the grammar C sources these queries
pair with (statically linked into the shipped wasm) live in `../NOTICE.md`.
