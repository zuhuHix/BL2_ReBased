## 2026-11-12: The landing script runs main's decisions check and waits for the required checks

AI-assisted (Claude Code). Follows the move to one file per decision (the entry before this one); like it, the hub's
recommendation, which the maintainer approved on 2026-11-12.

- `tools/land_batch.sh` runs `main`'s copy of `tools/decisions_index.py --check --at <commit>` on every rebased commit
  against the chain commit below it. `main`'s copy, so a PR cannot weaken the check that judges it; `--at`, so the
  check reads the committed blobs and an untracked file in the landing worktree cannot authorise an edit. A refusal
  stops with exit 5 before anything is pushed.
- After the branch push it waits for each check in `REQUIRED_CHECKS` (only `Leak gate`) on the chain's tip, under one
  deadline, `CHECK_WAIT` seconds (default 900). The new `tools/land_batch_checks.py` decides from GitHub's raw check
  runs and workflow runs: it counts every run of that name from GitHub Actions on that exact commit that did not
  complete more than seven days ago, and passes only when every counted run succeeded and at least one came from the
  `pull_request` event. A run started by hand cannot stand in for the PR's run. Any other conclusion stops with exit
  13; no passing answer in time with exit 14, which names `gh`'s last error when it failed. In both, `main` stays and
  the PR branches keep their rebased commits. An empty `REQUIRED_CHECKS` is refused. `DRY=1` waits for nothing.
- A rerun with the rebased heads of a stopped batch, which the script's messages ask for after exits 8, 12, 13 and 14,
  failed for batches of two or more PRs: the second PR's diff was taken from where its head meets `main`, which then
  includes the first PR's change. Now a head that sits on the previous listed head takes that head as the start of
  its own change, which also holds after `main` has moved.
- `docs/TOOLING.md` says what the maintainer clicks for a ruleset on `main` that requires `Leak gate`, and how to switch
  it off (Enforcement: Disabled) in an emergency, while Actions is down, or for a gate bootstrap. Its paragraph on the
  window without a decisions check goes, since this change closes it.

Verified: `tests/land_batch_test.py` (throwaway repositories, a local bare origin, a stub for `gh` that returns raw
JSON, no network) covers a success, a pending run, a failure, a time-out, `gh` failing, DRY mode, the rerun with new
heads after `main` moved, an empty `REQUIRED_CHECKS`, the decisions check with an untracked file in the way, and the
run selection on synthetic answers (failure, cancelled, skipped, in progress, queued without a start time, none, a
failed PR run with a later manual success, a manual run alone, another app, another commit, runs older than seven days,
a failed push run, unreadable answers). The two `gh` calls and the helper ran, read-only, on GitHub's answers for two
real commits: `main`'s 24cb691 (two runs, one from `pull_request`, one from `push`) and the head of PR #231 (one run,
from `pull_request`); both came out as success.
`UNVERIFIED`: a push to `main` with a ruleset switched on was never tried, and no batch has gone through the wait
against GitHub yet.
