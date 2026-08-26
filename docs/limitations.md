# Limitations and behavioral differences

This section catalogs where the browser changes what helix does. Everything
below was reproduced by hand in a build of this tree, except where an entry
says otherwise — it is what the port does today, not what its source suggests
it might do. The entries track `helix/` plus the `web/` crate, so re-check them
when the port is replayed onto a new release (see [Taking a new helix
release](development.md#taking-a-new-helix-release)).

### No subprocesses

Everything that shells out is gone. These fail cleanly, reusing the message
helix already has for "not configured":

| What | What you get |
| --- | --- |
| Language servers the host page did not supply — diagnostics, hover, rename, code actions | "No configured language server supports …"; `:lsp-restart` → "LSP not defined for the current document". A server the page *does* supply, as a Web Worker registered under its `languages.toml` name, runs helix's unmodified LSP client over `postMessage` — see [Language servers](embedding.md#language-servers) |
| The goto commands — `gd`, `gD`, `gy`, `gi` and `gr` — without such a server | "No definition found." / "No references found." — helix's own empty-result message, since these five queue their request before checking for a server. Native helix with no server configured says the same |
| Debugging — `:debug-start`, `:debug-remote`, the rest of the DAP layer | "No debug adapter available for language" |
| External formatters, including format-on-save (`:format`) | "A formatter isn't available, and no language server provides formatting capabilities" |
| Shell commands — `:sh`, `:!`, `:run-shell-command`, the `!` and `\|` keys, `:pipe`, `:pipe-to`, `:insert-output`, `:append-output` | "Shell commands are not supported on this platform" |
| Opening a URL — `gf` with the cursor on one | "Opening URLs in an external program is not supported on this platform" — there is no handoff to the browser either, so nothing here can open a URL. `gf` on a file path works normally |
| Git — the diff gutter, `:reset-diff-change`, the `<space>g` changed-file picker | "Diff is not available in the current buffer" / "Current working directory does not exist" |

Dynamic grammar loading is out as well (`libloading` is stubbed), so the
grammar set is whatever was linked at build time.

There is no command line either: the host page boots the module with default
arguments, so `hx <file>`, `-c` and `--tutor` have no equivalent. `:tutor`
itself works.

### Files live in an in-memory VFS

Documents are keys in `helix_stdx::vfs`, not files (see [Virtual file system](vfs.md)). What that changes:

- **Nothing survives a page reload.** Pull anything you care about out
  first: `:download` saves the current file to your machine,
  `:download-all` saves the session as a zip, and `helixVfs.read` /
  `helixVfs.list` hand the store to a page script.
- **`:download` exports one file, as it stands in the buffer.** Unsaved
  edits included — it is a copy of what you are looking at, not of what the
  store last saw, and it changes neither. That means it also skips the
  transforms `:w` applies on its way out (`insert-final-newline` and the two
  trims edit the *document*, and an export has no business doing that), and
  it writes UTF-8 whatever `:encoding` says: save a windows-1252 buffer and
  the store gets one byte for `é` where the download gets two. The name
  comes from the argument or the buffer, minus any directories — a download
  lands wherever your browser puts downloads. A scratch buffer with no name
  is refused rather than given one.
  Saving is the host page's half (`on_download`, see [Embedding the
  editor](embedding.md)), so a page whose handler throws reports what it threw, and one
  that registers no handler at all gets "Could not download …: this host
  cannot save files" — that last message read from
  `helix_stdx::download` rather than run, since the demo page always
  registers one and nothing here can reach the state. `Downloading <name>`
  says the file reached the page, which is all this side can know: `main.js`
  hands it to the browser and never hears back, so a save you cancel in a
  browser that asks is not reported either. Native helix has no such
  command: there, `:w <path>` is this.
- **`:download-all` exports the store, and only what this session wrote.**
  It is one zip of every file you have saved, with the store's directories
  kept as directories inside it; the entries are stored rather than
  compressed, which no extractor minds. Because it is the *store* it
  exports, it refuses while any buffer is modified and names them — `:w`
  them and the archive is right, or run `:download-all!` and accept the last
  saved copy of each. With nothing saved yet there is nothing to export, and
  it says so rather than handing over an empty archive.
  **Files boot seeded are never in it, even after you edit them.** The
  bundled themes, the `:tutor` text, the sample files and a `config.toml`
  the page supplied are the page's, not your session's, and the rule is on
  the key permanently — so `:o` a bundled theme, change a color, `:w`, and
  that edit is *not* in the archive. (The alternative rule would drag a
  whole copied theme into every export over one changed color.) To keep such
  an edit: `:w <new name>` and it is in the archive like anything else, or
  `:download` it, which always exports the buffer you are looking at, or
  read the key with `helixVfs.read` from a page script.
- **`:remove` is the only delete, and only where the page allows it.** There
  is no shell to `:sh rm` from, so `:remove` (`:rm`) drops the store key and
  closes the buffer on it — the current buffer's, or `:remove <path>` for
  another key, open or not; a scratch buffer is refused for want of a path.
  A buffer with unsaved changes is refused until `:remove!`. Keys only: there
  are no directories (see below), so nothing recursive. The host page is
  asked first (`on_remove`, see [Embedding the editor](embedding.md)) and a handler that
  throws refuses the deletion with the store untouched, reporting what it
  threw; a page that registers no handler gets "Could not remove: this host
  cannot remove files" for every `:remove`, so a page either offers deletion
  or does not — that message read from the `is_registered` gate in
  `helix-term/src/commands/typed.rs` rather than run, as the demo page
  always registers one. A buffer whose path has no key in
  the store — never `:w`'d, or `helixVfs.delete`'d since — is closed, with
  `Closed <path> (not in the store; nothing to remove)`, and the handler is
  *not* called, its contract being "this key is leaving the store": a page
  mirroring the store was either never told about that key or did the
  deleting itself. `helixVfs.delete` is the page's own
  deletion and bypasses the handler entirely (the page is the one deleting);
  it also leaves any buffer open on the key alone, the way `helixVfs.write`
  does.
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
- **`<space>/` (global search) greps the VFS.** Its candidate set is exactly
  the file picker's (the entry above): the store minus the boot-seeded
  runtime files, with `hidden` and `max-depth` honored and the rest of the
  `file-picker.*` options ignored for the same reasons. Open buffers are
  searched as they stand, so unsaved edits match — what native helix does —
  and everything else is searched by its last saved bytes. One thing is
  different, because there is no thread to hand the work to: the search
  runs inline on the main thread, like picker matching (see "No background
  work"), so a store big enough to grep slowly is a page frozen for that
  long. The debounce between keystrokes, regex syntax, smart-case and the
  picker's preview behave as they do natively.
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
  still offers nothing: its handler runs now (see "No background work"),
  but it lists the directory through `std::fs`, which has nothing behind it
  on wasm32, so the popup never opens.
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
- `languages.toml` is read out of the VFS the same way
  ([#144](https://github.com/Jecoms/helix-wasm/issues/144)): the user one at
  `/.config/helix/languages.toml` — `start()`'s fifth argument seeds it, the
  demo page reads it from `window.helixLanguages` — merged over the built-in
  set, with a workspace `.helix/languages.toml` over that, and re-read by
  `:config-reload`. A malformed one is reported like a malformed config and
  boots the built-in set; a missing one is not an error. It is where a page
  declares the language servers it supplies (see [Language servers](embedding.md#language-servers)); a `[language-server]` table's `command` and
  `args` are ignored there, since nothing can be spawned.
- **`.editorconfig` is still unreachable**: its reader is still `std::fs`.
- `.helix/` is never *detected*: `find_workspace` probes the real filesystem
  for a `.git`/`.jj`/`.helix` marker and nothing on wasm32 answers, so the
  workspace is always the working directory.
- `:set`, `:set-option` and `:toggle-option` still work, for the session.

### No background work

No threads and no tokio runtime, so what helix normally does off the main
loop either runs on the browser's own executor or happens inline:

- **Background jobs and async hooks run on the browser's microtask queue**,
  not a tokio runtime — `Jobs::add` hands them to
  `wasm_bindgen_futures::spawn_local` instead of `tokio::spawn`, and the
  debounced handlers (completion, signature help, diagnostics, auto-save,
  the pickers' dynamic queries) run through `helix-event`'s `task` and
  `time` shims, which are the browser's executor and `setTimeout` on wasm32
  and tokio's everywhere else. They are still detached and still resolve
  through the same channels, so the commands and handlers that use them
  behave as they do natively: the completion popup, signature help and
  `auto-save.after-delay` all work, given a language server to feed the
  first two (see [Language servers](embedding.md#language-servers)).
- **Picker matching runs inline** on the browser's main thread — the vendored
  `nucleo` calls the match job directly instead of handing it to a
  threadpool — so a large picker blocks rendering and input while it matches
  instead of streaming results in.
- **Tree-sitter parses on the main thread**, so the page is frozen for as
  long as a parse takes. Helix's 500 ms parse timeout bounds that, and it
  does fire here: the `clock_gettime` shim (`sysroot/shims.c`) reads the
  page's `performance.now()` through the web crate's `clock` module, so
  elapsed time is real and an oversized file drops its highlighting instead
  of parsing to completion. Upstream that budget covers the parse and nothing
  else — once a parse succeeds, tree-house walks the finished tree twice more
  per layer, running the language's injection and local queries on cursors it
  builds itself and threads no deadline into. Those walks are bounded here
  too: the vendored bindings arm the same 500 ms on every query cursor, so a
  walk that runs long stops yielding matches — an injected language goes
  unhighlighted, or local variables lose the highlight the locals query would
  have refined for them — rather than the tab locking up. See delta 3 in
  `stubs/tree-house-bindings/Cargo.toml`. It is a ceiling per walk, not per
  keystroke, which is the granularity the parse timeout already had: a
  document with many injection layers can spend the budget once per layer.
  It is also a ceiling on *every* query cursor, not only the two walks a
  syntax update makes — highlighting a viewport and matching a textobject run
  under it too, so an expensive textobject on a huge buffer can come back
  empty instead of slow. Deliberate: no query cursor is interruptible, and
  none of them run anywhere but the thread the page is drawn on.
  The queries are also linear in the size of the tree, which they were not
  before [#92](https://github.com/Jecoms/helix-wasm/issues/92): a quadratic in
  tree-sitter's tree cursor made a 100 kB file of unbalanced delimiters take
  26 s to open in Chromium, where it now takes ~150 ms. The vendored copy
  carries that fix — see delta 2 in `stubs/tree-house-bindings/Cargo.toml`.

### Terminal and browser differences

The editor drives xterm.js through the vendored crossterm bridge, inside a
web page:

- **The `+` and `*` registers reach the OS clipboard through
  `navigator.clipboard`, with the browser's own consent UX in the way of
  reads.** `"+y` (and `space y`) writes on every browser without a prompt —
  a keystroke counts as the user gesture writes require. `"+p` (and
  `space p`, `C-r +` in insert mode) reads it, and what that looks like is
  the browser's call: Chromium asks once for the clipboard-read permission
  and then reads silently; Safari shows a "Paste" button to tap each time;
  Firefox 125+ shows a one-item "Paste" context menu each time, except when
  the clipboard holds what this page copied, which it reads silently. A read
  you refuse or ignore (the prompt is waited on for 5 s) leaves the register
  holding its last in-page yank, so `"+y` → `"+p` always round-trips, and
  input typed meanwhile is held in order behind the paste, not lost. A
  browser has one clipboard, so `*` is the same clipboard as `+`. Two
  corners the bridge does not cover: it decides *which* keystrokes read the
  clipboard from the editor's state plus the key before (`space`, `C-r`) —
  never while a `:`/`/` prompt or a picker has the keyboard — so a typed
  `:clipboard-paste-after` pastes what the register last saw rather than
  asking the browser, and a `space` a popup swallowed can cost a spurious
  Paste prompt. On a plain `http://` origin that is not localhost
  there is no `navigator.clipboard` at all, and the registers stay
  editor-local. The browser's own paste (Ctrl/Cmd-V) works regardless — it
  arrives as a bracketed paste.
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
  `:tutor` 10.3 asks for) among them. None of this applies off macOS. On
  every platform, though, the punctuation- and digit-row Alt chords are read
  from the DOM's key name rather than xterm.js's legacy `keyCode` table,
  which is what keeps `A-;`, `A-=` and `A--` working in Firefox (Gecko
  numbers those three keys differently from Chromium and Safari). Firefox on
  macOS also starts the dead key's own composition even though the chord was
  already handled; the page swallows that one before xterm.js can draw the
  accent at the cursor or paste it, so `A-u` and `` A-` `` leave no stray
  `¨` or `` ` `` behind there. The composition half of this entry is the
  other one here not taken from a run: it is read from xterm.js's source and
  exercised with synthetic events, because browser automation drives the
  renderer directly and never goes through the OS input method, so nothing
  in this tree can compose a real Option keystroke.
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
in [Building from source](building.md#what-the-bundle-ships-with)), and `:theme` only the themes the
bundle embeds (`THEME_CATALOG` in `web/build.rs`). Anything else opens as plain
text —
`:set-language yaml` is accepted without complaint and simply highlights
nothing — and any other theme name is not found.

The default set is a size budget, not the whole catalog: the demo and the
release's default tarball link sixteen grammars, and the other twenty-five
(cpp, c-sharp, kotlin, haskell and the like) are opt-in — the `-full`
tarball, or a build from source with `HELIX_WEB_GRAMMARS`. A few languages
are missing from the catalog altogether, as toolchain limits rather than
choices: yaml, ruby, php and cmake ship a C++ external scanner at helix's
pinned revision, and the wasm C toolchain has no C++ sysroot, so they cannot
be linked until upstream ports the scanner to C or a newer revision that has
is acceptable; git-commit's grammar is left out because its generated parser
takes clang twenty minutes and nine gigabytes of memory to compile for wasm.
Whichever way a grammar is absent, nothing helix does for that language
beyond highlighting is available either — no syntax-aware text objects
(`mif`, `maf`), no `Alt-o`/`Alt-i` selection expansion, no indent queries.
