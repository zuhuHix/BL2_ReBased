## 2026-11-09: The capture finalizers re-check each backup before they restore (projectiles, weapon parity)

AI-assisted (Claude Haiku 5.5). Review found that `projectiles_capture.ps1` and `weapon_parity2_capture.ps1` restored each verified root with
`Restore-Tree` straight from the backup folder, without first checking that the folder still matched what the run verified. A backup changed
during the run would have been copied over the live saves, and the pending record was only checked afterwards.

- **Change:** each script keeps the file hashes it took as each root verified (`$backupHashes`), and writes the same table to
  `verified_hashes.json`. Its finalizer restores through `Restore-VerifiedRoots`, the shared path the other captures already use, which compares the
  backup with those hashes first. A changed backup is not restored (row `BACKUP CHANGED after verification`), the saves line reads NOT IDENTICAL,
  and the pending record and both locks stay. The manual recovery for an unconfirmed game exit now names `Restore-PendingRecord`, not a bare
  `Restore-Tree`.
- **Why:** fail closed. A backup that changed since it was verified proves nothing, and the saves are the maintainer's.
- **Not changed:** the restore of an unchanged backup, the driver and game checks, and the parsing code. No game run.
- **Verified (synthetic, no game):** `tests/real_game_finalizer_recheck_test.py` runs each script's own backup section and finally block with the
  game stubbed. A changed backup keeps the live saves, the record and the locks, and the intact roots are still restored; an unchanged backup is
  restored and its record removed. Against the previous scripts the changed-backup case fails (the live save is overwritten).
- **Not verified:** a real-game run of either capture after this change.
- **Review r1 on PR #200 (AI-assisted, Claude Opus 5.5):**
  - *Missing backup root (F3):* `Get-TreeHashes` reads a missing folder as empty, so a verified root whose backup folder vanished compared clean
    and reached `Restore-Tree`, which deleted every live file as "added" before failing. `Restore-Tree` now throws before touching the live
    folder when the backup folder is missing; `Restore-VerifiedRoots` reports `BACKUP CHANGED / MISSING after verification` and restores nothing
    for that root; `Get-PendingProblems` counts a missing backup root as changed, so the record and locks stay.
  - *One hash for compare and baseline (F1, part):* both scripts hash the staged copy once and use that table for the comparison with the live
    tree and as `$backupHashes` / the manifest snapshot, so a change between the two steps can no longer become the baseline. Not done: a
    staged-copy restore. The check-to-copy window at restore time stays, named in `docs/TOOLING.md` as an accepted risk (it needs another process
    writing our ignored backup folder during a run).
  - *Manifest checked against the record (F2):* `Restore-PendingRecord` passes its record to `Restore-FromVerifiedRecord`. A root is refused
    (row `REFUSED`, not restored, record kept, reason printed) when the record does not name it, its live folder differs from the record's, or
    its manifest snapshot does not give the record's tree hash for it (`Get-HashesDigest`, split out of `Get-TreeDigest`, same scheme). A root
    the record names and the manifest does not is refused too. Older records whose roots are a subset keep working, since both sides are checked
    by name.
  - *Finalizer test (F4):* the run lock is held in the test, its release is stubbed and counted: one release after a clean restore, none when
    the record is kept.
  - **Verified (synthetic, no game):** new cases in `tests/real_game_finalizer_recheck_test.py` (missing backup root, including a root verified
    empty; `Restore-Tree` on a missing backup; run-lock release) and `tests/real_game_pending_test.py` (backup and manifest changed to agree,
    manifest naming another live folder, missing empty backup root in the completion check). **Not verified:** any real-game run; no bar is lifted.
