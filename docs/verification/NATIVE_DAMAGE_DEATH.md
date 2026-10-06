# Native damage, death and respawn: hit pipeline, health, OnTakeDamage, status effects, FFYL, respawn (2026-10-05)

AI-assisted (Claude), analyst lane G11. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Script behaviour was read with `ow-package --disasm` / the local
WillowGame script listing; class field names were matched to the native code with `tools/ghidra/class_layout.py` (a
local copy that also sizes `MapProperty` as 60 bytes was needed for `WillowPlayerController`; that size is an
assumption, checked only by the attribute names landing on the expected fields). Data values come from the installed
packages (`ow-package --object-dump` / `--properties`, class defaults read from the `Default__` objects). Raw output lives
under ignored `local/p2/G11/` (and the Ghidra project copy's out folder).

Scope: the path from a weapon hit to the victim, health and shield bookkeeping, the `OnTakeDamage` behavior event, the
Incendiary status effect (the Fire mission weapon), death of an AI pawn, and the player's down state, second wind, death
and respawn. Weapon damage numbers are in [NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md) (not redone); player health in
[NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md) section 4; the behavior kernel and the dummy's provider in
[NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md); drops in [NATIVE_LOOT.md](NATIVE_LOOT.md). Overlaps:
G2 (behavior/population: raising and filtering of events), G3 (engine core: Actor/Pawn natives), G9 (objective
triggers: what the Fire chain does with the event), G10 (Kismet: `SeqEvent_TakeDamage`, `SeqEvent_Death`).

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| WillowDamagePipeline.AdjustDamage (virtual of DamagePipeline) | `native final function DamageEventSummary AdjustDamage(out float IncomingDamage, out float DamageSeverityPercent, Actor DamagedActor, Controller DamageInstigator, vector HitLocation, class<DamageType> DamageSource, out vector HitMomentum, out TraceHitInfo HitInfo)` | Fire: every hit; fight: every hit | stage order high, per-stage formulas medium | UNVERIFIED |
| DamagePipeline.ConvertDamageToHealing | `native function bool ConvertDamageToHealing(float IncomingDamage, Pawn DamagedPawn, Controller DamageInstigator, class<DamageType> DamageSource, TraceHitInfo InHitInfo)` | none | low (virtual, not read) | UNVERIFIED |
| Actor.ActorTakeDamageInner | `native final function ActorTakeDamageInner(float DamageAmount, Controller EventInstigator, vector HitLocation, class<DamageType> DamageType, DamagePipeline Pipeline)` | every hit (Kismet TakeDamage events) | medium | UNVERIFIED |
| Pawn.PawnCheckTakeDamagePreconditions | `native final function bool PawnCheckTakeDamagePreconditions()` | every hit | medium | UNVERIFIED |
| Pawn.SetHealth / GetHealth / GetMaxHealth / SetMaxHealth | `native final function SetHealth(float NewHealth)` etc. | health pool update | high | UNVERIFIED |
| ResourcePool.SetCurrentValue / AddCurrentValueImpulse | `native final function SetCurrentValue(float Value)`, `AddCurrentValueImpulse(float Delta)` | health and shield pools | medium | UNVERIFIED |
| Pawn.NotifyTakeHit -> Controller.NotifyTakeHit (WillowMind override) | `native final function NotifyTakeHit(Controller InstigatedBy, vector HitLocation, float Damage, class<DamageType> DamageType, vector Momentum, DamagePipeline Pipeline)` | Fire: raises OnTakeDamage | high | UNVERIFIED |
| AIClassDefinition.OnTakeDamage / OnVehicleTakeDamage | `native final function OnTakeDamage(BehaviorConsumerHandle ConsumerHandle, Controller Instigator, float Damage, float ShieldDamage, Object DamageSource, Object DamageType)` | Fire: the event the dummy listens to | high | UNVERIFIED |
| DamageTypeDefinition.GetSurfaceDamageTypeModifier / GetPawnDamageTypeModifier / RecordRecentDamage | `native function float GetSurfaceDamageTypeModifier(byte DamageSurfaceType, Controller DamageInstigator)` etc. | elemental effectiveness | high | UNVERIFIED |
| WillowDamageSource.ShouldDamageSourcePenetrateShields / CanDamageSourceBeAbsorbedByShields | `static native function bool ...(class<DamageType> DamageSource [, Controller DamageInstigator])` | shield absorption | medium | UNVERIFIED |
| StatusEffectsComponent.RollChanceForStatusEffect | `native final function RollChanceForStatusEffect(Controller InstigatedBy, vector HitLocation, vector Momentum, class<DamageType> DamageType, StatusEffectDefinition StatusEffectDefinition, DamageEventSummary DamageSummary, TraceHitInfo HitInfo, IDamageCauser DamageCauser, BodyHitRegionDefinition HitRegion, optional float ChanceModifier)` | Fire weapon's incendiary chance | medium (factor identities partly open) | UNVERIFIED |
| StatusEffectsComponent per-frame update (virtual, damage over time) | not a script function | incendiary ticks | medium | UNVERIFIED |
| WillowPawn.NotifyDamageTaken / AddDamageToHitRegion / GetHitRegionForTakenDamage | `native function NotifyDamageTaken(DamageEventSummary DamageSummary)` etc. | hit regions | low (interface thunks; bodies not resolved) | UNVERIFIED |
| BodyClassDeathDefinition.OnKilledBy / OnDeathNonGib / OnDeathGib / OnPlayDeathPizazz | `native final function OnKilledBy(BehaviorConsumerHandle ConsumerHandle, Controller Killer)` | AI death events | high | UNVERIFIED |
| WillowExperiencePipeline.CalculateExperiencePointsForKill / AwardCombatExperienceToParty | `static native function float CalculateExperiencePointsForKill(...)` | kill XP | low (structure only) | UNVERIFIED |
| Script rules (not native): Pawn/WillowPawn/WillowPlayerPawn.TakeDamage, AdjustDamage, HandleHealthDepleted, Died, injured state, ResurrectPlayer, ShowRespawnDialog | script | fight, die, respawn | high (read) | UNVERIFIED |

## 0. Flow overview

### 0.1 One hit on an AI pawn (the Fire dummy, a shot at the Maya-fired incendiary pistol)

Order of effects, all inside one `TakeDamage` call on the victim (script unless stated):

1. `WillowAIPawn.TakeDamage` returns early for an occupant of a vehicle that may not be damaged; otherwise it calls
   `WillowPawn.TakeDamage`, which clears `bWasLastDamageACriticalHit`, returns early while a level travel countdown runs,
   zeroes momentum for bodies that ignore it, may overwrite the Z momentum with the instigator's `ForcedShotZMomentum`,
   extends the barrel/plant "source" timers when the pipeline carries them, notes whether the victim has shield, and calls
   the engine's `Pawn.TakeDamage`.
2. `Pawn.TakeDamage` (engine script): refuses through `PawnCheckTakeDamagePreconditions` (section 3); clamps damage to at
   least 0; sets movement physics if the pawn had none; scales momentum by 1/Mass (100 x momentum when Mass is 0.01 or
   less) and adds an upward component for damage types that ask for it; works out friendly-fire/healing (a healing damage
   type or a blocked friendly hit goes to `HealDamage`/`TookDamageFromFriendly` instead of the normal path); then for a
   normal hit:
   1. `GameInfo.ReduceDamage` (damage 0 inside a neutral volume or for a god-mode pawn);
   2. `AdjustDamage` (virtual; `WillowPlayerPawn.AdjustDamage` -> `WillowPawn.AdjustDamage` -> the native pipeline,
      section 1). The result is the damage that reaches **health**, after shield absorption and all scaling;
   3. `Actor.TakeDamage` -> `ActorTakeDamageInner` (section 2);
   4. `DamageTypeDefinition.RecordRecentDamage` on the victim's recent-damage tracker (section 6);
   5. **health := health - damage** through `SetHealth` (a demigod pawn stops at 1);
   6. `NotifyTakeHit` -> controller: for an AI pawn this raises the **OnTakeDamage behavior event** (section 5);
   7. if the instigator is not the victim's own controller: remember `LastHitBy`, call `TookDamageFromEnemy`
      (stats, **status-effect roll** (section 8), skill events, melee/ammo steal);
   8. health <= 0 -> `HandleHealthDepleted` (death, section 9; for a player: the down state, section 10); otherwise
      `HandleMomentum` and `PlayHit`.
3. Back in `WillowPawn.TakeDamage`: shield-down dialog/player event when shield went from above 0 to 0; combat-time stamp,
   critical-hit and damaged dialog events for the instigating player; forwarding to a parent body when the body class
   asks (`bDamageParent`, scaled by `DamageParentScale`, limited by `ParentDamageLimit`); a tinnitus trigger for
   explosive damage on pawns that have the interface.
4. Back in `WillowAIPawn.TakeDamage`: cringe, hit location bookkeeping, `CheckInjured` (AI "injured" crawl), hard flinch
   (damage / max health above the damage type's `HardFlinchPercent` for types with `bCauseHardFlinch`), and
   `NotifyAttackedBy` on the AI mind.

So the OnTakeDamage event is raised **after health has been reduced and before** the status-effect roll, the death
handler and the hit reaction.

### 0.2 Player victim

Same chain; `WillowPlayerPawn.TakeDamage` adds: ignored while awaiting respawn; ignored for a vehicle occupant who may not
be hurt; HUD damage shake and a directional indicator; `LastCombatActionTime` stamp; the "attack fully absorbed by
shields" statistic for bullet/rocket hits that took shield and no health. `WillowPlayerPawn.AdjustDamage` multiplies
self-inflicted damage by `GlobalsDefinition.SelfInflictedDamageMultiplier` (0.5 in the data) and friendly-fire damage by
`FriendlyFireDamageMultiplier` when applicable. Zero health starts the down state (section 10), not death.

## 1. WillowDamagePipeline.AdjustDamage

- **Signature:** above. It is a virtual; the implementation is the one of `WillowGame.WillowDamagePipeline`.
- **Reads:** the pipeline object (`DamageTypeDef`, `WillowDamageTypeDef`, `WillowImpactDefinition`, `DamageInstigator`,
  `HitLocation`, `HitMomentum`, `HitInfo`, `DamageSource`, `Globals`, `GlobalsDef`, `WGRI`, the damaged actor and its views
  as pawn/AI pawn/interactive object, `DamageInstigatorWPC`, the interface views `DamageableInt`, `HitRegionConsumerInt`,
  `ProtectableInt`, reflectable and targetable views, `bCanReflect`, `BulletFromClipType`, the array
  `TakingRadiusDamageOnHitRegions`), the damage type definition, the instigator and victim controllers' attributes, the
  globals definition and the victim's health and shield pools.
- **Does (in order):**
  1. *Setup.* Stores the call's context in the pipeline, resolves the globals objects, the victim's views and the
     instigator's player-controller view, and records whether the victim had max health, max shield and was "injured"
     (health below 30 % of max is also remembered).
  2. *Split over regions.* If `TakingRadiusDamageOnHitRegions` is empty the whole damage runs through the stages once. If
     it has N entries (radius damage that touches N hit regions), the damage is divided by N, the stage chain runs N times,
     each time with that hit region preset, and the per-pass summaries are added field by field (skill-event tags merged,
     at most 8 distinct tags kept for each of the two lists).
  3. *Stage chain*, per pass. The working damage starts as the incoming damage (`InitialDamage`). Each stage multiplies or
     reduces it and writes how much it removed into its own field of the `DamageEventSummary` (listed in order):
     1. **AIDamageScaleReduction.** Applies only when the instigator is an AI (not a player controller and not a pawn with a
        player's arms). Multiplies by `GlobalsDefinition.GlobalAIDamageScale` (evaluated with the instigator as context) and,
        for bullet/rocket damage from an AI weapon, by `GlobalAIWeaponDamageScale` and the per-weapon-type global scale
        (pistol, shotgun, SMG, assault rifle, sniper, rocket) chosen by the weapon type of the instigator's weapon. In the
        data `GlobalAIDamageScale` is `Init_GlobalAIDamageBalanceMultiplier_Part1` (a formula over
        `Init_AdditionalEnemyDamagePerLevel` and a Part2 multiplier) and `GlobalAIWeaponDamageScale` is
        `GD_Globals.Balance.Init_EnemyGunDamage` (player-count dependent; values 0.8, 0.95, 1.15, 1.4 for 1, 2, 3, 4
        players, the first branch being the single-player one is the reading, **UNVERIFIED**); the six per-type scales
        default to 1.
     2. **DamageSourceReduction.** By the damage *source* class (bullet, melee, grenade, rocket, status effect, skill):
        multiplies by the instigator controller's `Instigated<Source>DamageModifier` and the victim controller's
        `Received<Source>DamageModifier` (Engine.Controller attributes: bullet, melee, grenade, rocket, status effect,
        skill), 1 when there is no controller. Extra factors folded into the same stage: for bullets the instigator's
        `TargetOverMinHealthGunDamageMultiplier` when the victim's health fraction is above the instigator's
        `TargetMinHealthDamageBoostPercent`; for melee the instigator's `AttackInjuredMeleeDamageModifier` when the victim
        is below 30 % health; for an instigating pawn that is in its injured bonus time, its `InjuredBonusDamageScale`
        (section 10); and the **amplify** attribute of the victim (`ReceivedAmplifyDamageModifier`, only when above
        1.0001): the extra damage it adds is also stored in `ExtraDamageDealtDueToAmplify`.
     3. **InstigatorDamageTypeReduction.** Multiplies by the instigator controller's per-element modifier chosen by the
        damage type definition's `DamageType` enum (`EDamageType`: 1 Incindiary, 2 Shock, 3 Explosive, 4 Corrosive, 5 Impact,
        7 Amp): `InstigatedIncindiaryDamageModifier`, `InstigatedShockDamageModifier`, `InstigatedExplosiveDamageModifier`,
        `InstigatedCorrosiveDamageModifier`, `InstigatedImpactDamageModifier`, `InstigatedAmpDamageModifier`; 1 for other
        values or no controller. Applied only if the damage type has no definition or has bit 3
        (`bUseStatusEffectInstigatorModifiersForDamage`) set, which every impact definition in the data does.
     4. **ExpLevelDifferenceReduction.** Reads the experience level of attacker and victim (the balanced-actor interface of
        each pawn). Equal levels: factor 1. Otherwise the difference (absolute) indexes a table (clamped to its length,
        entry = difference - 1) in the globals definition: `PlayerDamageScaleByLevelDifference` when the attacker is a
        player, `AIDamageScaleByLevelDifference` otherwise; an attacker of higher level uses the entry's
        `HigherLevelAttackerDmgScale`, of lower level `LowerLevelAttackerDmgScale`. Data: player table
        `HigherLevel` 1.0 everywhere, `LowerLevel` 1 (diff 1), 0.9, 0.8, 0.7, 0.6, 0.55, 0.5, 0.45, 0.4, 0.35, 0.3, 0.25,
        0.225, 0.2, 0.175, 0.15, 0.1, 0.05, 0.01 (diff 19, the last entry applies beyond); AI table `HigherLevel` 1.1 at diff
        1 rising by 0.1 per level to 2.2 at 12, 2.25 (13), 2.5 (14), 2.75 (15), 3.0 (16, applies beyond), `LowerLevel` 1.
        For an AI attacker an additional level-based factor (two helper functions of the attacker and victim levels and the
        instigator's level-dependent term) is multiplied in; **not read** (open).
     5. **Backstab and unsuspecting-target factors** (no summary field of their own; they only add skill-event tags 26
        and 25 to the dealt-events list): a melee-source hit on a victim facing roughly the same way as the attacker (yaw
        difference under 90 degrees) multiplies by the attacker's `MeleeAttackTargetFromBehindDamageModifier`; a hit by a
        victim-side check (the victim's inventory manager's "is the instigator aware" test false) multiplies by
        `AttackUnsuspectingTargetDamageModifier`.
     6. **Hit region lookup.** The hit region comes from the preset region (radius damage), else the interface
        `IHitRegionConsumer.GetHitRegionForTakenDamage` (bone name from `HitInfo` matched against the body class's
        `HitRegionList`, default `DefaultHitRegion`).
     7. **HitRegionReduction (critical hits and region scaling).** The critical flag is set when the region has
        `bCriticalHit`, there is an instigator and the damage source is not a status effect. A critical hit multiplies by
        the instigator player controller's `CurrentInstantHitCriticalHitBonus` (`CurrentMeleeCriticalHitBonus` when the
        source is melee), sets `bWasCrit` in the summary, and tells the instigator controller (the same virtual notification
        that the shield and health stages use for damage numbers; its identity was not resolved). Independently of crit,
        damage is multiplied by the region's per-impact damage modifier: the entry of the region's physical-material impact
        response table that matches the pipeline's impact definition, else the region's `DefaultImpactResponse` value.
        The base value of the crit attribute (and so the crit multiplier for Maya) was not found in the class defaults of the
        controller (the stored base is 0, so it must come from a modifier applied at start-up); **open**.
     8. **Projectile reflection** (bullet/impact hits on a victim whose region reflects; uses `BulletReflectionOffSelfChance`
        and the instigator-side reflectable chance; writes the `ReflectionData` and `ProjectileReflectionReduction`). Not
        relevant to the slice.
     9. **First/last shot in clip** (instigator player controller, only when `BulletFromClipType` is 1 or 2): damage times
        `FirstShotInClipBonusModifier` or `LastShotInClipBonusModifier`.
     10. **ShieldReduction.** Only if the victim has a damageable interface and shield strength above 0 (and the damage is
         not healing). Unless the damage source *penetrates* shields (section 7), the damage is converted to effective
         shield damage by the damage type's shield factor (`WillowDamageTypeDefinition.ShieldDamageModifier`, section 6)
         and the pawn damage-type modifier (1 when called without a pawn); the shield absorbs
         `min(effective damage, shield strength)`; the new shield strength is written back; `DamageDealtToShields` is that
         absorbed amount; the damage that carries on is `(effective - absorbed) / factor` (0 if the factor is not positive),
         i.e. the unabsorbed remainder in raw damage units. `ShieldReduction` is the difference of the working damage. The
         victim's recent-damage tracker gets the absorbed amount (shield side), the instigator controller gets its damage
         notification, and if the shield reached 0 the victim's shield-depleted handler runs (`OnShieldDepleted` with the
         instigator and damage type enum).
     11. **ProtectionTimerReduction.** If the victim implements the protection-timer interface: damage x
         `1 / (1 + X * 0.01)` where X is the interface's current "damage reduction percent" (0 unless a skill supplies one).
     12. **DamageSurfaceReduction.** The damage surface type (1 flesh, 2 armor, 3 shield; from the hit region's
         `DefaultDamageSurfaceType`, from the physical material of the hit, or the pawn default) selects the damage type's
         flesh/armor modifier (`GetSurfaceDamageTypeModifier`); with a hit region it is also multiplied by the victim's
         `GetPawnDamageTypeModifier` (the character resistance attribute of the damage type, e.g.
         `D_Attributes.DamageTypeModifers.IncendiaryImpactDamageModifier`, see section 6).
     13. **HitRegionCapReduction.** For regions with `bTrackDamage`, the hit-region consumer's `AddDamageToHitRegion` runs
         with the summary (independent region health; its result can lower the damage).
     14. **Averted death.** Victim pawns that support it: if health minus the current working damage would be 0 or less
         (within 0.0001), and a random roll (the C runtime `rand`, divided by 32767) is at most the victim's
         `PlayerAvertDeathChance` attribute, the damage becomes 0 and skill-event tag 22 is added to the taken-events list.
     15. **Protection timer cap.** For a player victim hit by an AI instigator with a protection timer interface: the
         timer starts when the hit would take health below `max health x GlobalsDefinition.ProtectionTimerThreshold` (0.5
         in the data, and the timer is not already running); while it is running, damage cannot take health below the
         pawn's `MinimumHealthMaintainedByProtectionTimer` (`max x ProtectionTimerMaintainedMaxHealthPct`, 0.1 in the data;
         duration `ProtectionTimerDurationInSeconds` 2 in the data). The reduction is written to a summary field.
  4. *Tail.* The victim's `NotifyDamageTaken` (interface) receives the summary, and the instigator player controller gets a
     "damage dealt" callback. The summary returned has `FinalDamage` = what remains for health, `PreviousHealth` and the
     flags (`bWasCrit`, `bWasInjured`, `bWasMaxShield`, `bWasMaxHealth`, `bWasOneShotKill`; the last three are set from the
     setup step) and `DamageSeverityPercent`.
- **Calls into script:** none directly (the instigator controller and victim interfaces are native overrides).
- **Calls other natives:** `GetHealth`/`GetMaxHealth`, `ResourcePool` getters/setters, the `DamageTypeDefinition` modifiers
  (section 6), the `WillowDamageSource` queries (section 7), the globals `EvaluateInitializationData`.
- **Constants / formulas:** as listed per stage. All damage arithmetic is single precision. The summary's per-stage fields
  are `InitialDamage`, `AIDamageScaleReduction`, `DamageSourceReduction`, `InstigatorDamageTypeReduction`,
  `ExpLevelDifferenceReduction`, `RecipientDamageTypeReduction`, `HitRegionReduction`, `ShieldReduction`,
  `IntrinsicArmorReduction`, `DamageSurfaceReduction`, `HitRegionCapReduction`, `ProtectionTimerReduction`,
  `ProjectileReflectionReduction`, `DamageSeverityPercent`, `DamageDealtToShields`, `ExtraDamageDealtDueToAmplify`,
  `FinalDamage`, `PreviousHealth`, `HitRegion`. A reduction is stored as (value before the stage - value after it); the
  field that does not map to a stage above (`RecipientDamageTypeReduction`, `IntrinsicArmorReduction`) was not matched to
  code (open).
- **Edge cases:** `WillowPawn.AdjustDamage` (script) wraps the call: an invulnerable pawn (`bIsInvulnerable`) gets damage 0
  and severity 0; momentum is rescaled by the surface type; if the caller passed no pipeline one is taken from the global
  pool and released afterwards; self damage from a causer is multiplied by the causer's `GetInstigatorSelfDamageScale`
  first; after the call a demigod pawn's damage is capped so health stays at least 1; a positive result sets
  `DamageSeverityPercent = damage / MaxHealth` and writes it into `FinalDamage`.
- **Implementer checklist:**
  - run the stages in the order above and store each stage's removal in its summary field;
  - shield absorbs `min(damage x shieldFactor, shield)` first and only the remainder (converted back by dividing by the
    factor) reduces health; sources that penetrate shields skip the stage;
  - a hit region with `bCriticalHit` multiplies by the controller's critical bonus unless the source is a status effect;
  - the pipeline object is reused per call; the victim's health is **not** changed by the pipeline (only by `TakeDamage`).
- **Open:** the AI extra level factor; `RecipientDamageTypeReduction`/`IntrinsicArmorReduction`; the crit base; the
  identity of the controller notification; the exact tag numbers' meaning (26 backstab, 25 unsuspecting, 22 averted death
  are taken from the stage that writes them); where `bWasLastDamageACriticalHit` is set (not in script; probably in the
  victim's `NotifyDamageTaken`, whose body was not resolved).

## 2. Actor.ActorTakeDamageInner

- **Reads:** the actor's `GeneratedEvents` array, `MostRecentDamageTaken`.
- **Does:** for every generated event that is a `SeqEvent_TakeDamage` (Kismet), runs its activation check with the damage
  (and the damage instigator), then stores the damage in `MostRecentDamageTaken`, then, if there is an instigator
  controller, calls that controller's damage-feedback virtual with the damage (a value strictly between 0.0001 and 1 is
  raised to 1 for the feedback).
- **Constants:** raise-to-1 band (0.0001, 1.0).
- **Implementer checklist:** call it with the post-pipeline damage, before health changes; Kismet `TakeDamage` events fire
  here (G10 owns their semantics).
- **Open:** the feedback virtual (damage numbers/hit markers).

## 3. Pawn.PawnCheckTakeDamagePreconditions

- **Returns true to abort.** Damage proceeds only when the pawn has authority, a pawn-level virtual (read as a dead/blocked
  test; the slot was not named) is false, and health is above 0 **or** the pawn `bCanBeInjured` (players). So a player at 0
  health is let through the check; a non-injurable pawn at 0 health ignores further damage.
- **Open:** the virtual's name.

## 4. Health and resource pools

- **Pawn.GetHealth:** the current value of the pawn's health pool if the pool reference resolves, else the plain
  `HealthVar`. **GetMaxHealth** returns the pool's current maximum (the "base maximum" when asked), else `HealthMaxVar`.
- **Pawn.SetHealth(v):** a value in [0, 1) is first rounded to 0; with a valid pool the value goes to the pool's
  `SetCurrentValue`, otherwise to `HealthVar`.
- **ResourcePool.SetCurrentValue(v):** does nothing unless the pool is active; clamps to [min, max] (if min exceeds max the
  value is min); pools of a resource with `bIntegerOnlyUpdates` truncate to an integer and keep the remainder; then it
  stamps the update time and calls the pool's changed virtual. **AddCurrentValueImpulse(d)** = `SetCurrentValue(current +
  pending impulse + d)`.
- **Health pools in the data:** player `D_Resourcepools.PlayerPools.HealthPool` (`StartWithMaxValue`, regeneration idle
  delay 0.5, rates default 0; skills/class mods add regeneration), shield `ShieldPool` (no base maximum; the equipped shield
  supplies it). Enemy pools use `GD_Balance_HealthAndDamage.ResourcePools.Pool_Health` with idle regeneration delay 45 and
  maximum `Init_EnemyHealth` = (class `Attribute_HealthMultiplier`, e.g. 5 for `CharClass_TargetDummy`) x
  (`Init_EnemyHealth_ByPlaythroughGlobal`, 1 on playthrough 1 and 2) x (`Init_BaseEnemyHealth`: a player-count boost x
  `Att_UniversalBalanceScaler` ^ the enemy's `AICharacterExperienceLevel`, minimum 1). The nested initialisers were not
  evaluated here; use the shared evaluator of [NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md) section 1.
- **Implementer checklist:** health pool writes clamp to [0, max]; fractional health below 1 reads as 0 after `SetHealth`;
  the pipeline never writes health, `TakeDamage` does (`health - damage`).
- **Open:** pool regeneration timing (its native update was not read); the shield pool's recharge rule (item data).

## 5. The OnTakeDamage behavior event (Pawn.NotifyTakeHit, WillowMind, AIClassDefinition.OnTakeDamage)

- `Pawn.NotifyTakeHit` hands the hit to the pawn's controller (or the driver's controller for a vehicle) through
  `Controller.NotifyTakeHit(InstigatedBy, HitPawn, HitLocation, Damage, DamageType, Momentum, Pipeline)`. The AI controller
  class (`WillowMind`) overrides it natively.
- **WillowMind.NotifyTakeHit does (in order):** nothing at all if the world has no AI context; registers the instigator
  with the AI's target/threat bookkeeping when there is an instigator; then, if the pawn has an AI class **with an AI
  definition**, raises the event:
  - instigator is not in a vehicle (null, or the instigating pawn is not driving): event **`OnTakeDamage`** on the AI
    definition (`GearboxFramework.AIDefinition`, no filter) and on the AI class definition
    (`WillowGame.AIClassDefinition`, **with a filter callback**);
  - instigator in a vehicle: **`OnVehicleTakeDamage`** (one extra object argument, the hit vehicle) on both;
  then calls the base controller handler.
- **Event arguments** (declaration order of `AIClassDefinition.OnTakeDamage`): `Instigator` (the damaging controller),
  `Damage` (the post-pipeline health damage that was just applied), `ShieldDamage` (`DamageDealtToShields` of the
  pipeline summary), `DamageSource` (the `class<DamageType>` the shot was fired with: a `WillowDmgSource_*` class such as
  Bullet, Rocket, Melee, Grenade, Skill, StatusEffect), `DamageType` (**the pipeline's damage type definition object**,
  e.g. `GD_Incendiary.DamageType.DmgType_Incendiary_Impact` for the Fire weapon's direct hit).
- **Filter** (only on the AI class raise): the event is rejected when `Damage + ShieldDamage` is **less than** the filter
  object's `DamageThreshold`; `EventFilter_OnTakeDamage` objects have class default 0, and the dummy's three have no
  threshold set, so every hit passes (a hit that does 0 damage and 0 shield damage also passes, 0 < 0 being false).
- **Fire dummy:** `CharClass_TargetDummy` carries the provider with the `FireDamage` sequence; that sequence compares the
  event's `DamageType` output with `DmgType_Incendiary_Impact`. The damage over time of the incendiary status effect
  reports `DmgType_Incendiary_Status` (section 8), which is **not** equal, so only the direct hit completes the objective
  (consequence of the data, UNVERIFIED in game). The dummy has an AI definition (`AIDef_TargetDummyBot`), so the raise
  happens.
- **Implementer checklist:** raise after health is reduced; pass the damage type **definition** (not the source class) as the
  `DamageType` output and the source class as `DamageSource`; apply the threshold test on `Damage + ShieldDamage`; a hit
  with a vehicle-driving instigator raises the vehicle variant instead.
- **Open:** the instigator-vehicle test's exact virtual; whether the controller registration step has side effects needed
  by the slice (target queues).

## 6. DamageTypeDefinition natives and data

- `GetSurfaceDamageTypeModifier(surface, instigator)`: surface 1 -> `FleshDamageModifier`, 2 -> `ArmorDamageModifier`,
  3 -> `ShieldDamageModifier` (each an `AttributeInitializationData` evaluated with the instigator as context); any other
  surface -> 1.
- `GetPawnDamageTypeModifier(pawn)`: 1 with no pawn, otherwise the value of the pawn's attribute
  `CharacterDamageTypeModifierAttribute` (class default 1 where the attribute has no override).
- `RecordRecentDamage(tracker, damage, instigator, damageTypeDef, bWasShieldDamage)`: adds the damage to the tracker's total
  and to either its shield-side or health-side sum.
- `IsHealingDamageType` (not read): the healing definition is `GD_Healing.DamageType.DmgType_Healing_Impact_NoDoT` (type 6).
- Class defaults: flesh/armor/shield modifier 1, `UpwardMomentumScale` 0.4, `RigidBodyMomentumScale` 0.1, radius falloff
  `MaxDamageRadius` 0.2, `MinDamageRadius` 0.8, `MinDamagePercent` 0.2.

Elemental effectiveness table (data in `Startup.upk`; "1" = class default; playthrough 1 / playthrough 2 constants from the
`..._ByPlaythrough` initialisers):

| Damage type definition | EDamageType | Flesh | Armor | Shield | Character resistance attribute |
|---|---|---|---|---|---|
| DmgType_Normal | 5 Impact | 1 | 0.8 | 1 | NormalImpactDamageModifier |
| DmgType_Incendiary_Impact (and _NoDoT, _Status) | 1 | 1.5 / 1.75 | 0.75 / 0.4 | 0.75 / 0.4 | IncendiaryImpact / IncendiaryStatusEffect DamageModifier |
| DmgType_Shock_Impact (_NoDoT, _Status) | 2 | 1 / 1 | 1 / 1 | 2 / 2.5 | ShockImpact / ShockStatusEffect |
| DmgType_Corrosive_Impact (_NoDoT, _Status) | 4 | 0.9 / 0.6 | 1.5 / 1.75 | 0.75 / 0.4 | CorrosiveImpact / CorrosiveStatusEffect |
| DmgType_Explosive_Impact | 3 | 1 | 1 | 0.8 | ExplosiveImpactDamageModifier |
| DmgType_Amp_Impact / _Status | 7 | 1 | 1 | 1 | AmpImpactDamageModifier |

(`DmgType_Amp_*` is read as the slag type, **UNVERIFIED**; Phaselock damage uses the skill source.) Which definition a weapon's
shot carries is in [NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md) (element parts).

## 7. WillowDamageSource natives

- `ShouldDamageSourcePenetrateShields(source, instigator)`: true only for an instigating **player controller**, when the
  source class is the bullet source (or derives from it) and a random roll (percent) is at most the controller's
  `PercentChanceInstigatedBulletDmgIgnoresShields`.
- `CanDamageSourceBeAbsorbedByShields(source)`: true for bullet-derived sources and one other class (read as the rocket
  source); grenade, melee, status effect and skill damage are not "absorbed" in this sense. In the shield stage this flag
  has no numeric effect in the reading (the multiplier it feeds is 1); **open**.
- Source class identification (damage source class -> which attribute pair is used) was recovered from the attribute
  pairs each branch reads: bullet (Instigated/ReceivedBullet), status effect, grenade, melee, rocket, skill.

## 8. Status effects (incendiary)

### 8.1 Chance roll: StatusEffectsComponent.RollChanceForStatusEffect
- Called from `WillowPawn.TookDamageFromEnemy` (and `TookDamageFromFriendly` with a friendly-fire chance modifier) only
  when: health is still above 0, the hit is not part of radius damage, the victim is not driving a vehicle, it has a
  `StatusEffectComp`, and the pipeline's damage type definition has a `StatusEffect` (Incendiary impact has
  `Status_Incendiary`; `_NoDoT` variants and `_Status` types have none).
- Gate: the component must be enabled, not owner-dead, allowed to apply effects, the effect definition non-null, and the
  target's `CanReceiveStatusEffects` true. The probability is clamped to [0, 1]; the roll is `rand / 32767` and the effect
  applies when the roll is at most the chance (a debug "guaranteed" switch bypasses it).
- Chance = (effect's chance base for the **damage surface** of the hit: `DamageSurfaceChanceModifiers` entry, Incendiary:
  surface 0 -> 20, flesh 1 -> 30, armor 2 -> 15, shield 3 -> 15) x (the damage type's chance factor evaluated for the
  instigator context) x (a level-difference factor from the globals `StatusEffectChanceBasedOnExpLevelDifferences`) x
  (the weapon/projectile status chance modifier carried in `ChanceModifier` and the instigator's chance modifiers) x (the
  victim's resistance: `IgniteChanceResistanceModifier` via the effect's `TargetStatusEffectChanceModifier`), all x 0.01.
  The identities of some multiplicative terms (a source-type term and a friendly-target term, `FriendlyStatusEffectChanceModifier`
  0.5) were not fully matched; **open**.
- A satisfied roll calls the script `StatusEffectsComponent.ApplyStatusEffect`.

### 8.2 Apply (script)
Duration = `BaseDuration` (Incendiary 5 s) x the target's duration modifier (`TargetStatusEffectDurationModifier` ->
`IgniteDurationResistanceModifier`); an effect with duration 0 and not infinite is not applied. The new active effect
records instigator, causer, `DamageSource = WillowDmgSource_StatusEffect`, the hit region and, for damage-over-time
effects, `DamagePerSecond` = the causer's `GetStatusEffectBaseDamage(instigator)` x the instigator's
`InstigatedStatusEffectStatusDamageModifier` x the per-element status modifier (`InstigatedIncendiaryStatusDamageModifier`
for fire) x the target's `TargetStatusEffectDamageModifier` (`IncendiaryStatusEffectDamageModifier` attribute) x any
region override. The weapon's status damage number is in [NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md). Re-application
refreshes the effect (elapsed and accumulated time reset). Spreading uses `BaseSpreadDistanceFromSource` 48,
`BaseSpreadTimeInterval` 1 and per-surface spread chances (not needed for the slice). `OnApplication` and `OnDurationEnd`
behaviors run (Incendiary: activate/deactivate the `Skill_IncendiaryEffects` skill, audio, a dialog event).

### 8.3 Damage over time (native update of the component, every frame)
Each active damage-over-time effect accumulates elapsed and "accumulated" time (the last frame is clipped to the remaining
duration). When the accumulated time exceeds **0.33 s** (or the effect ends) it deals `accumulated time x DamagePerSecond`
through a normal damage call with source `WillowDmgSource_StatusEffect` and the status effect's own damage type definition
(`DmgType_Incendiary_Status`), then subtracts 0.33 from the accumulator (a final partial tick is paid in full). Because it
goes through the same pipeline, the victim sees `OnTakeDamage` with `DamageSource` = the status-effect source and
`DamageType` = `DmgType_Incendiary_Status`.
- **Implementer checklist:** chance clamp [0,1], per-surface base chance 20/30/15/15, duration 5 s, tick 0.33 s, damage
  per tick = elapsed x DPS, damage type of ticks = the `_Status` definition, source = status-effect source.
- **Open:** exact chance factor set; spreading; the effect's own stat/skill hooks.

## 9. Death of an AI pawn

Order in script after `HandleHealthDepleted` (for pawns that cannot be injured):
1. `WillowPawn.HandleHealthDepleted`: shots from a projectile get their killed-enemy/friendly/neutral callbacks;
   then `MissionTracker.NotifyPawnDied(pawn, killer, damage source, pipeline damage type, causer, crit flag, hit info)`
   (the mission tracker's pawn-died notification, **before** the death itself; owned by G9/C1);
   then the engine's `Pawn.HandleHealthDepleted` (guarded by `bPlayedDeath`, adds killer feedback, `SetKillInstigator`,
   calls `Died`).
2. `WillowAIPawn.Died` tells a human killer's pawn `KilledEnemy(victim)` (this also drives the player's second wind while
   down), handles player-master/thoughtlock credit and calls the base.
3. `WillowPawn.Died`: detaches pickups; `DropLootOnDeath` (see [NATIVE_LOOT.md](NATIVE_LOOT.md)) before anything else, when
   the pawn has an inventory manager; `StatusEffectComp.OwnerDied`; `MyDeathDef.OnKilledBy(ConsumerHandle, Killer)` (the
   behavior event of that name on the body class death definition's provider); unlock-on-death achievement; action skill
   `OnActionSkillOwnerDied`; uncharm; unregister obstacle; then `Pawn.Died`.
4. `Pawn.Died`: a mutator `PreventDeath` can veto (health set to at least 1); `SetHealth(min(0, health))`;
   `DestroyHealthPool`; `SeqEvent_Death` triggered (after `KismetDeathDelayTime` if positive); latent actions aborted;
   driver/weapon handling (the weapon is thrown on death when the game allows and `bDropOnDeath`); `Game.Killed(Killer,
   victim controller, pawn, damage type, pipeline)`; inventory `OwnerDied`; `PlayDying`; opportunity notified
   (`TellOpportunityPawnIsDead`); allegiance parent/children removed.
5. `WillowGameInfo.Killed` (script): credits the killing player (or master), computes kill XP with
   `WillowExperiencePipeline.CalculateExperiencePointsForKill(killer, killed, crit, damage source, damage type)` and awards
   it with `AwardCombatExperience` (-> `AwardCombatExperienceToParty`), fires skill events and `AIDefinition.OnKilledPawn`.
6. `PlayDying`/`PlayDeathAnim` (presentation): tech deaths are matched by damage type (`BodyClassDeathDefinition.TechDeaths`;
   the dummy has Acid/Shock/Fire tech deaths, Fire = `Anim_TargetDummy_DeathFire` for the incendiary types), gibs by
   `GibTriggers`; `OnPlayDeathPizazz` and `OnDeathNonGib`/`OnDeathGib` (with the killer) are raised on the body class death
   definition's provider.
- **Experience natives (structure only, not read):** the base kill XP is `GlobalsDefinition.BaseEnemyExperienceFormula`
  (`Init_BaseEnemyExperience` x the pawn's `Attribute_ExperienceMultiplier`; the dummy's is `XPMultiplier_01_Chump`), then
  two further adjustments (level and damage-type/crit modifiers), distributed to the party with a per-player factor
  truncated to an integer. The Fire mission does not kill the dummy, so this is off the gate; the amount rule stays open.
- **Dummy specifics:** `CharClass_TargetDummy` has health multiplier 5, `bCountsTowardDamageAndKillStats` false,
  `Flag_Skills_DisablePhaseLock` set true; hit region `HitRegion_Head` has `bCriticalHit` and region name `BanditHead`
  (bone `Head`). The slice host's 20000 health stand-in is a host choice (see section 11).

## 10. The player: down state, second wind, death, respawn

All of this is script; the natives involved are the health/pool natives above, `WillowPawn.SetSecondWindReason`,
`ClearSecondWindReason`, `GetRevivePct/SetRevivePct`, `IsInjuredDead`, `WillowPlayerPawn.CheckLowHealthState` and
`GetMinimumHealthMaintainedByProtectionTimer` (not read beyond their role).

### 10.1 Entering the down state ("fight for your life")
When a player's health reaches 0, `WillowPawn.HandleHealthDepleted` calls `PlayInjured` (the pawn can be injured). The
`injured` state's `BeginState` then: marks `bIsInjured`; clears the second-wind reason; applies the injured definition's
attribute modifiers (health and shield regeneration disabled); runs its injured behaviors; crouches; ends zoom; clears
trade; clears status effects and disables the component; kills skills off; plays the player-down dialog events; tells
other players (`ServerNotifyIWentDown`, statistic times injured); and sets the injured state to 1.
The injured definition for Maya is `CharClass_Siren.CharacterInjuredDefinition` =
`GD_PlayerShared.injured.PlayerInjuredDefinition` over the class defaults of `InjuredDefinition`.

Values (data; override or default):

| Field | Value |
|---|---|
| `bDoBleedout` | true |
| `BaseRejuvenateDelay` (bleed-out base, one player) | 12 s (class default 10) |
| `BaseMultiplePlayersRejuvenateDelay` | 20 s (default 30) |
| `MaxDesiredSuccessiveInjuries` / `MaxTimeBetweenInjuries` | 3 / 30 s |
| `ReviveDuration` / `AutoReviveCheckDelay` | 5 s / 2 s |
| `InjuredMovementSpeed` | 150 (default 75) |
| `InjuredBonusEnabled`, `InjuredBonusTimePercent`, `InjuredBonusDamageScale` | true, 10, 1.5 |
| `RejuvenatedHealthPctOfMax` / `RejuvenatedShieldPctOfMax` | 0.25 / 0.9 (default 1) |
| `ResurrectedHealthPctOfMax` / `ResurrectedShieldPctOfMax` | 1 / 1 (defaults) |
| `RevivedHealthPctOfMax` / `RevivedShieldPctOfMax` | 0.3 / 0.9 (shield override) |
| `InjuredWeaponPutDownTime` / `InjuredWeaponEquipTime` | 0.1 / 0.2 |
| Globals `PctOfBleedoutToSecondWind` | 0.25 (statistic only) |

### 10.2 While down
- The bleed-out total is `BaseRejuvenateDelay` (solo) x the pawn's `TimeToBeRevivedMultiplier` attribute, then reduced by
  `total / MaxDesiredSuccessiveInjuries x NumSuccessiveInjuries` and floored at 1 s (solo defaults: 12 s, 8 s, 4 s, then
  1 s). `NumSuccessiveInjuries` grows by one when the previous injury was less than `MaxTimeBetweenInjuries` (30 s) ago,
  else resets to 0, and is also reset by a resurrection.
- The timer (`flInjuredTargetedTime`) only starts after the "switch to sidearm" special move ends (state 2, targeted). It
  runs at the game's delta time unless the pawn is being revived (then the revive time runs instead and movement stops).
- In the last `InjuredBonusTimePercent` percent of the bleed-out (remaining / total <= 0.10) the injured bonus is on: the
  stage of the pipeline multiplies this pawn's outgoing damage by `InjuredBonusDamageScale` 1.5.
- Further enemy damage is ignored by a downed player (`WillowPlayerPawn.injured.TakeDamage` only plays a camera shake);
  `injured.HandleHealthDepleted` does nothing.
- **Second wind from a kill:** `KilledEnemy` while down sets reason `ESECONDWIND_KilledEnemy`, runs the killed-enemy
  behaviors, updates statistics; the next tick sees a reason set and calls `GoFromInjuredToHealthy`
  (unless a pre-death recovery animation delays it). Other reasons: leveled up, partner revived, resurrect skill, auto
  revive (`CheckShouldBeAutoRevived` every `AutoReviveCheckDelay`: if the player's `ShouldGetResurrected` attribute is
  positive and a uniform roll is below it).
- **GoFromInjuredToHealthy:** health is raised to `max x RejuvenatedHealthPctOfMax x RevivalHealthMultiplier` only if it is
  below that (a missing definition falls back to 25 %); shield likewise to `max x RejuvenatedShieldPctOfMax`; the injured
  and injured-dead states are cleared, targetable again, the injured modifiers removed, `RecoveredBehaviors` run, a skill
  event is sent and the protection timer is armed. Revival by a partner uses the Revived percentages after `ReviveDuration`.

### 10.3 Bleed-out and death
When the timer exceeds the bleed-out total (and `bDoBleedout`): optionally the pre-death animation; otherwise the action
skill is deactivated, hardcore mode would run `PermadeathBehaviors` (not used), `InjuredDeadState` becomes 1 (start), the
body's injured-death animation plays, the player is recorded as killed, the death sequence starts (camera pull-back using
`InjuredDeadCameraStartDistance` 50 -> `EndDistance` 350 over 1.5 s in the data, HUD hidden, input ignored, crosshair
off). A fall into a kill volume or a forced death (`bCausePlayerDeath`) skips the bleed-out and enters the dead state at
once. When the dead camera ends (`EndInjuredDeadCamera`) the state's `InjuredRespawn` runs; a gib/crush
(`CheckGoToDyingState`) calls respawn with reason Gibbed.

### 10.4 Respawn
- `InjuredRespawn` -> `WillowGameInfo.HandlePlayerDeathResurrection(deadPlayer, reason)`: if
  `bResurrectAllPlayersWhenOneDies` and the reason is not Gibbed, the AI is reset and all players are resurrected
  (`ResurrectAllPlayers`, other players get reason OtherDied); else, for a single player (`EffectiveNumPlayers` 1), the AI
  is reset (`ResetAI`) and the pawn's `ResurrectPlayer(ERR_IDied)` runs. Then the injured definition's
  `ResurrectedBehaviors` (five pool refills) run and a skill event fires.
- `EResurrectReason`: 0 Unknown, 1 IDied, 2 OtherDied, 3 FellOutOfWorld, 4 LDRes, 5 Gibbed, 6 OutsideWorldBounds,
  7 LevelTravel. Falling out of the world and leaving world bounds call `ResurrectPlayer` directly with reason 3/6.
- **Station selection** (`WillowPlayerPawn.GetBestPlayerPlacementPoint`): the global replication info's
  `ActiveRespawnCheckpointTeleportActor` if set; else the first `TravelStation` with `bIsCurrentlyActive` or
  `bShouldBeActive` (its `TeleportDest`); else the **nearest** `TravelStation` whose `CanResurrectHere(bLevelTravel)` is
  true; else the nearest station of any kind. Distance is the straight 3D distance from the pawn. **Confirms** the slice's
  decode ([SLICE_WORLD_PLACEMENT.md](SLICE_WORLD_PLACEMENT.md) 1e: checkpoint actor, then first active station, then
  nearest capable station, then nearest other); the one reading detail that stays open is whether "first active station"
  stops the search at the first match (read as a break). The exit point comes from `TeleporterDestination.GetNextExitPoint`
  (round robin; not read here, G3).
- `ResurrectPlayer`: resets `NumSuccessiveInjuries` to 0, clears the injured screen fade, drops the player from a vehicle,
  takes a holding-cell destination unless skipped (`bSkipHoldingCell`, or reasons other than IDied/FellOut/Outside/Level/
  Gibbed paths), de-rezzes gear and calls `ResurrectAtLocation(holding, destination exit point, reason, deadPRI)`. A
  destination exit point below the level's `KillZ` makes it skip the holding cell and use the next exit point.
- `ResurrectAtLocation`: for a down or dead pawn (or reasons 1, 2, 3, 6) it calls `GoFromInjuredToHealthy`, then sets
  health to at least `max x ResurrectedHealthPctOfMax` (1.0 -> full) and the shield to at least `max x
  ResurrectedShieldPctOfMax` (1.0) (a missing definition: health 25 %), marks `bAwaitingInjuredRespawn`, plays the
  teleport effect, teleports the pawn (holding cell or directly, with a no-fail placement fallback), sets the anchor,
  and, when the destination is a station, runs the station's `OnArrivedAtStation` custom event.
- **Currency penalty:** `WillowPlayerPawn.ShowRespawnDialog` (scheduled 2.0 s after teleporting when the pawn is not
  entering the awaiting-respawn state; for a station respawn the state's `AwaitingRespawnDisplayRespawnCost` schedules it,
  called from the station's Kismet; the caller was not traced). Cost = `GlobalsDefinition.TeleportCost` evaluated with
  the controller as context = `GD_Balance_Inventory.Commerce.DeathPenaltyCost` = **0.07 x CreditsOnHand**, rounding mode
  1 (half up); multiplied by `OtherPlayerDiedCostMultiplier` when the reason was OtherDied (the class default is 0 and the
  globals instance does not set it, so that fee would be waived; reading only). The amount charged is `min(cost, cash)` as
  an integer, subtracted from the player's money (`AddCurrentOnHand(0, -amount)`), and the HUD shows the "fee" or "fee
  waived" text when 0.
- **Health/shield on respawn:** full health and full shield (`ResurrectedHealthPctOfMax` 1, shield 1) plus the five
  refill behaviors; max health follows the level formula of [NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md) section 4.
- **Mission state** is not touched (nothing in these paths calls the mission tracker); only `NotifyPawnDied` for
  non-injurable pawns reaches it.
- **Implementer checklist:** zero health -> down state, not death; bleed-out 12 s solo with the successive-injury
  reduction; kill during bleed-out = second wind (health 25 % of max if lower, shield 90 %); bleed-out expiry -> death
  sequence -> `HandlePlayerDeathResurrection` -> station choice above -> full health and shield -> fee 7 % of cash (rounded
  half up, at most cash) about 2 s after arrival.
- **Open:** the duration of the death camera before respawn; the fee timing for the station path; hardcore mode.

## 11. Slice consequences (Fire mission)

- The dummy's objective is completed by an `OnTakeDamage` whose `DamageType` output is
  `GD_Incendiary.DamageType.DmgType_Incendiary_Impact`; this is the weapon shot's **pipeline damage type definition**.
  `DamageSource` is the bullet source class. Not the DoT type.
- Damage must be dealt: the event fires after health is lowered, with `Damage + ShieldDamage` passing the (0) threshold;
  a 0-damage hit still raises it. A host that never lowers the dummy's health can still raise the event, but the real
  order is damage first, event second.
- The incendiary chance (20/30/15/15 by surface) is irrelevant to completion.
- The host's dummy health (20000) and a host "death" are not Gearbox behavior; the data gives enemy health through
  `Init_EnemyHealth` with multiplier 5.

## Not read yet
- `WillowPawn.NotifyDamageTaken`, `AddDamageToHitRegion`, `GetHitRegionForTakenDamage`, `HealDamageOnHitRegion` bodies
  (interface implementations on the pawn; they were not resolved).
- The controller damage-feedback virtual; the AI extra level factor; `IntrinsicArmor`; protection-timer natives of the
  player pawn; `WillowPawn.CheckInjured` (AI crawl).
- Resource pool regeneration update and the shield item's recharge rule (item data, `AWillowShield`).
- `WillowExperiencePipeline` amount rule (both natives, structure only).
- The status chance's remaining factors; status spreading; `GetStatusEffectChanceModifier`/`BaseDamage` natives.
- `ConvertDamageToHealing`, `IsFriendlyFire`, healing damage handling; vehicle damage; interactive-object damage.
- Crit multiplier base values; `bWasLastDamageACriticalHit` setter.
- `TeleporterDestination.GetNextExitPoint` (G3), `TravelStation.CanResurrectHere` and station activation
  (`ReplacePreviouslyActivatedStation`).

## Corrections to earlier notes
- [SANCTUARY_RPG_MISSION.md](SANCTUARY_RPG_MISSION.md) ("`DamageSource` is still passed empty: its stock value is not
  decoded"): the stock value of `DamageSource` is the damage **source class** of the shot (for a gun, the bullet source
  `WillowDmgSource_Bullet`; rocket, melee, grenade, skill and status-effect sources exist), and the `DamageType` output is
  the pipeline's `WillowDamageTypeDefinition` object. The host's mapping "fire damage -> DmgType_Incendiary_Impact" is the
  right object for `DamageType`.
- [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) (open item "`EventFilter_OnTakeDamage` evaluation"): the
  AI class raise **does** pass a filter callback; it rejects when `Damage + ShieldDamage < DamageThreshold`. The AI
  definition raise and the vehicle variants pass none. The dummy's filters (threshold 0) reject nothing.
- [SANCTUARY_RPG_MISSION.md](SANCTUARY_RPG_MISSION.md) respawn: the station rule is confirmed (section 10.4). The host's
  "respawn restores formula health" matches `ResurrectedHealthPctOfMax` 1.0; the host does not charge the 7 % fee or run the
  bleed-out (a missing step, not a contradiction).
