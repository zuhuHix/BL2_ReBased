## 2026-11-19: Read WillowPlayerController.RefreshBalanceDataFromMissionCompletion (analyst)

AI-assisted (Claude Code). Lane ANALYST-REFRESH-BALANCE-DATA-FROM-MISSION-COMPLETION, analyst only; a different lane
implements the note (clean room). Sources: the installed script, the packages' layouts, installed values read with
`ow-package --def-dump`, and `Borderlands2.exe` read locally with the project's Ghidra tooling under the machine lock.
Raw output stays under ignored `local/analysis/rbd/`. No code changed. There was no real-game session, so every rule
stays **UNVERIFIED**. The decision number is a placeholder.

- **New note:** `docs/verification/NATIVE_REFRESH_BALANCE_DATA_FROM_MISSION_COMPLETION.md`, with one row in
  `NATIVE_INDEX.md`.
- **What it settles:**
  - the native is the mission-completion route that clears a region's cached game stage (NATIVE_PROGRESSION.md, "Region
    game stage", step 4, left open when `ResetGameStageForRegion` runs);
  - its table is the mission's expansion's `RegionBalanceData`, else the globals definition's, indexed by the current
    playthrough's position;
  - every override naming the mission resets its region: the first `RegionGameStages` record of that region and the
    current playthrough is removed;
  - the result is true when any override matched, and the caller ignores it.
- **The slice:** the Fire mission is named by none of the first playthrough's 78 overrides, so the native answers
  false and writes nothing.
- **Proposed Off answer:** a write-free preflight and one array write. **A hub call:** whether the balance
  definitions may be loaded (a harness preload, or a counted load) or refused `LayerAbsent` when unloaded.
- **The probe** (local, uncommitted, on #554's stack with a no-op binding): link 12 ok, 6 of 6 driver steps.
