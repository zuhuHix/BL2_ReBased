## 2026-11-19: WillowPlayerController.UnlockAchievementIfConditionsMet read as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK12-UNLOCK-ACHIEVEMENT, analyst only (clean room: this thread does
not implement it). The decision number is pending; NNN is a placeholder.

- **What:** `docs/verification/NATIVE_UNLOCK_ACHIEVEMENT_IF_CONDITIONS_MET.md` and one NATIVE_INDEX row.
- **How:**
  - the installed script, with `research/script_disasm.py`;
  - the package's declared parameter names;
  - the globals' achievement data through the definition store (a local probe, not committed);
  - read-only Ghidra queries under the machine lock.

  Raw output stays under ignored `local/analysis/unlock/`. No bl-sdk evidence is on record for it, and no real-game
  session was run.
- **Found:**
  - it selects the globals definition's `AchievementData` entries whose type and string or int condition match;
  - it calls the controller's script `ClientUnlockAchievement` for each;
  - it returns whether any matched;
  - it writes nothing script-visible except the globals definition's first-use load;
  - no script caller reads the bool;
  - `ClientUnlockAchievement`'s only effect goes through the online subsystem.
- **The slice:** the turn-in passes type 16 with a side-mission count of 1. The one type-16 entry needs 20, so nothing
  matches and the answer is false.
- **Proposed Off answer:** the selection rule, false and nothing for no match, the script call per match. An R3-style
  "false, no call" is offered as the hub's call.
- **Probe** (throwaway merge, local binding to false, uncommitted): 3 ok / 2 NotYet / 9 errors. Link 12 still passes 1
  of 6 and stops in `turnin` at the port's existing `AllExpansionSideMissionsComplete` `NotYet { NotRead }`.
- Every rule is UNVERIFIED in the running game; the note lists two open questions and one bl-sdk check.
