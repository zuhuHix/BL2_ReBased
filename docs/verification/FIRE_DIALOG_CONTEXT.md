# Fire mission dialog: the context of `Behavior_TriggerDialogEvent_1184` (implementer lane I8, 2026-10-07)

AI-assisted (Claude), implementer lane I8. Source of the rule: the live record in
[FIRE_MISSION_GOLDEN_TRACE.md](FIRE_MISSION_GOLDEN_TRACE.md), "Dialog lines without the dialog skipper (lane L4)"; the behavior kernel's own notes
[NATIVE_BEHAVIOR_CONTEXT.md](NATIVE_BEHAVIOR_CONTEXT.md) (empty context list) and [NATIVE_DIALOG.md](NATIVE_DIALOG.md) (the context is the dialog's instigator).
No decompiler output was used in this lane. Labels: **observed** = in the live record; **UNVERIFIED** = a reading that no running-game observation confirmed.

## What the live game does (observed, lane L4)

- `Behavior_TriggerDialogEvent_1184` (line 03a) is applied with the **target dummy pawn** as its context object; it speaks the line (act 108, spoken by the
  pawn). Its only `LinkedVariables` entry is the provider's variable `TargetDummy` (`BVAR_NamedKismetVariable`, property `Context`); the mission's
  `ChangeRemoteBehaviorSequenceState_171` has the same link. The level's Kismet object variable named `TargetDummy` holds the pawn after the spawn.
- Inside the spawn frame (both launches): `_1186`'s first run, one kernel wake later its resumed run, then the dummy's spawn behaviors, then the Kismet
  `SeqEvent_PopulatedActor` that writes the variable.
- Unmodded, 03b lasts 6.59 s, so `_1184` is applied 0.073 s after it ends, long after the variable is set; 03a lasts 2.01 s and `_1183` (line 04, 5.01 s) follows.
- With the Dialog Skipper on, 03b ends in the spawn frame, `_1184` is not applied and `_1183` never starts.

## What the VM does now

| Piece | Where | Rule | Status |
|---|---|---|---|
| A named Kismet variable link resolves to the object bound under that name | `BehaviorProvider::bindKismetVariable`, `linkedObjects` (src/behavior.*), `MissionSystem::bindKismetVariable` | the owner of the world binds the Kismet object variable's value; an unbound or None name resolves to no object | the binding mirrors the live variable (observed); that the kernel reads it when the behavior is applied is UNVERIFIED |
| The dialog behavior runs on its context | `DialogSystem::behavior` (src/dialog.*) | at the first run the kernel's context list is read (`contexts(behavior, mission)`); **an empty list means the application does not run**: no Out, no line, no Finished, no wake; else the context's registered talker is the event's instigator (kept for the resumed runs) | empty list: UNVERIFIED (L4 read it from the order of rows); instigator = context: native note, UNVERIFIED |
| The dummy is a talker and the variable's value | `FireMissionSlice::spawnDummy` (src/slice.*) | the stock archetype `GD_TargetDummy.Character.Pawn_TargetDummy` is instantiated once; its body class's `DialogName` (`DialogName_HypEngineer`) is registered as a talker with the pawn as actor; `TargetDummy` is bound **after** the dummy's own spawn behaviors (the live order) | the order is observed; the registration follows the data |
| An instigator-talker act with no instigator actor takes a TalkData entry | `DialogSystem::talk` | line 03b (act 67: `bInstigatorTalker`, its only entry Marcus) is spoken by Marcus's pawn live although its context is the mission tracker; the VM resolves the entry by name tag as for any act (before: "no talker" and no line) | UNVERIFIED as a native rule (read from the observation of one act) |

`DialogSystem::behavior`'s polling (first run waits 0.001 s and selects Out; the resumed run triggers; poll every 0.1 s; Finished at the first inactive read) is unchanged:
the live poll matches it. `ChangeRemoteBehaviorSequenceState_171` is **not** made context-aware (its handler ignores the link; live the variable is long set when its 1 s delay ends).

## Which default is faithful (decision, UNVERIFIED in detail)

The default `--slice-run` and the host suite play no audio, so every dialog line takes no time: 03b ends before the dummy exists, `TargetDummy` is empty when 03a is
applied, the application is dropped and 04 never starts. That is exactly what the live **skipper-on** run did (observed), so the default reproduces a real condition and
reports no 03a/04 line (`absent` rule in `tools/verify_fire_event_order.py`). It is **not** the unmodded game, where the lines take seconds. A run that wants the unmodded
order needs lines that take time and a talker for 03b: `talker:GD_Dialog_NPC.Names.DialogName_Marcus lines:<s>` with gaps between the steps (the `STEPS_LINES` run of the
tool, `lines:2`): 03b is a live line, the dummy spawns during it and is bound, 03a starts when 03b ends and is spoken by the dummy, then 04.

Live line lengths, for a future host line player (they are audio lengths and are **not** hard-coded as VM timing): kickoff 01 17.88 s, 02 5.76 s, 03b 6.59 s, 03a 2.01 s,
04 5.01 s, `DET_NPC_OnUse_MissionsAvailable` 3.10 s.

## Checks

`python tools/verify_fire_event_order.py --reader build/Release/ow-package.exe` now runs two `--slice-run`s: the default one (the original 18 rules plus one: 03a and 04 absent) and
the LINES run (6 rules and a no-errors rule). `--self-test` covers both rule lists on invented streams. `tests/mission_script_test.py` scenario E4 covers the kernel rule on a synthetic
provider: an unset variable drops the application, a bound pawn speaks the line and Finished waits for it, another pawn's name tag finds no entry (the context is the instigator), and
the variable is read at application time.
