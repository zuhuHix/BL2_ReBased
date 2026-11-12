# The stock hit chain on the VM: a player hit on an AI pawn (plan step S-b, 2026-10-08)

AI-assisted (Claude), implementer lane Sb. Worked from the own-words notes [NATIVE_DAMAGE_DEATH.md](NATIVE_DAMAGE_DEATH.md) (sections 0 to 7) and
[NATIVE_COMBAT_GROUNDWORK.md](NATIVE_COMBAT_GROUNDWORK.md) (sections 2, 3 and 6), the live records [COMBAT_GOLDEN_TRACE.md](COMBAT_GOLDEN_TRACE.md) (section A, lanes L3, L5 and
L6) and the installed script (`research/script_disasm.py`). No decompiler output or game code was read. Every rule below is `UNVERIFIED` as a statement about the native
code unless a row says which live number it reproduces; "reproduces" always means "the VM's number equals the recorded live number", not "the native does exactly this".

## What runs where

`ow-package <any.upk> --combat-run --cooked <CookedPCConsole> ... player:<level> ... hit:<pawn>:<damage>:<damage type definition path>[:<source>]` (and the slice step
`pawnhit:`) runs the victim's **installed script** `TakeDamage` with a player controller as the instigator. In the order the script runs them (live order: COMBAT_GOLDEN_TRACE.md A):
`WillowAIPawn.TakeDamage` -> `WillowPawn.TakeDamage` -> `Pawn.TakeDamage` -> `GameInfo.ReduceDamage` (no game in the VM world: a call on None) -> `WillowPawn.AdjustDamage`
(script wrapper) -> native `AdjustDamage` -> `Actor.TakeDamage` -> `ActorTakeDamageInner` -> `RecordRecentDamage` -> `SetHealth` (the script's own `health - damage`, with its floor for a
demigod pawn) -> `NotifyTakeHit` -> `OnTakeDamage` on the class provider -> `TookDamageFromEnemy` -> `CheckInjured`. The damage floor is **script**: `WillowPawn.AdjustDamage` caps a demigod pawn's
damage at `health - 1` and `Pawn.TakeDamage` sets health to exactly 1.0 when a demigod pawn's health would drop below 1. What this lane adds is the natives under that script (`src/combat_hit.cpp`).

## Natives bound (`src/combat_hit.cpp`, `src/combat.cpp`)

| Native | Note | Status |
|---|---|---|
| `Actor.GetDamagePipeline / ReleaseDamagePipeline / GetPipelineDamageType / GetPipelineImpactDefinition` | NATIVE_COMBAT_GROUNDWORK 3.1 | UNVERIFIED; a fresh pipeline object per acquire, pool of in-use flags |
| `WillowDamagePipeline.AdjustDamage` | DD 1, GROUNDWORK 3.2 to 3.5 | numbers reproduce the live pistol and splash hits (below); stages not reached by a player-to-flesh-AI hit are not implemented and say so in the event list |
| `Pawn.PawnCheckTakeDamagePreconditions` | DD 3 | UNVERIFIED; the unnamed dead/blocked virtual is taken as false |
| `Actor.ActorTakeDamageInner` | DD 2 | UNVERIFIED; `MostRecentDamageTaken` only (Kismet TakeDamage events and the instigator's feedback are not in the VM) |
| `DamageTypeDefinition.RecordRecentDamage / GetSurfaceDamageTypeModifier / GetPawnDamageTypeModifier` | DD 6 | UNVERIFIED; the pawn resistance attribute is read as 1 when the VM never initialised it (stand-in, see open items) |
| `Pawn.NotifyTakeHit` | DD 5 | UNVERIFIED; raises through a handler the owner registers (`--combat-run`: the pawn's class provider; the slice: the dummy's provider); the event filter object is not evaluated |
| `Pawn.GetOpinion / IsEnemy / IsFriendly / IsNeutral` | GROUNDWORK 6 | UNVERIFIED generic four-step rule; the player's allegiance object is not in the data the note found, so a pawn without one is Neutral to its hitter |
| `WillowPawn.GetHitRegionForTakenDamage` | DD 1 stage 6 | UNVERIFIED; bone name against the body class's region list, else `DefaultHitRegion` |
| `AIComponent.HoldDemigod / ReleaseDemigod / DemigodOnHold`, `WillowMind.GetAIDemigod / SetAIDemigod`, `GearboxMind.GetAIComponent` | none (bodies unread) | UNVERIFIED; holds kept per reason, `DemigodOnHold` = any hold |
| `Actor.SetPhysics`; `SetupImpactsForPlayHit`, `UpdatePlayHitEffectCounters`, `UpdatePlayHitInfo`, `StartCringe`, `InterpRotation`, `SpecialMoveComponent.Play / IsPlayingAny / IsPlayingClass`, `NotifyInstinctSkillAction`, `NotifyShotStatus` | presentation | bound as documented no-ops |

Still logged stubs on this chain (visible in the run's `stubs`): `WillowAIPawn.CheckInjured`, `AddHitLocation`, `GetParent`, `WillowMind.NotifyAttackedBy`, `AIComponent.GetTargetRecordFor`,
`StatusEffectsComponent.RollChanceForStatusEffect` (the incendiary roll), `WillowDamageSource.GetAmmoTheftAmounts`, `Pawn.IsDead / InGodMode / GetController / IsUsingWalkingPhysics /
PawnCheckPlayHitPreconditions`, `DamagePipeline.GetBarrelSourceTime / GetPlantSourceTime / GetCanInflictFriendlyFire`, `DamageTypeDefinition.IsHealingDamageType`, `BodyClassDefinition.GetKnockbackBody`,
`WillowPawn.AdjustMomentumForSurfaceType`, `TryFullBodyGib`, and the engine iterator of `foreach`. None changes the health arithmetic of a living pawn.

## AdjustDamage as written (player instigator, flesh AI victim)

Stage order of GROUNDWORK 3.2, single precision: level difference (player table; the higher-level attacker takes `HigherLevelAttackerDmgScale`), damage source (`Instigated<Source>DamageModifier`
of the instigator controller times `Received<Source>DamageModifier` of the victim's mind, plus the extra factors of the note), instigator damage type (`Instigated<Element>DamageModifier`), hit
region and critical (region from the bone, default `DefaultHitRegion`; critical only for a `bCriticalHit` region and not for status-effect damage; the region's `DefaultImpactResponse` modifier),
intrinsic armor (`/ (1 + 0.01 x IntrinsicArmor)`), damage surface (the region's surface type, flesh when it has none; the damage type's `Flesh/Armor/ShieldDamageModifier` evaluated with the
instigator as context, times the victim's resistance attribute). The result goes into the summary's `FinalDamage`, which the script reads. Stage 1 (the AI instigator's scale) is implemented since plan step S-e ([COMBAT_MELEE.md](COMBAT_MELEE.md)). Not implemented and noted in the events: the shield stage (a victim with a shield pool), backstab, first/last shot in clip, reflection, the hit-region cap, averted death, the protection timer.

## Results (all from `--combat-run` on the installed packages, level-8 Maya, no game launched in this lane)

**L5 Q1, the Fire dummy (level 8, stage 8, health 778.658; `local/p2/Sb/run_seq.sh`).** The pistol shot (source `WillowDmgSource_Pistol`, damage type `DmgType_Incendiary_Impact`, arriving
74.013847, default region) applies **111.020767** with `NotifyTakeHit` and health 667.637573: the live row is 111.02076721 and 667.63757324 (flesh modifier 1.5 of the incendiary type at
playthrough 1; every other stage is 1). The splash (source `WillowDmgSource_Grenade`, arriving 59.2111) applies 88.8167 (x1.5) when Maya's controller has its class defaults, but the live row is
179.04184 (x3.0238). The extra factor is exactly **Maya's own `InstigatedGrenadeDamageModifier` as her controller snapshot of lane L1 read it** (a skill-modified attribute;
the VM has no skills; the value is in the maintainer's private record): with that attribute set as a state input (`attr:0:InstigatedGrenadeDamageModifier:<the read value>`) the splash applies 179.0419 against the live 179.0418 (relative difference
3e-7, inside the 4-decimal rounding of the recorded arriving value). The snapshot is from another launch than the L5 shots (same character file, level 8); that the two launches carry the same skills
is not shown. With it the whole sequence follows the live record: 778.658 -> 667.638 -> 488.596 -> 377.575 -> 198.533 -> 87.512 -> **1.0**; the hit that would pass the floor applies 86.512 and
`NotifyTakeHit` gets 86.512 (live 86.51234); then every later hit (pistol, splash, status damage) applies exactly 0 with health 1.0. Largest health difference from the live rows 0.00013.

**The floor.** `Pawn.IsDemigodMode()` is False before the first hit and True from it: the first `OnTakeDamage` runs the FireDamage sequence's `Behavior_AIHold_59` (Reason Safe, Type 3) through its
installed script, which selects `AIComponent.HoldDemigod` for Type 3. The enum is `AIHoldType_AI, Target, Movement, Demigod, God, Dialog` (ordinals 0 to 5, read from the package and from the
script's switch), so Type 3 is **Demigod**, not the sum of Target and Movement as lane L6 read its live enum value (its `Target 1, Movement 2, God 4` are ordinals, not bit flags): this answers
L6's open "which part of the hold sets it". UNVERIFIED: the bodies of the hold natives (release semantics, one hold per reason), and a stand-in: the script asks its context for
`IAIInterface`, which the VM's class tables list for the mind and not for `WillowAIPawn` (live the context is the pawn), so the hold is applied to the pawn's mind (`aiInterfaceFor`).

**L3 section A (shotgun pellets on an adult bullymong at level 5, Maya level 8).** Arriving at the gun's live damage (the amount is in the maintainer's private record) with `DmgType_Normal` and `WillowDmgSource_Shotgun`, each pellet
applies what arrives (level scale 1: the higher-level player takes `HigherLevelAttackerDmgScale` 1.0): from 360.876, three equal steps that match the live trail to 0.01. The bullymong's class provider reaches `Behavior_Conditional`, which has no
handler in this driver (recorded as a provider error; plan step S-c).

**Order of effects (VM, one hit):** `TakeDamage`, `AdjustDamage` stages, `ActorTakeDamageInner`, `RecordRecentDamage`, `SetHealth`, `NotifyTakeHit`, then the `OnTakeDamage` event and its behaviors. Health is
lowered **before** the event, as DD 0.1 says and the live order shows. In the Fire slice (`--slice-run` step `pawnhit:`) the host-visible event stream of the hit is identical to the one the direct
`hit:fire` call produces (the Fire event order tool's 32 rules hold on both), the dummy's health additionally falls.

## Open items

- Maya's skills are inputs, not VM state: the splash row needs the grenade attribute from a snapshot. Her other `Instigated*` attributes are 1.0 in the VM.
- The pawn damage-type resistance attributes (`IncendiaryImpactDamageModifier` and the others) are not initialised by the VM (they read 0 with base 0); the live dummy has 1.0 for all. The unit reads that
  state as the identity 1. A real initialisation is part of the stock spawn, not modelled.
- The hit region comes from a bone; the live hits had bones (Hips, Neck, Head ...) and the VM driver sends none (the default region, a non-critical body region, both matched the live numbers). Critical
  regions (Head) are implemented but not compared with a live row.
- `Behavior_Conditional` and the bullymong crawl (`CheckInjured`) are S-c; the AI instigator's stage 1 is done (S-e, COMBAT_MELEE.md); shields are not modelled.
- The host still takes the 20,000 stand-in health: the host swap (S-b2) is a separate lane and a maintainer decision.
