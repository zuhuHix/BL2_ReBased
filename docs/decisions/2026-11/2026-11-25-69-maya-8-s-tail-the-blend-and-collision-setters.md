## 2026-11-25: maya:8's tail, the blend and collision setters

AI-assisted (Claude Code)

Lane ANALYST-LINK1-TAIL, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds check
changed.

- **Why:** a look-ahead for link 1 past `SetActiveProfileByName` (#730), to the end of `ApplyBodyClassStartingValues`.
- **What:** the new note `docs/verification/NATIVE_LINK1_TAIL_SETTERS.md` (the walk and eight natives), plus two
  NATIVE_INDEX rows and one GATES row.
- **Evidence:**
  - `research/script_disasm.py`;
  - `ow-package --properties` of Maya's tree nodes;
  - a local read-only Ghidra reading under the machine lock (the execs, and the slot bodies in four class tables);
  - the layout tool.

  Raw output stays under ignored `local/analysis/tail/`.
- **Findings** (UNVERIFIED in game):
  - `FindAllAnimNodes` appends every node of the name, in the name lookup's list order;
  - `SetBlendTarget` clamps the target and writes the weights at once when the time is 0. The feather blend then
    forces child 0's weight to 1;
  - `SetRBChannel`, `SetBlockRigidBody` and `SetNotifyRigidBodyCollision` write their field, and touch physics only
    on existing bodies;
  - `SetActorCollision` writes three bits with the deferred reattach on a change. Its owner step is not read;
  - `SetTraceBlocking` writes two bits;
  - `SetActiveChild`'s body is not read.
- **Proposed Off answer:**
  - per native, with preflights;
  - physics bodies present is `LayerAbsent`;
  - unread parts are `NotRead`;
  - tests listed.
- **Next** (a prediction, not a run): the pawn's `InitializeRBPhysics`.
- **Review r1 (#734):**
  - `SetRBChannel`'s Off rule replaces the both-modes no-op in `death.rs` in Off, as `DROP_COMBAT_PERCH` does;
  - `FindAllAnimNodes` reads the live component's `AnimTickArray`, else the `NO_TICK_LIST` `NotRead`, a None
    `SkelComponent` included;
  - `SetBlendTarget` checks the render fields only on the path that reads them, and its dispatch loads no class code;
  - "bodies present" is named per class;
  - `SetActorCollision` names both reattach refusals and the helper to reuse, and its owner step runs only on a
    change;
  - tests added for each.
- **Clean room:** a different lane implements this note.
