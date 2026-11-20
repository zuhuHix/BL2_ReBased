## 2026-11-19: Links 10 to 12's next stop, and RecordMissionObjectiveChangedEventForPlayer, read as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK12, analyst only (clean room: this thread does not implement
it). The decision number is pending; NNN is a placeholder.

- **What:** `docs/verification/NATIVE_RECORD_MISSION_OBJECTIVE_CHANGED.md` and one NATIVE_INDEX row.
- **The probe:** throwaway local merges, never pushed, run with the installed Off `--slice-gate` under the machine lock.
  Variant A is origin/main with #496, #497, #500, #512, #524 and #526; variant B adds #509 (fix/rust-link10-next, the
  R1-R3 rulings).
  - **Variant A:** link 10 passes 3 of 8 driver steps (`stage:8`, `player:8:21000`, `use`). It stops in `accept` at "stopped after partial writes: the mission screen's Flash text list (not hosted)",
    `NotYet { LayerAbsent }`. Links 11 and 12 are blocked behind it.
  - **Variant B:** link 10 is ok, 8 of 8. Link 11 passes 4 of 8 and stops in `hit:fire` at the unbound native
    `WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer(object,int,int,int,name)`. Link 12 (0 of 6) is
    blocked behind it.
- **How:**
  - the installed script, with `research/script_disasm.py`;
  - read-only Ghidra queries under the machine lock (the native's body and its siblings', up to their shared submit
    routine).

  Raw output stays under ignored `local/analysis/link12/`. No bl-sdk evidence is on record for it, and no real-game
  session was run.
- **Found:** it builds a temporary event record and ends in the shared statistics submit routine of its three siblings
  (NATIVE_MISSION_TEXT_LIST.md section 5).
- **Proposed Off answer:** the hub's ruling R3 covers it. It answers nothing, with no write, and is bound Off only.
- Every rule is UNVERIFIED in the running game; the note lists two open questions and one bl-sdk check.
