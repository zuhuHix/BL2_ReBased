## 2026-11-19: RefreshBalanceDataFromMissionCompletion, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-REFRESH-BALANCE-DATA, slice link 12. Coded from
docs/verification/NATIVE_REFRESH_BALANCE_DATA_FROM_MISSION_COMPLETION.md alone (#559, accepted at be877676), with the
hub's ruling on its residency question. Clean room: lane ANALYST-REFRESH-BALANCE-DATA (another session) read the
executable; this lane did not open it or that lane's analysis output. Synthetic fixtures only. Built on #554
(6571aabb).

- **What changes:**
  - **`ow-mission` `balance_refresh`** (new): the native's plan, reading only and loading nothing. The rule is the
    note's section 5:
    - **the receiver:** a `WillowPlayerController`; a strict subclass is `NotRead`, and a negative playthrough index is
      `NotRead`;
    - **the mission:** a `MissionDefinition`; None or another class is `VmError::Script`, and an unloaded handle is
      `LayerAbsent`;
    - **the globals definition:** None answers false;
    - **the table:** the mission's expansion's `RegionBalanceData`, else the globals definition's. The playthrough's
      entry is taken by position, with `PlayThroughNumber` not compared. Past the end the answer is false, with no clamp;
    - **the search:** every balance definition, region record and override in order. A None balance entry is
      `VmError::Script`; an unloaded one is `NotYet { LayerAbsent }`, "RefreshBalanceDataFromMissionCompletion: the
      region balance definition <path>". An override's `Mission` is compared by object identity and never resolved.
      **Each matching override adds one reset, duplicates included;**
    - **the plan:** each reset removes the first `RegionGameStages` record of that region for the current playthrough,
      against the array as earlier removals left it.
  - **`ow-mission` `bridge`:** `n_refresh_balance`, bound Off only beside the other record natives, on the bridge's
    controller.
    - The globals definition is the bridge's own `GetGlobalsDefinition` answer, taken only from its cache. The
      lookup loads, and the native loads nothing, so before the lookup it is `NotYet { LayerAbsent }`.
    - The one write is `RegionGameStages`, only when a record is removed, after every check. The answer is true when an
      override matched.
    - `MissionScript::preload_balance_definitions` is the harness preload: it looks up the globals definition and
      loads every balance definition its `RegionBalanceData` names.
  - **`ow-cli` `slice`:** `balance_setup` runs the preload at set-up, Off only, logged as a harness input
    (UNVERIFIED stand-in for the game's residency: all four `Balance_P1_*` handles are unloaded at the turn-in, the
    accepted note says). `--slice-run` and `--slice-gate`'s Fire group both call it after `FireMissionSlice::new`.
- **Marks:** the native's refusals all come before its write, so none is marked here. The turn-in's caller
  (`UpdateMissionStatus`, which wrote first) marks them. Reference is unchanged.
- **Not done:** the expansions' tables are not preloaded (the slice's mission has none); an unloaded expansion is
  refused `LayerAbsent`.
- **Tests:**
  - `balance_refresh_tests.rs` (6):
    - the slice's case, false with no write;
    - a matching override removing the region's record for the current playthrough only;
    - duplicates resetting twice;
    - the expansion's table taken by position, and past the end false;
    - the preflight's refusals and plain answers;
    - unloaded handles: the balance definition `LayerAbsent` with nothing loaded, and an unloaded override mission
      never resolved.
  - `bridge/tests.rs`: the binding refuses before the globals lookup, and after the preload answers false with no
    write.
  - `ow-mission` 231 passed.
- **Checks:** `cargo test --workspace -j 2` 2,395 passed, 0 failed, 46 ignored (on #554's base); clippy shows nothing
  new; crate edges OK.
- **Installed `--slice-gate`, Off, strict, under the machine lock,** on a throwaway local merge with #546, #533, #497,
  #509, #512, #524 and #526, never pushed. Its conflicts were joined in `ow-cli` (`balance_setup` beside #546's
  `unlock_setup`); test and doc files took ours. Result: 4 ok / 1 not yet / 9 error.
  - **Link 12 is ok, 6 of 6 driver steps:** `use`, `turnin`, `tick:0.1`, `tick:1`, `confirmreward`, `close`.
- **UNVERIFIED.** Nothing here was checked in the running game. The note's live checks would confirm it.
