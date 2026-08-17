# Changelog

Notable changes to **`helix-web`** — the embeddable wasm bundle this repo publishes.

**Scope.** This file tracks the *artifact*, not the repository. The unit that gets
versioned is the `web/pkg` wasm-pack output that a `web-v<semver>` tag ships as
`helix-web-<version>.tar.gz` (see
[Embedding the editor](https://github.com/Jecoms/helix-wasm/blob/main/README.md#embedding-the-editor)),
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
absolute — the copy in an embedder's extracted tree has no README next to it.

## [Unreleased]

## [0.0.1] — 2026-08-17

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
in either direction, no persistence across a page reload. The README's
[Limitations and behavioral differences](https://github.com/Jecoms/helix-wasm/blob/main/README.md#limitations-and-behavioral-differences)
catalogs every one of those and is the section to read before deciding to embed this —
it is written from behavior reproduced by hand, not from what the source suggests.

### Added

- **The editor.** helix 25.07.1 boots in the browser and drives an xterm.js terminal
  through a wasm-bindgen module. This is upstream helix rather than a subset of it: the
  editor is unmodified except where the browser forced a change, and the "Known
  limitations" below — with the README catalog it points at — is the list of those
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

The short version of the README's
[catalog](https://github.com/Jecoms/helix-wasm/blob/main/README.md#limitations-and-behavioral-differences),
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

<!-- Both links below resolve once `web-v0.0.1` is pushed; until then they are
     pending rather than broken. Publishing is a separate, deliberate step —
     see the README's "Embedding the editor" for the procedure. -->

[Unreleased]: https://github.com/Jecoms/helix-wasm/compare/web-v0.0.1...main
[0.0.1]: https://github.com/Jecoms/helix-wasm/releases/tag/web-v0.0.1
