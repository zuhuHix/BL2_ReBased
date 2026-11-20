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

## Edits: review r1 of #611

- **S2's class set** (hub ruling): the AI rule is for exactly `WillowAIPawn` and its script-only children. A native
  descendant (today `WillowAICranePawn`) and every other `WillowPawn` child refuse S2.
- **Step 1** requires a `WillowPawn` receiver, as #604 does.
- **S1** has two texts, and the read goes by name with no offset. The 4-byte fit is cited
  (NATIVE_PLAYER_MOVEMENT.md) as evidence about the body only.
- **Small fixes:**
  - the struct type is named;
  - the shape test is split in three, and tests are added for a native child and an outside receiver;
  - `UnStagger`'s authority test is stated;
  - the "only writer" claim is narrowed to member writes.

## Edits: after review of #620

- `WillowGame.DeployableTurretActor` joins `WillowAICranePawn` in S2's recorded list of native descendants (the
  super chain's two children of `WillowAIPawn`), and its slot is added to "Not read". The list is taken from the
  super chain rather than `native_by_class.json`, which holds function counts only.
