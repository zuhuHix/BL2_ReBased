## 2026-11-09: Five barred real-game scripts are unbarred after the Z41 smoke run; two stay barred (real-game harness)

AI-assisted (Claude Haiku 5.5). Docs: `docs/TOOLING.md`, "Bar lifted (2026-11-09)" and "Not to be run".

- **Change:** the bar on `action_skill_hud_capture`, `hud_integrity_capture`, `hud_layout_capture`, `hud_parity_capture` and `weapon_part_session` is lifted. `projectiles_capture` and `weapon_parity2_capture` stay barred. The bar was documentation only: no script refused to start, so no code changed. The docs and one synthetic test say so.
- **Why the two stay barred:** their finalizer calls `Restore-Tree` on a root whose backup was verified at backup time, without re-checking that backup's recorded hashes right before the restore. A backup that changed during the run could be copied over the live saves, and `Complete-PendingRecord` notices only afterwards. A follow-up fix comes first (review r1 on PR #199).
- **Why the five are lifted:** run Z41 on zuhu (`hud_layout_capture.ps1 -Resolutions 1280x720` on main with PR #182, commit 6da8d34) exited 0, restored saves identically after two restores, restored sdk_settings and removed its pending record once the tree hash matched. The same day `phaselock_integrity_capture.ps1 -SkipDown` also restored cleanly. Z41 exercised the shared `Complete-BackupRun` cleanup, which the five use.
- **Not verified:** each of the five stays `UNVERIFIED` for its own capture until it has run once against the game.
