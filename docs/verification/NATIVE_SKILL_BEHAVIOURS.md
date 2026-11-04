# Native skill behaviours: Maya's Phaselock variants, kill skills and capstones (2026-11-03)

AI-assisted (Claude). Analyst note for the UE host implementer. Everything here was read from the installed
packages (data) or from UnrealScript bytecode (script). Nothing was observed in the running game, so **every rule
below is UNVERIFIED in the game** until a bl-sdk capture confirms it. Native bodies were not read (no Ghidra run).
Raw tool output stays in `local/skills/raw/` (implementers must not open it).

Confidence labels per rule:
- **read from script**: the UnrealScript function was read (control flow known).
- **read from data**: object properties / behavior graphs / behavior variable values were read.
- **UNVERIFIED guess**: the deciding code is native and was not read, or a data layout is fitted.

## 0. Shared building blocks (read these first)

### 0.1 Damage and status formulas used by the skills (read from data)
Let `L` = Maya's experience level and `S` = the value of attribute `Att_UniversalBalanceScaler` (see
NATIVE_PROGRESSION; the slice already evaluates it for health).
- `PlayerSkillDamage` (`Init_PlayerSkillDamage`) = 8 x S^L.
- `PlayerSkillStatusEffectDamage` (`Init_PlayerSkillStatusEffectDamage`) = 1 x S^L.
- `PlayerMeleeDamage` (`Init_PlayerMeleeDamage`) = 2.5 x (8 x S^L + `InstigatedMeleeDamageAddedBuff`) (offset inside the
  multiplier, NATIVE_PROGRESSION section 1 rule).
- `ProjectileDamage` (`Init_ProjectileDamage`) = 8 x S^(ProjectileLevel), minimum 6.
- An `AttributeInitializationData` with a scale constant `k` is the formula value times `k`.

### 0.2 "Grade of skill X" inside behaviors (read from data; the context-failure case is an UNVERIFIED guess)
Behavior graphs never read effect rows; they read attributes named `GD_Siren_Skills.SkillGradeModifiers.<Skill>`
(Converge, Ruin, SubSequence in the Siren package; Backdraft, BlightPhoenix, Elated, Helios, Immolate, Recompense,
SweetRelease in `Startup.upk`). Each is an int attribute resolved as: Maya's controller, then the active skill
instance of that definition, then its `Grade` property. So the value is the skill's current grade (at least 1 when
it exists, NATIVE_SKILLS 4.5). When Maya does not own the skill, no skill instance exists, the context chain fails and
the attribute falls back to its constant, 0. Implement as: `GradeOf(skill) = grade if the skill is active else 0`.

### 0.3 How behavior numbers are wired (data layout fitted, UNVERIFIED)
- A behavior property that is linked to a behavior variable takes the variable's value instead of its own
  property (src/behavior.hpp rule). Several radii below are 650 although the behavior's own property says 400 or 800:
  the linked variable wins.
- Behavior variable values were decoded with the fitted layout of BEHAVIOR_DATA_DECODE.md (Attribute variables give
  constant, attribute, init definition, scale). Math variables (`BVAR_BinaryMath`) hold two operand variables and an
  operation code; the code minus 2000002 matched the `EBinaryMathOperation` order (Add 0, Sub 1, Mul 2, Div 3, Pow 4,
  Rand 5) consistently in five uses. Fitted, UNVERIFIED.
- `Behavior_CompareFloat` outputs: 0 = A less than B, 1 = equal, 2 = A greater than B. Every upgrade gate below is
  "GradeOf(skill) compared to 0, continue on output 2", i.e. "Maya owns the skill". The meaning of outputs 0/1/2 is
  inferred from what each branch does (UNVERIFIED but consistent across all gates).
- `Behavior_Conditional` with one condition: output 1 = condition true, output 2 = false (inferred, UNVERIFIED).
- In the Phaselock action-skill graph the named variable `Siren` is Maya and `Target` is the selected/lifted pawn.
  `CauseDamage` with radius damage is centred on its target context.

### 0.4 Phaselock helper skills that the upgrades lean on (read from data)
- `Skill_Phaselock_CooldownManager` (on Maya): effect 0 = `ActiveSkillCooldownConsumptionRate` PreAdd -1 (stops the
  cooldown drain), effect 1 = `Att_EnemyIsPhaseLocked` PostAdd +1. Activated by `OnSelectedTarget` (t = 0 of a hit
  cast), deactivated by `OnReleasedTarget`. So **`Att_EnemyIsPhaseLocked` is 1 from target selection to release**,
  for lifted and for charmed targets. This is the condition of Wreck, Elated, Chain Reaction.
- `Skill_Phaselock_DiminishingReturns` (on the released target): 25 s timed, `PhaselockTimeScale` Scale -0.4.
- `Subsequence_Feedback` (on Maya): `ActiveSkillCooldownConsumptionRate` PreAdd -1 while active (holds the drain),
  HUD slot 8.
- `ThoughtLock_Feedback`, `Ruin_Feedback`, `Recompense_Feedback`, `CloudKill_Feedback` (5.5 s timed),
  `Backdraft_Feedback`, `Skill_DetonateAvailable`: no effect rows that matter and empty behavior providers; they are
  HUD tracked-skill icons (and Cloud Kill's internal cooldown, see 4.2). `ThoughtLock_Feedback` deactivates itself when
  `Att_EnemyIsPhaseLocked` is not greater than 0.

### 0.5 Phaselock event points used below (read from script; NATIVE_ACTION_SKILL section 7/10 has the timeline)
- t = 0: `PhaseLockTarget` fires `OnSelectedTarget`, sets timers (lock at +0.7 s, outro at SkillDuration - 1.1,
  EndSkill at SkillDuration + 1), decides charm, lifts (unless charmed), Helios particle.
- t = 0.7: `LockTarget`: charmed -> `OnCharmTarget`; otherwise bubble + `OnTargetBecomesLocked` (which raises the
  custom event `Phaselocked`).
- every tick: if the held pawn is incapacitated (`IsLiftedPawnIncapacitated`: not alive-and-well, or staggered while
  not charmed) -> `InterruptPhaseLock` -> `OnKilledTarget`.
- Target not liftable -> `OnTargetBlocked`; friendly downed player with Res -> resurrect; nothing -> fizzle.

## 1. Phaselock variants

### 1.1 Converge (Motion, max grade 1)
- **Trigger:** `OnSelectedTarget` (also on `OnTargetBlocked`), gate `GradeOf(Converge) > 0`.
- **What it does:**
  1. At +0.6 s: particle `Part_Converge` on the target.
  2. At +1.25 s: radius damage centred on the target: radius 2000, no falloff, no trace test, momentum -110000
     (negative = pulled toward the target), damage = 1 x PlayerSkillDamage, status chance 0, no damage type set,
     instigator Maya.
  3. Right after: the target performs a melee "attack" with a MeleeDefinition of radius 2000, height 256, cone 360.
     Without Thoughtlock it uses `Converge_MeleeDefinition` (affects the target's *friends*, i.e. other enemies; on hit
     it only spawns a particle). With Thoughtlock (Maya has it) it uses `Converge_MeleeDefinition_WithThoughtlock`
     (affects the target's enemies, which after the charm are the other enemies; on hit a particle).
- **Numbers:** 2000, 256, 360, -110000, 0.6 s, 1.25 s from the behavior graph; damage scale 1. Effect row 0 (7 / +7 per
  grade on a dummy attribute, target None) is presentation only and unused by the logic.
- **Blocked target:** if Converge is owned (and Ruin is not), the blocked chain waits 1.45 s before deactivating
  Phaselock, so the pull still happens.
- **Edge:** the melee definitions deal no damage of their own as far as data shows (only particles); whether
  `MeleeDefinition.MeleeAttack` (native) adds damage is UNVERIFIED.
- **Confidence:** read from data (graph, values); pull physics UNVERIFIED guess (native damage momentum).

### 1.2 Ruin (Cataclysm capstone, max grade 1)
- **Trigger:** gate `GradeOf(Ruin) > 0`, reached from three places: the `Phaselocked` custom event (lock at 0.7 s),
  the Thoughtlock charm chain (after `NotifyThoughtLockStatus`), and the blocked-target chain.
- **What it does (from the trigger, absolute times):**
  - +1 s: activate `Ruin_Feedback` (HUD), particle `Part_Ruin_Pulses` on the target, then radius damage centred on
    the target: **Slag** (`DmgType_Amp_Impact`), radius 650, damage = 7.2 x PlayerSkillStatusEffectDamage, status
    damage = 7.2 x PlayerSkillStatusEffectDamage, status chance 9001 (always applies); sound `Ak_Play_FX_Maya_Ruin`.
  - +2 s: same pulse with **Shock** (`DmgType_Shock_Impact`).
  - +3 s: same pulse with **Corrosive** (`DmgType_Corrosive_Impact`).
  - +4 s: deactivate `Ruin_Feedback`.
- **Blocked target:** if Ruin is owned the blocked chain waits 3.2 s, deactivates `Ruin_Feedback`, then deactivates
  Phaselock.
- **End:** `Skill_Phaselock`'s own `OnDeactivated` deactivates `Ruin_Feedback` (so pulses scheduled after the skill
  ends still run, only the HUD icon goes; whether delayed behaviors survive the action skill's destruction is UNVERIFIED).
- **Dead script path:** `LiftActionSkill` also has a script Ruin (state 4, `RuinDamageScalar`, `ApplyRuin` staggers the
  target and fires `OnRuinImpact`: incendiary radius 1000, 50 x PlayerMeleeDamage). It needs the archetype's
  `RuinSkill`, which is **not set** on `ActionSkill_Phaselock`, so it never runs. Do not implement it.
- **Confidence:** read from data (graph and variables) and read from script (dead path).

### 1.3 Thoughtlock (Motion capstone, max grade 1; charm)
- **Data rows:** row 1 `Cooldown_Phaselock` PostAdd +3 (cooldown 13 -> 16 s); row 2 `Att_Phaselock_Duration`
  PostAdd +4 (start grade 0; lock 5 -> 9 s before Suspension); row 0 is presentation.
- **Script (`PhaseLockTarget`):** `CanCharmTarget` is true when the skill manager reports `ThoughtLock` active for
  Maya (server only). Then `bCharmed` is set and **the target is not lifted** (no `LiftTarget`, no unstagger); Helios
  is still checked. Every Phaselock on a liftable target is a charm while Thoughtlock is owned.
- **At t = 0 (`OnSelectedTarget`, condition "Thoughtlock active"):** particle `Part_Thoughtlock_StartUp` on the
  target's `FX_Brain` socket (saved as `CharmStartParticle`), AI hold on the target (reason `Charima`).
- **At 0.7 s (`LockTarget`):** for a charmed target no bubble and no `OnTargetBecomesLocked`; instead
  `OnCharmTarget`: particle `Part_Thoughtlock` (saved `CharmParticle`) -> `Charm` the target (native
  `WillowPawn.Charm`, owner = the behavior's default owner context, presumably Maya) -> release the AI hold -> threat
  priority +0.5 on the target (PostAdd on `Attribute_Threat_Priority`, so enemies prefer it) -> custom event
  `ThoughtLockStart` -> activate `ThoughtLock_Feedback` on Maya -> `NotifyThoughtLockStatus` (sets the target's
  `ThoughtlockMaster` to Maya's controller) -> Ruin gate; also remove `CharmStartParticle`.
- **While charmed:** `IsLiftedPawnIncapacitated` ignores stagger (only "not alive and well" ends it). Death ->
  `InterruptPhaseLock` as usual.
- **Release (`ReleaseTarget`):** no `DropTarget` for a charmed pawn; `OnReleasedTarget` runs: AI flag cleared,
  cooldown manager off, `DiminishingReturns` on the target, then `Uncharm` -> threat -0.5 -> `ThoughtLockEnd` ->
  remove `CharmParticle` and `CharmStartParticle` -> clear `ThoughtlockMaster`. `ThoughtLock_Feedback` is also
  deactivated at the outro start and on `OnKilledTarget`.
- **Not fired for a charm:** `OnTargetBecomesLocked`, so no `Skill_DetonateAvailable`, no `IsPhaseLocked` AI flag, no
  `Phaselocked` custom event, no Elated particle. `Att_EnemyIsPhaseLocked` is still 1 (it comes from the cooldown
  manager).
- **Confidence:** read from script (selection, lock, release) and read from data (graph); what `Charm` does to
  allegiance and AI is native, UNVERIFIED guess.

### 1.4 Res (Harmony, max grade 1; resurrect)
- **Script (`SelectTarget`):** only when the target fails `CanPhaseLockTarget` (it is friendly, dead or already
  locked). Then `CanResurrectTarget`: server; Res active for Maya; target set; target is friendly to Maya; target is
  injured (Fight For Your Life); target is not injured-dead. If true, `ResurrectTarget`, else fizzle.
- **`ResurrectTarget` order:** if Maya's controller already has a `ReviveTarget`, tell it it is no longer being
  revived; set `ReviveTarget` := target; tell the target it is being revived by Maya; if the target has an injured
  definition: client revive notice, run its `RevivedBehaviors`, restore health and shield
  (`RevivePawnHealthAndShield`), set second-wind reason 4; fire `OnHealedTarget` (particle `Part_Siren_Res`, revive
  message `ReviveMessage_SirenRes`, stat `STAT_PLAYER_NUM_REVIVES_ON_OTHER_PLAYER`); arm `EndSkill` at 1 s
  (`ReleaseBufferTime`).
- **Cooldown:** the cooldown manager is never activated (no `OnSelectedTarget`), so the 13 s drain runs from the cast.
- **Edge:** whether the auto-aim target choice (native `GetPreferredTarget`) ever returns a downed friendly player is
  UNVERIFIED; in single player Res is unreachable for the slice.
- **Confidence:** read from script; target choice UNVERIFIED guess.

### 1.5 Sub-Sequence (Motion, ActionAugment, max grade 5)
- **Existence:** ActionAugment skills are active only while Phaselock runs (NATIVE_SKILLS 4.1).
- **Trigger:** `InterruptPhaseLock` (held target dies or is incapacitated), only in state 1 (lifting) or 3 (locked),
  not in the outro.
- **Chance:** `SubsequenceProjectileChance` on the archetype = GradeOf(SubSequence) x 0.2 (20 % per grade). Rolled as
  "chance >= random 0..1". Server only, skill must be active. Effect row 0 (0.2 per grade) is presentation; the
  logic reads the archetype value.
- **Success:** spawn `Subsequence_Projectile` at the dead target's location, launched straight up, instigator Maya;
  state := 5. No release and no EndSkill re-arm in this call.
- **Projectile (data):** speed 1200, homing; after 0.1 s it enables homing: every 0.2 s it looks for the nearest foe
  within 4000 uu (360 degrees; `bCanHomePhaselockedTargets` false) and sets its speed to 1300 x a distance factor;
  on hitting a damageable object it detonates; on explode: miss-impact particle, then `SelectPhaselockTarget` with
  the hit pawn (script: calls the action skill's `SelectTarget`), then deactivate `Subsequence_Feedback`.
- **Re-lock:** `SelectTarget` -> `PhaseLockTarget` again: `OnSelectedTarget` fires again (Converge, Helios and the
  cooldown manager run again), but the outro and end timers are computed from the **original** cast start and the
  original `SkillDuration`, so the new target only gets the remaining time.
- **Cooldown hold:** `OnKilledTarget` activates `Subsequence_Feedback` whenever GradeOf(SubSequence) > 0, even if the
  roll failed; it holds the cooldown drain until the next `OnTargetBecomesLocked`, the projectile explodes, or
  Phaselock deactivates.
- **Edge (read from script, needs a live check):** after a successful roll the dead pawn is still `LiftedPawn`; on
  the next tick `InterruptPhaseLock` runs again, fires `OnKilledTarget` a second time, now fails the state test (5),
  arms EndSkill at 1 s and releases. So the projectile has about 1 s to reach a new target, and Sweet Release may fire
  twice for one kill. `EndSkill` detonates a pending projectile.
- **Confidence:** read from script and data; the double `OnKilledTarget` and the 1 s window are UNVERIFIED.

### 1.6 Chain Reaction (Cataclysm, ActionAugment, max grade 5)
- **Effect row 0:** `BulletReflectionOffEnemyChance` PostAdd +0.08 per grade on Maya (TARGET_Self).
- **Active when:** Phaselock running (ActionAugment) and `Att_EnemyIsPhaseLocked == 1` (otherwise Paused), on foot
  (otherwise Deactivated).
- **What it does:** bullets that hit an enemy have that chance to ricochet to another enemy. The ricochet is native
  (pawn reflection; globals `BulletReflectionRadius` 1800, `BulletReflectionSpeedAdjustment` 0.7,
  `BulletReflectedOffEnemyDamageScale` attribute, natives `WillowPawn.GetBulletReflectedOffEnemyDamageScale`).
- **Confidence:** numbers read from data; the ricochet rule is an UNVERIFIED guess (native).

### 1.7 Helios (Cataclysm, max grade 5)
- **Trigger:** `OnSelectedTarget` (and the blocked chain), gate `GradeOf(Helios) > 0`, delay 0.75 s.
- **What it does:** radius damage centred on the target: radius 650 (linked variable; the property says 800), no
  falloff, incendiary (`DmgType_Incendiary_Impact`, impact `ExplosiveImpactIncendiary512`), damage =
  0.2 x `Init_Ablaze_DamageCalc`, status damage = 0.11 x `Init_Ablaze_DamageCalc`, status chance 9001, where
  `Init_Ablaze_DamageCalc` = GradeOf(Helios) x PlayerSkillDamage. So damage = 0.2 x g x 8 x S^L.
- **Script part:** at t = 0 `ShouldSpawnHelios` (server; skill manager; Helios active) spawns the `Part_Ablaze_Explosion`
  emitter at the target's location when charmed, else at `LiftEndLocation`. Quirk: the script checks that the charm
  skill reference is set instead of the Helios reference; harmless because both are set on the archetype.
- **Blocked target:** if Helios is the highest owned of Ruin/Converge/Helios, the blocked chain waits 0.95 s before
  deactivating Phaselock.
- **Effect row 0** (1 per grade on a dummy attribute) is presentation.
- **Confidence:** read from data (graph, variables, formula) and read from script (particle).

### 1.8 Blocked target (resolves an open item of PHASELOCK_STOCK_DATA; read from data)
`OnTargetBlocked`: damage to the target, Phaselock impact, `DmgType_Normal`: **10 x PlayerMeleeDamage for a
non-player, 5 x PlayerMeleeDamage for a player** (`IsObjectPlayer` output 0 -> the 5x branch; which output means
"player" is inferred). Then hand-fizzle and "cannot be locked" particles, sound, the Helios / Converge / Ruin gates,
and the deactivation wait: Ruin owned 3.2 s; else Converge owned 1.45 s; else Helios owned 0.95 s; else at once.

## 2. Sweet Release and Mind's Eye

### 2.1 Sweet Release (Harmony, max grade 5)
- **Trigger:** `OnKilledTarget` (target died or became incapacitated while held; also fires for a stagger of a
  non-charmed target, see 0.5), gate `GradeOf(SweetRelease) > 0`. ThoughtLock_Feedback is deactivated first.
- **Spawn:** `SweetRelease_Projectile`, count = GradeOf(SweetRelease) (1 to 5 orbs), from the target's `SnapZoom`
  socket +20 uu up, direction straight up within a 10 to 75 degree cone, evenly distributed; plus particle
  `Part_SweetRelease`.
- **Orb (data):** speed 800, lifespan 12 s, wave Z amplitude 16 / frequency 400, allegiance switched to Player on
  spawn. After 0.1 s it pings every friendly within 6000 uu (invisible damage). For each friendly *player* below its
  max health it computes weight = missing health / max health and keeps the friend whose weight beats the current
  best divided by a random 0..2 factor (weighted random, favouring the most hurt); after 0.2 s it homes on that
  friend, or on the nearest friend if none was chosen.
- **Heal on hitting a friendly:** healing damage (`DmgType_Healing_Impact_NoDoT`) =
  0.15 x (HealthMaxValue_Player - HealthCurrentValue), i.e. 15 % of the **missing** health, then the orb detonates.
  0.15 = `Att_SweetRelease_HealingPercent` (constant resolver). Whose max/current is used (the healed friend is the
  natural reading) is UNVERIFIED.
- **Effect rows:** row 0 (1 per grade) and row 1 (0.10 constant overridden by the attribute, 0.15) are presentation.
- **Confidence:** read from data; the homing choice and whose health the formula reads are UNVERIFIED.

### 2.2 Mind's Eye (Harmony, max grade 5)
- **Rows:** `PlayerCriticalHitBonus` Scale +0.05 per grade; `InstigatedMeleeDamageModifier` Scale +0.06 per grade;
  Paused when not on foot.
- `PlayerCriticalHitBonus` resolves to the controller property `CurrentInstantHitCriticalHitBonus` (instant-hit, i.e.
  bullets; melee crits use a separate `CurrentMeleeCriticalHitBonus`). It enters damage only in the native
  `WillowDamagePipeline.AdjustDamage`; no script reads it. The base value of that property was not decoded.
- **Confidence:** attribute wiring read from data; how the crit bonus multiplies damage is an UNVERIFIED guess (native).

## 3. Kill skills

### 3.1 The kill-skill switch (read from script; timer native, UNVERIFIED)
- `WillowPlayerController.UpdateKillSkills(bEnable)` (script): for every skill of type Kill in Maya's tree that has a
  tree state: enable -> `SkillEffectManager.ActivateSkill(controller, skill, none, tree grade)` (remember whether any
  activation succeeded); disable -> `DeactivateSkill`. Then show or hide the HUD kill-skill icons
  (enable and at least one activated) and return the same value. `ActivateSkill` refreshes all of Maya's skills first
  for Kill skills (NATIVE_SKILLS 4.1).
- `ForceKillSkillsOff` (script) and the tree reset set `KillSkillTimeRemaining` to 0 and call it with false.
- **Duration (data):** `GD_Globals.General.Globals.KillSkillDurationsPerPlayers`: 1 player 7 s, 2 players 8 s,
  3 players 9 s, 4 players 10 s, times the controller attribute `KillSkillDurationMultiplier` (base 1, set at
  `SharedPlayerInitialization`).
- **Timer (native, not read):** the caller of `UpdateKillSkills(true)`, the setting of `KillSkillTimeRemaining` and
  `DurationOfLastKillSkillActivation`, and the countdown are native. Best guess: an enemy kill sets the remaining time
  to the full duration (refresh, not additive) and enables kill skills; when it reaches 0 they are disabled. The HUD
  replica shows icons while the remaining time is above 0.0001. UNVERIFIED guess.
- Kill skills are DURATION_Infinite: they last exactly as long as the switch keeps them active; their constraints
  (on foot) still apply.

### 3.2 Kinetic Reflection (Motion, Kill, max 5)
Rows on Maya: `BulletReflectionOffSelfChance` +1 (always reflect); `BulletReflectedOffSelfDamageTakenScale` PostAdd
-0.1 per grade; `BulletReflectedOffSelfDamageScale` PostAdd -0.8 + 0.2 x (g - 1), i.e. -1.0 + 0.2 g. If both bases
are 1, bullets that hit Maya do (1 - 0.1 g) of their damage and the deflected bullet does 0.2 g of it to an enemy.
Row 3 (0.2 per grade) is presentation. Deactivated off foot. The deflection itself is native (globals radius 1800,
speed 0.7). **Confidence:** rows read from data; bases and deflection UNVERIFIED guess.

### 3.3 Inertia (Motion, Kill, max 5)
`ShieldPassiveRegenerationRate` PostAdd 0.008 x g x ShieldMaxValue (per second) and `WeaponReloadSpeed` Scale -0.1 g.
Row 2 is presentation. Re-evaluated every 0.5 s while active (`SkillEffectUpdateIterval` 0.5, `bAutoUpdateContexts`
true by default). **Confidence:** read from data; regen unit (per second) UNVERIFIED.

### 3.4 Life Tap (Harmony, Kill, max 5)
`PlayerConvertDamageToHealingPercent` PostAdd +0.012 per grade -> controller `ConvertDamageToHealingPercent`. Maya
heals that fraction of damage she deals; the conversion is native (not the friendly-fire `ConvertDamageToHealing`
script, which is unrelated). Its `DamagedEnemy` event only plays a sound and particle. Paused off foot.
**Confidence:** numbers read from data; heal rule UNVERIFIED guess (native).

### 3.5 Blight Phoenix (Cataclysm, Kill, max 5)
- Row: `Att_BlightPhoenix_IsActive` PostAdd g.
- `OnActivated`: instance-data switch `BlightPhoenix` on (body VFX); after 0.5 s, while `Att_BlightPhoenix_IsActive`
  on Maya is greater than 0: radius damage around Maya, radius 512, damage source `WillowDmgSource_Skill_IgnoreIOs`:
  first corrosive, then incendiary; each damage = 0.02 x g x PlayerMeleeDamage (`Init_BlightPhoenix_DamageCalc_*` =
  GradeOf(BlightPhoenix) x PlayerMeleeDamage x `Att_BlightPhoenix_DmgScaler_*` 0.02); status damage =
  0.5 x g x PlayerSkillStatusEffectDamage; status chance 0.4. Then wait 0.5 s and test again (a 0.5 s loop).
- `OnDeactivated`: switch off. The loop ends at the next test after the attribute drops.
- Whether it hurts Maya or allies is UNVERIFIED (no friendly flag set). **Confidence:** read from data.

## 4. Capstones and others

### 4.1 Scorn (Harmony, max 1; melee override)
- Row 1: `IsSkillListeningForMeleeOverrideEvent` +1. Row 0 (18) is presentation.
- **Script:** on melee, `CanPerformMeleeSkill` = listening attribute > 0 and the melee-skill cooldown pool is empty
  (current value not above 0) and (class shares weapon action -> `CanPerformWeaponAction`, else true). If true,
  `StartMeleeSkill` instead of a melee: raise skill event 27 (`MeleeOverrideSkillActivated`) and refill the melee-skill
  pool to 100 %.
- **Cooldown pool (data):** `MeleeSkillCooldownPool_Siren`, max = `Cooldown_Scorn` = **18 s** (attribute wins over the
  constant 2). Drain rate assumed 1 per second (UNVERIFIED).
- **Provider:** special move `SpecialMove_KissOfDeath_Master` (anim `Rolling_Thunder`, blocks weapon actions), tattoo
  glow, sound, and at +0.7 s spawn `Projectile_Scorn` from Maya along her aim (offset 65 forward, 70 up, cone 2 deg).
- **Orb:** speed 900, homing, player allegiance; carries a slag damage area (sphere; DPS = ProjectileDamage, status
  chance 1, beams to targets). On hitting a damageable object or the world it attaches and explodes at once; otherwise
  it explodes after 5 s; then it is destroyed 0.1 s later. Explosion: slag (`Explosion_ScornOrb`), radius 512,
  damage 10 x PlayerSkillDamage, status damage 1 x PlayerSkillStatusEffectDamage, status chance 2.
- **Confidence:** script and data read; sphere radius and the projectile level used by ProjectileDamage not read.

### 4.2 Cloud Kill (Cataclysm, max 1)
- **Trigger:** skill damage or kill events on an enemy with damage source **Bullet or Rocket**
  (`Proc Cloud Kill`, `Kill Enemy Proc Cloud Kill`).
- **Gates:** Maya not in a vehicle; `CloudKill_Feedback` not running.
- **Then:** activate `CloudKill_Feedback` (5.5 s; acts as the internal cooldown and HUD icon), sound at the enemy,
  spawn `CloudKill_Projectile` at the enemy (owner Maya), tattoo glow.
- **Cloud:** lifespan 5 s; every 0.25 s for 5 s: radius 220, no falloff, no trace, damage 0.25 x PlayerSkillDamage
  (linked variable; the property constant 901.47 is overridden), `DmgType_Corrosive_Impact_NoDoT`.
- **Confidence:** read from data; event dispatch from damage source is native (UNVERIFIED).

### 4.3 Backdraft (Cataclysm, max 5)
- **Melee hit on an enemy** (damage source Melee): incendiary hit on that enemy, damage = 0.05 x g x PlayerSkillDamage,
  status chance = 1 + 0.45 g, status damage = 0.4 x that damage; HUD pulse.
- **Shield depleted after being full:** `Explosion_IncendiaryMaster_ShieldNova` around Maya, radius 800, no falloff,
  damage = 0.856 x g x PlayerSkillDamage, status chance = 2 + 0.4 g, status damage = 0.23 x that damage; HUD pulse
  (feedback on, 1.5 s, off).
- Row 0 is presentation. Paused off foot. "Status chance" values above 1 are probably multipliers of the damage
  type's base chance (UNVERIFIED). **Confidence:** read from data (math variables fitted, see 0.3).

### 4.4 Immolate (Cataclysm, max 5)
On a bullet or rocket hit on an enemy, only while Maya's `HealthCurrentValue` is below 1 (down state, Fight For Your
Life): extra incendiary damage to that enemy = 0.1 x g x (health damage done + shield damage done) of the hit;
status damage = g x PlayerSkillStatusEffectDamage. Row 0 is presentation. **Confidence:** read from data; that the
down state means health below 1 is inferred from the sequence name `ShotEnemyWhileInDownState`.

### 4.5 Recompense (Harmony, max 5)
When an enemy damages Maya's health (health damage > 0): with chance g x `Att_Recompense_Chance` (0.10) the attacker
takes damage equal to the health damage Maya took (`DmgType_Normal`, Phaselock impact, no trace), particle
`Part_Recompense`, HUD pulse 2 s. Paused when not healthy or off foot. Shield-only hits do not count.
**Confidence:** read from data.

### 4.6 Elated (Harmony, max 5)
Row 0: `HealthPassiveRegenerationRate` PostAdd 0.01 x g x HealthMaxValue_Player per second on TARGET_Allies with self
included and non-players excluded (Maya and party players; `BaseRange` 4096 but range not enforced since
`bOnlyEffectTargetsInRange` is false). Active only while `Att_EnemyIsPhaseLocked` > 0 (selection to release, 0.4)
and on foot; re-evaluated every 0.5 s. A particle on the target at the `Phaselocked` event is cosmetic. Row 1 is
presentation. **Confidence:** read from data; whose max health feeds the value (instigator per NATIVE_SKILLS 4.3)
and the ally search are UNVERIFIED.

### 4.7 Reaper (Cataclysm, max 5)
`PlayerTargetOverMinHealthGunDamageMultiplier` Scale +0.08 g and `PlayerTargetMinHealthDamageBoostPercent` +0.5.
Reading: gun damage +8 % per grade against targets above 50 % health. Applied in native damage code.
**Confidence:** numbers read from data; rule UNVERIFIED guess.

### 4.8 Sustenance (Harmony, max 5)
- Row 0: `HealthPassiveRegenerationRate` PostAdd, base and per-grade both = `Init_Sustenance_RegenFormula_P1`, start 1,
  interval 1, so value = g x P1.
- `P1` = P2 x `Att_Sustenance_HealthRegenPerLvl` (designer attribute, 0.004).
- `P2` = HealthMaxValue + (-1 x HealthCurrentValue) = Maya's **missing health**.
- **Formula:** regen per second = 0.004 x g x (max health - current health), i.e. 0.4 % of missing health per
  second per grade (2 % at grade 5). The value is recomputed on the skill refresh (every 0.5 s), so it lags health by
  up to 0.5 s. Row 1 (0.004 per grade) is presentation. Paused off foot.
- **Confidence:** read from data; the regen unit and the 0.5 s refresh cadence are UNVERIFIED.

## 5. Corrections and notes for the tracked docs
- `ActionSkill_Phaselock` sets `bAllowAutomaticActiveAbilityActivation` to **true** (NATIVE_ACTION_SKILL section 2
  says the archetype does not set it). A second press while active can therefore reach
  `StartActionSkillActiveAbility` (subject to the limits) and raise skill event 24; the only Phaselock reaction there
  is the script Ruin, which is dead because `RuinSkill` is unset.
- The blocked-target damage amount (PHASELOCK_STOCK_DATA "not recovered") is 10 x PlayerMeleeDamage, or 5 x for a
  player (1.8).
- `Att_EnemyIsPhaseLocked` is driven by `Skill_Phaselock_CooldownManager` (0.4).

## 6. Open questions for a live bl-sdk read
1. CompareFloat / Conditional / IsObjectPlayer output meaning: hook a Converge-owned cast and a blocked target.
2. Kill-skill timer: hook `UpdateKillSkills`, poll `KillSkillTimeRemaining` and `DurationOfLastKillSkillActivation`
   across two quick kills (refresh vs add; which kills count).
3. Sub-Sequence: does `OnKilledTarget` fire twice per kill (Sweet Release orb count with both skills), and how long
   the projectile has before `EndSkill`.
4. Sweet Release heal: whose max/current health; homing choice with two hurt players.
5. Ruin: confirm three pulses at +1/+2/+3 s and radius 650 (linked variable overriding 400).
6. Helios: radius 650 vs 800; damage 0.2 x g x PlayerSkillDamage.
7. Mind's Eye: base of `CurrentInstantHitCriticalHitBonus` and the crit damage formula in `AdjustDamage`.
8. Reaper, Life Tap, Kinetic Reflection, Chain Reaction: confirm the native rules (bases of the reflection
   attributes, the 50 % threshold).
9. Sustenance / Elated / Inertia: regen units and the 0.5 s refresh.
10. Scorn: melee-skill pool drain rate, damage-area sphere radius and projectile level.
11. Thoughtlock: `Charm` owner and allegiance change.

## 7. Host implementation status (2026-11-03, skills lane PRs 3 to 5)

`OpenWillowSkillBehaviours.cpp` (part of `UOpenWillowSkills`, ticking on Maya's pawn) implements from this note:

| Skill | Host | Not done |
|---|---|---|
| Kill skills (3.1) | a kill (`AOpenWillowWalker::NotifyKilledEnemy`) restarts a 7 s timer; Kill skills' modifiers count while it runs | refresh vs add, which kills count (guess) |
| Inertia (3.3) | reload speed (through the PR 2 reload hook) | shield regeneration (no shield) |
| Helios (1.7) | +0.75 s blast on the lifted target, radius 650, 0.2 x g x PlayerSkillDamage | burn status; the blocked-target chain |
| Converge (1.1) | +1.25 s blast, radius 2000, 1 x PlayerSkillDamage | the pull, the melee definitions; blocked chain |
| Ruin (1.2) | slag, shock, corrosive blasts 1, 2, 3 s after the lock, radius 650, 7.2 x PlayerSkillStatusEffectDamage | the status effects; blocked chain |
| Sweet Release (2.1) | when the held target dies, g orbs each heal 15 % of Maya's missing health after flight time at 800 uu/s | the orb actor and homing; other players |
| Life Tap (3.4) | while the kill-skill timer runs, Maya heals `PlayerConvertDamageToHealingPercent` (0.012 x g) of each gun hit's applied damage | the native conversion rule (guess: applied damage x fraction) |
| Cloud Kill (4.2) | a gun hit on an enemy, outside a 5.5 s cooldown, leaves a cloud at the enemy: 20 corrosive blasts every 0.25 s, radius 220, 0.25 x PlayerSkillDamage | the cloud actor; host procs on Direct hits only (bullets, pellets, rocket bodies), not on splash or Tediore throws, because splash carries no damage-source flag yet |
| Immolate (4.4) | while Maya is down, each Direct hit adds an incendiary hit of 0.1 x g x its applied damage, sent as a non-critical splash event | the status damage |
| Blight Phoenix (3.5) | while the kill-skill timer runs, every 0.5 s a corrosive and an incendiary blast around Maya, radius 512, 0.02 x g x PlayerMeleeDamage each | the status damage; the body effect |
| Recompense (4.5) | when an enemy takes health from Maya, chance 0.1 x g that it takes the same amount back (DmgType_Normal, non-critical); not while Maya is down | shield-only hits (the host has no shield) |
| Mind's Eye (2.2) | the hit dispatcher's crit bonus is `PlayerCriticalHitBonus` = 2.0 (live base) with the skill's Scale +0.05 x g, so 2 x (1 + 0.05 g); a caller's own bonus wins | the melee row (no host melee); how the bonus multiplies damage is the dispatcher's rule |
| Sustenance (4.8), Elated (4.6) | health regeneration per second from `HealthPassiveRegenerationRate` (the effects file now carries balance formulas) | the 0.5 s refresh cadence; allies |

Phaselock events are read from the Walker's public state (cast time, hit, target, timeline), so the Phaselock code has
no new hook. The blast is the host's: every actor that takes Maya's hits (`IOpenWillowHitTarget`) within the radius gets
a splash `FOpenWillowHitEvent` through `AOpenWillowWalker::ApplyShotDamage` (weapon id `skill:<name>`, the skill's own
damage type and element, no falloff, no crit), so kills count for her and hit listeners see the skill, with the host's
explosion look. Not implemented yet: Thoughtlock's charm, Res, Sub-Sequence, Chain Reaction,
Kinetic Reflection (needs the incoming hit's source), Scorn and Backdraft (no host melee), Reaper (needs the
target's health inside the dispatcher), the blocked-target damage. All of it is UNVERIFIED in game; the live questions are in section 6.

The hit-driven skills (PR 4) listen to `OpenWillowHit::OnHit()`; they ignore status ticks and the skills' own hits
(weapon id `skill:<name>`), and queue their extra damage for the next tick instead of applying it inside the broadcast.
PlayerMeleeDamage is taken as 2.5 x PlayerSkillDamage (no melee buff in the host).
