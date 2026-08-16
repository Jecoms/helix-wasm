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
#
# The snapshot commit is intentionally unsigned — its SHA must stay
# deterministic across re-cuts — so this script also cuts a signed
# annotated tag `helix-<version>` (dash, not slash: a `helix/<version>`
# tag would make the refname ambiguous with the branch) that supplies the
# cryptographic attestation for the snapshot.
#
# Publishing: the `helix/*` branch and `helix-*` tag namespaces are frozen
# by creation-only rulesets (snapshot-branches-frozen /
# snapshot-tags-frozen, no bypass actors): pushing a new ref is allowed,
# moving or deleting an existing one is refused server-side. The unsigned
# snapshot commit passes the repo-wide required-signatures ruleset only
# via its repo-admin bypass, so publishing takes an admin push.
set -euo pipefail

if [ $# -ne 2 ]; then
    echo "usage: $0 <version> <workbench-rev>" >&2
    exit 2
fi
version=$1
src=$2
ref="helix/${version}"
tag="helix-${version}"

case $version in
*[/\ ]* | '')
    echo "error: '$version' is not a valid version" >&2
    exit 2
    ;;
esac

src_commit=$(git rev-parse --verify "${src}^{commit}")
tree=$(git rev-parse "${src_commit}^{tree}")
short=$(git rev-parse --short=8 "$src_commit")

# Provenance label: "<refname>@<sha>" when a ref was passed, bare short sha
# when the source was given as the sha itself. Prefix match, not equality:
# --short=8 can widen past 8 chars when abbreviations collide, and any
# unambiguous hex prefix of the commit should collapse the same way.
case $src_commit in
"$src"*) label=$short ;;
*) label="${src}@${short}" ;;
esac
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
snapshot_short=$(git rev-parse --short=8 "$snapshot")

# Append-only guard: an existing helix/* ref is never moved. Re-cutting the
# identical snapshot is a no-op; anything else must pick a new -rN ref.
# Plain assignments on purpose: under set -e/pipefail a failing ls-remote
# (offline, no remote named origin) aborts the script here instead of
# silently skipping the remote half of the check.
local_branch=$(git rev-parse --verify --quiet "refs/heads/$ref" || true)
remote_branch=$(git ls-remote origin "refs/heads/$ref" | cut -f1)
for existing in $local_branch $remote_branch; do
    if [ "$existing" != "$snapshot" ]; then
        echo "error: $ref already exists at $existing (would be $snapshot)." >&2
        echo "Snapshots are append-only; cut a respin ref instead:" >&2
        echo "helix/${version%-r*}-rN, with the next unused -rN suffix." >&2
        exit 1
    fi
done

# Same guard for the attestation tag. Compare peeled targets: each signing
# produces a distinct tag object, but the commit it points at must match.
local_tag=$(git rev-parse --verify --quiet "refs/tags/${tag}^{commit}" || true)
remote_tag=$(git ls-remote origin "refs/tags/$tag" "refs/tags/${tag}^{}" | tail -n1 | cut -f1)
for existing in $local_tag $remote_tag; do
    if [ "$existing" != "$snapshot" ]; then
        echo "error: tag $tag already points at $existing (snapshot is $snapshot)." >&2
        echo "Tags are append-only too; a respin ref gets its own helix-<version>-rN tag." >&2
        exit 1
    fi
done

git branch --force "$ref" "$snapshot" # --force is safe: guard above proved same SHA
echo "cut $ref -> $snapshot (tree $tree)"

if [ -z "$local_tag" ] && [ -z "$remote_tag" ]; then
    git tag -s "$tag" "$snapshot" -m "helix ${version} + wasm patch set

Signed provenance tag for snapshot branch ${ref} (commit ${snapshot_short}).
The snapshot commit itself is intentionally unsigned so its SHA stays
deterministic (reproducible via scripts/snapshot-helix.sh from ${label});
this tag supplies the cryptographic attestation."
    echo "cut signed tag $tag -> $snapshot"
elif [ -z "$local_tag" ]; then
    echo "note: $tag already exists on origin; fetch it with: git fetch origin tag $tag"
else
    echo "note: $tag already exists locally; leaving it as-is"
fi

echo "publish with: git push origin refs/heads/$ref refs/tags/$tag"
echo "(the creation-only rulesets accept new snapshot refs; the unsigned"
echo " snapshot commit needs the required-signatures ruleset's admin bypass)"
