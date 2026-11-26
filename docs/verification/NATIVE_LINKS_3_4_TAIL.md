# Links 3 and 4 past the spawn: the kill's and the swing's next steps (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-COMBAT-TAIL-LINKS-3-4 (decision 2026-11-26-69), the analyst half of
the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/l34tail/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here
is UNVERIFIED in the running game.**

**Why.** On main, links 3 (`kill_enemy`) and 4 (`enemy_attacks_maya`) stop in their spawns at
`AttributeInitializationDefinition.SetBaseValue` ("a definition it names is not loaded"), which lane E is reading. This
note reads on past that point: link 3's lethal hit and link 4's melee swing, with `SetBaseValue` stood in.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`, and a local scan of every script function in Core, Engine,
  GearboxFramework and WillowGame): `Pawn.TakeDamage`, `WillowPawn.PostBeginPlay`,
  `WillowPawn.NotifyActionSkillOfKilledEnemy`,
  `WillowPlayerPawn.AddKillStreakScore`, `WillowPlayerController.SharedPlayerInitialization`.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read:
  - `WillowPawn.AddDamageToHitRegion` (the exec), the hit-region consumer slot of the `WillowPawn`, `WillowAIPawn` and
    `WillowPlayerPawn` tables (one shared body), and the hit-region helper's routine with its callees;
  - `BaseBalanceDefinition.GetDisplayNameAtGrade` (the exec), its class virtual on `BaseBalanceDefinition` and
    `AIPawnBalanceDefinition`, and the playthrough and name-fixup lookups it uses;
  - `WillowPlayerController.NotifyKilledEnemy` (the exec and its class virtual), outlined past its gates;
  - `WillowPawn.IsActionSkillRunning` (the exec and the virtual, one body on the three Willow pawn tables);
  - `WillowDialogGlobalsDefinition.Get` (the exec and its getter).
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. Every field named below landed exactly where the
  routines read it (`BodyHitRegionDefinition`, its `DamageReactionData`, `HitRegionHelper.HitRegionDamage`,
  `DamageEventSummary`, `AIPawnBalanceDefinition` and its `AIPawnPlaythroughData`, `WillowPawn`,
  `WillowPlayerController`, `GlobalsDefinition`, `WillowGlobals`).
- **`ow-package --def-dump`:** `GD_PrimalBeastShared.HitRegions.HitRegion_Body` and its behaviour provider.
- **A local probe** (temporary code, reverted, never committed): installed Off strict `--slice-gate` runs under the
  machine lock, on main d118d2bb with lane E's two open branches merged locally (`fix/rust-combat-music`, for
  `ApplyGradeCustomizations`; `fix/rust-links-13-14`). Each stop below was stood in by the rule this note or an earlier
  one reads, in turn, with a call trace.

## 2. The probe's sequence

With `SetBaseValue`'s two load refusals stood in (the unloaded handle loaded, the preflight skipped), **both spawns
complete**. Then, in order (each row stood in for the next):

**Link 3** (`hit:{baby}:9999::Bullet`):

| # | Stop | Read? | This note |
|---|---|---|---|
| 1 | unbound `DamagePipeline.GetBarrelSourceTime()` (then `GetPlantSourceTime`, `GetCanInflictFriendlyFire`, `Pawn.IsUsingWalkingPhysics`) | yes, #788 section 4.2 | binding only |
| 2 | `GameInfo.IsFriendlyFire`: "InstigatingTargetable holds Object, not an interface" | port gap | section 3 |
| 3 | `AdjustDamage`'s hit-region cap (`bTrackDamage`), `NotPorted` | outline only | **section 4** |
| 4 | unbound `BaseBalanceDefinition.GetDisplayNameAtGrade(int)` | no | **section 5** |
| 5 | unbound `WillowPlayerController.NotifyKilledEnemy(string)` | no | **section 6** |
| 6 | unbound `WillowPawn.IsActionSkillRunning()` | no | **section 7** |
| 7 | unbound `WillowDialogGlobalsDefinition.Get()` (`WillowPlayerPawn.AddKillStreakScore`) | the mission bridge binds it | section 8 |

**Link 4** (`swing:{adult}:melee`):

| # | Stop | Read? | This note |
|---|---|---|---|
| 1 | the same getters as link 3's row 1 | yes, #788 section 4.2 | binding only |
| 2 | unbound `SpecialMoveComponent.IsPlayingAny()` (Maya's `SetMovementPhysics` in her `TakeDamage`) | yes, #788 section 4.2 | binding only |
| 3 | `AdjustDamage`'s shield stage for a victim with a shield pool, `NotPorted` | yes, NATIVE_DAMAGE_DEATH.md stage 10 | section 9 |
| 4 | the pawn's `NormalImpactDamageModifier`: 0 with a base of 0 | #796's ruling (the player's `Pawn` at set-up) | section 9 |
| 5 | `AdjustDamage`'s protection timer (stage 15), `NotPorted` | yes, NATIVE_DAMAGE_DEATH.md stage 15, NATIVE_DOWN_TIMERS.md | section 9 |
| 6 | `DoForceFeedbackWaveform`: "Engine.ForceFeedbackWaveform: an object the store has not loaded" | yes, #799 section 7 | **section 9** (an order fix) |

Link 3's row 2 also meets link 4: both `TakeDamage` calls reach `IsFriendlyFire` the same way. In the slice gate a
`NotYet` in link 3's hit is reported against link 4's first step too (the gate stops the shared combat run), so link 4's
own stops show only once link 3's are stood in.

## 3. `IsFriendlyFire` and the interface value (both links)

`Pawn.TakeDamage` fills its interface local `InstigatingTargetable` with a cast to `ITargetable` of the instigator's
pawn (or of the damage causer's instigator), and passes it to `GameInfo.IsFriendlyFire`. The cast's opcode is
`DynamicCast` (not `InterfaceCast`). The port's VM, as the C++ reference does, answers both casts with the object
itself, so the interface local holds an `Object`. #798's `IsFriendlyFire` (review finding 6) accepts only an
interface value or None, so it raises `VmError::Script`.

Not read: what the game's `DynamicCast` leaves in an interface variable. **For the hub, one of:**
- the VM converts an object stored into an interface-typed variable or parameter into an interface value, when the
  object's class implements the interface (the VM's interface check, as `QueryInterface` decides it);
- or `IsFriendlyFire` accepts an `Object` whose class implements `ITargetable` as the same value.

Either is UNVERIFIED. The first fixes every native that reads such a parameter; the second only this one.

## 4. `AdjustDamage`'s hit-region cap: `AddDamageToHitRegion` (link 3)

NATIVE_DAMAGE_DEATH.md section 1, stage 13 outlines it: "for regions with `bTrackDamage`, the hit-region consumer's
`AddDamageToHitRegion` runs with the summary". It is now read.

**The consumer** (`IHitRegionConsumer.AddDamageToHitRegion(out DamageSummary, InstigatedBy, DamageType,
DamageTypeDefinition, HitRegion, HitLocation, HitMomentum)`): `WillowPawn`'s exec hands to its interface slot. One body
serves `WillowPawn`, `WillowAIPawn` and `WillowPlayerPawn`:
- `MyHitRegionHelper` None: nothing;
- otherwise the helper's routine, with the pawn as owner, its `ReplicatedBehaviorConsumerState` as the consumer handle,
  and its `HitRegionHealthScaleAttribute` as `HitRegionHealthScale`.

`WillowPawn.PostBeginPlay` makes `MyHitRegionHelper` with `New` (a `HitRegionHelper`, outer the pawn).

**The helper's routine** (`HitRegionHelper.AddDamageToHitRegion(Owner, ConsumerHandle, out DamageSummary,
InstigatedBy, DamageType, DamageTypeDefinition, HitRegion, HitRegionHealthScale, HitLocation, HitMomentum)`), in order.
`R` is the region, `S` the summary, `D` its `FinalDamage`:
1. **`R.bTrackDamage` clear:** nothing.
2. **The record:** the helper's `HitRegionDamageList` entry whose `BodyHitRegion` is `R`; if none, a new entry is
   appended with `BodyHitRegion` := `R` and its two float arrays (`LastPctHealthForRunDamageTriggeredBehaviors`,
   `...HealingTriggeredBehaviors`) sized to `R`'s `DamageTriggeredReactions` and `HealingTriggeredReactions`.
3. **The region's share `M`:** the owner's maximum health (a pawn's `HealthMax`, by fit; an interactive object's own
   pair). Times `HitRegionHealthScale` when `R.bScaleHealthWithAttribute`. Then times `R.OwnerHealthPercent` when
   `R.bUseIndependentHealth`; otherwise times `R.PercentOfMaxHealthLimit` when that is above 0.
4. **What remains:** `left` := `M` − the record's `Damage`. The region is **unlimited** when it is not independent and
   its `PercentOfMaxHealthLimit` is 0.
5. **A spent limited region:** when the record's `bHasGored` is set, its `RecentDamage` is 0, and the region is not
   unlimited: `D` := 0.
6. **The limit:** when not independent, `M` above 0 and `PercentOfMaxHealthLimit` above 0: if `left` > 0.01, and
   `left` − `D` ≤ 0.01, then `D` := `left`; if `left` ≤ 0.01, `D` := 0.
7. **Allowed damage types:** a non-empty `R.AllowedDamageTypes` without `DamageTypeDefinition`: `D` := 0.
8. **Kismet:** each of the owner's `GeneratedEvents` that is a `SeqEvent_TakeHitRegionDamage` gets its activation check
   with the owner, the instigator, the damage type, its definition, `R` and `D` (not read further).
9. **With `M` above 0, and the owner answering `IHitRegionConsumer`** (else stop here):
   - `before` := 1 − `left`/`M`; the record's `Damage` and `RecentDamage` each += `D`; `left` −= `D`;
     `after` := 1 − `left`/`M`; the recent fraction := `RecentDamage`/`M`;
   - `dies` := the owner's current health − `D` < 0.01;
   - **the damage reactions:** for each `R.DamageTriggeredReactions[i]`, skipped when `bOnlyTriggeredOnOwnerDeath`
     and not `dies`. It fires when `PercentOfHealthTrigger` ≤ `after`, and either `before` < that trigger or the entry
     is not a pure `bDamageThreshold` one, and either its last fraction is 0 or `bRepeatableEvent`. A non-empty
     `DamageTriggers` list must also hold an entry whose minimum is at most the recent fraction and whose damage type
     (and class) match. A firing reaction raises the entry's behaviour event through the consumer (skipped when the
     game censors gore and the entry is `bCensored`), plays its reaction through the consumer, and the entry's last
     fraction := `after`;
   - **the region's death:** when `left` ≤ 0.001 and not `bHasGored`: the record's `Damage` := `M`;
     `R.RegionDeathBehaviors` run (with the owner's `Instigator`); `bHasGored` := true; and the region's behaviour
     event **`OnRegionDeath`** is raised through its `BehaviorProviderDefinition` and the consumer handle, with the
     gore-censorship flag, the instigator, the hit location in the owner's frame and the momentum.
10. **An independent region:** with an instigator and `R.bShowDamageNumbers`, the instigator's damage-number virtual
    runs with `D`. Then `D` := 0: an independent region never passes damage to the owner's health.

**On the slice** (`HitRegion_Body` of the primal beasts): `bTrackDamage` true; not independent, no limit, no scale; no
allowed types, no reactions, no death behaviours; its behaviour provider holds no sequences. So for link 3's 9999 hit:
`D` is **unchanged**; the record is made and gets `Damage` := `M` and `RecentDamage` += `D`; `bHasGored` := true;
`OnRegionDeath` finds nothing to run.

**Proposed Off answer** (replacing `damage.rs`' `NotPorted` "AdjustDamage hit-region cap (bTrackDamage)"):
- **preflight, write-free and load-free:** the victim's class is a Willow pawn (else `NotRead`, "the hit-region
  consumer of <class>"); `MyHitRegionHelper` None: nothing; the helper resident; every field above declared with its
  kind, else `VmError::Script`; the owner's `GeneratedEvents` holding a `SeqEvent_TakeHitRegionDamage`: `NotRead`,
  "the hit-region Kismet event";
- **decided in the preflight:** a reaction that would fire, non-empty `RegionDeathBehaviors`, or an `OnRegionDeath` with
  sequences on the region's provider: `NotRead`, "the hit region's behaviours" (they run through the existing behaviour
  rules once wired). Then the censorship flag is never needed on the slice's path;
- **commit:** the record (made or found), its two fractions, `Damage`, `RecentDamage`, `bHasGored`, and `D`. These are
  writes inside `AdjustDamage`, so a later refusal in the same pass carries the partial-writes prefix;
- the current health and `HealthMax` read as the pools rule reads them (NATIVE_DAMAGE_DEATH.md section 2).

## 5. `BaseBalanceDefinition.GetDisplayNameAtGrade(GradeIndex)` (link 3)

The exec reads the int and calls the class virtual.
- **`BaseBalanceDefinition`:** the engine's "pure virtual not implemented" log, and an empty string.
- **`AIPawnBalanceDefinition`:** **the grade is not used.** In order:
  1. **the name fixup:** only when the game's playthrough (the replication info's playthrough query, the one
     `ApplyGradeCustomizations` uses, NATIVE_COMBAT_MUSIC_AND_AFTER.md section 3) is above 1. A fixup list from the
     globals' playthrough data is searched once (cached in `HasLookedUpFixupIndex` and `FixupAIPawnNameIndex`) for an
     entry keyed by this balance's name (case-insensitive); a found entry with a non-empty name gives that name;
  2. **otherwise:** the playthrough index (the same query; −1 without the globals, 0 without a Willow replication
     info), capped at `PlayThroughs` count − 1. Out of range: an empty string. Else
     `PlayThroughs[index].DisplayName`.

**Proposed Off answer:** load-free reads of `PlayThroughs` and the current-playthrough rule the port already has
(decision 2026-11-19-109, the current-playthrough virtual and the engine-wide condition). A playthrough above 1:
`NotRead`, "GetDisplayNameAtGrade: the playthrough name fixup" (the
fixup list's source is not named). The two cache fields are written only on that path, so the slice's path writes
nothing. Another balance class's virtual: `NotRead`. Declared, else `VmError::Script`: `PlayThroughs` (an array of the
struct), `DisplayName` (a string).

## 6. `WillowPlayerController.NotifyKilledEnemy(EnemyName)` (link 3)

The exec calls the class virtual. Its body, outlined from its gates:
1. **the stat event:** when the world's `Game` is a Willow game info with authority (by fit), `PlayerClass`,
   `PlayerStats`, the globals definition (`WillowGlobals.GetGlobalsDefinition`, which loads it lazily) and
   `PlayerSkillTree` are all set, and one skill-tree check passes (not read): `PlayerStats` gets a stat incremented and
   a class-named stat event is raised. Not read further;
2. **without `WorldInfo`, the globals definition or `PlayerStats`:** it returns here;
3. **the kill history:** a new `RecentKillHistory` entry at the front (the name and `WorldInfo.RealTimeSeconds`), the
   list capped at 25; `CurrKillingSpree` restarts at 1 when the previous kill is
   `GlobalsDefinition.MaxTimeBetweenSpreeKills`
   or more ago, else rises by 1, and a higher spree than the stored stat sets that stat;
4. **the speed-kill achievements:** `GlobalsDefinition.AchievementData` entries of one kind are matched against the
   history. Not read.

`PlayerStats` is spawned by `WillowPlayerController.SharedPlayerInitialization`.

**Proposed Off answer:** load-free reads of `WorldInfo`, `PlayerStats` and the globals definition (resident, else
`NotRead`; a lazy load is a write). **`PlayerStats` None: nothing** (step 1's gate and step 2 both hold). `PlayerStats`
set: `NotRead`, "NotifyKilledEnemy: the player stats' kill bookkeeping". Declared, else `VmError::Script`:
`PlayerStats`, `PlayerClass`, `PlayerSkillTree`, `RecentKillHistory`, `CurrKillingSpree`.

## 7. `WillowPawn.IsActionSkillRunning()` (link 3)

The exec calls a class virtual with one body on the three Willow pawn tables: **`MyActionSkill` is not None.** This
resolves NATIVE_ACTION_SKILL.md's "body not located".

The script caller (`WillowPawn.NotifyActionSkillOfKilledEnemy`, from `KilledEnemy`) then calls
`MyActionSkill.OnActionSkillKilledEnemy(target)` when it answers true.

**Proposed Off answer:** that read, load-free, write-free; `MyActionSkill` declared as an object, else
`VmError::Script`.

## 8. `WillowDialogGlobalsDefinition.Get()` (link 3)

The exec returns the dialog globals from the engine-side globals' definition (one field, by fit `DialogGlobals`). Main
binds it only in the mission bridge (`ow-mission` `bridge.rs`; the C++ reference's rule:
`GD_Globals.Dialog.DialogGlobals`
of the mission package). The combat world does not, so `WillowPlayerPawn.AddKillStreakScore` errs there.

**Proposed Off answer:** bind the same rule in the combat world, load-free (the object resident, else `NotRead`).
`AddKillStreakScore` then reads `KillPointValue`, `KillStreakTargetScore` and `DET_KillStreak`, and may call the
pawn's `DialogComponent.TriggerEvent` and `PostPlayerEvent(3)`. Those are script and natives this note did not read.

## 9. Link 4's swing

- **Rows 1 and 2** (#788 section 4.2): bind the plain reads in the combat world. `IsPlayingAny` is bound today only as
  the C++ stand-in where the special-move pack is wired (`ow-combat` `hit.rs`).
- **Row 3, the shield stage** (NATIVE_DAMAGE_DEATH.md stage 10): Maya has a shield pool. The rule is read except the
  controller's damage-number notification, whose identity stays open. Off: port stage 10 as read; the notification
  `NotRead` when a player instigator's controller would get it (an AI instigator's controller does not).
- **Row 4, the resistance:** #796's ruling (the player's `Pawn` set at set-up, lane E's deferred setting). With it
  on, Maya's thirteen class values are written and this stop clears.
- **Row 5, the protection timer** (stage 15): read. In the probe the swing's damage was 69.7 against Maya's 212.7,
  above half after the hit, so the timer does not start. Off: port stage 15 as read.
- **Row 6, `DoForceFeedbackWaveform`:** `bcyl_tail.rs`' `force_feedback` checks the waveform resident before the gates
  (review r1 of #799, "the declarations first"). The headless controller's gates answer "nothing", and nothing past
  them reads the waveform, so the unloaded waveform should not refuse. **Proposed:** check that the waveform's class
  declares `Scale` as a float (a class lookup, loading nothing), and require the waveform resident only past the
  gates, where its `Scale` is written.

Past row 6 the probe stood the waveform in, and its next stop was an artefact of that stand-in. Link 4's next real
stop is not known.

## 10. Predicted stops

- **On main today:** both links at `SetBaseValue` (lane E).
- **With `SetBaseValue` answered:** link 3 at row 1 (an error, the unbound getter), link 4 behind it (the shared run).
- **With every row answered as proposed:** link 3 reaches `AddKillStreakScore`'s dialog steps (section 8); link 4
  passes the force feedback, then unknown. Neither is predicted to pass yet.

## 11. Tests (synthetic fixtures)

1. **The hit-region cap**, on a toy Willow pawn with a toy helper and region:
   - `bTrackDamage` clear: nothing; `MyHitRegionHelper` None: nothing;
   - an unlimited region: `D` unchanged, the record made with `Damage` and `RecentDamage` = `D`; a second hit: the same
     record, summed;
   - a limit of 0.5 with `HealthMax` 100: `M` 50; a hit of 60 on a fresh record: `D` := 50; the next hit: `D` := 0;
   - an independent region with `OwnerHealthPercent` 0.25: `D` := 0 after the bookkeeping; with `bShowDamageNumbers`
     and an instigator: the notification;
   - `AllowedDamageTypes` without the hit's type: `D` := 0;
   - a lethal hit: `bHasGored` true, `Damage` := `M`; a non-empty `RegionDeathBehaviors`, a reaction that would fire,
     or a provider with sequences: `NotRead` before any write;
   - a `SeqEvent_TakeHitRegionDamage` in `GeneratedEvents`: `NotRead`;
   - a later refusal in the same `AdjustDamage` pass: the partial-writes prefix.
2. **`GetDisplayNameAtGrade`:** playthrough 0 and 1: that entry's `DisplayName`; a playthrough beyond the list: the
   last entry; an empty list: an empty string; playthrough 2: `NotRead`; the base class: an empty string; the grade
   argument changes nothing.
3. **`NotifyKilledEnemy`:** `PlayerStats` None: nothing written; set: `NotRead`, nothing written; an unloaded globals
   definition: `NotRead`.
4. **`IsActionSkillRunning`:** `MyActionSkill` None: false; set: true.
5. **`WillowDialogGlobalsDefinition.Get`** in a combat world: the dialog globals; unloaded: `NotRead`.
6. **`DoForceFeedbackWaveform`** with an unloaded waveform and no gamepad: nothing, nothing loaded; with a gamepad, a
   manager and a replication info: the residency check, then the manager's `NotRead`.
7. **Each refusal before its native's first write,** with #518's three counters unchanged.
8. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:** what the game's `DynamicCast` to an interface leaves in an interface variable (section 3); the
   SeqEvent's activation check, the reaction's behaviour and play calls, the region-death impulse, and the censorship
   query (section 4); the name-fixup list's source (section 5); `NotifyKilledEnemy` past its gates (section 6); the
   shield
   stage's controller notification (section 9).
3. **By fit:** the owner's health pair the hit-region routine reads; the game check in `NotifyKilledEnemy`; the dialog
   globals field.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Kill a primal beast with one large hit and read its `MyHitRegionHelper.HitRegionDamageList` (the body region's
   `Damage`, `RecentDamage`, `bHasGored`).
2. Read an enemy's name as the HUD shows it in normal and true vault hunter mode, against its balance's `PlayThroughs`.
3. Read Maya's controller's `RecentKillHistory` and `CurrKillingSpree` after two quick kills.
