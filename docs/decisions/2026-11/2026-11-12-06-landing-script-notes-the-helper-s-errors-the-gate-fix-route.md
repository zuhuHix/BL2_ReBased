## 2026-11-12: Landing script notes: the helper's errors, the gate-fix route, stdin in the tests

AI-assisted (Claude Code). Small notes from the second review of the landing-script and decisions PRs.

- `tools/land_batch_checks.py` turns any exception while judging GitHub's answer (for example `"check_runs": null`, or
  a list item that is not an object) into an `error` verdict with its message. `tools/land_batch.sh` still polls on
  until its deadline, because a bad answer can clear on the next poll, and its exit 14 then names that reason. Before,
  the exception left the script with an empty verdict, and exit 14 lost the reason (the traceback went to the terminal
  only).
- `docs/TOOLING.md` gives the real route for a leak-gate fix that `main`'s own gate refuses: its PR shows a red
  `Leak gate`, so the script stops with exit 13 and has no override. On the maintainer's explicit yes, the hub pushes
  the reviewed and tested commit by hand as a fast-forward of `main`, with any ruleset set to Disabled for that push.
- Two readings of GitHub's documentation are marked `UNVERIFIED` in the helper and in TOOLING: seven days as the
  limit for a required check, and that a ruleset counts a `pull_request` run and not one started by hand. If either
  is wrong, the script is the stricter side. TOOLING also says that only the chain tip's `Leak gate` run is waited for.
- `tests/decisions_index_test.py` gives its subprocesses `stdin=subprocess.DEVNULL`, as the landing test does.

Verified: the synthetic tests named above; the new error cases fail without the catch-all. Not tried against GitHub.
