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
- **Off answers given** (final, after review r2 of #473):
  - both numbers through the VM's tracker and controller records, writing nothing;
  - each unresolved decision that changes their result is a named `NotYet { NotRead }`: the engine-wide condition
    when the two branches differ, and the controller's current-playthrough virtual;
  - `UpdateMissionTextList`: `NotYet { LayerAbsent }`, "the mission screen's Flash text list (not hosted)", terminal
    after the accept's writes. The focus pair is not reached; if it were, `GetVariableObject` would be the same kind of
    named `NotYet`;
  - the hub's ruling (a UI-only effect may be "nothing" only without read-back) is applied. The read-back evidence
    covers the 12,968 decoded script functions only, so no call is answered "nothing".
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

**Review r1 of #473 (five findings, fixed in the note).**
1. **The read-back check** now covers `FocusOn`'s state. No script reads `FocusObject`, `InterpFocusOffset` or the clip
   tables, and `FocusOffset` is read only by movie input handlers the slice does not drive. `GetVariableObject`'s result
   only feeds `FocusOn`. The three Off answers are labelled port Off answers for a headless world with no Flash movie
   hosted, UNVERIFIED.
2. **The chain** to `UpdateMissionTextList` at the accept is
   `UpdateMissionStatus` -> `ClientUpdateUIMissionList` -> `UpdateMissionList` -> `extPopulateQuestEntries`. It was
   shown from the full caller scan and matches FIRE_MISSION_GOLDEN_TRACE.md's observed route. The native's checks come
   before its own write, and a `NotYet` from it is terminal with the prefix.
3. **`GetActiveMissionNumber` in the port:**
   - failures from bound and unbound calls propagate unchanged;
   - a `None` list entry is `NotYet { NotRead }`;
   - the engine-wide gate is not guessed: both branches are computed, and a difference is `NotYet { NotRead }`;
   - the expected next stop is the bridge's `GetAllMissions` `LayerAbsent`, while the tracker's data is valid.
4. **The bl-sdk checks** now tell the rules apart. The rest stay open.
5. **NATIVE_CONTROLLER_HELPERS.md and NATIVE_GFX_BRIDGE.md** are fixed in place.

**Review r2 of #473 (five findings, fixed in the note).**
1. **The focus pair:** no "nothing" answer. It is not reached, and `GetVariableObject` would be a named
   `NotYet { LayerAbsent }`.
2. **The text list:** a named `NotYet { LayerAbsent }`, since the undecoded functions and native readers are not
   accounted for.
3. **The engine-wide condition and the playthrough virtual:** each a named `NotYet { NotRead }`; no guessed identity.
4. **The return** is traced through the bridge's `accept`, the slice's real caller.
5. **These bullets** give the final conditions, and the census claims are limited to the decoded functions.
