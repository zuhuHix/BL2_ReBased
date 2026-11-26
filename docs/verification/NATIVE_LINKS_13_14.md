# Links 13 and 14: the swings past link 4, the class-level attributes, and what stops the down (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINKS-13-14 (decision 2026-11-26-53), the analyst half of the clean
room: **another lane implements this note.** Raw output stays under ignored `local/analysis/l1314/`. This note is in
our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here is
UNVERIFIED in the running game.**

**Why.** Links 13 and 14 stop today where links 3 and 4 do. Once those stops clear, their own steps come next, and no
note covers them in `ParityMode::Off`. NATIVE_GO_DOWN_PATH.md is a look-ahead taken from a Reference run (its section
1: a lower bound, with zero answers standing in for unbound natives).

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`, and a local scan of every script function in the nine packages for one
  call or field):
  - `WillowMind.Possess`, `WillowMind.InitializeCharacterClass`;
  - `GameInfo.RestartPlayer`, `WillowGameInfo.SetPlayerDefaults`, `WillowPlayerController.ApplyCharacterClassDefaults`;
  - the callers of `RecalculateAttributeInitializedState` and the writers of `Controller.CharacterClass`;
  - `WillowPawn.TookDamageFromEnemy`, `WillowPawn.StealAmmo`, `WillowPlayerPawn.ProcessDamageTakenSkillEvents`,
    `Pawn.PlayHit`.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read: the execs named below, the
  `Controller` part of `RecalculateAttributeInitializedState`, and `WillowDamagePipeline`'s damage virtual's entry.
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. Every field below lands exactly where the routine
  reads it, except two on `WillowAIPawn`, which are one field after the tool's places. That is the layout gap
  NATIVE_MIND_ROUTE.md records, and those two are marked "by fit".
- **`ow-package --def-dump`:** class defaults and the two AI classes.
- **A local probe** (temporary code, reverted, never committed): an installed Off `--combat-run` on main fc860b5,
  under the machine lock, with a call trace and read-only prints. It ran the slice gate's own steps for links 1, 3, 4,
  13 and 14 on one world:
  - `maya:8`, `players:1`;
  - `spawn:baby:6`, `hit:0:9999::Bullet`;
  - `spawn:adult:7`, `swing:1:melee`;
  - `cash:3000`, then three more `swing:1:melee`;
  - `bleed:50`.

  It stubbed:
  - #788's and #793's earlier stops, with #793's reads;
  - `AdjustDamage`'s left-out stages (they were skipped, not answered);
  - then each stop below in turn, each by the rule read here.

## 2. The links' steps (`crates/ow-cli` slice gate, link table)

| Link | Driver | Steps | Needs | Evidence |
|---|---|---|---|---|
| 13 `go_down` | combat-run | `cash:3000`, then the adult's swing (`swing:1:melee` in the probe) three times | link 4 | a `down.total.` row |
| 14 `bleed_out_respawn` | combat-run | `bleed:50` | link 13 | a `down.respawn_health.` row |

**They have no spawn of their own.** Link 13 swings link 4's adult again, on the same world, and link 14 bleeds out
the down that link 13 made. `cash:3000` is a state input (the credits the respawn fee is a share of) and reaches no
native.

## 3. The class-level part of `RecalculateAttributeInitializedState`: the pawn properties named

The `Controller` routine is shared: the player controller's override and the mind's override both call it.
NATIVE_LEVEL_UP_ATTRIBUTES.md section 1 step 1 reads its rule by kind ("Normal impact", ...), from the class's
`BaseEncumbranceResistance` and `Base<Kind>DamageModifiers`, with the controller as the context. The port refuses it
(`ow-world` `levelup.rs`, `refuse_class_parts`) as "the pawn properties they land in are not named". **They are now
named**, each read landing exactly on the layout tool's place on `Engine.Pawn`:
1. the value of **`EncumbranceResistance`** (only the value, as that note says);
2. for each damage kind, in this order: **Normal, Explosive, Shock, Corrosive, Incendiary, Amp**:
   - **`<Kind>ImpactDamageModifierBaseValue`** := the class's `Base<Kind>DamageModifiers.ResistanceToImpact`;
   - **`<Kind>ImpactDamageModifier`** := recomputed from that base and **`<Kind>ImpactDamageModifierModifierStack`**;
   - then the same three for **`<Kind>StatusEffectDamageModifier`**, from `ResistanceToStatusEffect`.

Then, always, the pools' rebase (unchanged).

**The count:** one value and twelve modifier pairs, 13 writes of a value. That reconciles NATIVE_MIND_ROUTE.md
section 3's 13 against NATIVE_LEVEL_UP_ATTRIBUTES.md's 12, which did not count the encumbrance value.

**The gates** (NATIVE_LEVEL_UP_ATTRIBUTES.md section 1, "Does" step 1 and "Edge cases"): with no pawn, a pawn that is a
`Vehicle`, or no `CharacterClass`, only the pools' rebase runs (review r1 of #796).

**Proposed Off answer** (replacing `refuse_class_parts`' class-level `NotRead`):
- **preflight**, write-free and load-free:
  - every field must be declared with its kind, else `VmError::Script`;
  - each of the thirteen init-data attribute definitions, and the resolvers of its chains, must already be resident,
    else `NotRead` naming it;
- **writes** in the order above, with the controller as the context, through `ow-attr`'s evaluator and stack fold;
- **prefixes:** a refusal after the first write carries the partial-writes prefix;
- **Reference:** unchanged.

## 4. Who has a `CharacterClass` when the recalculation runs

**In the game:**
- **an AI mind:** `WillowMind.Possess` sets the mind's `CharacterClass` to the pawn's `AIClass`, then sets
  `HomeLocation`, then calls `InitializeCharacterClass`. That reaches the recalculation through the mind's
  `ApplyCharacterClassDefaults`. So the class-level part runs for an enemy, from its AI class's `Base*` data.
  **NATIVE_COMBAT_GROUNDWORK.md section 1.11 already records this** (the script route of `WillowMind.Possess`); this
  lane read it again. NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md 4.1's claim that no step sets it contradicted
  that record, and is corrected (review r1 of #796);
- **the player:** `WillowPlayerController.SwitchPlayerClass` sets `CharacterClass` to `PlayerClass`. After the possess,
  `GameInfo.RestartPlayer` calls `WillowGameInfo.SetPlayerDefaults(pawn)`, which calls the controller's
  `ApplyCharacterClassDefaults(pawn, PlayerClass)` and then `ApplyGlobalPlayerMovementSettings`. That script ends in
  `RecalculateAttributeInitializedState` with the controller's `Pawn` set. So Maya's thirteen values come from her
  class.

**In the port** (probe):
- **the enemy:** `hit::adopt` runs the mind's `InitializeCharacterClass` and leaves the rest of `Possess` out, on
  purpose (decision 2026-11-19-26). So the mind's `CharacterClass` is None at the recalculation, and the class-level
  part is skipped;
- **Maya:** the combat run's player set-up (`run/player.rs`) runs the controller's `ApplyCharacterClassDefaults` with
  the controller's `CharacterClass` set but its `Pawn` None. The mission bridge sets the `Pawn` only during a level-up.
  So the class-level part is skipped there too, and Maya's twelve modifiers stay 0 with a base of 0.

**The effect:** on every swing, `ow-combat`'s `GetPawnDamageTypeModifier` (`damage.rs`, `resistance`) refuses "the
pawn's NormalImpactDamageModifier: 0 with a base of 0", a `NotRead`. That is links 4 and 13's first stop past the
reads of #793.

**The hub's rulings on #796** (each Off only, logged, UNVERIFIED; Reference unchanged):
1. **the AI adopt** writes the mind's `CharacterClass` := the pawn's `AIClass` before `InitializeCharacterClass`, as
   `Possess` does. This narrows decision 2026-11-19-26 by that one write;
2. **the player set-up** runs `ApplyCharacterClassDefaults` with the controller's `Pawn` set to the player's pawn,
   logged as a harness input naming the pawn. **The `Pawn` stays set afterwards**, as the possess leaves it in the
   game. The later Off answers that read it:
   - `Pawn.GetController` (section 6.1) on the victim;
   - the god-mode test: #793 section 5's `InGodMode`, and section 6.5's `PawnCheckPlayHitPreconditions`;
   - `UpdateTookDamageFromEnemyStats`' credited player (section 6.2), through the instigator's `Pawn`;
3. **no cap on link 13's swings:** what the rules do is what the slice shows (section 7).

With section 3's rule, both recalculations then write the thirteen values.

## 5. The swing's damage, past the resistance (links 4 and 13)

With the resistance stubbed, each swing passes these stages of `AdjustDamage`. Both are read in NATIVE_DAMAGE_DEATH.md
section 1 but **not ported** (`damage.rs`, `left_out`, `NotPorted`):
- the shield stage for a victim with a shield pool (stage 10): Maya has one;
- the protection timer (stage 15, a player hit by an AI).

**Newly read: the summary's reset at entry.** `WillowDamagePipeline`'s damage virtual begins each pass by zeroing the
whole `DamageSummary`. It then sets **all eight bytes of both `DamageTakenSkillEventsTriggered` and
`DamageDealtSkillEventsTriggered` to 255** (255 is the list's end), and the stages add their tags after that. The
class defaults hold 0 in both lists. **The port never writes these lists**, so after its `AdjustDamage` they hold
0s. **Proposed:** the port's `AdjustDamage` writes the same reset at its start in Off. Section 6.4's iterator depends
on it. `DamageSummary` must be declared as a `DamageEventSummary` struct, and its two lists as eight-byte static
arrays, else `VmError::Script`, checked in the preflight (review r1 of #796). The reset is a write, so a refusal after
it carries the partial-writes prefix.

## 6. The natives on `TookDamageFromEnemy` and `PlayHit` (links 3, 4 and 13)

### 6.1 `Pawn.GetController()`

The pawn's `Controller`; else, when `DrivenVehicle` is set, that vehicle's `Controller`; else None. It is a plain
read, not a class virtual. **Off:** that rule. Both fields must be declared as objects, else `VmError::Script`, and
the vehicle must be resident, else `NotRead`.

### 6.2 `WillowGameInfo.UpdateTookDamageFromEnemyStats(Victim, InstigatedBy, MyCharacterClass, DamageType, DamageCauser, Pipeline, bool)`

**The call:** `WillowPawn.TookDamageFromEnemy` calls it only when the game info is a `WillowGameInfo` and the damage is
not the pawn's own (the instigator is not its controller).

**The native:**
1. **an AI class that does not count:** when `MyCharacterClass` is an `AIClassDefinition` whose
   `bCountsTowardDamageAndKillStats` is clear, nothing happens;
2. **the credited player P:**
   - `InstigatedBy` when it is a `WillowPlayerController`;
   - else, when `InstigatedBy`'s `Pawn` is a `WillowAIPawn`: its `PlayerMaster`, else its `ThoughtlockMaster` (both by
     fit, across the layout gap);
   - with no P, or no `InstigatedBy`: nothing happens;
3. **no stats object:** when P's `PlayerStats` is None, nothing happens;
4. **otherwise:** statistics writes. These are P's stat increments by name (damage-dealt shares and a distance), the
   challenge and telemetry recorders, and one skill event (26) on a P-side object. They were outlined, not read in
   full.

**Proposed Off answer:**
- steps 1 to 3 as read, write-free;
- step 4: `NotRead`, "UpdateTookDamageFromEnemyStats: the player's damage statistics";
- each field must be declared with its kind, else `VmError::Script`, and each object must be resident, else `NotRead`.

**On the slice** (probe):
- links 4 and 13: the instigator is the adult's mind, whose pawn has no master, so nothing happens;
- link 3: P is Maya's controller, whose `PlayerStats` is None in the port (also its class default), so nothing
  happens. In the game a live player has stats; that was not observed.

### 6.3 `WillowDamageSource.GetAmmoTheftAmounts(out Instigated, out Received, Source, InstigatorController, VictimController)`

A static native:
- **with no source, or a source class it does not match:** both outs are 0;
- **otherwise:** `Instigated` := the instigator controller's `Instigated<Kind>DamageAmmoTheftAmount`, and `Received`
  := the victim controller's `Received<Kind>DamageAmmoTheftAmount`. A missing controller gives 0. The kind comes from
  the source class:
  - Bullet, matched with its subclasses;
  - StatusEffect, Grenade and Rocket, exactly;
  - Melee, by the last test (by fit; it is the remaining pair of fields).

**The script, `WillowPawn.StealAmmo`:** a non-zero `Received` goes to the victim's own `Weapon.AddAmmo`, and a
non-zero `Instigated` to the instigator pawn's weapon.

**Off:** that rule. The values are the controllers' current int attribute values, write-free. Each of the ten
`Instigated<Kind>` and `Received<Kind>DamageAmmoTheftAmount` fields read must be declared as an int attribute, else
`VmError::Script`; an attribute read whose resolvers are not resident is `NotRead` (review r1 of #796). **On the
slice:** the swing is melee and both amounts are 0, so nothing is added.

### 6.4 `WillowDamagePipeline.DamageTakenSkillEventsTriggered(out byte)` and `DamageDealtSkillEventsTriggered(out byte)`

**Engine iterators** over `DamageSummary`'s two eight-byte lists. They yield each entry in order, and stop at the
first 255. As read, the loop ends when it reads the eighth entry, so at most seven are visited (UNVERIFIED: a quirk of
this reading). `WillowPlayerPawn.ProcessDamageTakenSkillEvents` hands each yielded tag to the skill manager's
`NotifySkillEvent`. `WillowPawn`'s version is empty.

**Off:** an iterator bound with that rule. `DamageSummary` and the list must be declared as in section 5, else
`VmError::Script`. It needs section 5's reset; without it the lists hold 0s, and the loop would yield tag 0 seven
times. **On the slice:** no stage adds a tag on the swing, so nothing is yielded.

### 6.5 `Pawn.PawnCheckPlayHitPreconditions(Pipeline)`

Answers false when the pipeline is set and its `DamageSummary.FinalDamage` is above 0. Else false when the pawn's
`Controller` is in god mode (#793 section 5's virtual). Else true. `Pawn.PlayHit` tests it first.

**Off:** that rule, load-free, as #793's `InGodMode`. `DamageSummary.FinalDamage` must be declared as a float, else
`VmError::Script`. **On the slice:** `FinalDamage` was 13.95, so false.

## 7. Predicted stops

**Link 13**, once link 4 passes. On each swing, in order:
1. the resistance `NotRead` (section 4), until section 4's inputs and section 3's rule are in;
2. the two `NotPorted` stages of section 5;
3. past sections 6.1 to 6.5: **`Actor.GetBoundingCylinder`'s "the component's world transform" `NotRead`**
   (NATIVE_COMPONENT_BOUNDS.md). The script reaches it in `WillowPawn.PlayHit`, then `Actor.CheckHitInfo`, then
   `Pawn.GetBoundingCylinder`. **The next read.**

**The swings may not down Maya.** In the probe each swing took about 13.9 health: 212.7 fell to 156.9 after link 4's
swing and link 13's three. That was with the shield stage skipped and the resistance stubbed at 1.0. So link 13's
three swings leave her near 157, not down (a further three would leave her near 115), and link 13's `down.total.` row
would not appear even with every stop cleared. Her class's twelve modifiers are 1 (NATIVE_LEVEL_UP_ATTRIBUTES.md's data), the stub's value. Her
shield, skipped here, takes damage first, so her health would fall less, not more. The game's numbers were not
observed: UNVERIFIED. The link's steps came from the C++ oracle (decision 2026-11-16-54). **The hub's ruling on #796:
no cap on the swings**; what the rules do is what the slice shows.

**Link 14:** it needs link 13's down. In the probe Maya was never down, so `bleed:50` found no injured state and did
nothing. Its own natives are NATIVE_GO_DOWN_PATH.md section 4's (from Reference, not reached in Off here).

**By the way, link 3** (#793 section 9): with its hit-region cap skipped, the kill then stops at `WillowAIPawn.GetParent`'s
`NotRead` (NATIVE_GO_DOWN_PATH.md 3.2).

## 8. Tests (synthetic fixtures)

1. **Section 3**, on a toy controller with a toy class and constant init data:
   - the thirteen values in order;
   - each value with a one-entry modifier stack;
   - a `Vehicle` pawn, no pawn, or no class: only the pools' rebase;
   - an unloaded init-data definition: `NotRead` before any write;
   - a mistyped field: `VmError::Script`.
2. **Section 4:**
   - a toy AI adopt with the `CharacterClass` write: the class-level part runs on the mind's recalculation;
   - the player set-up with the controller's `Pawn` set: Maya's toy modifiers are written; with it unset, skipped.
   - after the set-up the controller's `Pawn` is still the player's pawn, and the harness-input line names it;
   - Reference: no `CharacterClass` write and no `Pawn` change.
3. **Section 5:** after a toy `AdjustDamage`, both lists hold 255 in every byte except the tags a stage added.
4. **`GetController`:** the controller; none, with a vehicle's controller; none at all.
5. **`UpdateTookDamageFromEnemyStats`:**
   - a non-counting AI class: nothing;
   - an AI instigator with no master: nothing;
   - with a `PlayerMaster`, no `PlayerStats`: nothing;
   - a player instigator with `PlayerStats`: the `NotRead`.
6. **`GetAmmoTheftAmounts`:** each of the five kinds with toy controller values; a Bullet subclass; no source: 0 and 0;
   no victim controller: `Received` 0.
7. **The iterators:**
   - lists `[3, 9, 255, ...]`: yields 3 then 9;
   - eight non-255 entries: the first seven;
   - all 255: nothing.
8. **`PawnCheckPlayHitPreconditions`:** `FinalDamage` 5: false; 0 with no god mode: true; 0 with `bGodMode`: false;
   no pipeline: the god-mode test only.
9. Each read is write-free; #518's counters move only for section 3's writes. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - `UpdateTookDamageFromEnemyStats`' statistics writes (step 4);
   - the component world transform behind `GetBoundingCylinder`;
   - NATIVE_GO_DOWN_PATH.md section 4's natives in Off.
3. **By fit:**
   - `PlayerMaster` and `ThoughtlockMaster`' places;
   - the Melee pair in 6.3;
   - the iterators' seven-entry limit.
4. **Ruled by the hub on #796:** section 4's two inputs, and no cap on link 13's swings.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After the player spawns, read Maya's `NormalImpactDamageModifier` and its base value. Do the same for a fresh enemy,
   and read the enemy mind's `CharacterClass`.
2. Read `PlayerStats` on the player controller, and the `DamageSummary` tag lists right after a hit.
3. Count enemy melee hits until Maya goes down, at level 8, with and without a shield.
