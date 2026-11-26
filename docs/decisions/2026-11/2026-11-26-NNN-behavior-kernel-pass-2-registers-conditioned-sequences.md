## 2026-11-26: the behavior kernel's pass 2 registers a conditioned sequence and evaluates it at once (Off)

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-BEHAVIOR-KERNEL-PASS2, the implementer half of the clean room
(lane F wrote NATIVE_BEHAVIOR_KERNEL_PASS2.md, #771 with its r1 fix; this lane did not read the executable), on main
c345fdb0. No bl-sdk session was granted. **Every rule is UNVERIFIED in the running game.**

**What changed, Off only.** Reference is unchanged; the C++ binds none of this.
- **Pass 2** of `IntializeBehaviorProviderForConsumer` (`ow-behavior` `kernel/natives/pass2.rs`) replaces the pass-2
  `LayerAbsent`, both in the kernel rule and in `InitializeBehaviorProviders`' preflight 7.
  - **The preflight**, the note's steps 1 to 9, is write-free and load-free. It covers the condition's residency, class
    and fields; the client test, by the kernel's `is_client`; `bInstanced`; the walk `WorldInfo` → `GRI` as a
    `WillowGameReplicationInfo` → `MissionTracker`; the tracker's `Role`; `LinkedMission`; the objective's residency
    and `bRememberItemsWithinObjective`; the verdict and the store decision with `bRepeatable`; and the waypoint test
    over every record of the condition's shared list.
  - **The commit runs after pass 1, in sequence order:**
    - A: the link, in the kernel's own per-condition list (native state);
    - C: the verdict applied to every record of that list, the events firing only on a change;
    - B: the store in the service, which cannot fail.
- **Ruling 1, the service.** `ow-vm` `mission_conditions` holds the `MissionConditionAnswers` trait (the verdict, and
  "is the mission Complete") and the `MissionConditions` service, which also keeps the kernel-registered conditions by
  mission. `ow-mission`'s bridge implements it (`conditions::BridgeAnswers`) and installs it in Off in `bind_natives`.
  With no service, a pass 2 that needs an answer is `NotYet { LayerAbsent }`, "the mission tracker's answers".
- **Ruling 2, the separate list.** The bridge's script notification still walks only its own observers. After them,
  in Off, it asks the kernel (`reevaluate_conditions`) to re-evaluate every kernel-registered condition stored under
  its mission, through the same verdict. A refusal there is typed and carries the partial-writes prefix.
- **Ruling 3, one verdict.** `ow-mission` `conditions::verdict` is the note's section 2.4 over a `MissionSystem`. The
  Fire slice's `condition_holds` now asks it. **The slice's difference:** a mission other than the slice's own was
  Complete when the slice's completed set named it. It is now NotStarted, as `GetMissionStatus` answers. Test
  `another_mission_is_not_started_even_in_the_completed_set` records the change.
- **The side finding** (the note's section 5). The direct native does nothing for a None definition (before: a
  `NotRead`), and nothing for one with no sequences, as the game's gate does (`SetDeathDefinition` with `Death_Siren`).
- **Two port details:**
  - the decoder keeps a resident run-time condition object as it is, where it used to ask the store to load it (the
    plan has checked residency; the store's objects load as before);
  - a condition stored twice under one mission is kept once.
- Provider has index-based enable methods (`index_enabled`, `set_index_enabled`), since pass 2's records are per
  sequence. `set_sequence_enabled` now runs through them.

**Tests.** `kernel/natives/tests/pass2.rs` covers the note's tests 1 to 11 and 13, test 8's refusals among them. Test
12 is the kernel side: a later re-evaluation follows the new verdict, and an unloaded objective is the typed `NotRead`.
The bridge-level case (a script observer beside a stored condition) is not a separate test. The three tests that
pinned the old pass-2 `LayerAbsent` now assert the new refusals.

**Measured** (installed Off strict `--slice-gate`, release build, under the machine lock, on local merges with E's pair
#770 + #773, not pushed). Before (main c345fdb0 + #773) and after (+ this change): 6 ok, X 51.2%, every row
unchanged. Links 1, 3, 4, 5, 13 and 14 stop in `maya:8` at the unbound `Pawn.IsPlayerPawn()`. In
`WillowPawn.ApplyBodyClassStartingValues` that call sets `bAlwaysRelevant`, **before** `InitializeBehaviorProviders`,
so pass 2 is not reached on the installed run yet. The prediction that link 1 finishes `maya:8` waits on
`IsPlayerPawn`.
