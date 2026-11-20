## 2026-11-19: IsStaggered on link 9: an own-words note and a proposed Off answer

AI-assisted (Claude Code)

Lane ANALYST-ISSTAGGERED, analyst only, with Claude Opus 5.5. The file number is a placeholder until the maintainer
gives one. Docs only: no code, no dependency, and no bounds check changed.

- **What:** `docs/verification/NATIVE_IS_STAGGERED.md` and one NATIVE_INDEX row, from the script and a local,
  read-only Ghidra reading under the machine lock. Raw output stays under ignored `local/analysis/stg/`.
- **Findings** (UNVERIFIED in game):
  - both declarations share one exec step that answers a pawn virtual;
  - `WillowAIPawn`'s answers `StaggerState.bIsStaggered` (a layout fit), and `WillowPawn`'s answers false;
  - `WillowPlayerPawn`'s was not read;
  - nothing is written, loaded or drawn.
- **On link 9:** `PhaseLockTarget` unstaggers a staggered target before lifting it. The fresh target answers false.
- **Proposed Off answer:** bind both keys beside `SetDefaultPhysics` in `ow-combat`, keep-existing, with Reference
  unchanged. The preflight has S1 (an unloaded receiver) and S2 (a class whose virtual was not read). Tests are
  listed.
- **Probe** (local, never pushed; stand-ins named in the note): installed Off `--slice-gate`, strict, ok 2, notyet 9,
  error 3. Link 9 passes 1 of 2 steps and stops at the unbound `WillowAIPawn.DropCombatPerch()`.
- **Clean room:** a different lane implements this note.
