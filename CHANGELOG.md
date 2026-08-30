# Changelog

Notable changes to **`helix-web`** — the embeddable wasm bundle this repo publishes.

**Scope.** This file tracks the *artifact*, not the repository. The unit that gets
versioned is the `web/pkg` wasm-pack output that a `web-v<semver>` tag ships as
`helix-web-<version>.tar.gz` (see
[Embedding the editor](https://github.com/Jecoms/helix-wasm/blob/main/docs/embedding.md)),
so an entry earns its place by changing what an embedder gets: the editor's behavior in
the browser, the JS surface, or what the bundle contains. That is a narrower thing than
the repo but a wider one than the `web/` crate — the patch set under `helix/`, the
dependency stubs and the C sysroot are all statically linked into the same `.wasm`, so a
change to any of them is in scope. Repo-internal work with no reach into the artifact —
CI, the demo page's own chrome, README edits, the release plumbing — is not, and is left
to the git history.

The format is [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions are
[semantic](https://semver.org/spec/v2.0.0.html), read against the stability note in each
entry. This file ships *inside* the tarball as well as living here, so its links out are
absolute — the copy in an embedder's extracted tree has no repo next to it.

## [Unreleased]

## [0.0.5] — 2026-08-29

### Added

- **Eight more grammars in the default set, and a catalog of forty-one to opt into**
  ([#149](https://github.com/Jecoms/helix-wasm/issues/149)). The bundle now links
  sixteen grammars by default: bash, css, html, json, markdown, markdown_inline, tsx and
  typescript join c, go, java, javascript, python, regex, rust and toml, all at helix's own
  pins. Another twenty-five — c-sharp, clojure, cpp, diff, dockerfile, elixir, git-config,
  git-rebase, gitattributes, gitignore, haskell, hcl, heex, ini, kotlin, lua, make, nix,
  ocaml, scala, scss, sql, swift, xml, zig — are in the catalog but opt-in, because their
  parsers are large (the seven biggest alone would triple the bundle): each release now
  attaches a second tarball, `helix-web-<version>-full.tar.gz`, that links the whole
  catalog, and a build from source picks any set with `HELIX_WEB_GRAMMARS`, which gained
  the aliases `default` and `full` (`HELIX_WEB_GRAMMARS=default,kotlin` is the default
  set plus one). With a grammar come the tree-sitter features that depend on one —
  syntax-aware text objects, `Alt-o`/`Alt-i` expansion, indent queries — for every helix
  language it serves (`jsonc`, `env`, `jsx` and the like included: the build now embeds
  the queries of every language that names a linked grammar, not only the one sharing
  its name). yaml, ruby, php and cmake are still missing — their scanners are C++ at
  helix's pin, which the wasm C toolchain cannot build — and so is git-commit, whose
  generated parser is impractically slow to compile for wasm. The default bundle grows
  from 9.66 MB raw / 3.14 MB gzip to 13.2 MB raw / 3.80 MB gzip; the full one is
  45.9 MB raw / 7.09 MB gzip.

## [0.0.4] — 2026-08-24

### Added

- **Language servers, over a Web Worker the page supplies**
  ([#144](https://github.com/Jecoms/helix-wasm/issues/144)). The browser cannot
  spawn a server process, but LSP is JSON-RPC over any byte stream, so the new
  **`register_language_server(name, port)`** export takes a `Worker` (or
  `MessagePort`) per server name and helix's unmodified LSP client runs over
  `postMessage` — one JSON-RPC message per string, no framing. The `initialize`
  handshake, request gating, document sync, the completion popup, hover,
  signature help, `gd` and its siblings, diagnostics, code actions and rename all
  work as far as the server implements them; a scripted one
  (`web/www/toy-lsp-worker.js`) is what the browser tests drive. **`start()`
  gains a fifth argument**, the text of a `languages.toml`, seeded and re-read
  like `config.toml`: it is where a page declares its servers (a
  `[language-server.<name>]` table's `command` is ignored; the registered name
  is the match) and the languages that use them. The demo page reads both from
  `window.helixLanguages` and `window.helixLanguageServers`. A server name with
  no port registered fails the way an unconfigured server always has. See the
  port's docs'
  [Language servers](https://github.com/Jecoms/helix-wasm/blob/main/docs/embedding.md#language-servers).
- **The async hooks run.** Helix's debounced handlers — completion, signature
  help, diagnostics, auto-save, the pickers' dynamic queries — used to be spawned
  only inside a tokio runtime, so on wasm32 they swallowed their events; they now
  run on the browser's executor and `setTimeout`. Beyond the LSP features above,
  that turns on `auto-save.after-delay`, gives `<space>/` global search its native
  debounce between keystrokes (it dispatched every keystroke before), and lets a
  picker's preview highlight the file it shows.
- **`"+y` / `"+p` talk to the OS clipboard**
  ([#140](https://github.com/Jecoms/helix-wasm/issues/140)). The `+` and `*`
  registers — `space y`, `space p`, `C-r +` in insert mode included — are bridged
  to `navigator.clipboard`: a yank writes it (no prompt; a keystroke is the
  gesture writes need), and a paste reads it under whatever the browser asks for
  that — a one-time permission in Chromium, a per-paste "Paste" affordance in
  Safari and Firefox (silent in Firefox for the page's own copy). A refused or
  unanswered read (5 s) leaves the register holding its last in-page yank, so a
  `"+y` → `"+p` round trip always works, and keys typed while the browser asks
  are held in order behind the paste. `*` is the same clipboard as `+` — a browser
  has one. `:clipboard-provider` reports `browser`; an embedder that wants the
  registers editor-local again can set `editor.clipboard-provider = "none"`. The
  bridge needs no host-page change. See the docs'
  [Terminal and browser differences](https://github.com/Jecoms/helix-wasm/blob/main/docs/limitations.md#terminal-and-browser-differences)
  for the per-browser detail and the two corners it does not cover.

### Fixed

- A redraw requested from a background task — a picker's injector finishing, a
  debounced hook firing — reached the editor only when the next keystroke
  happened to poll it. The event-loop driver now registers its waker with
  `helix-event`, so such requests render on their own.
- **Firefox dropped `Alt-;`, `Alt-=` and `Alt--`**
  ([#137](https://github.com/Jecoms/helix-wasm/issues/137)). Gecko reports legacy
  key codes for the punctuation row that xterm.js's keyCode table does not know, so
  those chords never reached the editor. Every Alt chord on the punctuation and digit
  rows is now resolved from the DOM `key`/`code` on every platform, in front of xterm.
  One consequence: on a non-Latin layout, a key that composes into a character
  (Russian `Alt-ж` on the `;` key) forwards that character rather than resolving to
  `A-;` through the key code — the README's stated stance, applied consistently.
- **A stray accent after an Option chord in Firefox on macOS**
  ([#142](https://github.com/Jecoms/helix-wasm/issues/142)). `A-u` ran its command,
  then Firefox's dead-key composition drew a `¨` at the cursor and pasted it on
  release. The composition is now swallowed when an Alt dead-key keydown started it;
  real IME compositions still arrive as before.

## [0.0.3] — 2026-08-22

### Added

- **`:remove` (`:rm`)** — a file out of the virtual file system, at last
  ([#132](https://github.com/Jecoms/helix-wasm/issues/132)). Native helix has no delete
  command because `:sh rm` is always there; this build has no shell, so a key written into
  the store was there for the life of the page. `:remove` drops the current file's key and
  closes its buffer in one act, `:remove <path>` names another key (open or not), and
  `:remove!` goes ahead over unsaved changes. Keys only — the store has no directories.
  It is host-gated the way `:download` is: the command works only on a page that
  registered the new **`on_remove(handler)`** export, which is called with the store key
  before it goes and may throw to refuse (the message lands on the statusline);
  unregistered, `:remove` reports that this host cannot remove files, so deletion is a
  per-page capability. A buffer whose path has no key in the store (never `:w`'d, or
  deleted by the page since) just closes, without the handler. **`vfs_delete(path)`** completes the `vfs_write` / `vfs_read` / `vfs_list` set
  for the page's own deletions; it bypasses the handler. The port's docs'
  [Files live in an in-memory VFS](https://github.com/Jecoms/helix-wasm/blob/main/docs/limitations.md#files-live-in-an-in-memory-vfs)
  carries the full entry.
- **`editor_state()` reports the theme.** The snapshot carries a `theme` field: the
  name of the theme the editor is rendering with — what `:theme` last set (a preview
  still showing from the prompt included), or `"default"`. An embedding page that wants
  to remember the theme a reader picked can poll for it instead of watching the `:theme`
  prompt's keystrokes, which completion and aborted previews make unreliable. Additive;
  the rest of the shape is unchanged.

## [0.0.2] — 2026-08-20

### Added

- **`<space>/` global search** — the search picker that opened and never answered now
  greps the virtual file system
  ([#130](https://github.com/Jecoms/helix-wasm/issues/130)). The candidate set is
  exactly what `<space>f` offers — the store, minus the boot-seeded runtime files, with
  `hidden` and `max-depth` honored — searched with helix's own smart-case regex engine:
  open buffers as they stand, so unsaved edits match, everything else by its last saved
  bytes, with the preview and line-jump behaving as they do natively. Two trades against
  native, both from the missing runtime: no debounce (every keystroke dispatches its
  search immediately) and the search runs inline on the main thread, like picker
  matching. The port's docs'
  [Files live in an in-memory VFS](https://github.com/Jecoms/helix-wasm/blob/main/docs/limitations.md#files-live-in-an-in-memory-vfs)
  carries the full entry.
- **`:download-all`** — the whole session out of the page as one zip, where `:download`
  gets one file ([#110](https://github.com/Jecoms/helix-wasm/issues/110)). It packs every
  file this session saved into `helix-session.zip` (or the name you give it) and hands
  that to the host page's existing `on_download` handler, so a page that already wired
  `:download` up gets the new command for free — the archive is just another file to save.
  Entries are stored rather than deflated and the zip writer is in-tree
  (`helix_stdx::archive`), so this adds no dependency to the bundle.
  Two behaviors to know before relying on it. It exports the *store*, so it refuses while
  any buffer is modified and names what to `:w`, with `:download-all!` for "export it as it
  stands". And **what boot seeded is never in the archive, edited or not** — the bundled
  themes, the `:tutor` text, the sample files and a page-supplied `config.toml` belong to
  the page, and that stays true after you edit one; save such a file under a new name, or
  `:download` it, to keep the edit. A page that seeds its own files with `vfs_write` before
  `start` puts them on the same side of that line. The port's docs'
  [Files live in an in-memory VFS](https://github.com/Jecoms/helix-wasm/blob/main/docs/limitations.md#files-live-in-an-in-memory-vfs)
  is the full statement of both.

### Fixed

- **The parse timeout now covers the injection and local queries too.** Helix gives
  tree-sitter 500 ms to parse a layer; tree-house then walks the finished tree twice more
  — the injection query and the local query — on query cursors that carried no deadline
  at all, so how long opening or editing a buffer could freeze the tab for was set by the
  size of its tree rather than by the timeout. The vendored bindings now arm the same
  500 ms on every query cursor, so a walk that runs long stops early and the buffer loses
  some injected highlighting instead of the tab locking up. As with the parse timeout it
  is a ceiling per walk rather than per keystroke: a document with many injection layers
  can spend the budget once per layer.
  ([#120](https://github.com/Jecoms/helix-wasm/issues/120))

## [0.0.1] — 2026-08-18

First tagged release: [Helix](https://github.com/helix-editor/helix) 25.07.1 compiled to
`wasm32`, running in a browser tab against an xterm.js terminal, with the patch set that
gets it there carried in-tree.

**What `0.0.1` promises.** Not stability. The JS surface is unstable by design
(`web/src/session.rs`, `web/src/vfs.rs`) — the one exception is the read-only inspection
surface (`editor_state()` / `editor_text()`, `web/src/inspect.rs`), which is meant to be
kept stable. Pin a tarball rather than tracking `main`, and check the package's `.d.ts`
when you upgrade. A future release replays this port onto a newer helix, so editor
behavior can move under you for reasons that are upstream's rather than this port's.

**What it does not do.** The browser takes away subprocesses, a real filesystem and
threads, and that is load-bearing rather than incidental: no language servers, no
debugger, no shell commands, no external formatters, no git integration, no OS clipboard
in either direction, no persistence across a page reload. The port's docs'
[Limitations and behavioral differences](https://github.com/Jecoms/helix-wasm/blob/main/docs/limitations.md)
catalogs every one of those and is the section to read before deciding to embed this —
it is written from behavior reproduced by hand, not from what the source suggests.

### Added

- **The editor.** helix 25.07.1 boots in the browser and drives an xterm.js terminal
  through a wasm-bindgen module. This is upstream helix rather than a subset of it: the
  editor is unmodified except where the browser forced a change, and the "Known
  limitations" below — with the limitations catalog it points at — is the list of those
  changes, so treat anything not named there as behaving the way upstream's
  documentation says. Modal editing, splits, the `:` command prompt and `:tutor` are
  covered directly by the browser smoke suite. Mouse input (click, drag, wheel) and focus
  changes are forwarded.
- **Syntax highlighting** for a static grammar set linked at build time — c, go, java,
  javascript, python, regex, rust, toml — with queries read out of the in-tree helix
  runtime. `HELIX_WEB_GRAMMARS` narrows the set to slim the bundle; the published tarball
  carries the full catalog.
- **Ten bundled themes** (`catppuccin_latte`, `catppuccin_mocha`, `dracula`,
  `everforest_dark`, `gruvbox`, `nord`, `onedark`, `onelight`, `rose_pine`,
  `tokyonight`), selectable with `:theme` and completing from the same set. RGB themes
  render: the port answers the true-color question the browser has no `COLORTERM` or
  terminfo to answer.
- **An in-memory virtual file system** (`helix_stdx::vfs`) behind document IO, so the
  commands that want a filesystem have one to want. `:w`, `:o`, `:r`, `:move`, `:cd`,
  `:pwd`, path completion, the `<space>f` file picker (with preview) and the `<space>e`
  file explorer all read and write it. Store entries carry a modification time, so
  helix's external-modification guard works — a `:w` over a key changed underneath the
  buffer is refused, and `:w!` overrides it.
- **File injection and extraction from the host page:** `vfs_write` / `vfs_read` /
  `vfs_list` let a page seed the store before boot and pull work back out afterwards.
  The demo exposes them as `window.helixVfs`.
- **`:download`**, the way out that needs no page script: it hands the current buffer to
  the page, which saves it the way any other download arrives. The host registers
  `on_download(handler)` and decides what saving means.
- **A read-only inspection surface** for embedders that drive the editor rather than
  scrape the terminal: `editor_state()` returns mode, path, cursor and selections for the
  focused view; `editor_text()` returns the live buffer text, unsaved edits included.
  This is the one part of the JS surface intended to stay stable.
- **`config.toml` support**, read out of the VFS: the global config and the workspace one
  under the working directory, merged the way native helix merges them. `[keys]` remaps,
  `[editor]` settings and `theme` all apply. A page passes the text to `start()`;
  `:config-open` + `:w` + `:config-reload` edits it live, and a malformed config is
  reported rather than fatal.
- **A host-page contract** to replicate: `init()`, then `start(writeBytes, cols, rows,
  config)`, with `key_event(...)`, `mouse_event(...)`, `focus_event(...)`, `paste(...)`
  and `resize(...)` forwarding input, and `on_exit(handler)` reporting a real quit.
  `web/www/main.js` is the reference wiring.
- **Alt chords and dead keys reach the editor on macOS.** The page claims Option as Meta
  the way iTerm's "Option as Meta" setting does, including the accent starters that
  compose nothing until the next keystroke; the trade is that Option-composed character
  entry no longer works in insert mode.
- **A real monotonic clock on wasm32.** The `clock_gettime` shim reads the page's
  `performance.now()`, so helix's 500 ms tree-sitter parse timeout actually fires and an
  oversized file drops its highlighting instead of freezing the tab to parse to
  completion. The bound is not airtight — it covers the parse and not the injection and
  local queries that run after it, see
  [#120](https://github.com/Jecoms/helix-wasm/issues/120).
- **Files with unbalanced delimiters open in milliseconds, not minutes.** The vendored
  tree-sitter's tree cursor answered "does this node have a later sibling?" by rescanning
  the rest of its parent's child list, which is quadratic over the single flat `ERROR`
  node an unclosed delimiter produces. It now summarizes each child list once, so the
  walk is linear: in headless Chromium a `.rs` file of 100 000 unclosed `(` went from
  26.4 s to open to 153 ms. Highlighting is unchanged — the query output was diffed
  match for match against a pristine build.
- **Background jobs run on the browser's executor** rather than trapping on a tokio
  runtime that is not there, so the commands that queue one behave as they do natively.
- **`:q` ends the session instead of trapping.** The exit teardown runs, the main screen
  is restored, and the registered `on_exit` handler fires; the page reloads to start
  over.
- **`LICENSE`, `NOTICE.md` and this file ride in the tarball.** The bundle is MPL-2.0 —
  helix's files and this port's alike — and the `package.json` wasm-pack generates
  declared that without shipping the text, so the license text now travels with the
  artifact. `NOTICE.md` beside it covers the third-party components the wasm statically
  links.
- **A Playwright smoke suite** (`web/www/tests/`) that boots the built bundle in headless
  Chromium and asserts on editor behavior through `helixState` / `helixVfs` and the
  terminal buffer — the parity check when this port is replayed onto a new helix release.

### Known limitations

The short version of the docs'
[catalog](https://github.com/Jecoms/helix-wasm/blob/main/docs/limitations.md),
which is the source of truth and goes further than this list:

- **Nothing survives a page reload.** The VFS is memory. `:download`, `helixVfs.read`
  and `helixVfs.list` are the ways out; a persistent backend is the host page's to build
  on those hooks.
- **No language servers, no DAP, no shell commands, no external formatters, no git.**
  Everything that shells out fails cleanly with helix's own "not configured" message.
  Completion and signature help are silently absent. `<space>/` (global search) opens but
  never matches.
- **No directories, only key prefixes.** An empty directory and a missing one are the
  same thing, and `:move /some/dir` renames *to* that key rather than moving into it.
- **The OS clipboard is not wired up.** `"+y` and `"*y` are editor-local registers.
  Browser paste works and arrives bracketed; copying out means selecting with the mouse.
- **Your browser claims some chords first** — `C-w`, helix's window prefix, closes the
  tab in most of them. Reach the window menu at `space w` instead.
- **Picker matching and tree-sitter parsing run on the main thread**, so a large picker
  or a big parse blocks input and rendering rather than streaming. Helix's 500 ms parse
  timeout does fire here, but it bounds the parse only: the injection and local queries
  that run over the finished tree have no deadline at all.
- **`languages.toml` and `.editorconfig` are unreachable**, and `.helix/` is never
  detected — the workspace is always the working directory.
- **One editor per page**, and no shell command line — the page boots the module with
  default arguments, so `hx <file>`, `-c` and `--tutor` have no equivalent (helix's own
  `:` prompt is unaffected). No kitty keyboard protocol, no suspend, and only the
  grammars and themes linked into the bundle.

<!-- The `0.0.2` links below resolve once `web-v0.0.2` is pushed; until then
     they are pending rather than broken. Publishing is a separate, deliberate
     step — see [Embedding the editor](docs/embedding.md) for the procedure. -->

[Unreleased]: https://github.com/Jecoms/helix-wasm/compare/web-v0.0.5...main
[0.0.5]: https://github.com/Jecoms/helix-wasm/releases/tag/web-v0.0.5
[0.0.4]: https://github.com/Jecoms/helix-wasm/releases/tag/web-v0.0.4
[0.0.3]: https://github.com/Jecoms/helix-wasm/releases/tag/web-v0.0.3
[0.0.2]: https://github.com/Jecoms/helix-wasm/releases/tag/web-v0.0.2
[0.0.1]: https://github.com/Jecoms/helix-wasm/releases/tag/web-v0.0.1
