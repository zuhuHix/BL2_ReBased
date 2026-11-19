## 2026-11-19: Read the two mission numbers and the accept's next stop as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-MISSION-NUMBERS, analyst only (clean room). New note
`docs/verification/NATIVE_MISSION_NUMBERS.md`, with NATIVE_INDEX rows.

- **Sources:**
  - `Borderlands2.exe`, through two `tools/ghidra/run.ps1` read-only queries, one headless run at a time under the
    machine lock;
  - `research/script_disasm.py` on the accept screen's script;
  - `tools/ghidra/class_layout.py` for field names;
  - one local probe slice-gate run with both numbers answered 0. The probe patch was never committed and its output
    stays under `local/`.
- Raw output stays under ignored `local/analysis/missionnumbers/`. There is no bl-sdk session and no recorded live
  evidence, so every rule is **UNVERIFIED**.
- **Read:**
  - `GetActiveMissionNumber`: the tracked mission, else the tracker's plot scan; its fallback is the plot-critical
    function. This corrects NATIVE_CONTROLLER_HELPERS.md.
  - `GetActivePlotCriticalMissionNumber`: as NATIVE_SAVE_LOAD.md says.
  - After the refresh, the accept reaches `QuestAcceptGFxMovie.UpdateMissionTextList`, called from
    `extPopulateQuestEntries`. This corrects NATIVE_GFX_BRIDGE.md's "no script caller".
- **Off answers given:**
  - both numbers through the VM's tracker and controller records, writing nothing;
  - `UpdateMissionTextList` on the stand-in screen (its `MissionTextList` is None): nothing;
  - `GetVariableObject`: None; `FocusOn(None)`: nothing.
  - These treat "no Flash movie hosted" as the engine's fact. That is a hub call, with a named `LayerAbsent` refusal
    as the alternative.
- Not implemented: an implementer lane binds these from the note.

**Hub ruling on the screen (2026-11-19), folded in.** A UI-only side effect may be answered "nothing" in Off only when
nothing on the slice path reads it back. The note now labels `UpdateMissionTextList`'s "nothing" as a port Off answer
for a headless world with no Flash movie hosted (UNVERIFIED) and records the read-back check:
- every script function of the nine code packages was decoded and searched (12,978, with 10 not decoded);
- nothing reads `MissionCategories`;
- `MissionTextList` is read only by the movie's own script. Its one reader the slice can reach, `GetSelectedIndex`,
  reads None either way.

The script path is also corrected: the slice's accept reaches the movie through `ClientUpdateUIMissionList` and
`UpdateMissionList`, not `extAcceptConfirmed`.
