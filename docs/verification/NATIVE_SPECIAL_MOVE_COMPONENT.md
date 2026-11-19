# Native rules of the pawn's special-move component: GearboxPawn.GetSMComponent (draft, 2026-11-19)

AI-assisted (Claude), analyst lane GETSMCOMPONENT (decision 2026-11-19-28). Script first: the installed packages read
with `research/script_disasm.py`, and the definitions with `ow-package --def-dump`. The native was then read from the
game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock). Written in our own words: no
decompiler output, pseudo-code, addresses, offsets or listing structure is reproduced here. **Every rule is
UNVERIFIED in the running game.** The last section lists the live observations that would confirm them.

**Clean room.** This lane implements nothing. In the Rust port the native stays unbound in Off, so the stub policy
refuses it, until a fresh implementer codes it from this note alone.

**Why it matters.** With PHASELOCK-CAST part B (#407), Maya's cast in Off stops at the unbound
`GearboxPawn.GetSMComponent()`. That is slice link 9. Seen on main 834b30aa with the installed test
`phaselock::installed::installed_game_maya_skills_duration_and_lift` (Off: "unbound native
GearboxPawn.GetSMComponent()"). Its Reference run stops earlier, inside the parent `ActionSkill.OnActionSkillStarted`,
on the unbound `ActionSkill.OnActionSkillActivated`, so it says nothing about this native.

## Summary

| Native | Script signature (from the package) | Reached on the slice | Confidence | Status |
|---|---|---|---|---|
| `GearboxPawn.GetSMComponent` | `()` returning `SpecialMoveComponent` (native, simulated) | `LiftActionSkill.OnActionSkillStarted`, the Phaselock cast; about 30 other script sites (special-move behaviors, anim definitions, interactive objects, population points, the execute skill) | high | UNVERIFIED |

## Where the cast calls it

`LiftActionSkill.OnActionSkillStarted` (given the caster's pawn, its controller and the target pawn) runs the parent's
version and
`UpdateTargetPawn`. Then, on the authority (`Role` 3) with a target pawn, it:
1. sets `SkillDuration` (`LiftDuration` plus an attribute evaluation on the caster);
2. plays, on the caster's special-move component (`GetSMComponent`), the special move `GetPlayerAnimation` picks for
   the target;
3. sets `SkillStartTime` to the world time;
4. runs `SelectTarget(TheTargetPawn)`.

UnrealScript evaluates the context object of a call before its arguments. If `GetSMComponent()` answers None, the
`Play` call is skipped as an access to None, and `GetPlayerAnimation` is never evaluated.

## GearboxPawn.GetSMComponent

- **What it returns:** the pawn's `SMComponent` (a component property of `GearboxPawn`, class `SpecialMoveComponent`),
  as it is: a plain read of `SMComponent`, the same for `GearboxPawn` and `WillowPlayerPawn`. Nothing is created,
  checked or registered.
- **When the component exists:** it is a default sub-object, not something the native or a script makes. The class
  defaults of `GearboxPawn` and `WillowPlayerPawn` each hold a `SpecialMoveComponent0` sub-object, listed both in
  `SMComponent` and in the actor's `Components`. Maya's pawn archetype (`GD_Siren_Streaming.Pawn_Siren`, a
  `WillowPlayerPawn`, named by `CharClass_Siren.PawnArchetypePath`) has its own `SpecialMoveComponent0` in
  `SMComponent`.
  - In stock UE3 an actor spawned from a template gets its own instance of each component sub-object, and its
    component properties point at that instance (public engine behaviour, not read here). So a spawned Maya has her
    own `SpecialMoveComponent` from the moment she exists.
  - The component's `Owner` is None in the stored template; the engine sets it when the component is attached.
- **None cases:** the answer is None only when the pawn's `SMComponent` is None. That happens for a class or archetype
  whose defaults clear it; none of the stock ones read here does. The native does not check whether the pawn or the
  component is destroyed.
- **What a script sees:** a `SpecialMoveComponent` (an `ActorComponent`), the same one on every call for that pawn.
  Every function of `SpecialMoveComponent` is native: `Play`, `PlayLocal`, `PlayReplicated`, `Queue`, `Stop`,
  `StopAny`, `StopLocal`, `StopAnyLocal`, `ClearQueue`, `IsPlaying`, `IsPlayingAny`, `IsPlayingClass`, `IsQueued`,
  `RemoveFromQueue`, `GetCurrent`, `GetData`, `GetDuration`, `GetAnimLength`, `GetAnimDelta`, `GetPlayRateScale`,
  `GetSMNode`, `LocalAnimFinished`, `RunBehaviors`, `CanPlayBehaviorData` and `IsOwnerAlwaysNetRelevant`. None is read
  here.
- **For an implementer, in the port's terms:** the native is a field read of `SMComponent` on the pawn. What matters
  is what the port's pawn holds there:
  - an archetype spawn needs the instanced copy, and the world's spawn refuses a template other than the class default
    for exactly that reason (it does not read how instanced sub-objects are copied);
  - the slice's player is made from class defaults (link 1's scope), so its `SMComponent` may be the class default's
    own sub-object, shared, unless the port instances it.

  Which of the two the port should answer with is the port's decision. The game is expected to answer with a per-pawn instance (stock UE3), UNVERIFIED; the bl-sdk check below settles it.
- **Live confirmation:** with bl-sdk in Sanctuary:
  - call `GetSMComponent()` on Maya's pawn twice, and on a second pawn;
  - expect the same object for Maya, a different one for the other pawn, and neither the archetype's template;
  - read the component's `Owner` (expected: Maya's pawn).

## The next native on the cast after it

If `GetSMComponent()` answers a component, the statement goes on in script to `LiftActionSkill.GetPlayerAnimation`:
1. it chooses `PhaselockSMD_Hit` or `PhaselockSMD_Miss` from `CanPhaseLockTarget(NewTarget)`, the optional
   `CanLiftTargetIf` evaluator, and `CanResurrectTarget(NewTarget)`;
2. `CanPhaseLockTarget` calls, in script order, `Pawn.IsFriendly`, `Pawn.IsAliveAndWell` and `IsPhaselocked`, plus the
   skill manager's `IsSkillActive` through `WillowGameInfo.GetSkillManager`;
3. `CanResurrectTarget` calls `IsFriendly`, `IsInjured` and `IsInjuredDead`;
4. the chosen definition then goes to `SpecialMoveComponent.Play`.

So the next stop is the first of these natives the port has not bound, with `SpecialMoveComponent.Play` the last. Which
one that is was not run: the installed cast test stops at `GetSMComponent` until it is bound (main 834b30aa, Off). None of these natives is
read here.

## Open questions

1. Whether the port's class-default player pawn should hold its own instance of the special-move component, or answer
   the class default's sub-object (the game's spawned pawn holds an instance).
   **Port decision (decision 2026-11-19-44, lane SUBOBJECT-INSTANCING):** answered by stock UE3 component instancing,
   not by this game: in Off the world's spawn gives each actor its own copy of each component its template owns
   (`ow_world::actors::instance_components`), and `GetSMComponent` answers it. A pawn that still shares a template's
   component stays `NotYet`. UNVERIFIED in the running game; the bl-sdk check above settles it.
2. The rules of `SpecialMoveComponent.Play` and the other component natives (not read).
3. The next native the cast actually stops at, once `GetSMComponent` is bound (run the installed `OnActionSkillStarted`
   test in Off). **Seen** on pc-hummer, decision 2026-11-19-44 (main e59bff01 plus that lane): the unbound
   `Pawn.IsFriendly(object)`, the first native of `CanPhaseLockTarget`.
