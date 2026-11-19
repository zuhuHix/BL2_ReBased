## 2026-11-19: Read link 10's next stop as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK10-NEXT, analyst only (clean room). New note
`docs/verification/NATIVE_MISSION_TEXT_LIST.md` and its NATIVE_INDEX row. The decision number is a placeholder until
the hub assigns one.

- **Sources:**
  - one local Off slice-gate run on a throwaway merge of #496 (629d0b9) and #497 (d4c57cc) on main d72b96d, and two
    local, uncommitted probes on it;
  - `research/script_disasm.py` on `SetFocus`;
  - `Borderlands2.exe`, through `tools/ghidra/run.ps1` read-only queries and the local table reader, one headless run
    at a time under the machine lock;
  - field names from `tools/ghidra/class_layout.py` and the export tables.

  Raw output stays under ignored `local/analysis/link10next/`. There is no bl-sdk session, so every rule is
  **UNVERIFIED**. The research branch carries no merged code.
- **The stop:** links 10 to 12 stop at the accept, on `UpdateMissionTextList`'s named `LayerAbsent` (link 10: 3 of 8
  steps; links 11 and 12 blocked).
- **The probes** show what follows: `GetVariableObject`, then, with the focus pair passed, the unbound
  `WillowLeviathanService.RecordMissionStatusChangedEventForPlayer`.
- **Read:**
  - `UpdateMissionTextList`'s body: with `MissionTextList` None it does nothing at all. This settles
    NATIVE_MISSION_NUMBERS.md open question 3;
  - `GetVariableObject` makes no wrapper without a native player, so it answers None (the return is read by fit);
  - `FocusOn` writes `FocusObject` and `FocusOffset` in every case, and with no native player its clamp reads a stage
    size that was never written;
  - the telemetry native builds a temporary record and hands it to a submit routine shared with the revived event.
- **Off answers proposed:**
  - the None-list branch is a reading, but the port's None state is not the game's, so it is a hub call;
  - `GetVariableObject` keeps its `LayerAbsent` unless the hub allows the no-player answer;
  - `FocusOn` keeps its `LayerAbsent`;
  - the telemetry native is bound: a None player does nothing, any other player is `NotRead`, unless the hub extends
    NATIVE_RESPAWN_STATIONS.md 3.5's "telemetry only".
- Not implemented here: an implementer lane codes it from the note.
