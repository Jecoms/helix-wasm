# helix-wasm

[Helix](https://github.com/helix-editor/helix) compiled to `wasm32`, running in
a browser tab: the editor drives an xterm.js terminal through a wasm-bindgen
module, with modal editing, tree-sitter highlighting, themes, pickers and
`:tutor` all present. What the browser cannot give it — a subprocess, a real
filesystem, a thread — is either adapted or missing, and
"[Limitations and behavioral differences](#limitations-and-behavioral-differences)"
lists every one of those.

**Live demo: <https://jecoms.github.io/helix-wasm/>**

## Run it locally

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

## Virtual file system

Documents live in an in-memory virtual file system (`helix_stdx::vfs`, part
of the wasm patch set): `:w /notes.txt` saves there, `:o` (with path
completion) and the `<space>f` file picker (with preview) read from it, and
`:reload` picks up outside changes. Nothing survives a page reload, and a few
document commands behave differently against it — see "Files live in an
in-memory VFS" below.
The wasm module exports `vfs_write` / `vfs_read` / `vfs_list` so an
embedding page can inject and extract files; the demo page exposes them
as `window.helixVfs` — try `helixVfs.write("hello.rs", "fn main() {}")`
in the devtools console, then `:o hello.rs` in the editor. Persistent
backends (localStorage/OPFS) can be layered on those hooks by the host
page. Boot seeds a couple of sample files (`web/src/samples.rs`) so the
picker opens on something selectable, and a `config.toml` the host page
supplied lands in the store the same way — see "Configuration" below.

`:download` is the way *out* that needs no script
([#67](https://github.com/Jecoms/helix-wasm/issues/67)): it hands the file
you are editing to the page, which saves it to your machine the way any
other download arrives. `:download` alone uses the buffer's own name and
`:download notes.txt` names it, which is also how you get an unnamed
scratch buffer out. See "Files live in an in-memory VFS" below for what it
does not do.

## Editor state inspection

The wasm module also exports a read-only inspection surface
([#18](https://github.com/Jecoms/helix-wasm/issues/18)) so embedding pages
(interactive docs, tutorials, test harnesses) can poll editor state instead
of scraping the rendered terminal: `editor_state()` returns
`{ mode, path, cursor: { row, col }, selections: [{ anchor, head }] }` for
the focused view, and `editor_text()` returns the live buffer text —
unsaved edits included, unlike `vfs_read`, which sees what was last saved.
The demo page exposes them as `window.helixState` — try
`helixState.state()` in the devtools console while switching modes. Both
return `undefined` when helix is not running; see `web/src/inspect.rs` for
the coordinate semantics (0-based rows, grapheme-cluster cols, char-index
anchors/heads).

## Limitations and behavioral differences

This section catalogs where the browser changes what helix does. Everything
below was reproduced by hand in a build of this tree, except where an entry
says otherwise — it is what the port does today, not what its source suggests
it might do. The entries track `helix/` plus the `web/` crate, so re-check them
when the port is replayed onto a new release (see "Taking a new helix
release").

### No subprocesses

Everything that shells out is gone. These fail cleanly, reusing the message
helix already has for "not configured":

| What | What you get |
| --- | --- |
| Language servers — diagnostics, hover, rename, code actions | "No configured language server supports …"; `:lsp-restart` → "LSP not defined for the current document" |
| The goto commands — `gd`, `gD`, `gy`, `gi` and `gr` | "No definition found." / "No references found." — helix's own empty-result message, since these five queue their request before checking for a server. Native helix with no server configured says the same |
| Debugging — `:debug-start`, `:debug-remote`, the rest of the DAP layer | "No debug adapter available for language" |
| External formatters, including format-on-save (`:format`) | "A formatter isn't available, and no language server provides formatting capabilities" |
| Shell commands — `:sh`, `:!`, `:run-shell-command`, the `!` and `\|` keys, `:pipe`, `:pipe-to`, `:insert-output`, `:append-output` | "Shell commands are not supported on this platform" |
| Opening a URL — `gf` with the cursor on one | "Opening URLs in an external program is not supported on this platform" — there is no handoff to the browser either, so nothing here can open a URL. `gf` on a file path works normally |
| Git — the diff gutter, `:reset-diff-change`, the `<space>g` changed-file picker | "Diff is not available in the current buffer" / "Current working directory does not exist" |

One language-server feature is not in that table because it fails some other
way: completion goes quiet rather than saying anything at all (see "No
background work").

`<space>/` (global search) opens its picker, but the query handler it needs
is never spawned, so typing never returns a match. Dynamic grammar loading is
out as well (`libloading` is stubbed), so the grammar set is whatever was
linked at build time.

There is no command line either: the host page boots the module with default
arguments, so `hx <file>`, `-c` and `--tutor` have no equivalent. `:tutor`
itself works.

### Files live in an in-memory VFS

Documents are keys in `helix_stdx::vfs`, not files (see "Virtual file system"
above). What that changes:

- **Nothing survives a page reload.** Pull anything you care about out
  first: `:download` saves the current file to your machine, and
  `helixVfs.read` / `helixVfs.list` hand the store to a page script.
- **`:download` exports one file, as it stands in the buffer.** Unsaved
  edits included — it is a copy of what you are looking at, not of what the
  store last saw, and it changes neither. That means it also skips the
  transforms `:w` applies on its way out (`insert-final-newline` and the two
  trims edit the *document*, and an export has no business doing that), and
  it writes UTF-8 whatever `:encoding` says: save a windows-1252 buffer and
  the store gets one byte for `é` where the download gets two. The name
  comes from the argument or the buffer, minus any directories — a download
  lands wherever your browser puts downloads. There is no whole-store
  export; a scratch buffer with no name is refused rather than given one.
  Saving is the host page's half (`on_download`, see "Embedding the
  editor"), so a page whose handler throws reports what it threw, and one
  that registers no handler at all gets "Could not download …: this host
  cannot save files" — that last message read from
  `helix_stdx::download` rather than run, since the demo page always
  registers one and nothing here can reach the state. `Downloading <name>`
  says the file reached the page, which is all this side can know: `main.js`
  hands it to the browser and never hears back, so a save you cancel in a
  browser that asks is not reported either. Native helix has no such
  command: there, `:w <path>` is this.
- **`:w` will not clobber an outside write, and nothing is read-only.**
  Store entries carry a modification time, so helix's "file modified by an
  external process" guard works: a `:w` over a key that a `helixVfs.write`
  changed since the buffer was opened or last saved is refused, and `:w!`
  overrides it exactly as it does natively. File permissions have no
  counterpart here, so nothing is ever read-only.
- **There are no directories**, only keys with separators in them. A
  directory is therefore any prefix the keys extend, which is what `<space>e`
  lists, what `:o /some/dir` opens a picker on, and what path completion
  offers a trailing separator for. It is a way of looking at the key set, not
  an entry the store holds, and two things follow. An empty directory and a
  missing one are the same thing — `<space>e` on a prefix no key lives under
  shows nothing but its `../` row rather than reporting the directory gone,
  and `:o` on one opens the new buffer it opens natively for a path that does
  not exist. And `:move /some/dir` renames the file *to* that key instead of
  moving it into the directory: native helix appends the original file name
  when the target is a directory, and there is none here to recognize. `:cd`
  works, does change how relative paths resolve, and accepts a directory no
  key lives under — there is no way to create one first, so that is where a
  first `:w` lands. `:pwd` reports the working directory; nothing can delete a
  directory the store never held. A key that is *also* a prefix (`/proj`
  beside `/proj/alpha.txt`) reads as the directory everywhere a choice has to
  be made — `<space>e` and `:o` descend into it rather than opening it, and it
  previews as its listing rather than its contents even in `<space>f`, which
  offers it as a file — since descending is the only one of the two things a
  picker can do. Its contents are still readable by any command that takes a
  file, `<space>f` included: selecting the row opens the file the preview did
  not show. `:r` meets the same rule from the other side: on a prefix no key
  sits at it is refused with `path is not a file`, which is what native says
  about a real directory, and on a name that is both it reads the key.
- **The file picker lists the VFS, minus the files boot seeds.** The bundled
  themes and the tutor text live under `/.config/helix/runtime/` and are
  artifacts of the build rather than anything you put there, so `<space>f`
  does not offer them. They are still in the store and still open by name
  (`:o /.config/helix/runtime/tutor`), which is how `:tutor` and `:theme`
  reach them, and `:cd`-ing into the runtime directory puts them back in the
  picker — asking for it by name is the one case where an empty list would
  be the wrong answer. Of the `file-picker.*` options, `hidden` and
  `max-depth` apply
  — a leading `.` on any path component below the picker's root, and the
  number of components below it, the same two things `WalkBuilder` counts
  natively. The rest have nothing here to act on and are ignored: `parents`
  and `ignore` want ignore files scoped to directories, `git-ignore`,
  `git-global` and `git-exclude` want a repository, and `follow-symlinks`
  and `deduplicate-links` want symlinks.
  Not every seeded file is hidden this way: the sample buffers, and a
  `config.toml` the page booted with, sit outside `/.config/helix/runtime/`
  and are ordinary keys, so `<space>f` offers them like anything you wrote.
- **The file explorer reads the VFS, and filters nothing.** `<space>e` lists
  one prefix at a time, with `../` below the root and no `../` at `/`, and
  descending into a row re-reads the store — as does the preview pane, which
  shows a directory row's entries. It does *not* drop the seeded runtime
  files the way `<space>f` does: that filter exists because the picker lists
  everything below its root at once, and it already lifts as soon as the root
  *is* the runtime directory. Every explorer level is a directory you named,
  so the exception is the whole of it — `.config/` is on the list at the root
  and the bundled themes are there once you walk down to them. The parts of
  it that want a file system are still gone: no `file-picker.*` option
  applies (the explorer never consulted them natively either), and there is
  nothing to hide, ignore, or follow.
- **Path completion reads the VFS.** `:o`, `:cd` and the other
  path-completing commands offer the keys under the directory you have typed,
  one level at a time. A name that other keys extend counts as a directory
  and completes with a trailing separator (`proj/`) — the one place a shared
  prefix is read as a directory, which is a way of looking at the key set and
  not an entry the store holds. There is nothing on disk to hide or ignore
  here, so `hidden` and gitignore filtering do not apply. `:theme` completion
  reads the VFS too. In-buffer path completion is a separate surface and
  still offers nothing — see "No completion popup and no signature help".
- `:config-open` opens the config the page booted with, or an empty buffer to
  write one in — either way a real, editable key in the store, which `:w`
  saves back to (see "Configuration" below). `:log-open` opens an empty buffer
  at `/.cache/helix/helix.log`: that key is real too, but nothing ever writes
  it, because log output goes to the browser console instead.

### Configuration

`config.toml` is read out of the VFS
([#75](https://github.com/Jecoms/helix-wasm/issues/75)): the global one at
`/.config/helix/config.toml`, the workspace one at `.helix/config.toml` under
the working directory, merged the way native helix merges them. `[keys]`
remaps, `[editor]` settings and `theme` all work. A host page passes the text
as `start()`'s fourth argument, which seeds the global path before the editor
boots (the demo page reads it from `window.helixConfig`); an embedder that
would rather write either path itself can `helixVfs.write` it first. A
malformed config is reported and the editor boots on the defaults — what
native helix does, minus the "press ENTER" prompt there is no stdin for. The
report goes to the browser console and to the statusline, where it lasts
until the first event helix handles, a click included, clears it like any
other status message; the console line is the durable half.

- `:config-reload` re-reads both files, so a config that arrives after boot
  applies without a page reload — including one edited in the editor itself
  with `:config-open` and `:w`. On a page that booted without any config it
  fails with "Failed to load config: no such virtual file", which is native
  helix's own behavior with no `config.toml` on disk, in the vocabulary of
  the store.
- RGB themes need a true-color claim that wasm32 has no `COLORTERM` or
  terminfo to make. The port answers for the terminal emulator rather than
  overriding the loaded config, so the claim survives `:config-reload`.
- **`languages.toml` and `.editorconfig` are still unreachable**, through
  different readers that are still `std::fs`. A missing `languages.toml` is
  not an error, so it does not block `:config-reload`.
- `.helix/` is never *detected*: `find_workspace` probes the real filesystem
  for a `.git`/`.jj`/`.helix` marker and nothing on wasm32 answers, so the
  workspace is always the working directory.
- `:set`, `:set-option` and `:toggle-option` still work, for the session.

### No background work

No threads and no tokio runtime, so what helix normally does off the main
loop either does not happen or happens inline:

- **Background jobs run on the browser's microtask queue**, not a tokio
  runtime — `Jobs::add` hands them to `wasm_bindgen_futures::spawn_local`
  instead of `tokio::spawn`. They are still detached and still resolve
  through the same callback channel, so the commands that queue one behave
  as they do natively; what is missing is anything a job might want from a
  runtime, which is why the entries below and the "No subprocesses" table
  read the way they do.
- **No completion popup and no signature help.** `C-x` in insert mode does
  nothing; both are driven by handlers that need a runtime, and there is no
  language server to feed them in any case.
- **`auto-save.after-delay` never fires.** An explicit `:w` still works.
- **Picker matching runs inline** on the browser's main thread — the vendored
  `nucleo` calls the match job directly instead of handing it to a
  threadpool — so a large picker blocks rendering and input while it matches
  instead of streaming results in.
- **Tree-sitter parses on the main thread**, so the page is frozen for as
  long as a parse takes. Helix's 500 ms parse timeout bounds that, and it
  does fire here: the `clock_gettime` shim (`sysroot/shims.c`) reads the
  page's `performance.now()` through the web crate's `clock` module, so
  elapsed time is real and an oversized file drops its highlighting instead
  of parsing to completion. That budget covers the parse and nothing else:
  once a parse succeeds, tree-house runs the language's injection and local
  queries over the finished tree with no deadline at all, so the freeze is
  bounded by how big the tree is rather than by the timeout — see
  [#120](https://github.com/Jecoms/helix-wasm/issues/120). Those queries are
  linear in the size of the tree, which they were not before
  [#92](https://github.com/Jecoms/helix-wasm/issues/92): a quadratic in
  tree-sitter's tree cursor made a 100 kB file of unbalanced delimiters take
  26 s to open in Chromium, where it now takes ~150 ms. The vendored copy
  carries that fix — see delta 2 in `stubs/tree-house-bindings/Cargo.toml`.

### Terminal and browser differences

The editor drives xterm.js through the vendored crossterm bridge, inside a
web page:

- **The OS clipboard is not wired up in either direction.** `"+y` and `"*y`
  behave like ordinary editor-local registers: the yank round-trips inside
  helix but never reaches the system clipboard, and `"+p` cannot read what
  you copied somewhere else. The browser's own paste (Ctrl/Cmd-V) does work —
  it arrives as a bracketed paste. To copy out, select with the mouse and use
  the browser's copy.
- **Your browser claims some chords first.** `C-w` — helix's entire window
  prefix — closes the tab in most browsers, as do `C-n` and `C-t`, and
  `Ctrl-Shift-<key>` is generally spoken for too. Upstream binds the same
  window menu under `space w`, so reach it that way (`space w v` splits,
  `space w hjkl` moves, `space w q` closes); `:vsplit`, `:hsplit` and
  `:wclose` work too. Which chords get taken is the
  browser's policy rather than this port's, so check yours — this entry is
  taken from that policy rather than from a run, because
  automation drivers hand these straight to the page and a headless run says
  nothing about what a real tab does.
- **On macOS, Option is Meta and stops composing.** `A-` chords reach the
  editor — the page claims Option the way iTerm's "Option as Meta" setting
  does — and the trade is that Option-composed character entry (`é`, `ß`,
  `…`) no longer works in insert mode; paste those in instead. Where macOS
  still composes before the page sees the keystroke, the chord's character
  comes from xterm.js's US-layout `keyCode` table rather than from the DOM's
  name, so those chords follow US key positions; a layout that composes
  Option into a plain ASCII character is left alone and still forwards its
  own (macOS UK Option-3 gives `A-#`, not `A-3`). The accent starters compose
  nothing at all until the next keystroke, so they arrive carrying no
  character to read; those are looked up in that same US table by physical
  key rather than by the character — one layout, read two ways, not two
  layouts. That covers both the letter starters (`A-e`, `A-i`, `A-n`, `A-u`)
  and the punctuation ones, `` A-` `` (`switch_to_uppercase`, the chord
  `:tutor` 10.3 asks for) among them. None of this applies off macOS. The
  composition half of this entry is the other one here not taken from a run:
  it is read from xterm.js's source and exercised with synthetic events,
  because browser automation drives the renderer directly and never goes
  through the OS input method, so nothing in this tree can compose a real
  Option keystroke.
- **No kitty keyboard protocol.** The bridge reports no keyboard
  enhancement, so this is the classic terminal key space: no key-release or
  repeat events, and no `Tab`/`C-i` or `Enter`/`C-m` disambiguation.
- **IME and other composed input arrive as a paste**, which inserts fine in
  insert mode but cannot trigger normal-mode commands.
- **Suspend is gone.** `:suspend` is not a command at all ("no such
  command") and the `C-z` binding does nothing. No signal handling is
  compiled in either, so there is no SIGUSR1 config reload and no graceful
  SIGTERM exit.
- **One editor per page.** `start()` refuses a second call, and `:q` ends the
  session for good. Quitting restores the main screen and prints `Helix has
  exited. Refresh the page to start a new session. (exit code N)`, then calls
  the `on_exit` handler an embedding page registered; the page stops
  forwarding input from there on. Reload to start over.

Mouse input is forwarded (click to move the cursor, drag to select, wheel to
scroll), and so are focus changes.

### Bundled content only

Syntax highlighting covers only the grammars linked into the bundle (listed
under "What the bundle ships with" above), and `:theme` only the themes the
bundle embeds (`THEME_CATALOG` in `web/build.rs`). Anything else opens as plain
text —
`:set-language haskell` is accepted without complaint and simply highlights
nothing — and any other theme name is not found.

## Embedding the editor

Each `web-v<semver>` tag publishes the `web/pkg` wasm-pack output — the same
unit the demo consumes as `file:../pkg` — as a GitHub release with a
`helix-web-<version>.tar.gz` attached (the `Publish web bundle` workflow,
`.github/workflows/web_release.yml`). Embedders should pin one of those
instead of linking into the deployed demo, whose asset names are
content-hashed and replaced on every push to `main`:

```sh
curl -LO https://github.com/Jecoms/helix-wasm/releases/download/web-v0.1.0/helix-web-0.1.0.tar.gz
tar xzf helix-web-0.1.0.tar.gz    # extracts helix-web-0.1.0/
```

The extracted directory is a standard wasm-pack `--target web` package (ES
module + `.wasm` + `.d.ts`, plus `NOTICE.md` with the license notices for
the statically linked grammar parsers). Consume it the way the demo's
`web/www/package.json` does:

```json
"dependencies": { "helix-web": "file:../helix-web-0.1.0" }
```

`web/www/main.js` is the reference host wiring to replicate: call `init()`
(fetches and instantiates the wasm module), then `start(writeBytes, cols,
rows, config)` with a callback that feeds editor output bytes to an xterm.js
`Terminal`, and forward input with `key_event(...)`, `paste(...)`, and
`resize(cols, rows)`. `config` is the text of a `config.toml`, or `undefined`
for helix's defaults (see "Configuration" above). Register `on_exit(handler)`
before `start` to learn
when helix quits (`:q` and friends really do exit, and nothing can restart
it in place — the page has to reload), and route the calls into wasm
through a `try`/`catch` as the demo page does: a panicked instance traps on
every later call, and a host that keeps forwarding into it silently
swallows the user's input. Input calls made after a clean exit are inert
(the module drops them rather than queueing for an event loop that is gone),
but a page still forwarding is a page still pretending to have an editor —
stop on the exit and tell the reader.

`on_download(handler)` is the other callback worth registering: `:download`
calls it with the file name to save under and a `Uint8Array` of the bytes,
and the page decides what saving means — a `Blob` and an object URL (what
`main.js` does, replaceable at `window.helixDownload` for a devtools
session), a File System Access handle, a POST to a server. Throwing from it
refuses the save and puts the message on the statusline; registering
nothing leaves `:download` reporting that this host cannot save files, so
wire it up or expect readers to have no way out of the page.

Beyond the terminal loop, the module
exports the file-injection hooks (`vfs_write` / `vfs_read` / `vfs_list`,
see "Virtual file system" above) and the read-only inspection surface (`editor_state()`
/ `editor_text()`, see "Editor state inspection") — the intended surface
for tutorial-style embedders that drive and assert on the editor rather
than scrape the rendered terminal. The JS surface is unstable by design
(`web/src/session.rs`, `web/src/vfs.rs`), with one exception: the
read-only inspection surface (`web/src/inspect.rs`,
[#18](https://github.com/Jecoms/helix-wasm/issues/18)) is meant to be
kept stable. Either way, pin a tagged tarball and check its `.d.ts` when
upgrading.

To cut a release: bump `version` in `web/Cargo.toml`, merge, then tag that
commit `web-v<version>` and push the tag. The workflow verifies the tag
against the crate version, rebuilds the bundle with `--locked`, and
attaches the tarball to a release on the tag.

## Working on the port

`main` is a version line rooted at an upstream release: `helix/` carries the
pristine 25.07.1 release tree with the wasm patch set as ordinary commits on
top, and everything that does not belong in helix — the browser frontend, the
dependency stubs, the C sysroot — sits alongside it. The helix crates are path
dependencies, so patching helix is editing a file in this workspace and a
checkout of `main` is the whole build input.

### Layout

| Path | Purpose |
| --- | --- |
| `helix/` | The patched Helix source: upstream's `25.07.1` release tree plus this port's patches. Its own cargo workspace — upstream's, left pristine — excluded from the root one and consumed as path dependencies |
| `Cargo.toml` | Wrapper workspace: the helix crates as path dependencies on `helix/`, plus `[patch.crates-io]` stub swaps |
| `stubs/` | Stand-ins for third-party dependencies with no wasm32 support: transitive crates (`home`, `which`, `libloading`, and `url` with a wasm cfg), a vendored `crossterm` whose OS terminal layer is replaced by a browser bridge, and a vendored `nucleo` that runs picker matching inline instead of on a threadpool. A vendored `tree-house-bindings` rides here too — not a missing-support stand-in but an ABI fix, without which every syntax-highlighted buffer traps on wasm32, plus a fix for a quadratic in the tree-sitter it vendors that froze the page on malformed input — the only stub shipping third-party C (a vendored tree-sitter; see `web/NOTICE.md`) |
| `sysroot/` | Stub libc headers, the `wasm-cc` clang shim that lets tree-sitter's stock build script compile its C for wasm32, and the libc shim implementations (`shims.c`, `wctype.c`) the final wasm link needs |
| `web/` | The browser frontend: a wasm-bindgen cdylib that boots helix-term against the crossterm bridge, plus the xterm.js host page in `web/www/` |
| `.cargo/config.toml` | Wires `wasm-cc` up as the C compiler for the wasm32 target |

### Checking the crates

The wasm32 type-check, crate by crate — part of what CI gates on, and the
fast loop while patching:

```sh
rustup target add wasm32-unknown-unknown
cargo check -p helix-core --target wasm32-unknown-unknown
cargo check -p helix-view --target wasm32-unknown-unknown
cargo check -p helix-term --target wasm32-unknown-unknown
```

### Patching helix

`helix/` is ordinary source in this workspace, so a helix change is an
ordinary edit:

```sh
$EDITOR helix/helix-view/src/document.rs
cargo check -p helix-view --target wasm32-unknown-unknown
```

Commit it like any other change — the path dependency picks the edit up
directly.

Two things keep the patch set cheap to carry onto the next release. Shape:
localized insertions and `#[cfg(target_arch = "wasm32")]` arms replay clean,
while re-indenting a block of otherwise-untouched native code conflicts with
any upstream edit to it — prefer extracting a native body into its own
function over wrapping it. And blast radius: `helix/Cargo.toml` is
byte-identical to upstream on purpose (it is the file upstream churns most),
so declare a new dependency in the individual crate manifests rather than in
its `[workspace.dependencies]`.

`helix/Cargo.lock` is byte-identical to upstream for the same reason, and it
takes no upkeep to keep it that way: `helix/` is excluded from the root
workspace, so every build here resolves against the root `Cargo.lock` and
nothing reads helix's. It is upstream's lockfile riding along with upstream's
tree — deliberately stale against the crate manifests, and left alone rather
than regenerated, because upstream rewrites it on every dependency bump and
any hunk we hold there is a conflict on the next replay. Regenerating it is
not a fix; if a `cargo` run rooted at `helix/` ever needs a current lockfile,
it re-resolves one (and fails under `--locked`).

That re-resolution rewrites the file in place, so any command run from
`helix/` leaves the tree dirty — `cargo test -p helix-stdx`, the way to
exercise the unit tests in helix crates (the `helix_stdx::vfs` ones build
under `cfg(test)` on the host), is the one that comes up. It is only the
lockfile, and the fix is the same as everywhere else: restore upstream's copy
before committing.

Two patches in the series still edit that file on their way past, so a replay
can stop on it even though the net diff is empty. Resolve it by taking
upstream's copy every time it comes up — `git checkout upstream/$V --
helix/Cargo.lock`. That is always the right answer, because upstream's copy
*is* the target state; there is nothing of ours in the file to preserve.

What the patch set changes, at any point:

```sh
git diff upstream/25.07.1 main -- helix/
```

### Browser smoke tests

A Playwright suite (`web/www/tests/`) boots the built bundle in headless
Chromium and asserts on editor behavior through `helixState` / `helixVfs`
and the terminal buffer — the same checks CI runs in the `wasm32 check`
workflow. Run it against a fresh build:

```sh
wasm-pack build web --target web
cd web/www
npm install
npm run build                      # tests run against dist/, not the dev server
npx playwright install chromium    # first run only
npm test
```

### Deploying the demo

The demo deploys to <https://jecoms.github.io/helix-wasm/> via the
`Deploy web demo` workflow (`.github/workflows/web_demo.yml`), which builds
the full-catalog bundle and publishes it with `actions/deploy-pages`.

Every push to `main` deploys automatically; a manual
`gh workflow run web_demo.yml` works too. Deploys are gated by the
`github-pages` environment's deployment branch policy — only `main` is on
the allowed list.

### Taking a new helix release

Each helix release gets a permanent **base branch**: a single parentless
commit holding that release's pristine tree under `helix/`, and nothing else.
Cut and publish it first — nothing is built on top until it verifies.

```sh
V=25.10                                          # upstream's release tag
SRC=$(git rev-parse "${V}^{commit}")
ROOT=$(printf '040000 tree %s\thelix\n' "$(git rev-parse "${V}^{tree}")" | git mktree)
BASE=$(
  GIT_AUTHOR_NAME=$(git log -1 --format=%an "$SRC") \
  GIT_AUTHOR_EMAIL=$(git log -1 --format=%ae "$SRC") \
  GIT_AUTHOR_DATE=$(git log -1 --format=%ad --date=raw "$SRC") \
  GIT_COMMITTER_NAME=$(git log -1 --format=%cn "$SRC") \
  GIT_COMMITTER_EMAIL=$(git log -1 --format=%ce "$SRC") \
  GIT_COMMITTER_DATE=$(git log -1 --format=%cd --date=raw "$SRC") \
  git -c commit.gpgsign=false commit-tree "$ROOT" -m "helix ${V} (pristine upstream release tree)"
)
test "$(git rev-parse "${BASE}:helix")" = "$(git rev-parse "${V}^{tree}")"
git push origin "$BASE":"refs/heads/upstream/$V"
```

Identity and dates come from the release commit and the commit is left
unsigned, so re-running the recipe reproduces the same SHA — the base is
verifiable by anyone rather than taken on trust. Being unsigned it needs an
admin push past the repo-wide signature requirement; that is the one
privileged step, once per release.

Then replay the port onto it:

```sh
git checkout -b "port/$V" main
git rebase --onto "upstream/$V" upstream/25.07.1 "port/$V"
```

Open `port/$V` → `upstream/$V`. The base is the merge base, so the diff is
exactly this repo's commits and none of helix's source. The wrapper commits
touch files upstream never touches and replay clean, which narrows the
conflict set to the helix files this port patches. One of those resolves
without thinking about it: whenever the replay stops on `helix/Cargo.lock`,
take upstream's copy (`git checkout upstream/$V -- helix/Cargo.lock`) and
continue — see "Patching helix" above for why that is always correct.
**Parity is the Playwright suite passing** (see "Browser smoke tests" above).
Promote by moving `main` to the reviewed tip, keeping the outgoing line as a
versioned branch.

### Branch and tag map

- `main` (this branch) — the current version line: `upstream/25.07.1` plus
  the wasm patch set plus the wrapper. Self-sufficient; every other ref below
  is a label or a release artifact.
- `upstream/<version>` (e.g. `upstream/25.07.1`) — the permanent base
  branches described above: one parentless commit per helix release, holding
  that release's pristine tree under `helix/` and nothing else. They are
  `main`'s root, the merge base for release-review PRs, and the reference for
  `git diff upstream/<version> main -- helix/`. Frozen by the
  `upstream-branches-frozen` ruleset (creation only, no bypass actors): a new
  base can be pushed, an existing one can never move or be deleted.
- `web-v<semver>` (e.g. `web-v0.1.0`) — release tags for the embeddable web
  bundle. Pushing one runs the `Publish web bundle` workflow
  (`.github/workflows/web_release.yml`), which checks the tag against
  `web/Cargo.toml`'s `version`, rebuilds the full-catalog `web/pkg`
  wasm-pack output, and attaches it to a GitHub release as
  `helix-web-<version>.tar.gz` — the artifact "Embedding the editor" above
  pins.

## Credits and license

- [helix-editor/helix](https://github.com/helix-editor/helix) — the editor
  itself, MPL-2.0. Its [documentation](https://docs.helix-editor.com/) is the
  place to learn helix; all of it applies here except what "Limitations and
  behavioral differences" carves out.
- [makemeunsee/helix](https://github.com/makemeunsee/helix), branch `wasm32` —
  the browser port this one grew out of.

The port keeps upstream's MPL-2.0 (`helix/LICENSE`). The tree-sitter parsers
and runtime the wasm bundle statically links carry their own notices, in
`web/NOTICE.md`.
