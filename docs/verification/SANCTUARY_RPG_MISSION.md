# Sanctuary slice route: "Rock, Paper, Genocide: Fire Weapons!" (2026-10-01)

AI-assisted. This records the route chosen for the Sanctuary + Maya gameplay loop and what is and is
not established. It is a progress record, not a parity claim: **no paired original-game capture
exists yet for anything below**, so every behavioural statement about the native layers is
`UNVERIFIED`. Extraction, automated checks, host (UE5) results and original-game parity are reported
separately.

## Why this mission

`GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire` (Marcus, Sanctuary) has the smallest dependency
graph found: two objectives (go to the range, "Fire"), two objective sets, one dependency mission
(`GD_Episode03.M_Ep3_CatchARide`), an XP-only reward, a lent mission weapon (a Maliwan elemental pistol
balance, tied to the Fire objective), one enemy class (`GD_TargetDummyBot`) and a Kismet sequence in
`Sanctuary_Dynamic.upk` that already contains the working door. All of it is installed data.

## What is native in this build (so it cannot be run from bytecode)

Disassembly and export sizes show that `MissionTracker` (125 functions), `BehaviorKernel` (28) and the
mission behaviors `AdvanceObjectiveSet`, `MissionRemoteEvent`, `ActivateMission`, `CompleteMission`,
`ClearObjective` have **no script**. `Behavior_UpdateMissionObjective` and `Behavior_ShowMissionInterface`
are script and call into those natives. Kismet op activation (`SequenceOp::Activated`/`UpdateOp`) is
native too. The installed *data* for all of these is readable, so the project implements the native
executors over that data.

## What exists now

| Piece | Where | Evidence |
|---|---|---|
| Reflection-typed arrays/structs inside structs in the VM's tagged-property reader (previously `SeqOpOutputLink.Links` silently decoded empty) | `src/vm.cpp` | `--kismet-run` links now resolve; census below |
| Kismet executor: impulse propagation, delays, Gate, Delay, SetBool, CompareBool, ActivateRemoteEvent, remote and mission remote events; world-acting ops are host bindings, unhandled ops are errors | `src/kismet.*` | `tests/kismet_test.py` (synthetic, in CTest), `--kismet-census` |
| Stock activation of the door: `RE_Ep14_OpenMarcusDoor` / `..CloseMarcusDoor` drive the existing Matinee mover through the installed sequence | `src/mover.*`, `OpenWillowMover.*` | in-engine `tools/test_mover.ps1`: 16/16 PASS (log `local/doors/run-20261001-092749.log`) |
| Mission/behavior native executor: loads the definition, objective sets, behavior provider, packed links; emits ordered effects (remote events, dialog triggers, set-sequence, objective set/complete, status, XP reward, mission weapon grant/removal); save/load of its state | `src/mission.*`, `--mission-run` | CLI run on the real mission, below |
| Event-name oracle over all 133 missions | `local/phase2/mission_events_census.json` (ignored) | 2,277 events: 133 `Default`, 724 objective-named, 633 set-named; 787 other names (custom events, unnamed sets) |
| Original-game call/state trace mod (not yet run) | `tools/sdk_trace/openwillow_gametrace` | **never executed**; see "Capture blocker" |

### Kismet census (Sanctuary_Dynamic, project reader)

19 sequences, 637 ops, 890 outputs, 422 links, **0 unresolved links**, one empty container sequence.
This proves decoding and resolution, not behaviour.

### Mission run (`ow-package Startup.upk --mission-run <mission> --cooked <dir> accept ...`)

Steps `accept`, `tick`, `obj:RockPaper_GoToRange`, `tick`, `obj:Fire`, `turnin` produce, in order: status
Active; `RocksPaper_MoveMarcusToRange`; set `GoToRange_ObjSet`; Marcus dialog 01 (+02 after 1 s);
`RocksPaper_TargetBack` (3 s later); on the first objective: set `RocksPaper_FinalObj`, mission weapon
granted, `RocksPaper_TargetForward`, `RocksPaper_SetFireTargetBool`; 5 s later the Hyperion dialog and
`SetSequence(BehaviorProviderDefinition_5, "Targetable")` (the dummy enemy's provider); on `Fire`: dialog 05,
`RocksPaper_FireCompleted`, weapon removed, status ReadyToTurnIn; turn-in: Complete and the XP reward
(`GD_MissionRewardBalance.XP.XPReward_02_Small`). No errors; no unsupported behavior class on this mission.

## Structural checks and the guesses they leave open

Checked on this mission (and by construction of the loader on every mission it loads): packed
`ArrayIndexAndLength` (index << 16 | length) ranges tile the consolidated link array exactly (21 of 21)
and every linked behavior index is in range. Guesses, all `UNVERIFIED`:

- The high byte of `LinkIdAndLinkedBehavior` is ignored (not an input index for single-input behaviors;
  its real meaning is unknown). Duplicate links to one behavior are collapsed per fired event, because
  the kernel exposes `RecentlyRunBehaviorsForSequence`; that this is its rule is a hypothesis.
- Event firing rule (activation = `Default`; set activation = set name; objective completion =
  objective name), and when an objective set auto-advances to `NextSet` or the mission becomes ready to
  turn in.
- Status names/transitions, when the mission weapon is lent/removed, XP amount (the reward attribute is
  identified, its value is not resolved).

## What is not done

Marcus as a placed NPC with interaction/dialog UI and scripted movement (`WillowSeqAct_AIScripted_2`,
`ArrivedAtMoveNode` events), the target-dummy enemy (spawn through `SeqEvent_PopulatedActor`,
pawn/AI/damage/death/loot), the lent pistol in the host, Phaselock stock behavior, player death/respawn and
persistence in the host, audio, and UE-side wiring of the mission system. The 4 Kismet world ops reached
by this mission's events (`SeqAct_Interp_0` target mover, `WillowSeqAct_AIScripted`, `SeqAct_Toggle`,
`GearboxSeqAct_TriggerDialogName`) are reported at the host boundary, not run.

## Capture blocker (needs the maintainer)

State-faithful comparison needs the real game driven interactively (menus, loading a character, accepting
the mission) with `openwillow_gametrace` enabled. That needs exclusive use of keyboard and screen for
roughly 30-40 minutes and a decision on which save to use. On 2026-10-01 a launch was started and
immediately stopped because the maintainer was using the machine; no input was injected, no trace was
produced. The saves were backed up under ignored `local/capture/save-backup-20261001/`; launching the game
also made Steam Cloud refresh `Save000A.sav`/`profile.bin` (live files carry the newer cloud timestamps;
the backup holds the older content). Nothing was restored over them.

## Reproduce

```powershell
cmake --build build --config Release
ctest --test-dir build -C Release --output-on-failure
$G = "$env:OPENWILLOW_BL2\WillowGame\CookedPCConsole"
build\Release\ow-package.exe "$G\Startup.upk" --mission-run GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire --cooked $G accept obj:RockPaper_GoToRange obj:Fire turnin
build\Release\ow-package.exe "$G\Sanctuary_Dynamic.upk" --kismet-run TheWorld.PersistentLevel.Main_Sequence.RocksPaperGenocide --cooked $G --remote RE_Ep14_OpenMarcusDoor
python tools/export_index.py        # ignored export caches under local/census/exports
```
