#!/usr/bin/env bash
# Land several reviewed and tested PRs in one push. See docs/TOOLING.md, "Landing PRs in a batch".
#
# usage: tools/land_batch.sh -w <landing-worktree> [-t <tested-commit>] "<pr>:<head-sha7> <pr>:<head-sha7> ..."
#        DRY=1 tools/land_batch.sh ...   checks everything, pushes nothing
#
# 1. Rebases the PRs, in the order given, into one straight chain on top of origin/main. git rerere is on, so a
#    conflict already resolved once (for example in an integration branch) is resolved the same way again, and
#    DECISIONS.md merges as a union (it is an append-only log).
# 2. Checks that each rebased PR changes exactly what it changed when it was reviewed (DECISIONS.md excluded) and,
#    with -t, that the chain's final tree is identical to the tree the in-engine suites ran on.
# 3. Moves each PR branch to its commit in the chain (only if the branch still points at the reviewed head), then
#    fast-forwards main once. GitHub finds each PR's head inside main and marks the PR Merged, and CI runs once on
#    the new main instead of once per PR.
# It stops at the first thing it cannot prove, and nothing is pushed before every check has passed.
set -u
wt=; tested=
while getopts "w:t:" o; do case $o in w) wt=$OPTARG;; t) tested=$OPTARG;; *) exit 64;; esac; done
shift $((OPTIND - 1))
list=${1:-}
[ -n "$wt" ] && [ -n "$list" ] || { echo "usage: $0 -w <landing-worktree> [-t <tested-commit>] \"<pr>:<sha7> ...\"" >&2; exit 64; }
here=$(cd "$(dirname "$0")/.." && pwd)
attr="$here/tools/merge-union.attributes"
log="$here/local/hub/land_batch.log"
mkdir -p "$(dirname "$log")"
# Exit 4 (a conflict) leaves the rebase open so it can be resolved by hand; every other stop undoes it.
stop() { echo "STOP: $1" | tee -a "$log"; [ "${2:-9}" = 4 ] || git rebase --abort >/dev/null 2>&1; exit "${2:-9}"; }
cd "$wt" || { echo "no worktree $wt (make one with: git worktree add --detach $wt origin/main)" >&2; exit 1; }
repo=$(gh repo view --json nameWithOwner --jq .nameWithOwner) || exit 1
R=repos/$repo
export GIT_EDITOR=true
# Kept on for the whole repository so resolutions made while building integration branches are recorded too.
git config rerere.enabled true

inrebase() { [ -d "$(git rev-parse --git-path rebase-merge)" ] || [ -d "$(git rev-parse --git-path rebase-apply)" ]; }

git fetch -q origin main || stop "fetch main"
main=$(git rev-parse origin/main)
chain=$main
plan=()
echo "$(date -u +%FT%TZ) land $list on ${main:0:7}" >> "$log"
for item in $list; do
  IFS=: read -r n want <<<"$item"
  info=$(gh api "$R/pulls/$n" --jq '[.state,.head.sha,.head.ref]|@tsv') || stop "#$n api"
  state=$(cut -f1 <<<"$info"); old=$(cut -f2 <<<"$info"); branch=$(cut -f3 <<<"$info")
  [ "$state" = open ] || stop "#$n state=$state" 2
  case "$old" in "$want"*) ;; *) stop "#$n head $old is not the listed $want" 3;; esac
  git fetch -q origin "$branch" || stop "#$n fetch $branch"
  base=$(git merge-base "$old" "$main")
  git checkout -q --detach "$old"
  git -c core.attributesFile="$attr" -c rerere.autoupdate=true rebase -q --onto "$chain" "$base" >/dev/null 2>&1
  # A rebase stops once per conflicting commit; carry on while rerere or the union driver resolved every file.
  for _ in $(seq 1 200); do
    inrebase || break
    left=$(git diff --name-only --diff-filter=U)
    [ -z "$left" ] || stop "#$n conflict in: $left. The rebase is left open in $wt: resolve the files, git add them, run git rerere (it records the fix), then git rebase --abort and rerun this command." 4
    git -c core.attributesFile="$attr" -c rerere.autoupdate=true rebase --continue >/dev/null 2>&1
  done
  inrebase && stop "#$n rebase did not finish" 10
  new=$(git rev-parse HEAD)
  a=$(git diff "$base" "$old" -- . ':!DECISIONS.md' | git patch-id --stable | cut -d' ' -f1)
  b=$(git diff "$chain" "$new" -- . ':!DECISIONS.md' | git patch-id --stable | cut -d' ' -f1)
  [ "$a" = "$b" ] || stop "#$n: the rebased change differs from the reviewed one; check it by hand" 5
  echo "  #$n ${old:0:7} -> ${new:0:7} on ${chain:0:7}" | tee -a "$log"
  plan+=("$n:$old:$new:$branch")
  chain=$new
done
if [ -n "$tested" ]; then
  [ "$(git rev-parse "$chain^{tree}")" = "$(git rev-parse "$tested^{tree}")" ] || stop "the chain's tree differs from the tested $tested" 6
  echo "  chain tree == tested ${tested:0:9}" | tee -a "$log"
fi
if [ -n "${DRY:-}" ]; then echo "DRY: every check passed, nothing pushed; chain tip ${chain:0:9}" | tee -a "$log"; exit 0; fi

for p in "${plan[@]}"; do
  IFS=: read -r n old new branch <<<"$p"
  git push -q --force-with-lease="$branch:$old" origin "$new:refs/heads/$branch" || stop "#$n branch push refused" 7
done
git push -q origin "$chain:refs/heads/main" || stop "main fast-forward refused (did main move?)" 8
sleep 15
for p in "${plan[@]}"; do
  IFS=: read -r n old new branch <<<"$p"
  merged=$(gh api "$R/pulls/$n" --jq .merged)
  for c in $(gh api "$R/pulls?state=open&base=$branch&per_page=100" --jq '.[].number'); do
    gh api -X PATCH "$R/pulls/$c" -f base=main >/dev/null && echo "  retarget #$c to main" | tee -a "$log"
  done
  # Only the remote branch is deleted: a local worktree on that branch keeps its files.
  if [ "$merged" = true ]; then git push -q origin --delete "$branch" >/dev/null 2>&1; d=deleted; else d=KEPT; fi
  echo "OK #$n landed ${new:0:7} merged=$merged remote branch $d" | tee -a "$log"
done
echo "main is now ${chain:0:7}; read its CI run before landing the next batch" | tee -a "$log"
