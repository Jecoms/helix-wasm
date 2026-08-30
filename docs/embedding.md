# Embedding the editor

Each `web-v<semver>` tag publishes the `web/pkg` wasm-pack output — the same
unit the demo consumes as `file:../pkg` — as a GitHub release with a
`helix-web-<version>.tar.gz` attached (the `Publish web bundle` workflow,
`.github/workflows/web_release.yml`). Embedders should pin one of those
instead of linking into the deployed demo, whose asset names are
content-hashed and replaced on every push to `main`:

```sh
curl -LO https://github.com/Jecoms/helix-wasm/releases/download/web-v0.0.5/helix-web-0.0.5.tar.gz
tar xzf helix-web-0.0.5.tar.gz    # extracts helix-web-0.0.5/
```

Each release carries two tarballs that differ only in which tree-sitter
grammars are linked into the wasm. `helix-web-<version>.tar.gz` links the
default grammar set — the languages a developer opens most, listed under
"What the bundle ships with" in [Building from source](building.md#what-the-bundle-ships-with) —
and is the one to reach for; `helix-web-<version>-full.tar.gz` links the
whole catalog and is several times the size. Any other selection is a build
from source with `HELIX_WEB_GRAMMARS` (same page): grammars are linked at
build time, and there is no loading one at runtime.

The extracted directory is a standard wasm-pack `--target web` package (ES
module + `.wasm` + `.d.ts`, plus the MPL-2.0 `LICENSE` the bundle is under,
`NOTICE.md` with the license notices for the Rust crates the wasm links and
the C it statically links, `CHANGELOG.md` as of that version, and
`GRAMMARS.txt` naming the grammars that tarball links, one per line, so an
extracted tree says which tier it is). Consume it
the way the demo's `web/www/package.json` does:

```json
"dependencies": { "helix-web": "file:../helix-web-0.0.5" }
```

`web/www/main.js` is the reference host wiring to replicate: call `init()`
(fetches and instantiates the wasm module), then `start(writeBytes, cols,
rows, config, languages)` with a callback that feeds editor output bytes to
an xterm.js `Terminal`, and forward input with `key_event(...)`,
`paste(...)`, and `resize(cols, rows)`. `config` is the text of a
`config.toml` and `languages` the text of a `languages.toml`, each
`undefined` for helix's defaults (see [Configuration](limitations.md#configuration)). Register
`on_exit(handler)` before `start` to learn when helix quits (`:q` and
friends really do exit, and nothing can restart it in place — the page has
to reload), and route the calls into wasm through a `try`/`catch` as the
demo page does: a panicked instance traps on every later call, and a host
that keeps forwarding into it silently swallows the user's input. Input
calls made after a clean exit are inert (the module drops them rather than
queueing for an event loop that is gone), but a page still forwarding is a
page still pretending to have an editor — stop on the exit and tell the
reader.

`on_download(handler)` is the other callback worth registering: `:download`
and `:download-all` call it with the file name to save under and a
`Uint8Array` of the bytes, and the page decides what saving means — a `Blob`
and an object URL (what `main.js` does, replaceable at
`window.helixDownload` for a devtools session), a File System Access handle,
a POST to a server. One handler serves both commands; the archive arrives as
one more file, so a handler that does not care can stay unaware there are
two. Throwing from it refuses the save and puts the message on the
statusline; registering nothing leaves both commands reporting that this
host cannot save files, so wire it up or expect readers to have no way out
of the page. If your page seeds files with `vfs_write` *before* `start`,
note that `:download-all` treats those as the page's rather than the
reader's and leaves them out — seed after `start` for files a reader should
get back.

`on_remove(handler)` is what turns `:remove` on: `:remove` calls it with the
store key about to go (absolute, as `vfs_list` reports it) *before* the key
is dropped and the buffer on it closed, and throwing refuses the deletion
with the store untouched and the message on the statusline. Register it
where a page should offer deletion — it is also where a page that mirrors
the store prunes its mirror, and it may `vfs_delete` the key itself while
it is at it; a key already gone after consent counts as removed — and
register nothing where it should not (a read-only lesson, say):
unregistered, `:remove` reports that this host cannot remove files. It is not called for a path with no key in the store
(never saved, or `vfs_delete`'d since — the buffer just closes), nor by
`vfs_delete` itself, which is the page deleting on its own behalf. The demo's
handler is a no-op at `window.helixRemove`, replaceable for a devtools
session.

### Language servers

The browser cannot spawn the server process helix talks LSP to over stdio,
but LSP is JSON-RPC over any byte stream, and a Web Worker's `postMessage`
is one ([#144](https://github.com/Jecoms/helix-wasm/issues/144)). A page
supplies a language server as a `Worker` (or a `MessagePort`) registered
under the name its `languages.toml` uses:

```js
window.helixLanguages = `
[language-server.toy]
command = "toy-lsp"        # ignored: the registered name is the match

[[language]]
name = "toy"
scope = "source.toy"
file-types = ["toy"]
roots = []
language-servers = ["toy"]
`;
window.helixLanguageServers = { toy: new Worker("./toy-lsp-worker.js") };
```

That is the demo page's wiring (`register_language_server(name, port)` for
each entry, before `start`); an embedder calls the export directly. The
wire format is one complete JSON-RPC message per `postMessage`, as a string,
in each direction — no `Content-Length` framing. Helix's unmodified LSP
client runs on top: the `initialize` handshake, the pending-request map,
the gating that holds requests until the server has answered `initialize`,
`didOpen`/`didChange` sync, and everything the client drives from there —
the completion popup, hover, signature help, `gd` and its siblings,
diagnostics, code actions, rename — as far as the server on the other end
implements them. What that server *is* is the page's business: a scripted
responder (`web/www/toy-lsp-worker.js` is the one the browser tests drive
completion, hover and `gd` through), or a real server compiled to wasm and
loaded in the worker, with no network involved either way. Servers are
launched lazily, on the first document of a language that lists them, so
register before that document opens; `:lsp-restart` connects to the same
port afresh — the old connection is severed rather than shut down, so the
`exit` meant for it never reaches the worker, and the server just sees a
second `initialize` (`:lsp-stop` does deliver `exit`, and a worker that
honors it is gone until the page registers a new one). A name in
`languages.toml` with no port registered fails the way an unconfigured
server always has (the "No subprocesses" table). Three things a real server
would notice: `initialize` carries no `processId` (there is none), it can
arrive more than once, and `workspace/didChangeWatchedFiles` registrations
are accepted but never fire — the VFS has no watcher.

Beyond the terminal loop, the module
exports the file-injection hooks (`vfs_write` / `vfs_read` / `vfs_list` /
`vfs_delete`, see [Virtual file system](vfs.md)) and the read-only inspection surface (`editor_state()`
/ `editor_text()`, see [Editor state inspection](vfs.md#editor-state-inspection)) — the intended surface
for tutorial-style embedders that drive and assert on the editor rather
than scrape the rendered terminal. The JS surface is unstable by design
(`web/src/session.rs`, `web/src/vfs.rs`, `web/src/lsp.rs`), with one exception: the
read-only inspection surface (`web/src/inspect.rs`,
[#18](https://github.com/Jecoms/helix-wasm/issues/18)) is meant to be
kept stable. Either way, pin a tagged tarball and check its `.d.ts` when
upgrading — [`CHANGELOG.md`](../CHANGELOG.md) is what changed between two of
them, and its scope note says which changes reach the bundle at all.

To cut a release: bump the version everywhere the old one appears — `version`
in `web/Cargo.toml` and the root `Cargo.toml` (and `Cargo.lock`), the crate
table in `web/NOTICE.md`, the download snippet and `file:../helix-web-<version>`
line above, the README's quick start, and the demo's `web/www/package.json` /
`package-lock.json` — then
turn the changelog's `[Unreleased]` section into the new version's entry and
add its link reference at the bottom. Grep for the previous version string
before merging; it is the only reliable list. Merge, then tag that commit
`web-v<version>` and push the tag. The workflow verifies the tag
against the crate version, rebuilds the bundle with `--locked`, and
attaches the tarball to a release on the tag.
