## 2026-11-25: The owner's replicated special-move record: SetReplicatedSMData, an own-words note for link 9

AI-assisted (Claude Code)

Lane ANALYST-SETREPLICATEDSMDATA, analyst only, with Claude Opus 5.5. The hub numbered it 19 (18 collided with
#683). Docs only: no code, no dependency, and no bounds check
changed.

- **Why:** Off's `Play` refuses every authority start at the Play note's step 4.4, "the record the owner keeps of the
  current move for replication" (decision 2026-11-19-52). With the Queue port, link 9 is predicted to stop there.
- **What:** `docs/verification/NATIVE_SET_REPLICATED_SMDATA.md`, one NATIVE_INDEX row and one GATES row. They come from the script,
  a local read-only Ghidra reading under the machine lock, and the layout tool. Raw output stays under ignored
  `local/analysis/repsm/`.
- **Findings** (UNVERIFIED in game):
  - the hook is `SpecialMoveInterface.SetReplicatedSMData`, tied to the component's start and stop by the pawn's
    interface table. No script calls it. The same body serves `GearboxPawn`, `WillowPawn`, `WillowPlayerPawn` and
    `WillowAIPawn`;
  - it copies the move's `SMD`, `Data`, `PlayRateScale` and `Duration` into `ReplicatedSMData`, steps the byte
    `DirtyCounter`, and sets the pawn's `bNetDirty`. There is no net-mode test, and nothing else is done;
  - the start writes the record on an `Owner` at `ROLE_Authority`, after `CurrentSMData` and `StartTime` and before
    `ServerStarted` and `ClientStarted`;
  - the stop, on the authority, runs `ServerFinished` and then writes the record with `SMD` None and the stopped
    move's other fields;
  - the server events test no net mode. On the Phaselock chain they reach the behavior provider's `OnServerStart`
    and `OnServerStop`.
- **Proposed Off answer:** an internal record write in `ow-skills` (no script binding needed). It applies to the four
  pawn classes read, by their first native class; other owners refuse as `NotRead`. A write-free shape check comes
  where the replication refusal stands today, the writes at the game's points, and no "standalone, so skip" rule.
  Reference unchanged; tests listed.
- **Clean room:** a different lane implements this note.
