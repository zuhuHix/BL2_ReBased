## 2026-11-19: Rust mission tracker record queries in Off: the active set, the list queries NotYet

AI-assisted (Claude Code). Lane FIX-MISSION-OBJECTIVES, the fresh implementer of the FIX-MISSION-OFF clean-room pair
(decision 2026-11-19-05). This lane coded from `docs/verification/NATIVE_MISSION_OBJECTIVES.md` alone and opened no
analysis output. There was no real-game session, so every rule is **UNVERIFIED** in the running game. No dependency was
added, no bounds check was loosened and no existing `NotYet` guard was relaxed. Reference (`OW_PARITY=reference`) keeps
the C++ stubs (R3). The lane was stacked on `fix/rust-mission-off` (ffecf7b3), whose PR had not landed when it started;
the hub rebased it onto main after that PR landed (#391), with the code unchanged. The installed runs below were made on
the pre-rebase base.

- **What changed** (`crates/ow-mission/src/bridge.rs`, outside Reference only, on the bridge's tracker, next to
  `GetObjectivesProgress`):
  - **`MissionTracker.GetActivePrimaryObjectiveSet(Mission)`**, implemented as the note's "in the bridge's terms":
    - the bridge holds one record, its own mission's, so any other mission (a None one included) has no record and
      answers None;
    - its own mission answers its active objective set, or None while no set is active (before acceptance);
    - the object returned is the mission's `ObjectiveSetDefs` entry whose path is the tracker's active set (the tracker
      read its sets from that list). An active set outside the list is a data error, never a default;
    - `SubObjectiveSets` is not consulted, and nothing changes.
  - **`MissionTracker.GetAllMissions(out Missions)`: `NotYet { LayerAbsent }`.** The note settles the rule: every
    record's `MissionDef`, in the list's name order, appended to the caller's array. But the rule walks the tracker's
    whole `MissionList`, and the bridge holds one record. Answering with that one record would tell the turn-in's
    `CheckPlaythroughComplete` and `CheckPlaythroughAlmostComplete` that the game's missions are just this one, so it
    stays `NotYet` until the tracker holds the game's list.
  - **`MissionTracker.AllExpansionSideMissionsComplete(Controller, Expansion)`:**
    - false for a None controller, as the note settles;
    - otherwise `NotYet { NotRead }`. The note does not resolve by name the controller virtual that gives the
      playthrough index ("read as `GetCurrentPlaythrough`, not proven"), and the test walks the same whole list.
  - The three are bound with `bind_optional`, as `GetObjectivesProgress` is, so a package without them is unaffected.
- **Also changed:**
  - `ow-cli` `slice.rs`: the reason text of the three Reference stubs. Their keys and the allow list are unchanged.
  - The note's status paragraph, and the three NATIVE_INDEX rows' code column.
- **Questions left open** (from the note's "Not read", for an analyst or a bl-sdk session):
  - the controller virtual behind `AllExpansionSideMissionsComplete`'s playthrough index;
  - the name ordering of the record lookup, and what keeps `MissionList` sorted. Both matter only once the tracker
    holds the whole list;
  - in the running game, the active set another mission's record holds. The note's bridge guidance answers None for
    any other mission, and on the Fire route the director names only the Fire mission.
- **Proof.**
  - **Synthetic tests** (invented packages; `bridge/tests.rs`; the toy tracker gained the three native declarations,
    one with an `array<object>` out parameter):
    - `the_active_primary_objective_set_in_off_mode` (Off): the three are bound. Before acceptance the answer is None;
      after acceptance it is `SetA`'s object. Another mission and a None mission answer None, and the status is
      unchanged.
    - `the_mission_list_queries_wait_for_the_full_list_in_off_mode` (Off): `GetAllMissions` returns `NotYet`
      (LayerAbsent). The expansion check gives false for a None controller and `NotYet` (NotRead) with one.
    - `the_record_queries_are_unbound_in_reference_mode` (Reference): none of the three is bound.
  - **Installed, before and after** (pc-hummer). "Before" is ffecf7b3 with `--slice-gate` (c1d332f1) applied; "after"
    is the same plus this lane. Outputs are under ignored `local/p2/mission-objectives/`.
    - `--mission-run` on the Fire mission, with decision 05's steps (`stage:8 player:8:2000 script:accept tick:0.5
      script:screen script:turnin pool`):
      - Off: the step records are equal. The one error line moves from `GetActivePrimaryObjectiveSet(object)` to
        `PlayerController.IsLocalPlayerController()`. No `NotYet` is raised (exit 1 both times).
      - Reference: stdout is byte-identical.
    - `--slice-run` Off, the verify tool's first 12 steps: the step records are equal. Errors before were
      `TriggerDetachSprites`, `AIComponent.ActivateEvent` and `GetActivePrimaryObjectiveSet`; after, the third is
      `IsLocalPlayerController`.
    - `--slice-run` Off, all 22 steps: byte-identical. It exits 3 at `hit:fire` on the Pawn IAIInterface refusal.
    - `--slice-gate`: strict 0 ok / 5 notyet / 9 error before and after; past-errors 0 / 12 / 2 before and after.
      Link 10's second error line (at `accept`) moves from `GetActivePrimaryObjectiveSet` to
      `IsLocalPlayerController`, and link 11 still stops at `hit:fire`.
- **The next Off stops**, for the hub:
  - at the accept, `PlayerController.IsLocalPlayerController()`. This is a stock UE3 controller query; the Rust port
    has no own-words rule for it, and the C++ stubs it under Reference;
  - at `use`, `AIComponent.ActivateEvent` (NATIVE_SLICE_OFF_STOPS.md, waiting for an implementer);
  - at `hit:fire`, the Pawn IAIInterface refusal (a maintainer-approved live read);
  - at the turn-in, once it is reached, `GetAllMissions` with the tracker's full mission list.
