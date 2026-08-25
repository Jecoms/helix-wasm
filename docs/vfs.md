# Virtual file system and state inspection

Documents live in an in-memory virtual file system (`helix_stdx::vfs`, part
of the wasm patch set): `:w /notes.txt` saves there, `:o` (with path
completion) and the `<space>f` file picker (with preview) read from it,
`<space>/` global search greps it, and `:reload` picks up outside
changes. Nothing survives a page reload, and a few document commands
behave differently against it — see [Files live in an in-memory VFS](limitations.md#files-live-in-an-in-memory-vfs).
The wasm module exports `vfs_write` / `vfs_read` / `vfs_list` /
`vfs_delete` so an embedding page can inject, extract and drop files; the demo page exposes them
as `window.helixVfs` — try `helixVfs.write("hello.rs", "fn main() {}")`
in the devtools console, then `:o hello.rs` in the editor. Persistent
backends (localStorage/OPFS) can be layered on those hooks by the host
page. Boot seeds a couple of sample files (`web/src/samples.rs`) so the
picker opens on something selectable, and a `config.toml` the host page
supplied lands in the store the same way — see [Configuration](limitations.md#configuration).

`:download` is the way *out* that needs no script
([#67](https://github.com/Jecoms/helix-wasm/issues/67)): it hands the file
you are editing to the page, which saves it to your machine the way any
other download arrives. `:download` alone uses the buffer's own name and
`:download notes.txt` names it, which is also how you get an unnamed
scratch buffer out. See [Files live in an in-memory VFS](limitations.md#files-live-in-an-in-memory-vfs) for what it
does not do.

`:download-all` is the same door for a whole session
([#110](https://github.com/Jecoms/helix-wasm/issues/110)): it packs
everything this session saved into one zip — `helix-session.zip` unless you
name it, `:download-all work.zip` — and hands that to the page. The archive
holds the *store*, so it refuses while a buffer is unsaved and names what to
`:w`; `:download-all!` builds it anyway, with those buffers at their last
saved state. What boot seeded is never in it, edited or not — see [Files
live in an in-memory VFS](limitations.md#files-live-in-an-in-memory-vfs), which is the section to read before you
edit a bundled theme and expect it back.

`:remove` (`:rm`) is the way a file leaves the store
([#132](https://github.com/Jecoms/helix-wasm/issues/132)): native helix has
no delete command because `:sh rm` is always there, and here it never is.
`:remove` deletes the current file and closes its buffer in one act,
`:remove notes.txt` names another key, and `:remove!` goes ahead over unsaved
changes. It only works on a page that registered `on_remove` — deletion is a
per-page capability, like `:download` — and the demo page does. See [Files
live in an in-memory VFS](limitations.md#files-live-in-an-in-memory-vfs) for the edges.

## Editor state inspection

The wasm module also exports a read-only inspection surface
([#18](https://github.com/Jecoms/helix-wasm/issues/18)) so embedding pages
(interactive docs, tutorials, test harnesses) can poll editor state instead
of scraping the rendered terminal: `editor_state()` returns
`{ mode, theme, path, cursor: { row, col }, selections: [{ anchor, head }] }`
for the focused view (`theme` is the name of the theme in effect — what
`:theme` last set, previews included, or `"default"`), and `editor_text()` returns
the live buffer text — unsaved edits included, unlike `vfs_read`, which sees what
was last saved.
The demo page exposes them as `window.helixState` — try
`helixState.state()` in the devtools console while switching modes. Both
return `undefined` when helix is not running; see `web/src/inspect.rs` for
the coordinate semantics (0-based rows, grapheme-cluster cols, char-index
anchors/heads).
