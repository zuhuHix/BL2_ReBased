## 2026-11-19: DropCombatPerch on link 9: an own-words note and a proposed Off answer

AI-assisted (Claude Code)

Lane ANALYST-DROPCOMBATPERCH, analyst only, with Claude Opus 5.5. The file number is a placeholder until the maintainer
gives one. Docs only: no code, no dependency, and no bounds check changed.

- **What:** `docs/verification/NATIVE_DROP_COMBAT_PERCH.md` and one NATIVE_INDEX row, from the script and a local,
  read-only Ghidra reading under the machine lock. Raw output stays under ignored `local/analysis/dcp/`.
- **Findings** (UNVERIFIED in game): the native acts only when the pawn has authority, a `PerchProjectile` and
  `bPerchAttached`. It then spawns and launches the perch projectile, which this note reads only to its edge.
  Otherwise it does nothing.
- **On link 9:** `BeginLifting` calls it on the fresh target, the guard fails, and nothing happens.
- **Proposed Off answer:** bind it beside `IsStaggered` in `ow-combat`, keep-existing, with Reference unchanged. The
  preflight has D1 (unresident receiver), D2 (a native descendant) and D3 (the drop itself, not read), and is followed
  by the guard. Tests are listed.
- **Probe** (local, never pushed; stand-ins named in the note): installed Off `--slice-gate`, strict, ok 2, notyet 9,
  error 3. Link 9 passes 1 of 2 steps and stops at the unbound `Actor.Trace(...)`.
- **Clean room:** a different lane implements this note.
