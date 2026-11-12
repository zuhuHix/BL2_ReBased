#!/usr/bin/env bash
# Land several reviewed and tested PRs in one push. See docs/TOOLING.md, "Landing PRs in a batch".
#
# usage: bash tools/land_batch.sh -w <landing-worktree> -t <tested-commit> "<pr>:<head-sha> <pr>:<head-sha> ..."
#        bash tools/land_batch.sh -w <landing-worktree> -D "<pr>:<head-sha> ..."   (documentation-only PRs, no tested tree)
#        DRY=1 bash tools/land_batch.sh ...                                          (checks everything, pushes nothing)
# <head-sha> is the full 40-character commit that was reviewed and tested.
#
# 1. Rebases the PRs, in the order given, into one straight chain on top of origin/main. Every PR must target main;
#    stacked PRs are not supported (land the parent first). git rerere reuses conflict fixes recorded earlier, for
#    example while building the integration branch. The rebases use merge.conflictStyle=diff3 themselves.
# 2. Checks that each rebased PR changes exactly the lines it changed when reviewed, and that the chain's final tree is
#    identical to the tested commit's tree (-t). With -D instead, every changed file must be a regular, non-executable
#    Markdown text file.
# 3. One atomic push moves the PR branches to their commits in the chain (all or none, leased on the reviewed heads);
#    once GitHub shows the new heads, main fast-forwards, leased on the main the chain was built on. GitHub finds each
#    PR's head inside main and marks the PR Merged; the script waits for that, then deletes the landed remote branches.
#    (In one combined push GitHub closes the PRs instead of merging them.)
# It stops at the first thing it cannot prove, and nothing is pushed before every check has passed.
set -u
wt=; tested=; docsonly=
while getopts "w:t:D" o; do case $o in w) wt=$OPTARG;; t) tested=$OPTARG;; D) docsonly=1;; *) exit 64;; esac; done
shift $((OPTIND - 1))
list=${1:-}
usage="usage: bash $0 -w <landing-worktree> (-t <tested-commit> | -D) \"<pr>:<40-char-head-sha> ...\""
[ -n "$wt" ] && [ -n "$list" ] || { echo "$usage" >&2; exit 64; }
[ -n "$tested" ] || [ -n "$docsonly" ] || { echo "-t <tested-commit> is required (or -D for Markdown-only PRs)" >&2; exit 64; }
[ -n "$tested" ] && [ -n "$docsonly" ] && { echo "use -t or -D, not both" >&2; exit 64; }
here=$(cd "$(dirname "$0")/.." && pwd)
log="$here/local/hub/land_batch.log"
mkdir -p "$(dirname "$log")"
started=
inrebase() { [ -d "$(git rev-parse --git-path rebase-merge)" ] || [ -d "$(git rev-parse --git-path rebase-apply)" ]; }
# Exit 4 (a conflict) leaves this run's rebase open so it can be resolved by hand; other stops undo only a rebase
# this run started.
stop() { echo "STOP: $1" | tee -a "$log"; if [ "${2:-9}" != 4 ] && [ -n "$started" ] && inrebase; then git rebase --abort >/dev/null 2>&1; fi; exit "${2:-9}"; }
cd "$wt" || { echo "no worktree $wt (make one with: git worktree add --detach $wt origin/main)" >&2; exit 1; }
inrebase && { echo "a rebase is already in progress in $wt; finish or abort it first" >&2; exit 1; }
# A change to a tracked file makes git refuse the checkouts and rebases below.
dirty=$(git status --porcelain --untracked-files=no) || exit 1
[ -z "$dirty" ] || { printf 'the landing worktree %s has changes to tracked files; discard them first:\n%s\n' "$wt" "$dirty" >&2; exit 1; }
repo=$(gh repo view --json nameWithOwner --jq .nameWithOwner) || exit 1
R=repos/$repo
export GIT_EDITOR=true
# Kept on for the whole repository so resolutions made while building integration branches are recorded too.
git config rerere.enabled true

# The PR's diff with its context lines and full binary content, minus what a rebase legitimately moves (blob ids and
# hunk line numbers): every changed byte and its surroundings must match, wherever the hunk now sits.
changes() { git diff --no-color --no-ext-diff --binary -U3 "$1" "$2" | grep -v -E '^index ' | sed -E 's/^@@ -[0-9,]+ \+[0-9,]+ @@/@@/'; }
# -D: every path on both sides is a regular, non-executable Markdown text file.
docs_only_problem() {
  local raw numstat
  raw=$(git diff --no-renames --raw "$1" "$2") || { echo "diff failed"; return; }
  echo "$raw" | awk '{ split($0, t, "\t"); m = substr($2, 1, 6); if (t[2] !~ /\.md$/) print "not Markdown: " t[2]; else if (m != "100644" && m != "000000") print "mode " m ": " t[2] }'
  numstat=$(git diff --no-renames --numstat "$1" "$2")
  echo "$numstat" | awk -F'\t' '$1 == "-" { print "binary: " $3 }'
}
# Open, still at the reviewed head, targeting main, and from a branch of this repository (not a fork).
pr_ok() {
  local n=$1 want=$2 info state head branch base_ref head_repo
  info=$(gh api "$R/pulls/$n" --jq '[.state,.head.sha,.head.ref,.base.ref,(.head.repo.full_name // "")]|@tsv') || { echo "#$n api"; return 1; }
  IFS=$'\t' read -r state head branch base_ref head_repo <<<"$info"
  [ "$state" = open ] || { echo "#$n state=$state"; return 1; }
  [ "$head" = "$want" ] || { echo "#$n head $head is not the reviewed $want"; return 1; }
  [ "$base_ref" = main ] || { echo "#$n targets $base_ref, not main; land its parent first and retarget it"; return 1; }
  [ "$head_repo" = "$repo" ] || { echo "#$n comes from ${head_repo:-a deleted fork}, not $repo; land fork PRs by hand"; return 1; }
  echo "$branch"
}

git fetch -q origin main || stop "fetch main"
main=$(git rev-parse origin/main)
chain=$main
plan=()
echo "$(date -u +%FT%TZ) land $list on ${main:0:7}" >> "$log"
for item in $list; do
  IFS=: read -r n want <<<"$item"
  [[ "$want" =~ ^[0-9a-f]{40}$ ]] || stop "#$n: give the full 40-character reviewed head, not '$want'" 3
  branch=$(pr_ok "$n" "$want") || stop "$branch" 3
  old=$want
  git fetch -q origin "$branch" || stop "#$n fetch $branch"
  [ "$(git rev-parse "origin/$branch")" = "$old" ] || stop "#$n branch moved while landing" 3
  base=$(git merge-base "$old" "$main")
  [ -n "$(git diff --name-only "$base" "$old")" ] || stop "#$n changes nothing" 5
  if [ -n "$docsonly" ]; then
    other=$(docs_only_problem "$base" "$old")
    [ -z "$other" ] || stop "#$n is not Markdown-only ($other); land it with -t <tested-commit>" 5
  fi
  git checkout -q --detach "$old" || stop "#$n: cannot check out ${old:0:7} in $wt" 10
  started=1
  # diff3 here and wherever fixes are recorded: rerere replays a fix only for a conflict that looks the same, and the
  # style can change that (docs/TOOLING.md).
  rout=$(git -c merge.conflictStyle=diff3 -c rerere.autoupdate=true rebase -q --onto "$chain" "$base" 2>&1); rc=$?
  # A rebase stops once per conflicting commit; carry on while rerere resolved every file. A
  # --continue that fails the same way three times in a row (a refusing hook, a lock) stops with git's message.
  last=; same=0
  for _ in $(seq 1 200); do
    if ! inrebase; then
      # A rebase that fails without stopping at a conflict (it refused to start, for example) rebased nothing.
      [ "$rc" = 0 ] || stop "#$n: the rebase failed and left no conflict open; nothing pushed. git said: $rout" 10
      break
    fi
    left=$(git diff --name-only --diff-filter=U)
    [ -z "$left" ] || stop "#$n conflict in: $left. The rebase is left open in $wt: resolve the files, git add them, run git rerere (it records the fix), then git rebase --abort and rerun this command." 4
    if [ "$rc" != 0 ]; then
      if [ "$rout" = "$last" ]; then same=$((same + 1)); else same=1; last=$rout; fi
      [ "$same" -lt 3 ] || stop "#$n: the rebase failed the same way 3 times in a row; nothing pushed. git said: $rout" 10
    fi
    rout=$(git -c merge.conflictStyle=diff3 -c rerere.autoupdate=true rebase --continue 2>&1); rc=$?
  done
  inrebase && stop "#$n rebase did not finish; nothing pushed. git said: $rout" 10
  started=
  new=$(git rev-parse HEAD)
  git merge-base --is-ancestor "$chain" "$new" || stop "#$n: the rebased ${new:0:7} does not contain the chain ${chain:0:7}; nothing pushed" 10
  [ "$(changes "$base" "$old")" = "$(changes "$chain" "$new")" ] || stop "#$n: the rebased change differs from the reviewed one; check it by hand" 5
  echo "  #$n ${old:0:7} -> ${new:0:7} on ${chain:0:7}" | tee -a "$log"
  plan+=("$n:$old:$new:$branch")
  chain=$new
done
if [ -n "$tested" ]; then
  [ "$(git rev-parse "$chain^{tree}")" = "$(git rev-parse "$tested^{tree}")" ] || stop "the chain's tree differs from the tested $tested" 6
  echo "  chain tree == tested ${tested:0:9}" | tee -a "$log"
fi
# main may only fast-forward from exactly the main the chain was built on: this check that the chain contains that main,
# and the lease on it at the push below, allow nothing else together.
git merge-base --is-ancestor "$main" "$chain" || stop "the chain ${chain:0:9} does not contain main ${main:0:7}; nothing pushed" 10
if [ -n "${DRY:-}" ]; then echo "DRY: every check passed, nothing pushed; chain tip ${chain:0:9}" | tee -a "$log"; exit 0; fi

refs=(); leases=()
for p in "${plan[@]}"; do
  IFS=: read -r n old new branch <<<"$p"
  # Checked again right before the push: a PR closed, retargeted or moved during the rebase must not land.
  again=$(pr_ok "$n" "$old") || stop "$again (changed while the batch was built; nothing pushed)" 3
  [ "$again" = "$branch" ] || stop "#$n branch changed to $again; nothing pushed" 3
  [ "$new" = "$old" ] || { refs+=("$new:refs/heads/$branch"); leases+=(--force-with-lease="$branch:$old"); }
done
# Two steps, because GitHub closes (rather than merges) a PR whose branch moves in the same push as the base: first
# the PR branches move to their rebased commits (all or none) and GitHub updates each PR; then main fast-forwards.
if [ ${#refs[@]} -gt 0 ]; then
  git push -q --atomic "${leases[@]}" origin "${refs[@]}" || stop "branch push refused (a branch moved); nothing changed" 8
  for p in "${plan[@]}"; do
    IFS=: read -r n old new branch <<<"$p"
    seen=
    for _ in $(seq 1 24); do
      seen=$(gh api "$R/pulls/$n" --jq .head.sha 2>/dev/null) && [ "$seen" = "$new" ] && break
      sleep 5
    done
    [ "$seen" = "$new" ] || stop "#$n: GitHub still shows head $seen after the branch push; main NOT pushed. The PR branches now hold the rebased commits; rerun with the new heads after checking them." 12
  done
fi
# The lease refuses a main that moved at all since the chain was built, forward or back (moved back, a plain push would
# be a fast-forward and bring back what was taken off); the ancestor check above refuses a chain that does not extend it.
perr=$(git push -q --force-with-lease="main:$main" origin "$chain:refs/heads/main" 2>&1) || stop "main push refused, main unchanged; git said: $perr. The PR branches hold the rebased commits; rerun with the new heads, and for a -t landing build and test a new tested commit first if main has moved." 8
echo "  pushed: main ${main:0:7} -> ${chain:0:7}" | tee -a "$log"
fail=
for p in "${plan[@]}"; do
  IFS=: read -r n old new branch <<<"$p"
  merged=
  for _ in $(seq 1 24); do
    merged=$(gh api "$R/pulls/$n" --jq .merged 2>/dev/null) && [ "$merged" = true ] && break
    sleep 5
  done
  if [ "$merged" != true ]; then
    echo "NOT MERGED #$n after 2 minutes (merged=$merged): its commits are on main as ${new:0:7}; branch kept" | tee -a "$log"
    fail=1; continue
  fi
  # GitHub retargets open PRs based on this branch when it is deleted. Only the remote branch goes; a local
  # worktree on it keeps its files.
  # Leased on the landed commit: a branch someone moved after the push is kept.
  if git push -q --force-with-lease="$branch:$new" origin --delete "$branch" >/dev/null 2>&1; then d=deleted; else d="NOT deleted (moved since?)"; fi
  echo "OK #$n landed ${new:0:7}, remote branch $d" | tee -a "$log"
done
echo "main is now ${chain:0:7}; read its CI run before landing the next batch" | tee -a "$log"
[ -z "$fail" ] || exit 11
