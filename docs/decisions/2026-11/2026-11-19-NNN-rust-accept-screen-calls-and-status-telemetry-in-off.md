## 2026-11-19: Rust accept-screen calls and status telemetry in Off

AI-assisted (Claude Code / Claude Opus 5.5). Lane IMPL-LINK10-NEXT, implementer, for slice links 10 to 12. Every
two-digit number of 2026-11-19 is taken, so `NNN` is a placeholder until the maintainer rules on three-digit numbers;
the decisions check refuses it until then.

- **The branch.** `fix/rust-link10-next`, on #496 (`fix/rust-mission-list-build` 9b591ada). The gate ran on a local,
  unpushed merge with #497 (`fix/rust-playthrough-virtual` 7af3ab76), which links 10 to 12 need.
- **The source.** NATIVE_MISSION_TEXT_LIST.md at `research/rust-link10-next` 8862a45c (#504's accepted head, analyst
  lane ANALYST-LINK10-NEXT), and the hub's rulings R1 to R3 on its questions. The rules were coded from that note and
  the rulings alone.
- **Landing order.** The code cites that note as the record of its new `NotYet`s (the focus clamp's bound among
  them), and the note comes with #504, so **#504 lands before or with #509**.
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
- **`WillowGFxMovie3D.FocusOn`** (ruling R2; review r1 of #509):
  - any `Thing` but None: `NotYet` (layer absent), "the 3D movie's Flash focus (not hosted)", at once, before any
    field is read;
  - with `Thing` None, both writes are planned with nothing written, and the plan checks first:
    - that `FocusObject` is declared as an object, and `FocusOffset` as a struct with float `X`, `Y` and `Z`;
    - the clamp's actual Y bound: half the margin between the stage height and the stage height times the scale.
      The port's stand-in for a movie with no native player is height 0 and scale 0, so the bound is 0. A
      negative or NaN bound is `NotYet` (not read), "WillowGFxMovie3D.FocusOn: the focus offset's clamp bound",
      record NATIVE_MISSION_TEXT_LIST.md;
  - then the two writes, in order: `FocusObject` = None, then `FocusOffset` = the zero `Vector` (X, Y and Z all 0,
    built fresh, so a prior Z does not survive). There is no other write; `InterpFocusOffset` is untouched.
- **`WillowLeviathanService.RecordMissionStatusChangedEventForPlayer`** (ruling R3, newly bound): nothing, with no
  write, for every player. A port answer, the same as #500's revived event.
  - **Known limit (review r1 of #509, item 4), not fixed here.** The binding goes to the native's dispatch key, which
    names the class and the function but not the package, as the C++ `nativeKey` does. The VM dispatches every native
    through one key map, and its only per-declaration binding (`override_script`) takes script functions. So a
    same-named class with the same native in another package would get the no-op too. Scoping a native to its
    resolved declaration would be a VM design change; the hub queues it as a separate VM item.
- The partial-writes mark stays where the writes are known (the mission bridge's status hook): these natives return
  their `NotYet`s unmarked.

**Tests** (synthetic, `ow_package::testkit`), in `mission_numbers_tests.rs`:
- the screen calls;
- `FocusOn` (review r1 of #509), with every focus field seeded nonzero before the call (X, Y and Z, `FocusObject`
  and `InterpFocusOffset`):
  - on success, the plan's order, the native's own writes in order (through a test-only write log), and an end state
    equal to the seeded heap with exactly those two writes;
  - zero writes on the set-`Thing`, negative-bound and NaN-bound refusals;
  - a set `Thing` refusing before any field is read;
  - two mutations, run locally, each fail a test: the native's writes reversed, and the bound checked after the
    writes;
- the categories' declaration;
- the focus bound as computed (0 for the stand-in, positive, zero at scale 1, negative and NaN refused);
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
