# Phaselock presentation: the lifted target's animation, the first-person cast effect, the bubble (2026-10-04)

AI-assisted (Claude). Behaviour notes from the installed script (`research/script_disasm.py`), installed class defaults
and archetype data (`ow-package --properties`) and a local Ghidra reading of `Borderlands2.exe`, under the policy in
[LEGAL.md](../LEGAL.md) ("Analysing the executable") and [NATIVE_ANALYSIS.md](../NATIVE_ANALYSIS.md). Nothing below is
a listing or pseudo-code. Companion to [NATIVE_PHASELOCK_TARGETING.md](NATIVE_PHASELOCK_TARGETING.md) (target choice,
constraints, lift bob) and [PHASELOCK_STOCK_DATA.md](PHASELOCK_STOCK_DATA.md) (identities, numbers, event chain).

**Confidence.** Almost all of this is *script* (the whole presentation chain is UnrealScript plus data, not native C++),
so it is read exactly; "read" is not "seen in the running game". Everything is `UNVERIFIED` in game unless a line says
otherwise. The native part is only how a special move turns into a played clip (section 2.3). Raw output stays under
ignored `local/analysis/E/`.

## 1. Summary of the answers

| Question | Answer | Confidence |
|---|---|---|
| How is the lifted target animated? | Not physics and not a pose node. The target pawn plays four stock SpecialMove definitions (lift, loop, fall, land) from its own AnimSets while the skill moves its location by script; its physics mode is "custom" so nothing else moves it | read |
| What selects the clips? | `PhaseLockDefinition` (per `BodyTag`, from `LiftActionSkill.LiftBodyMap`); each definition names clips by `AnimName` | read |
| When does the arm effect appear? | Not at the cast instant: a notify in the first-person cast clip fires a custom event 0.25 s into `Phase_Lock_Lift` (0.05 s into the fizzle clip) | read |
| Which socket / template? | `FirstPersonAttachmentName` socket on the *arms* mesh, template `FirstPersonParticleSystem` (or `_Fizzled`) | read |
| Anything native that sets bubble/emitter parameters? | No. The only instance parameters are two floats on the loop emitter (section 4); everything else comes from the particle templates | read |

## 2. The lifted target

### 2.1 Which clips

`LiftActionSkill.GetPhaseLockDefinition(target)` picks a `PhaseLockDefinition` (default or the entry of `LiftBodyMap` whose
`BodyTag` the target carries: Loader, Surveyor/Probe, Rakk, Gyrocopter). Fields: `LiftAnim`, `LoopAnim`, `DropAnim`,
`LandAnim` (all `SpecialMove_PhaseLock`), `CanPlayDropAnims` (an optional evaluator on the target), `HeightFromGround`,
`DropTime`. The stock default (`Default__PhaseLockDefinition`, `PhaseLockDef_Default`):

| Move | `AnimName` (looked up in the target's own AnimSets) | Ending | Other |
|---|---|---|---|
| lift | `PhaseLock_Lift` | `EC_OnAnimEnd` (class default of `SpecialMove_PhaseLock`) | `BlendInTime` 0.4 (archetype override; class default 0.1) |
| loop | `PhaseLock_Loop` | `EC_Loop` | blends 0.1 / 0.1 |
| drop | `PhaseLock_Fall` | `EC_StopOnLastFrame` | blends 0.1 / 0.1 |
| land | `PhaseLock_Land` | `EC_OnAnimEnd` | blends 0.1 / 0.1 |

`HeightFromGround` 200 and `DropTime` 0.5 are class defaults. Per-body definitions in the archetype package:
Loader (all four clips; loop `BlendOutTime` 1; drop gated by "neither leg gone"), Probe/Surveyor and Gyrocopter
(`HeightFromGround` 0, so flyers are not raised; Gyrocopter and Rakk have no lift clip, only a loop; Rakk loop
`BlendInTime` 1; Rakk and Gyrocopter drop gated by a leg-gone flag / flag evaluator). The stock clip lengths
(from the local census `local/phaselock/census/target_anims.txt`): Nomad lift 1.4 / loop 2.533 / fall 0.633 / land
1.867 s, Psycho 0.867 / 3.033 / 0.2 / 0.833, Goliath 1.033 / 3.5 / 0.467 / 0.933, Spiderant 0.933 / 1.033 / 0.2 / 0.6.
The target dummy of the slice has none of these clips (PHASELOCK_STOCK_DATA.md).

### 2.2 Sequence (script)

All of this is driven by the skill's script on the server side; `ReplicatedEvent` mirrors the state to clients.

1. **Cast (t = 0).** With a target, `OnActionSkillStarted` plays Maya's own cast move (`PhaselockSMD_Hit`, or `_Miss` if
   the target cannot be lifted, per `GetPlayerAnimation`) and in the same call `SelectTarget` -> `PhaseLockTarget` ->
   `LiftTarget`. So **the target starts lifting at the cast, not when the hand effect appears.** With no target at all
   the cast-move block of that function appears to be skipped and `SelectTarget(none)` ends in `FizzleOut`; who plays
   the miss clip in that case was not read (`UNVERIFIED`).
2. **`LiftTarget`**: `BeginLifting(target, HeightFromGround)` (stores start and end location, the end is the ground under
   the target plus its collision height plus `HeightFromGround`, clamped by a ceiling trace; attaches the Phaselock light
   to the target); then `Play(LiftAnim)` and `Queue(LoopAnim)` on the target's `SpecialMoveComponent`; the target's
   physics is set to the custom mode (value 14) and `bCollideWorld` is cleared.
3. **Per tick** (`OnActionSkillTick`): `UpdateLiftedPawnMeshOffset`, `UpdateLiftedPawn`, `UpdatePhaselockLight`,
   `CheckLandTarget`, `UpdateEffects`, and an incapacitation check that interrupts the lock.
   `UpdateLiftedPawn` sets the actor location every tick (lift curve for `LiftDuration` 0.7 s, then the bob; see
   NATIVE_PHASELOCK_TARGETING.md section 5) and sets collision with `SetCollision(collide actors = true, block actors =
   false)` during the lift and `(true, true)` after it. So the target is shootable while rising but does not block the
   player until it has arrived.
4. **`UpdateLiftedPawnMeshOffset`.** After `LiftDuration * LiftSnapTimePct` (0.35 s) and not for a charmed target: the
   difference between the mesh's bounds centre and the actor location is pulled toward zero with `VInterpTo` speed 5
   and the Z part of that change is added to the mesh's translation. Effect: whatever the clip does to the body, the
   visible body is centred on the bob point within about 0.2-0.5 s. (Host: no counterpart today.)
5. **Lock at 0.7 s** (`LockTarget`): state 3, `OnTargetBecomesLocked` (the provider chain already documented), bubble
   fade-in emitter (section 4).
6. **Outro** at `SkillDuration - LockFadeOutTime`: state 2, `OnTargetIsAboutToBecomeUnlocked`, then `ReleaseTarget` after
   1.1 s.
7. **Release** (`ReleaseTarget` -> `DropTarget`): remove the Phaselock attribute modifiers, then if the pawn is not
   incapacitated, not a flyer (its mind is not "flying") and `CanPlayDropAnims` is absent or true: remember it as the
   dropped pawn, restore default physics (falling), zero velocity and acceleration, and `Play(DropAnim)` with the
   `Duration` argument = `DropTime`. Otherwise: collision with world back on and `Stop(LoopAnim)`.
8. **Landing** (`CheckLandTarget`, every tick while a dropped pawn exists): once the dropped pawn is on the ground
   (walking physics or a forced check) and the fall move is not the one playing, `Play(LandAnim)` and forget the pawn.

### 2.3 What the special-move play does (native + script)

* `SpecialMoveComponent.Play(def, PlayRateScale, Duration, ...)` hands the request to the pawn's special-move node (an
  `AnimNodeSpecialMoveBlend` in its AnimTree; `GearboxAnimDefinition.AnimNodeName` is empty, so the component's default
  node). The node plays the definition's `AnimName` from the pawn's AnimSets (or `AnimSet` if the definition names
  one; there is also a "reverse search order" flag) as a **full-body blend**: the stock phase-lock definitions have no
  bone-blend definition, no per-bone weights and no root motion.
* **Rate.** `rate = def.PlayRate * PlayRateScale`. The script wrapper (`GearboxAnimDefinition.PlayAnim`, read) computes
  `PlayRateScale = clip length / Duration` when `Duration > 0` and the move is not a loop; otherwise it passes the
  requested scale. Hence the **fall clip is time-stretched to last exactly `DropTime` = 0.5 s** (Nomad 0.633 s -> x1.27,
  Psycho 0.2 s -> x0.4); lift, loop and land play at rate 1.
* **Blends** (native node, read): blend-in / blend-out are the definition's values (an override of -1 means "use the
  definition"); negative values become 0; `EC_StopOnLastFrame` and `EC_OnAnimEnd` force the blend-out to 0; and **if the
  clip's length divided by |rate| is shorter than blend-in + blend-out (and it is not a loop), both blends are set to 0.**
  Ending conditions (decoded enum order): `EC_StopOnLastFrame` 0, `EC_OnAnimEnd` 1, `EC_OnBlendOut` 2, `EC_Loop` 3. Only `EC_Loop`
  loops. `Queue` starts the queued move when the current one finishes; the loop therefore begins at the end of the lift
  clip.
* **Side effects of the move** (`SpecialMove_PhaseLock`): on the server it tells the AI component to hold (`Hold('Phaselock')`
  and `HoldMovement`) and forces the pawn uncloaked (`CLOAKOVERRIDE_Uncloak`); when it finishes it releases both and
  clears the cloak override; on the client it re-enables `bCollideWorld`.

### 2.4 Host consequences

`OpenWillowCombatTarget.cpp` (Lane A): start the lift clip at the cast with blend-in 0.4 s (clip length permitting;
below the clip length the blend is dropped), loop from its end, stretch the fall clip to 0.5 s, play land after touch
down; hold the AI and uncloak; add the mesh-centring offset after 0.35 s; allow shots through during the 0.7 s lift but
block the player only after it.

## 3. The first-person arm / hand cast effect

* The cast move on Maya's own components is `GD_Siren_Skills.Misc.Phaselock_1st3rd_SM` (hit) or
  `Phaselock_Fizzle_1st3rd_SM` (miss). The first-person clips are `Phase_Lock_Lift` (0.867 s) and `Phase_Lock_Fail`
  (0.533 s) in `Anim_Siren.Siren_1st`.
* Each clip carries one `AnimNotify_UseBehavior` (client only) at **0.25 s** (lift) / **0.05 s** (fail). Its behavior is
  `Behavior_CustomEvent` with `CustomEventName = PlayPhaselockHandFXFirstPerson`, context = the instigator's
  instance-data context `ActionSkill`; this reaches `LiftActionSkill.RunCustomEvent`.
* `RunCustomEvent` (first-person branch), in order: create a new `ParticleSystemComponent` owned by the player pawn; attach
  it to the **arms mesh at socket `FirstPersonAttachmentName`** (class default `LilithMeleeFX`; socket on `L_Weapon_Bone`
  in `Char_Siren.Hands_Siren`); depth priority group foreground; in-world foreground; owner-only visibility; relative
  translation `FirstPersonTranslation` (3.5, -4.0, -0.5); scale `FirstPersonScale` (0.35); template =
  `FirstPersonParticleSystem` (`Part_SirenASHandOrb`) or, if the skill has fizzled, `FirstPersonParticleSystem_Fizzled`
  (`Part_SirenASHandFizzle`); activate; hook `OnSystemFinished` to detach and clear; kill-while-idle on. **No instance
  parameters, colours or lifetime are set.** `EndSkill` also deactivates and detaches it.
* The third-person branch (`PlayPhaselockHandFXThirdPerson`) spawns a replicated emitter hard-attached to the pawn mesh at
  `ThirdPersonAttachmentName` with `ThirdPersonTranslation` and `ThirdPersonScale` (draw scale), hidden from the owner.
* The screen effect (`Part_PhaseLockScreenEffect`) and the hand tattoo glow (`Phaselock_TatooGlow` coordinated effect) are
  behaviors in the provider, as already recorded in PHASELOCK_STOCK_DATA.md.

Host consequence: the hand orb should be spawned from the cast clip's time 0.25 s (hit) or 0.05 s (miss), not at key
press, attached at the arms socket with the offsets above; no parameter to set.

## 4. The bubble (all of it, script)

* Three `WillowEmitter` actors spawned on the target pawn (location `LiftEndLocation`, relative offset
  `BubbleFXTranslation`, `SetTemplate`): FadeIn at lock (`BubbleFXParticleSystem_FadeIn`), Loop `BubbleFXIntroTime` (0.2 s)
  later (`BubbleFXParticleSystem`), FadeOut at the outro (`BubbleFXParticleSystem_FadeOut`, the loop is destroyed first).
* **Draw scale of each** = target mesh `Bounds.SphereRadius` **divided by** `BubbleFXScale` (66.7 in the installed
  defaults). This is the DrawScale rule already recorded; note the radius is the mesh component's bounds sphere, not the
  collision cylinder.
* Instance parameters, **loop emitter only**: `PhaselockLifeTimeParamName` ("PhaselockLifeTime") = `StateDuration +
  BubbleFXOutroOverlapTime`, set once at spawn, and the emitter's `LifeSpan` is set to the same value;
  `SphereCollapseParamName` ("SphereCollapse") = `(now - CollapseStartTime) / CollapseDuration * MaxCollapseValue`,
  **unclamped**, set every tick (`UpdateEffects`) with `CollapseStartTime = spawn time + StateDuration - CollapseDuration
  - BubbleFXIntroTime` (CollapseDuration 2, MaxCollapseValue 0.75 in the defaults), so it is negative before the collapse
  starts. FadeIn and FadeOut get no parameters.
* Light: the `PhaselockLight` component (class default component, radius 500, brightness 4, colour 96/128/255, no shadows)
  is attached to the target in `BeginLifting`; every tick its brightness is the base brightness times a factor: elapsed /
  duration of the lift state while lifting, 1 while locked, 1 minus elapsed / duration during the outro.
* Nothing native touches these emitters; colour and size come from the templates.

## 5. UNVERIFIED and how to confirm

| Statement | What would confirm it |
|---|---|
| Lift clip starts at the cast with 0.4 s blend-in; loop follows | Original game, SDK trace (`tools/real_game/`) of the target's current special move and clip name over time after a cast |
| Fall clip time-stretched to 0.5 s | Trace of the playing clip's rate during release, or a 60 fps capture of a Psycho (0.2 s clip) dropping |
| Mesh centring offset after 0.35 s | Trace of the target mesh's relative Z translation during the hold |
| Hand orb appears 0.25 s into the lift clip | Frame-accurate capture of the cast, or a trace of `RunCustomEvent` against the cast time |
| Bubble parameter values | Trace of the loop emitter's two instance parameters |
| Collision (non-blocking while rising) | Walk into a rising target |

## 6. What was not read

How the special-move node starts a queued move (`Queue` ordering, callback timing), the exact blend curve shape, the
`SpecialMoveInterface` events the pawn runs when a move starts, the AI component's Hold semantics, and the
`SetDefaultPhysics` body.
