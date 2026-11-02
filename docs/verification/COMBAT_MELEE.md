# A bullymong's melee blow on Maya, on the VM, without AI (plan step S-e, 2026-10-08)

AI-assisted (Claude), implementer lane Se. Worked from the own-words notes [NATIVE_AI_MELEE.md](NATIVE_AI_MELEE.md) ("MeleeDefinition.MeleeAttack", "Hit application"),
[NATIVE_DAMAGE_DEATH.md](NATIVE_DAMAGE_DEATH.md) (section 1, the stages) and [NATIVE_COMBAT_GROUNDWORK.md](NATIVE_COMBAT_GROUNDWORK.md) (3.2 to 3.5), the live rows of
[COMBAT_GOLDEN_TRACE.md](COMBAT_GOLDEN_TRACE.md) (lane L3 step 9, lanes L5b and L6) and the installed data and script. No decompiler output was read. Every rule below is `UNVERIFIED` as a
statement about the native code unless a row says which live number it reproduces; "reproduces" means "the VM's number equals the recorded live number", not "the native does exactly this".
It builds on the hit chain of [COMBAT_HIT_CHAIN.md](COMBAT_HIT_CHAIN.md) (S-b), the death chain of [COMBAT_DEATH.md](COMBAT_DEATH.md) (S-c) and the bridge's player of [COMBAT_KILL_XP.md](COMBAT_KILL_XP.md) (S-d).

## What a swing is

`swing:<attacker>:<kind | definition path>[:<cause damage path>]` in `ow-package <any.upk> --combat-run --cooked <dir> maya:8 spawn:adult:5 ... swing:0:melee` is a **fixture**: one blow that always
connects. There is no AI, no target choice, no animation, no hit volume, no cone, no allegiance test, no knockback and no `OnHitEnemy` behavior; those are what `MeleeDefinition.MeleeAttack`
does around the damage (NATIVE_AI_MELEE.md, the radius volume and the behavior lists) and plan step S-h will have to model. What the step does model is the part after the blow lands:

1. **The damage** is read from the definition's data and evaluated with the attacker's `WillowMind` as the context source (plan 1.3). A `MeleeDefinition` carries `Damage`, `DamageTypeDefinition` and
   `DamageSource`; the five bullymong definitions (`GD_PrimalBeastShared.Melee.MeleeDef_PrimalBeast_Melee`, `_RadiusLeap`, `_RadiusBadass`, `_MeleeGrabBaby`, `MeleeDef_Melee_Charge`; kinds `melee`, `leap`,
   `badass`, `grab`, `charge`) all have `Damage` = `Init_EnemyNonWeaponDamage` (the badass one with scale 1.5), type `DmgType_Normal` and, except the badass one (see below), source `WillowDmgSource_Melee`.
   The Slinger's rock (kind `slinger`) is not a melee definition: its damage is the `ProjectileDamage` entry of the `ChildProjectileBaseValues` of its throw behavior
   (`Behavior_AIThrowProjectileAtTarget`, again `Init_EnemyNonWeaponDamage`), and the type and source come from the projectile's `Behavior_CauseDamage` (`DmgType_Normal`, `WillowDmgSource_Grenade`; its damage
   formula reads the same attribute, constant 0, scale 1, which multiplies in). UNVERIFIED which throw animation and projectile the Slinger uses live: the kind names the first of the throw animations
   and `Projectile_IceRock`; the live causer names (`WillowServerSideProjectile`) do not say.
2. **The delivery** is the victim's installed `TakeDamage` (for Maya `WillowPlayerPawn.TakeDamage` -> `WillowPawn.TakeDamage` -> `Pawn.TakeDamage`), with the attacker's mind as `InstigatedBy` (the note: the
   attacker's controller), the definition's damage type and source class, and no damage causer (the live causer of a melee row is the melee definition object itself; the VM passes none). The victim is the
   bridge's VM pawn (`pawnFor(Player)`) with its health pool (the `maya:<level>` step; `maya:8` again refills it between swings); a run without `maya` swings at `player:<level>`'s pawn, which has no pool
   (the synthetic test uses that).
3. **`AdjustDamage`** got stage 1 and a level lookup for an AI instigator (`src/combat_hit.cpp`):
   - **Stage 1, the AI damage scale:** for an instigator that is not a player controller the damage is multiplied by the globals definition's `GlobalAIDamageScale`, evaluated with the instigator as the
     context source. In the data it is `Init_GlobalAIDamageBalanceMultiplier_Part1` = Part2 x (1 + `Init_AdditionalEnemyDamagePerLevel`), Part2 = the per-player, per-playthrough, per-champion table (0.65 for
     one player on the first playthrough, not a champion) and the per-level term 0.02 x the attacker's `AICharacterExperienceLevel`. The evaluator needed nothing new: it gives 0.715 / 0.728 / 0.741 for
     attackers of level 5 / 6 / 7 (the factor of NATIVE_DAMAGE_DEATH.md section 1 stage 1 and of the plan's section 0 item 3, here read from the data and not coded). The weapon scales that bullet and
     rocket damage from an AI weapon also takes (`GlobalAIWeaponDamageScale` and the six per-weapon-type ones) are **not implemented** and said so once if such a hit comes.
   - **Stage 2, the level difference:** `levelOf` now answers for an AI mind (its pawn's `ExpLevel`; before, only for the pawn). For a non-player instigator the table is `AIDamageScaleByLevelDifference`: an
     attacker below Maya takes `LowerLevelAttackerDmgScale` (1.0 in the data), an attacker above her `HigherLevelAttackerDmgScale` (1.1 at one level up, 1.2 at two, ... 3.0 from sixteen). The attacker's
     effective level is read as the pawn's `ExpLevel` (the note: a balanced-actor interface slot, "medium"); the playthrough-2 extras of GROUNDWORK 3.5 are the identity on playthroughs 1 and 2 and not coded.
   - **Stage 3, the damage source:** Maya's controller's `ReceivedMeleeDamageModifier` / `ReceivedGrenadeDamageModifier` are read (class defaults 1.0 in the VM; 1.0 live outside a Defense Boost, lane L6); the
     instigator mind's `InstigatedMeleeDamageModifier` (1.0 class default) is read too, as for a player instigator.
   - Stages 4, 7 (Maya's VM pawn has no body class, so no hit region: the default-region and flesh fallbacks of S-b), intrinsic armor and the surface stage as in S-b (`DmgType_Normal`: modifier 1).
4. **Not implemented, said in the run's events:** stage 14 (averted death: the pawn's `PlayerAvertDeathChance` reads 0 here, as live; a value above 0 is reported as not implemented), stage 15 (the protection timer of a player victim hit by an AI: "only
   acts on a hit that would take health below half the maximum", noted once per run), the shield stage (Maya has no shield in the VM; the live 8.229 overflow case, where a hit emptied the shield and the
   rest went to health, cannot be reproduced), backstab (`MeleeAttackTargetFromBehindDamageModifier`: needs positions), the **Defense Boost** of a second wind (L6: x 1/(1 + 0.5 n) while n boosts are active), the
   `HitRegionCap`, projectile reflection. A blow that takes Maya to 0 health would start her down state (plan step S-f), which is not modelled: the fixture is meant for hits that leave her alive.

## Results against the live rows

`local/p2/Se/final_rows.sh` (one `--combat-run`, level-8 Maya refilled by `maya:8` before each swing), `python tools/verify_combat_pawn.py --vm local/p2/Se/final_rows.json`: **25 PASS, 0 FAIL, 4 MISSING of 29**
(the four are the Defense-Boost ratio rows). The new rows: `arrival.<class>.L<level>` (the evaluated damage) and `ratio.<class>.L<level>` (the damage the pipeline passed to health over the arrival).

| attacker (level) | delivery | VM arrival | live arrival | VM applied | live applied | VM ratio |
|---|---|---|---|---|---|---|
| adult 5 | `melee` | 73.6974 | 73.6974 (L5b, 48 hits) | 52.6936 | 52.693 / 52.694 | 0.7150 |
| adult 6 | `melee` | 83.2781 | 83.2781 (L5b, 3 hits) | 60.6264 | 60.63 (L3 step 9, a leap's 83.28) | 0.7280 |
| adult 7 | `melee` | 94.1042 | 94.1042 (L5b) | 69.7312 | 69.731 (L5b), 69.73 (L3) | 0.7410 |
| regular 6 | `melee` | 24.9834 | 24.9834 | 18.1879 | 18.188 | 0.7280 |
| baby 6 | `melee` | 12.4917 | 12.4917 | 9.0940 | 9.094 | 0.7280 |
| ranged 5 | `slinger` (grenade) | 44.2184 | 44.2184 | 31.6162 | 31.616 / 31.617 | 0.7150 |

The comparator compares each VM row with the closed form (`8 x 1.13^level x damage multiplier`; `0.65 x (1 + 0.02 x level)`), not with the live dump; the same closed form is what `--live` checks against
the dumps (`--live`: the arrival rows and `ratio` rows for adult 5 and 7, regular 6 and baby 6 PASS against the dumps; adult level 6 has **no unboosted hit in the L5b/L6 dump** - all three are boosted - so its 0.728 is
the formula plus the one L3 observation above; the comparator has no `ratio` row for the ranged grenade, only its boost row; the VM still writes `ratio.PrimalBeast_Ranged.L5` = 0.715, which nothing compares). The applied values above are an independent check against
the L5b hits (taken at full shield-less health, the health lost equals the pipeline's `FinalDamage`: `health_lost` and `applied` agree to 1e-5 in every step result). Maya's health after the first swing is
212.676 - 52.694 = 159.982. The extra deliveries (not live rows): `leap` and `grab` and `charge` of an adult 5 give the same 73.6974 / 0.715; an adult 10 against level-8 Maya gives arrival 135.783, stage 1
scale 0.78, level scale **1.2** (AI table, two levels up), applied 127.093, ratio 0.936 (UNVERIFIED: no live hit from an attacker above Maya).

**Gap, reported and not guessed:** `MeleeDef_PrimalBeast_RadiusBadass` has `DamageSource` = None in its data (the other four do not). The note says the engine call takes "the definition's `DamageSource`
class"; what the game does with none is unread, so `swing:<n>:badass` stops with an error naming the definition instead of choosing a class. No badass melee hit is in any live dump either.

## What the step reaches that is still a stub (the run's `stubs`, a swing at Maya)

`Controller.InGodMode`, `Pawn.InGodMode`, `Pawn.GetController`, `Pawn.IsUsingWalkingPhysics`, `Pawn.PawnCheckPlayHitPreconditions`, `DamagePipeline.GetBarrelSourceTime / GetPlantSourceTime / GetCanInflictFriendlyFire`,
`DamageTypeDefinition.IsHealingDamageType`, `WillowDamageSource.GetAmmoTheftAmounts`, `WillowPawn.AdjustMomentumForSurfaceType / GetMaxShieldStrength / GetParent / TryFullBodyGib`,
`WillowPlayerPawn.HasActiveDuel / IsVulnerableToFriendlyFire`, `WillowPlayerController.ShakeView`, `PlayerController.DoForceFeedbackWaveform`, and the engine iterator of the skill events. None changes the health arithmetic
of a living, shield-less Maya (the numbers above come out with them as stubs).

## Open items

- A live check of the fixture's edges would settle them: hook `Engine.Actor:TakeDamage` on Maya for a bullymong melee and compare the instigator (the mind), the damage type and source, the causer (the
  melee definition object) and the momentum; and a live `GlobalAIDamageScale` read with an attacker above Maya's level (the higher-attacker table entry).
- Shields, the Defense Boost (second wind), the protection timer and averted death stay noted-not-implemented (see above). The second wind is plan step S-f.
- The hit volume, cone and allegiance flags (`bAffectEnemy` / `bAffectFriendly` / `bAffectNeutral`), the knockback (`Knockback` evaluated the same way, momentum = the horizontal direction x Knockback x 100000 in
  the note, medium-low), the `OnHitEnemy*` behaviors (camera anim, sound) and the over-time melees (`MeleeGrabBaby`, `Melee_Charge`) belong to S-h; `grab` and `charge` here deliver their damage once.
- Which throw animation and projectile the live Slinger uses (the `slinger` kind names one of them; the `ChildProjectileBaseValues` of every throw behavior read was `Init_EnemyNonWeaponDamage`, but only one was dumped).
