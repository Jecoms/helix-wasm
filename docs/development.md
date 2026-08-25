# Working on the port

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
| `stubs/` | Stand-ins for third-party dependencies with no wasm32 support: transitive crates (`home`, `which`, `libloading`, and `url` with a wasm cfg), a vendored `crossterm` whose OS terminal layer is replaced by a browser bridge, and a vendored `nucleo` that runs picker matching inline instead of on a threadpool. A vendored `tree-house-bindings` rides here too — not a missing-support stand-in but an ABI fix, without which every syntax-highlighted buffer traps on wasm32, plus two fixes for freezing the page on syntax work: a quadratic removed from the tree-sitter it vendors, and a wall-clock budget on query cursors — the only stub shipping third-party C (a vendored tree-sitter; see `web/NOTICE.md`) |
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

CI runs no Rust tests, so the one covering a stub's delta is a local command:
`cargo test -p tree-house-bindings --lib` exercises the wall-clock budget the
vendored query cursor arms (delta 3). It builds for the host — the vendored
tree-sitter C compiles there without the `wasm-cc` shim.

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

One wrapper file shadows a helix one and so replays clean whatever upstream did
to it: the root `LICENSE` is a verbatim copy of `helix/LICENSE`. Diff the two
after the replay and re-copy if the release moved them apart — "Credits and
license" says they are byte-identical.

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
- `web-v<semver>` (e.g. `web-v0.0.1`) — release tags for the embeddable web
  bundle. Pushing one runs the `Publish web bundle` workflow
  (`.github/workflows/web_release.yml`), which checks the tag against
  `web/Cargo.toml`'s `version`, rebuilds the full-catalog `web/pkg`
  wasm-pack output, and attaches it to a GitHub release as
  `helix-web-<version>.tar.gz` — the artifact [Embedding the editor](embedding.md)
  pins, and the thing [`CHANGELOG.md`](CHANGELOG.md) versions.
