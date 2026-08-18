#!/usr/bin/env bash
#
# Prune this repository's GitHub Actions cache.
#
# The cache is a single 10 GB pool for the whole repo, evicted least-recently-used.
# `Swatinem/rust-cache` writes a key that ends in a hash of the dependency graph and
# never removes the key the previous hash wrote, so every `Cargo.lock` change leaves a
# ~200 MB entry behind on whichever ref built it. Left alone the pool sits at its limit
# and the eviction that follows is LRU, not least-useful: a branch that has been quiet
# for a few days loses its live entry to a pile of dead ones.
#
# Two rules, both chosen so that nothing a future run could have restored is deleted:
#
#   (a) Superseded rust-cache keys. `rust-cache` asks for its full key and falls back to
#       a restore key — that key minus the trailing deps hash — and GitHub resolves a
#       prefix match to the *most recently created* entry. So within one
#       (ref, restore-key) group exactly one entry is reachable, the newest; every
#       entry behind it is unreachable by construction. Keep the newest, drop the rest.
#
#   (b) Caches owned by a closed pull request. A `refs/pull/N/merge` ref is only ever
#       built by runs on PR N, so once that PR is not open nothing will read them again.
#       Restores never cross from one PR ref to another; PR runs fall back to the
#       default branch's entries, which rule (a) preserves.
#
# Deliberately not pruned: keys that are not `rust-cache`'s (the Playwright browser
# entry is keyed on the resolved browser version rather than a deps hash, so it has no
# superseded set), and the last entry of a group whose env hash has rotated after a
# rustc bump — that one is left to GitHub's own 7-day untouched-entry eviction.
#
# Usage, by hand or from the workflow:
#
#   GH_REPO=Jecoms/helix-wasm DRY_RUN=true .github/scripts/prune-caches.sh

set -euo pipefail

repo=${GH_REPO:-${GITHUB_REPOSITORY:?set GH_REPO or GITHUB_REPOSITORY}}
dry_run=${DRY_RUN:-false}
summary=${GITHUB_STEP_SUMMARY:-/dev/stdout}

gb() { awk -v b="$1" 'BEGIN { printf "%.2f GB", b / 1e9 }'; }

caches=$(gh api --paginate "repos/$repo/actions/caches?per_page=100" \
  --jq '.actions_caches[]' | jq -sc 'map({id, ref, key, size_in_bytes, created_at})')

# Every open PR, in one call, so rule (b) needs no per-ref lookup.
open_prs=$(gh api --paginate "repos/$repo/pulls?state=open&per_page=100" \
  --jq '.[].number' | jq -sc .)

doomed=$(jq -c --argjson open "$open_prs" '
  ( [ .[]
      | select(.key | test("^v0-rust-.*-[0-9a-f]+$")) ]
    | group_by([.ref, (.key | sub("-[0-9a-f]+$"; ""))])
    | map(sort_by(.created_at) | .[:-1])
    | flatten
    | map(. + {why: "superseded"}) )
  +
  ( [ .[]
      | select(.ref | test("^refs/pull/[0-9]+/merge$"))
      | select((.ref | ltrimstr("refs/pull/") | rtrimstr("/merge") | tonumber) as $n
               | ($open | index($n)) == null) ]
    | map(. + {why: "closed PR"}) )
  | unique_by(.id)
  | sort_by(.ref, .key)
' <<<"$caches")

before_n=$(jq 'length' <<<"$caches")
before_b=$(jq '[.[].size_in_bytes] | add // 0' <<<"$caches")
doomed_n=$(jq 'length' <<<"$doomed")
doomed_b=$(jq '[.[].size_in_bytes] | add // 0' <<<"$doomed")

failed=0
if [[ $dry_run != true ]]; then
  while read -r id; do
    [[ -n $id ]] || continue
    # A cache GitHub evicted between the listing and now 404s; that is the outcome we
    # wanted anyway, so note it and keep going rather than failing the run.
    if ! gh api -X DELETE "repos/$repo/actions/caches/$id" --silent 2>/dev/null; then
      echo "could not delete cache $id (already gone?)" >&2
      failed=$((failed + 1))
    fi
  done < <(jq -r '.[].id' <<<"$doomed")
fi

{
  echo "### Actions cache prune"
  echo
  if [[ $dry_run == true ]]; then
    echo "**Dry run** — nothing was deleted."
    echo
  fi
  echo "| | entries | size |"
  echo "| --- | ---: | ---: |"
  echo "| before | $before_n | $(gb "$before_b") |"
  echo "| pruned | $doomed_n | $(gb "$doomed_b") |"
  echo "| after | $((before_n - doomed_n)) | $(gb "$((before_b - doomed_b))") |"
  echo
  if ((failed > 0)); then
    echo "$failed entr(ies) could not be deleted; see the step log."
    echo
  fi
  if ((doomed_n > 0)); then
    echo "<details><summary>Pruned entries</summary>"
    echo
    echo "| ref | key | size | reason |"
    echo "| --- | --- | ---: | --- |"
    jq -r '.[] | "| \(.ref) | `\(.key)` | \(.size_in_bytes) | \(.why) |"' <<<"$doomed"
    echo
    echo "</details>"
  else
    echo "Nothing to prune."
  fi
} >>"$summary"
