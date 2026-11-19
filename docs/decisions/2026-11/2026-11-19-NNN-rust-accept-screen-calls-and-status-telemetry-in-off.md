## 2026-11-19: Rust accept-screen calls and status telemetry in Off

AI-assisted (Claude Code / Claude Opus 5.5). Lane IMPL-LINK10-NEXT, implementer, for slice links 10 to 12. Every
two-digit number of 2026-11-19 is taken, so `NNN` is a placeholder until the maintainer rules on three-digit numbers;
the decisions check refuses it until then.

- **The branch.** `fix/rust-link10-next`, on #496 (`fix/rust-mission-list-build` 9b591ada). The gate ran on a local,
  unpushed merge with #497 (`fix/rust-playthrough-virtual` 7af3ab76), which links 10 to 12 need.
- **The source.** NATIVE_MISSION_TEXT_LIST.md at `research/rust-link10-next` cb42b97a (#504, analyst lane
  ANALYST-LINK10-NEXT), and the hub's rulings R1 to R3 on its questions. The rules were coded from that note and the
  rulings alone.
- **Clean room.** This lane did not read the natives and opened no `local/analysis/`, Ghidra or REA output. The
  packages were read only for field names and parameter order.
- **No dependency, no parsing change.** No real-game session took place, so every rule is **UNVERIFIED** in the
  running game. Reference is unchanged.

**In `ow_world::world::mission_numbers`** (Off only), replacing decision 2026-11-19-93's conservative `LayerAbsent`s
where the note and the rulings give an answer:
- **`QuestAcceptGFxMovie.UpdateMissionTextList`** (ruling R1):
  - with `MissionTextList` None it returns, with no write;
  - with it set it keeps `NotYet` (layer absent), "the mission screen's Flash text list (not hosted)";
  - a movie class without `MissionTextList` or `MissionCategories` is a `VmError::Script`.
- **`GFxMoviePlayer.GetVariableObject`** (ruling R1): None, no write. The port's movies have no native player.
- **`WillowGFxMovie3D.FocusOn`** (ruling R2):
  - the declarations of `FocusObject` and `FocusOffset` are checked first;
  - with `Thing` None: `FocusObject` = None, then `FocusOffset` = (0, 0), and no other write. The port's stage height
    is 0, so the clamp's Y bound is 0. A negative or NaN bound, or a non-zero stage height (whose scale was not read),
    is `NotYet` (not read) before any write;
  - any other `Thing`: `NotYet` (layer absent), "the 3D movie's Flash focus (not hosted)".
- **`WillowLeviathanService.RecordMissionStatusChangedEventForPlayer`** (ruling R3, newly bound): nothing, with no
  write, for every player. A port answer, the same as #500's revived event.
- The partial-writes mark stays where the writes are known (the mission bridge's status hook): these natives return
  their `NotYet`s unmarked.

**Tests** (synthetic, `ow_package::testkit`), in `mission_numbers_tests.rs`:
- the screen calls, including `FocusOn`'s two writes and its refusal leaving the fields unchanged;
- the categories' declaration;
- the focus bound;
- the telemetry for a None player and a player.

**Verified.**
- `cargo test --workspace`: 2,382 passed, 0 failed, 46 ignored.
- Installed Off `--slice-gate` strict on the local merge with #497, before and after:

| Link | Before: steps passed / driver steps, first stop | After |
|---|---|---|
| 10 mission_accept | 3 / 8, `accept`: the text list `LayerAbsent` | **ok**, 8 / 8 |
| 11 mission_objective | 0 / 8, blocked by link 10 | 4 / 8, `hit:fire`: a pawn's `QueryInterface(IAIInterface)` routed to its `SkeletalMeshComponent` (NotYet) |
| 12 mission_turn_in | 0 / 6, blocked by link 11 | 0 / 6, blocked by link 11 |

  The totals went from 0 ok / 5 NotYet / 9 errors to 1 / 4 / 9.

**Open.**
- The next stop: what a skeletal mesh component answers to `QueryInterface(IAIInterface)`.
- The note's open questions 1 to 5.
