# helix-wasm

A wasm32 port of [Helix](https://github.com/helix-editor/helix). `main` is a
**version line rooted at an upstream release**: `helix/` carries the pristine
25.07.1 release tree with the wasm patch set as ordinary commits on top, and
everything that does not belong in helix — the browser frontend, the
dependency stubs, the C sysroot — sits alongside it. The helix crates are
path dependencies, so patching helix is editing a file in this workspace, and
the repo has no cargo git dependencies at all: a checkout of `main` is the
whole build input.

## Layout

| Path | Purpose |
| --- | --- |
| `helix/` | The patched Helix source: upstream's `25.07.1` release tree (the parentless `upstream/25.07.1` commit) plus this port's patches. Its own cargo workspace — upstream's, left pristine — excluded from the root one and consumed as path dependencies |
| `Cargo.toml` | Wrapper workspace: the helix crates as path dependencies on `helix/`, plus `[patch.crates-io]` stub swaps |
| `stubs/` | Stand-ins for third-party dependencies with no wasm32 support: transitive crates (`home`, `which`, `libloading`, and `url` with a wasm cfg), a vendored `crossterm` whose OS terminal layer is replaced by a browser bridge, and a vendored `nucleo` that runs picker matching inline instead of on a threadpool. Plus one that is not a wasm32 gap: a vendored `tree-house-bindings` carrying a one-declaration ABI fix, without which every syntax-highlighted buffer traps on wasm32 — the only stub shipping third-party C (a vendored tree-sitter; see `web/NOTICE.md`) |
| `sysroot/` | Stub libc headers, the `wasm-cc` clang shim that lets tree-sitter's stock build script compile its C for wasm32, and the libc shim implementations (`shims.c`, `wctype.c`) the final wasm link needs |
| `web/` | The browser frontend: a wasm-bindgen cdylib that boots helix-term against the crossterm bridge, plus the xterm.js host page in `web/www/` |
| `.cargo/config.toml` | Wires `wasm-cc` up as the C compiler for the wasm32 target |

## Building

```sh
rustup target add wasm32-unknown-unknown
cargo check -p helix-core --target wasm32-unknown-unknown
cargo check -p helix-view --target wasm32-unknown-unknown
cargo check -p helix-term --target wasm32-unknown-unknown
```

A clang that can emit wasm is required. Linux distro clang works; on macOS the
system clang cannot emit wasm, so install LLVM (`brew install llvm`) or point
`HELIX_WASM_CLANG` at a suitable clang.

## Patching helix

`helix/` is ordinary source in this workspace, so a helix change is an
ordinary edit:

```sh
$EDITOR helix/helix-view/src/document.rs
cargo check -p helix-view --target wasm32-unknown-unknown
```

Commit it like any other change. There is no ref to publish first, no pin to
bump, no `cargo update` — the path dependency picks the edit up directly.

Two things keep the patch set cheap to carry onto the next release. Shape:
localized insertions and `#[cfg(target_arch = "wasm32")]` arms replay clean,
while re-indenting a block of otherwise-untouched native code conflicts with
any upstream edit to it — prefer extracting a native body into its own
function over wrapping it. And blast radius: `helix/Cargo.toml` is
byte-identical to upstream on purpose (it is the file upstream churns most),
so declare a new dependency in the individual crate manifests rather than in
its `[workspace.dependencies]`.

What the patch set changes, at any point:

```sh
git diff upstream/25.07.1 main -- helix/
```

## Running the browser demo

```sh
wasm-pack build web --target web
cd web/www
npm install
npm run dev      # serves the demo on a local vite dev server
```

The demo boots helix into an xterm.js terminal with a scratch buffer, with
syntax highlighting for a small static grammar set (c, go, java,
javascript, python, regex, rust, toml — try `:set-language rust`) and a
curated set of bundled color schemes (the `THEMES` catalog in
`web/build.rs` — try `:theme gruvbox`).
It is helix, not a subset of it — but the browser has no subprocesses, no
filesystem and no threads, so some things behave differently and some do not
work at all. "Limitations and behavioral differences" below collects them.
`:tutor` works, with a handful of steps the browser cannot honor as written;
`web/runtime/README.md` lists those under "Known gaps in the browser".

The grammar build fetches pinned parser sources at
build time, so it needs network access and `git`. Set `HELIX_WEB_GRAMMARS`
to a comma-separated subset (e.g. `HELIX_WEB_GRAMMARS=rust,toml wasm-pack
build web --target web`) to slim the bundle; to add a grammar to the
catalog, see `GRAMMARS` in `web/build.rs`. Its queries and themes are read
out of the in-tree port at `helix/runtime/`, not copied into `web/` — see
`web/queries/README.md` and `web/themes/README.md` for how the build picks
which of them to embed.

### Virtual file system

Documents live in an in-memory virtual file system (`helix_stdx::vfs`, part
of the wasm patch set): `:w /notes.txt` saves there, `:o` and the `<space>f`
file picker (with preview) read from it, and `:reload` picks up outside
changes. Nothing survives a page reload, and a few document commands behave
differently against it — see "Files live in an in-memory VFS" below.
The wasm module exports `vfs_write` / `vfs_read` / `vfs_list` so an
embedding page can inject and extract files; the demo page exposes them
as `window.helixVfs` — try `helixVfs.write("hello.rs", "fn main() {}")`
in the devtools console, then `:o hello.rs` in the editor. Persistent
backends (localStorage/OPFS) can be layered on those hooks by the host
page. Boot seeds a couple of sample files (`web/src/samples.rs`) so the
picker opens on something selectable.

### Editor state inspection

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

## Limitations and behavioral differences

The aim is to be helix, not a lookalike, so this section catalogs the places
where the browser makes that impossible. Everything below was reproduced by
hand in a build of this tree, except where an entry says otherwise — it is
what the port does today, not what its source suggests it might do. The
entries track `helix/` plus the `web/` crate, so re-check them when the port
is replayed onto a new release (see "Taking a new helix release").

### Commands that crash the session

`Jobs::add` spawns onto a tokio runtime, and there is no runtime in the
browser, so a command that queues a background job panics rather than bailing
out politely. A panic wedges the instance: the editor stops drawing and
keystrokes stop landing. The page notices, prints `Helix has stopped
responding. Refresh the page.` on the terminal and stops forwarding input, so
a reload is still the only recovery — but a wedged editor never passes for a
hung page. This is a bug rather than something the browser forces — tracked in
[#71](https://github.com/Jecoms/helix-wasm/issues/71). Confirmed triggers:

- **`gd`, `gD`, `gy`, `gi`, `gr`** — goto definition, declaration, type
  definition, implementation and reference. Bare `g`-prefix keys, which makes
  these the easiest crash in the list to hit without meaning to. Helix's
  other language-server entry points check for a configured server before
  they queue anything and bail cleanly (see "No subprocesses"); the five
  goto commands do not.
- `:sh`, `:!`, `:run-shell-command`
- `:redraw`
- `:tree-sitter-scopes`
- `:tree-sitter-subtree` and `:tree-sitter-highlight-name`, but only once the
  buffer has a syntax tree — after `:set-language rust`, say. On a plain
  scratch buffer both return before queuing anything and bail cleanly.
- `:lsp-workspace-command`
- `gf` with the cursor on a URL — opening one would hand off to an external
  program. `gf` on a file path works normally.
- **Enter on an invalid regex at a `/` or `?` search prompt** (`/[` then
  Enter). Valid searches are fine, and so is backing out of a bad one with
  Escape.

Treat that list as incomplete: it is whatever path reaches `Jobs::add`.

### No subprocesses

Everything that shells out is gone. These fail cleanly, reusing the message
helix already has for "not configured":

| What | What you get |
| --- | --- |
| Language servers — diagnostics, hover, rename, code actions | "No configured language server supports …"; `:lsp-restart` → "LSP not defined for the current document" |
| Debugging — `:debug-start`, `:debug-remote`, the rest of the DAP layer | "No debug adapter available for language" |
| External formatters, including format-on-save (`:format`) | "A formatter isn't available, and no language server provides formatting capabilities" |
| Shell piping — the `!` and `\|` keys, `:pipe`, `:pipe-to`, `:insert-output`, `:append-output` | "Shell commands are not supported on this platform" |
| Git — the diff gutter, `:reset-diff-change`, the `<space>g` changed-file picker | "Diff is not available in the current buffer" / "Current working directory does not exist" |
| `<space>e` file explorer | "Workspace directory does not exist" |

Two language-server features are not in that table because they fail some
other way: the goto commands crash instead of reporting a missing server (see
above), and completion goes quiet rather than saying anything at all (see "No
background work").

`<space>/` (global search) opens its picker, but the query handler it needs
is never spawned, so typing never returns a match. There is no handoff to the
browser either, so nothing can open a URL. Dynamic grammar loading is out as
well (`libloading` is stubbed), so the grammar set is whatever was linked at
build time.

There is no command line either: the host page boots the module with default
arguments, so `hx <file>`, `-c` and `--tutor` have no equivalent. `:tutor`
itself works.

### Files live in an in-memory VFS

Documents are keys in `helix_stdx::vfs`, not files (see "Virtual file system"
above). What that changes:

- **Nothing survives a page reload.** Pull anything you care about out
  through `helixVfs.read` / `helixVfs.list` first.
- **`:w` is last-write-wins and never warns.** There are no mtimes, so the
  "file modified externally" guard can never fire — a `:w` silently
  overwrites whatever a `helixVfs.write` put there in the meantime. `:w!`
  does exactly what `:w` does, and nothing is ever read-only.
- **`:move` renames the buffer but moves nothing.** The rename is guarded by
  an existence check the VFS can never satisfy, so `:move /b.txt` retargets
  the buffer and stops there: the old key keeps its contents, and the new one
  does not exist until the next `:w`. You are left with both, the original
  holding a stale copy, and nothing says so.
- **There are no directories.** `:o /some/dir` opens an ordinary empty buffer
  named `/some/dir`. `:cd` works and does change how relative paths resolve,
  but `:pwd` always reports the directory as `(deleted)` — the existence
  check behind that label has no directory to find.
- **The file picker lists the whole VFS**, seeded runtime files (the themes
  and the tutor text) included, and every `file-picker.*` option — `hidden`,
  `git-ignore`, `max-depth` — is inert.
- **Path arguments do not tab-complete.** `:o`, `:cd` and friends walk the
  real filesystem to build their candidate list, so they offer nothing; use
  `<space>f` instead. `:theme` completion does work — it reads the VFS.
- `:config-open` and `:log-open` open empty buffers at
  `/.config/helix/config.toml` and `/.cache/helix/helix.log`. Those keys are
  real, but nothing ever writes them; log output goes to the browser console.

### Configuration is runtime-only

`config.toml`, `languages.toml`, `.editorconfig` and `.helix/` are all read
through `std::fs`, which is unconditionally an error on wasm32, so none of
them is reachable:

- **Custom keymaps are not possible.** The default keymap is the only keymap.
- `:set`, `:set-option` and `:toggle-option` work as usual, for the session.
- `:config-reload` reports "Failed to load config: operation not supported on
  this platform" and leaves the running config alone — including the
  boot-time `true-color` override, so the active theme survives it.

### No background work

No threads and no tokio runtime, so what helix normally does off the main
loop either does not happen or happens inline:

- **No completion popup and no signature help.** `C-x` in insert mode does
  nothing; both are driven by handlers that need a runtime, and there is no
  language server to feed them in any case.
- **`auto-save.after-delay` never fires.** An explicit `:w` still works.
- **Picker matching runs inline** on the browser's main thread — the vendored
  `nucleo` calls the match job directly instead of handing it to a
  threadpool — so a large picker blocks rendering and input while it matches
  instead of streaming results in.
- **Tree-sitter's 500 ms parse timeout never fires.** The libc clock shim is
  frozen at zero (`sysroot/shims.c`), so every parse runs to completion and a
  pathological file can hang the tab.

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
  own (macOS UK Option-3 gives `A-#`, not `A-3`). None of this applies off
  macOS. One gap remains: the accent starters `A-e`, `A-i`, `A-n` and `A-u`
  work, but `` A-` `` (`switch_to_uppercase`, the chord `:tutor` 10.3 asks
  for) does not — xterm.js drops that keystroke before the page can see it,
  tracked in [#81](https://github.com/Jecoms/helix-wasm/issues/81). The
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
under "Running the browser demo" above), and `:theme` only the themes the
bundle embeds (`THEMES` in `web/build.rs`). Anything else opens as plain text —
`:set-language haskell` is accepted without complaint and simply highlights
nothing — and any other theme name is not found.

## Live demo

The demo deploys to <https://jecoms.github.io/helix-wasm/> via the
`Deploy web demo` workflow (`.github/workflows/web_demo.yml`), which builds
the full-catalog bundle and publishes it with `actions/deploy-pages`.

Every push to `main` deploys automatically; a manual
`gh workflow run web_demo.yml` works too. Deploys are gated by the
`github-pages` environment's deployment branch policy — only `main` is on
the allowed list.

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
rows)` with a callback that feeds editor output bytes to an xterm.js
`Terminal`, and forward input with `key_event(...)`, `paste(...)`, and
`resize(cols, rows)`. Register `on_exit(handler)` before `start` to learn
when helix quits (`:q` and friends really do exit, and nothing can restart
it in place — the page has to reload), and route the calls into wasm
through a `try`/`catch` as the demo page does: a panicked instance traps on
every later call, and a host that keeps forwarding into it silently
swallows the user's input. Input calls made after a clean exit are inert
(the module drops them rather than queueing for an event loop that is gone),
but a page still forwarding is a page still pretending to have an editor —
stop on the exit and tell the reader. Beyond the terminal loop, the module
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

## Taking a new helix release

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
privileged step left, once per release instead of once per patch.

Then replay the port onto it:

```sh
git checkout -b "port/$V" main
git rebase --onto "upstream/$V" upstream/25.07.1 "port/$V"
```

Open `port/$V` → `upstream/$V`. The base is the merge base, so the diff is
exactly this repo's commits and none of helix's source. The wrapper commits
touch files upstream never touches and replay clean, which narrows the
conflict set to the helix files this port patches. **Parity is the Playwright
suite passing** (see "Browser smoke tests" above). Promote by moving `main`
to the reviewed tip, keeping the outgoing line as a versioned branch.

## Branch and tag map

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
