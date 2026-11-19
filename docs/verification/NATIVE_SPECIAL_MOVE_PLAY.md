# Native rules of playing a special move: SpecialMoveComponent.Play and what it starts (draft, 2026-11-19)

AI-assisted (Claude), analyst lane SM-PLAY (decision 2026-11-19-35). Script first: the installed packages read with
`research/script_disasm.py`, and the definitions with `ow-package --def-dump`. The natives the script does not show
were then read from the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock).
Written in our own words, as behaviour only: no decompiler output, pseudo-code, quoted script statements, addresses,
offsets or call structure is reproduced here. Field and event names come from the cooked packages. **Every rule is
UNVERIFIED in the running game.** The last section lists the live observations that would confirm them.

**Clean room.** This lane implements nothing. Everything here stays unbound or `NotYet` in Off until a fresh
implementer codes it from this note alone.

**Why it matters.** Slice link 9 (Phaselock). After `GetSMComponent` (NATIVE_SPECIAL_MOVE_COMPONENT.md), the cast's
`LiftActionSkill.OnActionSkillStarted` asks the caster's special-move component to play the definition
`LiftActionSkill.GetPlayerAnimation` chose. That is the hit or the fizzle animation:
- `PhaselockSMD_Hit` = `GD_Siren_Skills.Misc.Phaselock_1st3rd_SM`;
- `PhaselockSMD_Miss` = `GD_Siren_Skills.Misc.Phaselock_Fizzle_1st3rd_SM`.

Both are `SpecialMove_FirstAndThirdPersonAnimation` definitions. Both set `bBlocksWeaponActions` and
`bClientHasAuthority`, end on `EC_OnBlendOut`, and have a first-person counterpart (`FirstPersonSM`) and a behavior
provider. The cast passes only the definition: every other argument of `Play` is left at its default, and the cast
ignores the answer.

## Summary

| Piece | Kind | Confidence | Status |
|---|---|---|---|
| `SpecialMoveComponent.Play(NewSMD, optional PlayRateScale, Duration, CallbackName, CallbackObject, Data)`, answering a float | native | high | UNVERIFIED |
| `SpecialMoveDefinition.GetSMDToPlay(SMI)` | native | high (answers itself) | UNVERIFIED |
| `SpecialMoveDefinition.IsLocalAuthority(SMI)` | native | high | UNVERIFIED |
| `AuthorityCanPlay`, `ServerStarted`, `ClientStarted`, `ServerFinished`, `ClientFinished`, `AnimFinished`, `PlayAnim` on the definition's class chain | script events | high | UNVERIFIED |
| `SpecialMoveComponent.PlayLocal`, and the component's stop and queue steps | native | medium | UNVERIFIED |
| `SpecialMoveComponent.GetSMNode`, `IsPlayingClass` | native | high | UNVERIFIED |
| `Pawn.IsAliveAndWell`, and the target tests of `LiftActionSkill.GetPlayerAnimation` | native and script | high | UNVERIFIED |
| `RunBehaviors`, `GearboxAnimDefinition.OnStart` / `OnServerStart`, the blend node's `PlayData` | native | not read | — |

The class chain of the Phaselock definitions is `SpecialMove_FirstAndThirdPersonAnimation` → `SpecialMove_WeaponAction`
→ `WillowAnimDefinition` → `GearboxAnimDefinition` → `SpecialMoveDefinition`. The first two have no native code of
their own.

## 0. Before Play: which definition the cast chooses

`LiftActionSkill.GetPlayerAnimation(target)` is script. It chooses the hit definition when the target can be
phaselocked and passes `CanLiftTargetIf`, or when it can be resurrected; otherwise the fizzle definition.
- **Can be phaselocked** (script): the target is set; it is not friendly to the caster (`Pawn.IsFriendly`, already in
  NATIVE_INDEX.md); it is alive and well (`Pawn.IsAliveAndWell`, below); and it is not already phaselocked.
  "Phaselocked" is the target's value of the skill's `IsPhaselockedAttribute` (the attribute's value, read through
  `ow-attr`), taken as true when nonzero.
- **`CanLiftTargetIf`:** Maya's skill sets it to a `FlagExpressionEvaluator`. With it unset the test passes. Its
  evaluation is the attribute system's (not read here).
- **Can be resurrected** (script): only off a network client, and only when all of these hold:
  - there is a skill manager (`GetSkillManager`), a controller and a `ResurrectSkill`;
  - that skill is active for the caster (`SkillEffectManager.IsSkillActive`, already in NATIVE_INDEX.md);
  - the target is set and friendly;
  - it is injured (`IsInjured`), and dead while injured (`IsInjuredDead`).

  `IsInjured` and `IsInjuredDead` are already in NATIVE_INDEX.md.
- **`Pawn.IsAliveAndWell()`** (one rule for every pawn class read): true when the pawn is not being destroyed
  (`bDeleteMe` clear) and neither `bIsInjured` nor `bIsDead` is set.

## 1. What Play does

The component holds the current move (`CurrentSMData`: the definition, data, rate, duration and callback), its
`StartTime` and a queue (`SMDQueue`).

1. **Nothing to play.** With no definition, or a component that has no owner interface (`SMI`), the answer is 0 and
   nothing changes.
2. **The definition to play** is the one the given definition names for this owner (`GetSMDToPlay`). The base answers
   the definition itself, and no class on the Phaselock chain changes that. With none, the answer is 0.
3. **Two gates:**
   - the owner must hold authority for the move (`IsLocalAuthority`: the owner actor's `Role` is at least
     `ROLE_Authority`, or at least `ROLE_AutonomousProxy` when the definition's `bClientHasAuthority` is set; the
     Phaselock definitions set it);
   - the definition's script event `AuthorityCanPlay` must answer true (section 3).

   If either fails, nothing starts and nothing is queued. The answer is still 0, except that step 6 may run.
4. **Start.** The queue is emptied, then the move starts:
   1. a move that is already playing is stopped first, as interrupted, with the queue cleared (section 4);
   2. the request becomes the current move, and `StartTime` = the world time;
   3. a chain of follow-ups (`NextSpecialMove`) is queued (section 4); the Phaselock definitions have none;
   4. on the authority (`Role` 3), the owner records the current move for replication, and the definition's
      `ServerStarted` event runs;
   5. the definition's `ClientStarted` event runs, and its answer is the move's duration. A duration of 0 or less
      stops the move again at once (not interrupted, queue cleared).
5. **The answer** is the duration from step 4.5.
6. **A client** (an owner actor below `ROLE_Authority`, on a network client) also forwards the request to the server
   through the owner's `ServerSpecialMove_Play`. In the single-player slice the owner is the authority, so this never
   runs.

`PlayLocal`, which the first-person step below uses, is a lighter start. Without a definition or owner interface it
answers 0. Otherwise it resolves the definition to play the same way, runs its `ServerStarted` and then its
`ClientStarted`, and answers the duration. It does not use the gates, the current move, `StartTime` or the queue.

## 2. ClientStarted on the Phaselock chain, outcome by outcome

Each class runs its parent's version first, so the effects come in this order:
1. **`GearboxAnimDefinition`:**
   - runs the definition's `StartBehaviors` through the component;
   - if the owner is a behavior consumer, initialises the definition's `BehaviorProviderDefinition` for it and fires
     the definition's start event on it (`OnStart`);
   - adds the definition's anim sets to the owner's mesh;
   - plays the animation (`PlayAnim`), whose answer becomes `ClientStarted`'s answer. The play rate is the animation's
     length divided by the requested duration, when a positive duration was asked for and the ending is not
     `EC_Loop`; otherwise it is the requested `PlayRateScale` (default 1). The owner's special-move blend node does
     the playing (`PlayData`), and its answer is the duration.
2. **`WillowAnimDefinition`:**
   - holds the left hand's IK when `bAllowLeftHandIK` is set;
   - zeroes the owner's `Velocity` and `Acceleration` when `bStopMovement` is set;
   - takes an AI pawn out of cover.
3. **`SpecialMove_WeaponAction`:** with `bBlocksWeaponActions` (set on both Phaselock definitions), it hands the
   move's weapon blocking to the owner's controller (`PerformSharedWeaponActions`, not read).
4. **`SpecialMove_FirstAndThirdPersonAnimation`:** when the owner is locally controlled, it also plays `FirstPersonSM`
   on the same component with `PlayLocal`, the first-person animation beside the third-person one.

`ServerStarted`, on the authority only:
- `GearboxAnimDefinition` initialises the behavior provider for a consumer owner and fires its server start event
  (`OnServerStart`);
- `WillowAnimDefinition` puts AI holds on an AI owner when `bStopAI`, `bStopMovement` or `bStopHeadLook` is set. The
  Phaselock definitions set none of them.

## 3. AuthorityCanPlay on the Phaselock chain

True only when all of these hold:
- the owner's component has a special-move blend node (`GetSMNode` answers one);
- the component exists;
- it is not playing a `SpecialMove_Spawned`, a `SpecialMove_Vehicle` or a `SpecialMove_PhaseLock` (the class a
  phaselocked target plays).

### The two component queries AuthorityCanPlay uses

- **`GetSMNode()`:**
  1. asks the owner for its skeletal mesh;
  2. answers the cached `SMBlendNode` when it belongs to that mesh;
  3. otherwise looks up the mesh's animation node named `SpecialMoveBlend`, keeps it when it is an
     `AnimNodeSpecialMoveBlend`, caches it in `SMBlendNode` and answers it.

  The answer is None with no owner interface, no mesh, or no node of that name and class. The start routine of section
  1 also asks the owner for its mesh, but does nothing with the answer.
- **`IsPlayingClass(Class)`:** true when a move is current and its definition is of that class or a subclass; false
  with no current move or no class.

## 4. How a move ends

`Play` sets no timer. With `EC_OnBlendOut` the animation node ends the move: the definition's `AnimFinished` event
stops it through the component when it is still the one playing. Stopping a move:
1. empties the queue when asked;
2. clears the current move;
3. on the authority, runs `ServerFinished` and updates the replicated record;
4. runs `ClientFinished`, which on this chain stops the blend node, runs the definition's `StopBehaviors`, fires the
   stop event for a consumer owner (`OnStop`), restores the mesh's root motion modes and releases the left-hand IK;
5. calls the request's callback, when the move passes `IsLocalAuthority`: the function `CallbackName` on
   `CallbackObject` (or on the owner when none), with its `bInterrupted` parameter set when it declares one;
6. then starts the first queued move, if the queue holds one and that move passes both gates.

Follow-ups from `NextSpecialMove` are put at the front of the queue one by one, so a chain of more than one waits in
reverse order (as read; UNVERIFIED, and not used by Phaselock).

## 5. What script sees

- **The answer of `Play`:** the duration from the animation (seconds), or 0 when nothing started. The cast ignores it.
- **After a start:** the component reports the move as current (`IsPlaying`, `GetCurrent`).
- **The script events above** run on the definition object, with the owner's interface as their argument.

## 6. For an implementer, in the port's terms

- The natives this path reaches, in order, any of which can be the next stop:
  `GetSMDToPlay`, `IsLocalAuthority`, `GetSMNode` and `IsPlayingClass` (all read here);
  `RunBehaviors`; the behavior kernel's provider initialisation (`IntializeBehaviorProviderForConsumer`); `OnStart`;
  the blend node's `PlayData`; then `PlayLocal` (read here) for the first-person definition.
- Before `Play`: `IsAliveAndWell` (read here) and the attribute read behind `IsPhaselocked`, then the
  `FlagExpressionEvaluator`; `IsFriendly`, `IsSkillActive`, `IsInjured` and `IsInjuredDead` are already indexed.
- **A headless world has no animation tree.** `GetSMNode` would then find no `SpecialMoveBlend` node, `AuthorityCanPlay`
  answers false, and `Play` answers 0 without starting anything or firing an event. Whether the port should give the
  pawn a stand-in blend node, or treat the move as played with a fixed duration, is the port's decision. The game
  answers with the animation's length.

## Open questions

1. `RunBehaviors`, `OnStart` / `OnServerStart` and the blend node's `PlayData` (not read).
2. What `PerformSharedWeaponActions` does with a weapon-blocking move.
3. Whether the reverse queue order for a `NextSpecialMove` chain holds in the game.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- Cast Phaselock on a target and on nothing. Hook `ClientStarted`, `ServerStarted`, `AnimFinished` and
  `ClientFinished` on the two definitions, and record their order, the duration `Play` returns and the component's
  `StartTime`.
- During the move, read the component's current definition and its queue (empty).
- Call `Play` with a positive `Duration` and check that the animation length divided by it gives the play rate.
