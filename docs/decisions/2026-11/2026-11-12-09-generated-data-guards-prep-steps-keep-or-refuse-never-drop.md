## 2026-11-12: Generated-data guards: prep steps keep or refuse, never drop silently

AI-assisted (Claude Code). Tools and their tests only; no host change and no parsing change. The follow-up to
`2026-11-12-08-range-den-the-regular-s-and-the-slinger-s-cycle-and-no.md`, where `local/phaselock/target/host_target.json` had lost
its attachment placements and eight in-engine suites ran green on enemies without their worn parts. Records:
`docs/verification/PHASELOCK_STOCK_DATA.md` (the attach pipeline and the attachment check), `docs/TOOLING.md` (the ambient
citizens' steps), `docs/verification/SANCTUARY_RPG_MISSION.md` (the Fire checker).

- **`tools/prepare_phaselock_target.py editor-job` keeps the attach step's placements.** `-Steps prepare` runs identity, extract and
  editor-job; editor-job wrote `host_target.json` from scratch, so a prepare run after an attach run dropped the `attachments` lists the
  later attachments step had written, the likely cause of the incident. Chosen: keep what the step does not own rather than refuse,
  because the placements depend only on the identity and the attach editor report, and editor-job does not change either. They are
  kept only when their number equals the identity's worn pieces; otherwise they are left out and the step prints
  `powershell -File tools/import_phaselock_target.ps1 -Steps attach` to rerun (and `tools/prepare_range.py` refuses the result, as before).
- **`tools/phaselock_attachment_check.py` refuses a missing `attachments` field** (exit 2, naming the attach step). A missing field means
  the placements were never written or were dropped; an empty list written by the attach step stays a valid answer.
- **`tools/prepare_ambient_world.py --compose` stops on a worn piece missing from the attach report.** Line 313 itself was not a hole: a
  donor spawn always went through `annotate_looks`, which sets `attachments`. The hole was next to it: `annotate_looks` dropped every
  captured piece whose mesh was not in `local/slice/ambient/editor_report_attach.json`. A piece on a bone the host uses whose mesh is
  neither in the report nor in the attach job's own `missing_meshes` now stops the tool without writing, naming
  `tools/seed_ambient_npc_assets.ps1 -Steps attach`. Meshes the job recorded as unavailable are still left out, as before.
- **`tools/verify_fire_event_order.py` has one whole-run rule: no `level` event in the default run.** The engine raises a level in the
  experience pool's per-frame update, so a level-up lands in a tick step after the turn-in, where no rule looked. The invented total
  (21,000, plus the turn-in's experience) stays inside level 8, so a level event means the total or the reward changed.

Checked: synthetic tests for each (each fails on the tool before its change), the Fire checker's `--self-test`, and the checker on the
real packages, 33 of 33 rules (32 before). The real editor-job could not run here: this worktree has no UModel exports, so it stopped
before writing, and the local `host_target.json` stayed byte-identical. The ambient step was not run on real data (no `amb_compose`
capture in this worktree). Nothing ran in the engine.

Left open: `OpenWillowCombatTarget.cpp` still gives the Phaselock test pawn a bare body, with no message, when its `attachments` field is
missing (a host change for another time). The other holes of the same kind found in the prep and check tools are listed in the PR text,
not fixed.
