#!/usr/bin/env bash
# Cut an append-only snapshot ref `helix/<version>` from the helix workbench.
#
# The workbench branch (helix-patched) is a moving target: it gets rebased
# onto each upstream helix release, so pinning Cargo deps to it costs a full
# upstream-history fetch per cold cache and can strand lockfiles when old
# SHAs become unreachable. Instead, main pins `helix/<version>`: a parentless
# commit of the workbench's tree — one commit, one tree, no history.
#
# Usage: scripts/snapshot-helix.sh <version> <workbench-rev>
#   e.g. scripts/snapshot-helix.sh 25.07.1 helix-patched
#
# The snapshot commit's author/committer identity and dates are copied from
# the source commit, so re-running with the same inputs recreates the same
# SHA (idempotent, not a respin). Existing `helix/*` refs are never moved:
# they are freeze points main's history depends on. A changed patch set
# against the same upstream base gets a revision suffix instead:
# `helix/<version>-r2`, `-r3`, ... (bare version = r1).
set -euo pipefail

if [ $# -ne 2 ]; then
    echo "usage: $0 <version> <workbench-rev>" >&2
    exit 2
fi
version=$1
src=$2
ref="helix/${version}"

case $version in
*[/\ ]* | '')
    echo "error: '$version' is not a valid version" >&2
    exit 2
    ;;
esac

src_commit=$(git rev-parse --verify "${src}^{commit}")
tree=$(git rev-parse "${src_commit}^{tree}")
short=$(git rev-parse --short=8 "$src_commit")

# Provenance label: "<refname>@<sha>" when a ref was passed, bare sha otherwise.
if [ "$src" = "$src_commit" ] || [ "$src" = "$short" ]; then
    label=$short
else
    label="${src}@${short}"
fi
msg="helix ${version} + wasm patch set (source: ${label})"

snapshot=$(
    GIT_AUTHOR_NAME=$(git log -1 --format=%an "$src_commit") \
    GIT_AUTHOR_EMAIL=$(git log -1 --format=%ae "$src_commit") \
    GIT_AUTHOR_DATE=$(git log -1 --format=%ad --date=raw "$src_commit") \
    GIT_COMMITTER_NAME=$(git log -1 --format=%cn "$src_commit") \
    GIT_COMMITTER_EMAIL=$(git log -1 --format=%ce "$src_commit") \
    GIT_COMMITTER_DATE=$(git log -1 --format=%cd --date=raw "$src_commit") \
    git commit-tree "$tree" -m "$msg"
)

# Append-only guard: an existing helix/* ref is never moved. Re-cutting the
# identical snapshot is a no-op; anything else must pick a new -rN ref.
for existing in $(git rev-parse --verify --quiet "refs/heads/$ref" || true) \
                $(git ls-remote origin "refs/heads/$ref" | cut -f1); do
    if [ "$existing" != "$snapshot" ]; then
        echo "error: $ref already exists at $existing (would be $snapshot)." >&2
        echo "Snapshots are append-only; cut a respin ref instead, e.g. helix/${version%-r*}-r2." >&2
        exit 1
    fi
done

git branch --force "$ref" "$snapshot" # --force is safe: guard above proved same SHA
echo "cut $ref -> $snapshot (tree $tree)"
echo "publish it with: git push origin refs/heads/$ref"
