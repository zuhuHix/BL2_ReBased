## 2026-11-11: Leak gate: a machine check on what may be published (pre-push hook and CI job)

AI-assisted (Claude Opus 5.5). Tool: `tools/leak_gate.py`; rules and use in `docs/TOOLING.md`, "Leak gate".

- **Why:** the safety lines (no game-derived data, no decompiler output, no personal identifiers) were prose checked by people
  and one edit hook inside one AI tool. Nothing looked at a commit, a push or a PR made any other way.
- **Where it stands (maintainer, 2026-11-11):** the publishing ban stays in every brief; every PR keeps an independent review;
  only the coordinator publishes, through the gate. The hook can be bypassed and is a backstop; the CI job runs after publication
  and is detection. When the gate cannot establish something it refuses; it never guesses and never downgrades.
- **Change:** a plain-Python gate with 24 rules (17 errors; 7 warnings: `unknown-type`, `address-hex`, `local-copy`, `dump-like`,
  `gate-change`, `stale-baseline`, `inspection-incomplete`), a maintainer-owned allow-list (`tools/leak_gate_allow.txt`: the README
  banner pinned by sha256, the project's UE5 `.ini` folder, the exact allowed e-mail addresses), a baseline of what `main` held on
  2026-11-11 (`tools/leak_gate_baseline.txt`, rule, path and line hash only), `.githooks/pre-push` (installed like `post-checkout`),
  a separate `Leak gate` CI job, and `/Bl2Screens/` and `/PromptUpgrade.md` in `.gitignore`. Every commit the pushed head reaches
  and neither the destination's old value nor `refs/remotes/origin/main` reaches is judged against its first parent, and every
  new blob is scanned; added lines come from comparing blobs, so git attributes cannot hide them. Every commit and tag header is
  read, unknown ones refused. The hook refreshes `origin/main` before every run and judges with its gate and lists, whatever the
  push target; it refuses when that fails. There is no fallback to the working tree or the branch: the one way out is a bootstrap
  run by the hub, naming the exact commit whose gate and lists to use (`OPENWILLOW_LEAK_GATE_BOOTSTRAP=<commit id>`, printed),
  for landing a gate that `main` does not hold yet; CI bootstraps by itself only while the trusted commit holds no gate. A change
  to the gate, its lists, a hook, a workflow or git attributes gets a `gate-change` warning and needs the maintainer's yes.
  Private tokens and their baseline live in an ignored `local/` list. No existing content was changed.
- **Baseline on `origin/main` 8a73c76 (after PR #226 replaced personal paths and names with placeholders and PR #225 removed the
  review corpus):** tracked, 8 entries: `user-path` 4 lines in `THIRD_PARTY.md` (the maintainer has been asked about them),
  `address-hex` 2, `unknown-type` 1 (a stray UTF-16 log at the root), and `dump` 1: `research/native_by_class.json`, whose 5433
  one-number lines add up past the per-file numeric sum, so any edit to its numbers is refused until the baseline is regenerated.
  In the ignored local baseline, `private-name` 4 lines. Whether `native_by_class.json` stays tracked is the maintainer's decision.
  (On c73e14e, before those PRs, the tracked baseline held 31 entries, 27 of them `user-path` lines in 18 files.)
- **The publish path:** `--strict` exits 3 when there is no error but any warning other than `stale-baseline`; the hook and CI run
  without it.
- **Closing fixes:** a baselined per-file sum is bound to a hash of the numeric lines (any changed value, or the file removed and
  added again, is judged anew); names get every content rule and a 160-character limit; signature bodies get the identifier and
  credential rules; a shallow or grafted history whose outgoing commits lack a recorded parent is refused; the hook runs Python
  with `-I -S` (the gate needs only the standard library).
- **Checked:** 79 synthetic tests in five files (`tests/leak_gate_test.py`, `_repo`, `_policy`, `_hook`, `_push`). On the zuhu
  desktop a whole-tree run takes about 3 s, one commit about 2 s, 30 commits about 6 s and 300 commits about 49 s (in a clone whose
  trusted ref sat 300 commits back); a branch that merged `main` judges only its own commits (the reviewers' 300-commit probe
  is now 1 commit, 3.4 s). No parsing change.
- **Not verified:** the CI job's runs on GitHub beyond this PR, and the settings that make it binding, which are the maintainer's: a
  ruleset on `main` requiring the `Leak gate` check, and required review for `.github/workflows/`, `.githooks/` and the gate's files
  (the workflow is part of the PR it checks). The dump sizes are fitted to today's tree, not proven against real dumps. The gate is
  partial detection of marks, types and identifiers; it does not establish provenance or licence.

