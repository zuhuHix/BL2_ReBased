# Links 3 and 4 after the mind: the base search, SetPhysics on the player, IsFriendlyFire, InGodMode and the damage path (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK34-AFTER-MIND (decision 2026-11-26-51), the analyst half of the
clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/l34/`. This note is
in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here is
UNVERIFIED in the running game.**

**Why.** NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md (#788) predicts the next stops past the spawns:
- link 3's hit at `GameInfo.IsFriendlyFire`;
- link 4's swing at `Actor.SetPhysics` on a `WillowPlayerPawn`.

Its review r1 also found that both spawns now stop first in `SetDefaultPhysics`' own `SetPhysics` call, at "the base
search for a walking-type mode". This note reads those three, then follows links 3 and 4 as far as the probe went.
Link 5 inherits link 3's spawn and kill.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`):
  - `Pawn.TakeDamage` (its two `IsFriendlyFire` calls);
  - `Pawn`, `GearboxPawn` and `WillowPawn.SetMovementPhysics`;
  - `WillowPawn.AdjustDamage`.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read: the execs named below, and
  their class-table slots in the primary tables of these classes (the NATIVE_COMPONENT_SKIP_FLAG.md section 6 method):
  - `Actor`, `Pawn`, `WillowPawn`, `WillowAIPawn` and `WillowPlayerPawn`;
  - `GameInfo`, `WillowGameInfo` and `WillowCoopGameInfo`;
  - `Controller`, `PlayerController`, `WillowPlayerController`, `GearboxMind` and `WillowMind`;
  - `DamageTypeDefinition` and `WillowDamageTypeDefinition`;
  - the pawns' damage-surface interface table.
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. Fields are named where the routine's read lands
  exactly on the tool's place, except where a row says "by fit".
- **Constants:** three single floats read from the executable's data, recorded below (0.0001, 1.0 and 8.0).
- **`ow-package --def-dump`:** the game-info class defaults and the two archetypes.
- **A local probe** (temporary code, reverted, never committed): an installed Off `--combat-run` on main d3be3e5, under
  the machine lock, with a call trace and read-only prints. The steps were links 3 and 4's own after `player:8`:
  `spawn:baby:6`, `hit:0:9999::Bullet`, `spawn:adult:7`, `swing:1:melee`.

  It stubbed:
  - #788's earlier stops: the targetable resolver's step 0, `SetDefaultPhysics` past its gates, the mind part's
    refusal and the five plain reads;
  - then each stop below in turn. Each stub answered by the rule read here, except where a row says otherwise.

## 2. The base search (the spawns' first stop)

NATIVE_ACTOR_SET_PHYSICS.md section 2 leaves one `Actor` step unidentified: "into `PHYS_None`, `PHYS_Walking`,
`PHYS_Rotating` or `PHYS_Spider`, with no new floor, an actor virtual is called". It is now read.

**It is the base search**, one body shared by all five pawn and actor tables read (no class overrides it). In order:
1. **the floor query**, with a distance of **8.0**: a single line trace straight down by that distance, starting from
   the actor's `Location`. When the actor has a `CollisionComponent`, the start is offset by a translation the
   component holds (by fit, the component's own translation). The trace runs through the world's line check. Its
   answer is the actor hit and the hit normal, or none;
2. **when that actor differs from the current `Base`**, the base setter is called with it and the normal: the same
   setter as the transition's "the base is cleared" step, with the floor in place of None.

The trace's flags were not decoded, so **which kinds of objects it can hit is not read**.

**Proposed Off answer** (in `Actor.SetPhysics`' preflight, replacing "the base search for a walking-type mode"):
- no collision service in the world: `LayerAbsent`, "Actor.SetPhysics: the base search's floor trace", layer
  "collision (the world has no collision service)";
- with a service: `NotRead`, "Actor.SetPhysics: the base search's floor trace (its trace flags are not decoded)";
- **both are decided in the preflight, before the mode write** (review r1 of #793). Neither carries the
  partial-writes prefix, and the base search has no commit-time refusal;
- **for the hub, a possible stand-in:** in a world whose service is `NoLevel` (no level), and where no live actor
  holds collision, the trace hits nothing, so the floor is None:
  - with `Base` already None, the step changes nothing. That is a harness input to log, and only with the hub's
    ruling;
  - with a `Base` set, the floor (None) differs from it, so the base setter would run. Until lane E's SETBASE note (in
    progress) lands, that case is `NotRead`, "Actor.SetPhysics: the base search's base setter", decided in the
    preflight.

**What the `--combat-run` world gets** (probe; review r1 of #793): it installs no collision service. Only the slice
gate's pickup worlds install `NoLevel`. So the combat world's answer is the `LayerAbsent`.

**On the slice** (#788 r1): both spawns ask `SetPhysics` for `PHYS_None` from `PHYS_Walking`, with `Base` None.
Under the stand-in nothing changes past the mode write, and `Velocity` and `Acceleration` are zeroed (the existing
`PHYS_None` rule). `SetDefaultPhysics` then goes on to `bCollideWorld` and the collision copy.

## 3. `Actor.SetPhysics` on a `WillowPlayerPawn` (link 4)

**The player pawn has no physics-change body of its own.** Its class table keeps `WillowPawn`'s body, so its layers
are `WillowPawn`'s: the `WillowPawn`, `Pawn` and `Actor` layers, without the AI layer.

**Proposed Off answer:** NATIVE_ACTOR_SET_PHYSICS.md's layer table gains `WillowGame.WillowPlayerPawn` with
`WillowPawn`'s three layers, replacing its `NotRead` "the physics change of WillowGame.WillowPlayerPawn". Nothing else
in that rule changes.

**On the slice** (probe):
- **how the swing reaches it:** link 4's swing reaches it inside the player pawn's `TakeDamage`, through
  `SetMovementPhysics`:
  - `WillowPawn`'s version checks `bUnderMatineeControl`, `PHYS_Custom` and `bCanFly`;
  - `GearboxPawn`'s skips the change while a special move plays;
  - `Pawn`'s asks for `PHYS_Swimming` in a water volume, else `PHYS_Falling` unless the pawn is already falling;
- **the request:** `PHYS_Falling`. The port's player pawn was in `PHYS_None` at the call; a live player in the game
  walks, which was not observed. It had no `Base`, no `PhysicsVolume`, no `CollisionComponent`, and `bHardAttach`
  clear;
- **the outcome:** with the layer row added, the preflight passes, and the change writes `Physics` and runs the
  `WillowPawn` events whose flags are set.

## 4. `GameInfo.IsFriendlyFire(Victim, InstigatingTargetable)` (links 3 and 4)

**The call:** `Pawn.TakeDamage` asks the world's game info `IsFriendlyFire(Self, <the instigator's targetable>)`, twice
in one condition. No script body overrides it (#788 r1). The exec calls the game info's class-table virtual:
- **`GameInfo` and `WillowGameInfo`:** one body, **false**, reading nothing;
- **`WillowCoopGameInfo`:** its own rule (4.1).

**Proposed Off answer:**
- bind both native keys (`GameInfo.IsFriendlyFire(object,interface)` and `WillowCoopGameInfo`'s) in Off;
- dispatch on the receiver's class: `WillowCoopGameInfo` or a child takes 4.1's answer; any other class gives false;
- the arguments must be declared as an object and an interface, else `VmError::Script`;
- write-free.

**On the slice:** the combat-run world's game info is the harness's class-default `WillowGameInfo` (the stand-in of
decision 2026-11-16-49, `ow-combat` `death.rs`). So both calls answer **false**. The installed `DefaultGame` is
`WillowCoopGameInfo`, which the `--map` route spawns, and there 4.1 applies. **For the hub:** the stand-in's class
differs from the game's on this one native.

### 4.1 `WillowCoopGameInfo`'s rule (outline; four parts not named)

With V the victim and T the instigating targetable:
1. **the instigating controller C:**
   - when T is set: T's `GetTargetableActor()`;
   - when an actor query of that actor (not named) gives a pawn: that pawn's `Controller`, else its `DrivenVehicle`'s
     `Controller`;
   - T also answers one more query of its own (K, not named).
2. **"another's hit" (O):**
   - true when C's `Pawn` is set and is not V;
   - false when C has no pawn, or its pawn is V;
   - with no C found: true when V is set.
3. **"friendly" (F):**
   - with C and its pawn: that pawn's `IsFriendly(V)`;
   - else, when K: an allegiance test of V (an opinion lookup equal to 2; not named);
   - else false.
4. **H:** the game info's own `IsHumanControlled(V)`.
5. **the gate flag:**
   - with H, the answer is true at once when V answers a pawn query (not named) and C's pawn is V. Otherwise the gate
     flag is `bIgnoreFriendlyFire`;
   - without H, the gate flag is `bIgnoreNPCFriendlyFire`.
6. **true** when the gate flag is set and one of these holds:
   - O and F;
   - no H, C has a pawn, and either that pawn is not V and is `IsFriendly(V)`, or it is V and C has no
     `CharacterClass` or the class's `CanHarmSelf` is clear.
7. **otherwise** `GameInfo`'s false.

The two flags are `GameInfo` bits. Both are true in the `WillowGameInfo` and `WillowCoopGameInfo` class defaults
(`--def-dump`); an ini may change them, which was not read.

**Proposed Off answer for this class:** `NotRead`, "WillowCoopGameInfo.IsFriendlyFire: four parts are not named"
(the actor's pawn query, K, V's pawn query and the allegiance test), until a lane names them.

## 5. `Pawn.InGodMode()` (link 3, and the player on link 4)

**The pawn's native:** the pawn's `Controller`'s god-mode virtual. With no `Controller`, its `DrivenVehicle`'s
`Controller`'s. Else **false**.

**The controller classes:**
- **`Controller`, `PlayerController`, `WillowPlayerController`:** `bGodMode` or `bNonCheatGodMode`;
- **`GearboxMind`:**
  - the `Controller` rule first;
  - else, when **the mind's own `Pawn`** is set and has `bCanBeInjured` set (by the layout tool's place, UNVERIFIED):
    false. This is the mind's `Pawn`, read from the controller, also when the call comes through `Pawn.InGodMode` or
    directly through a controller key. With no `Pawn` the step is skipped and the `GodOnHold` step decides (review r1
    of #793);
  - else true exactly when the mind's `AIComponent` is set and its **`GodOnHold()`** answers true (that exec reaches the
    same class-table slot; NATIVE_AI_HOLDS.md, bound in Off);
- **`WillowMind`:** true when the **process-wide AI-god flag** is set. That is the static value behind
  `WillowMind.GetAIGod()` and `SetAIGod(bool)`, initially false as an unset static (UNVERIFIED). Otherwise
  `GearboxMind`'s rule.

The script keys `Controller.InGodMode`, `GearboxMind.InGodMode` and `WillowMind.InGodMode` share one exec, which calls
the same virtual.

**Proposed Off answer:**
- bind `Pawn.InGodMode` and the controller keys in Off to this rule, dispatched on the controller's class;
- the AI-god flag is a VM-wide value, initially false. `GetAIGod` and `SetAIGod` are bound in Off to read and write it;
- each field must be declared with its kind, else `VmError::Script`;
- **load-free** (review r1 of #793): every object it reads (the `Controller`, the `DrivenVehicle`, that vehicle's
  `Controller`, the mind's `Pawn` and its `AIComponent`) must be resident, else `NotRead`, "Pawn.InGodMode: an object
  the store has not loaded", decided before anything is evaluated;
- `GodOnHold` answers through the existing hold lists;
- write-free.

**On the slice** (probe and `--def-dump`):
- link 3, the baby: its mind has `bGodMode` and `bNonCheatGodMode` false. Both archetypes have `bCanBeInjured` false,
  and the mind's god hold list is empty. So the answer is **false**;
- link 4, Maya: her `WillowPlayerController` has both flags false, so the answer is **false** (review r1 of #793).

## 6. `DamageTypeDefinition.IsHealingDamageType()` (link 4)

- **`DamageTypeDefinition`:** reports an internal error (an engine log line, nothing script-visible) and answers false;
- **`WillowDamageTypeDefinition`:** true exactly when its `DamageType` is `DAMAGE_TYPE_Healing` (6).

**Proposed Off answer:** that rule. `DamageType` must be declared as a byte, else `VmError::Script`; the base class
gives false. **On the slice:** the swing's damage type has `DAMAGE_TYPE_Impact` (5), so false.

## 7. `WillowPawn.AdjustMomentumForSurfaceType(Magnitude, InstigatedBy, DamageType, HitInfo)` (both links)

**The script:** `WillowPawn.AdjustDamage` makes `Momentum` its own direction times this native's answer for
`VSize(Momentum)`, the instigator, the pipeline's damage type and the hit.

**The native:**
- no damage type: `Magnitude` unchanged;
- else `Magnitude` times the damage type's **momentum scale** for the victim's **surface** at the hit, with
  `InstigatedBy` as the context.

**The momentum scale:**
- **`WillowDamageTypeDefinition`:**
  - surface 1: `FleshMomentumModifier`;
  - surface 2: `ArmorMomentumModifier`;
  - surface 3: `ShieldMomentumModifier`;
  - each is attribute init data, evaluated with `InstigatedBy` as the context source;
  - any other surface: **1.0**;
- **base `DamageTypeDefinition`:** 0.0.

**The victim's surface for a hit**, through the pawn's damage-surface interface (one body for `WillowPawn`,
`WillowAIPawn` and `WillowPlayerPawn`):
1. **3** (shield) when `ShieldArmor` names a live pool whose `CurrentValue` is above 0;
2. else, with `MyHitRegionHelper` and `BodyClass` set:
   - the region is the first entry of `BodyClass.HitRegionList` whose `RegionBoneNames` holds `HitInfo.BoneName`,
     else `BodyClass.DefaultHitRegion`. With no region the answer is 0;
   - the region's `PhysicalMaterial`'s `PhysicalMaterialProperty`, when that is a `WillowPhysicalMaterialProperty`, gives
     its `DamageSurfaceType` (by fit: the routine's class check names a `WillowGame` class, and this is the class that
     declares the field);
   - else the region's `DefaultDamageSurfaceType`;
3. else 0.

**Proposed Off answer:**
- **preflight**, write-free and load-free:
  - every field must be declared with its kind, else `VmError::Script`;
  - every object read must be resident, else `NotRead`;
  - the chosen init data's attribute definition and its resolvers must already be resident, else `NotRead`;
- **the value:** through `ow-attr`'s evaluator;
- nothing is written.

**On the slice:**
- link 3's hit has no damage type, so the magnitude is unchanged;
- link 4's swing has a damage type and a magnitude of 0, so the answer is 0 whatever the scale. The rule still
  evaluates it.

**A lead for AdjustDamage's stage 12** (NATIVE_DAMAGE_DEATH.md section 1: "the hit region's
`DefaultDamageSurfaceType`, the physical material of the hit, or the pawn default"). This interface's rule gives
exactly those three sources, with 0 as the pawn's default. Whether that stage calls this same interface was **not
checked**.

## 8. `Pawn.IsVulnerableToFriendlyFire()` (link 4)

- **`Pawn`, `WillowPawn` and `WillowAIPawn`:** one shared body, **false**;
- **`WillowPlayerPawn`:** true when its **`FriendlyFireDamageAcceptedBaseValue`** is above **0.0001**: the field at
  the layout tool's place for the read (review r1 of #793). UNVERIFIED: with the known one-field gap on late Willow
  pawn fields, the read would be the attribute's value, `FriendlyFireDamageAccepted`, instead.

**Proposed Off answer:** bind the `Pawn` key and `WillowPlayerPawn`'s own key in Off to this rule, dispatched on the
receiver's class; the field must be declared as a float attribute, else `VmError::Script`. **On the slice:** the
player's `FriendlyFireDamageAccepted` was 0.0 in the probe, so false under either reading.

## 9. Predicted stops (from the probe, each earlier stop stubbed)

- **The spawns of links 3 and 4** (link 5 runs on link 3's, links 13 and 14 on link 4's): `SetDefaultPhysics`' base
  search (section 2) comes first, as the `LayerAbsent` in the combat world. With the hub's stand-in they pass, to #788 section 5's next stop.
- **Link 3's `hit`** passes `IsFriendlyFire` (false, twice), `InGodMode` (false) and `AdjustMomentumForSurfaceType`
  (unchanged). It then stops at **`AdjustDamage`'s hit-region cap** for `GD_PrimalBeastShared.HitRegions.HitRegion_Body`,
  whose `bTrackDamage` is set. That is the port's existing `NotPorted`; its rule (the hit-region consumer's
  `AddDamageToHitRegion`) is only outlined in NATIVE_DAMAGE_DEATH.md section 1 stage 13. **The next read.**
- **Link 4's `swing`** passes, in the probe's order:
  1. `SetPhysics` (section 3);
  2. `IsFriendlyFire` (false);
  3. `IsHealingDamageType` (false);
  4. `IsFriendlyFire` again (false);
  5. `InGodMode` on Maya's controller (false);
  6. `IsVulnerableToFriendlyFire` (false);
  7. `AdjustMomentumForSurfaceType` (0).

  It then stops at **`AdjustDamage` stage 11's existing `NotRead`**, "the damage surface of a hit whose region names
  none". Section 7's surface rule is the lead for it.
- **Link 5** stops with link 3. **Links 13 and 14** have no spawn of their own: link 13 swings link 4's adult three
  more times and link 14 bleeds out, so they meet link 4's stops first; they were not probed here.

In the slice gate these links start where link 1 ends (NATIVE_MAYA9_AND_AFTER.md section 5).

## 10. Tests (synthetic fixtures)

1. **The base search:**
   - a toy pawn going from `PHYS_Walking` to `PHYS_None` in a world with no collision service: `LayerAbsent`, nothing
     written;
   - with a collision service: `NotRead`;
   - (only if the hub rules for the stand-in) under `NoLevel` with `Base` None: the mode written, `Velocity` and
     `Acceleration` zeroed, `Base` unchanged; with a `Base` set: the base setter's `NotRead`, nothing written.
2. **`SetPhysics` on a toy `WillowPlayerPawn`:**
   - from `PHYS_None` to `PHYS_Falling`: `Physics` 2, and `BeganFalling(0)` when `bNotifyBeginFalling` is set;
   - `PHYS_NavMeshWalking`: no AI step.
3. **`IsFriendlyFire`:**
   - a toy `WillowGameInfo` and a plain `GameInfo`: false;
   - a toy `WillowCoopGameInfo`: the `NotRead`;
   - a mistyped argument: `VmError::Script`.
4. **`InGodMode`:**
   - a pawn with no controller and no vehicle: false;
   - a controller with `bGodMode`: true; with `bNonCheatGodMode`: true;
   - a toy `WillowMind`:
     - the AI-god flag set: true;
     - flag clear, pawn `bCanBeInjured` set: false;
     - clear, a god hold present: true; none: false;
   - `SetAIGod` / `GetAIGod` round trip;
   - an unloaded `Controller`, `DrivenVehicle` or `AIComponent`: the not-loaded `NotRead`, nothing evaluated;
   - a toy mind with no `Pawn`, called through the controller key: the `GodOnHold` step decides;
   - a vehicle driver's controller.
5. **`IsHealingDamageType`:** `DamageType` 6: true; 5: false; a base `DamageTypeDefinition`: false.
6. **`AdjustMomentumForSurfaceType`:**
   - no damage type: the magnitude;
   - a toy `WillowDamageTypeDefinition` with constant init data, on a toy pawn:
     - a live shield pool with value: the shield scale;
     - a hit region by bone whose physical material's property gives 2: the armor scale;
     - a region with neither: its `DefaultDamageSurfaceType`;
     - no helper: 1.0;
   - a base `DamageTypeDefinition`: 0.0;
   - an unloaded init-data definition: `NotRead` before anything is evaluated.
7. **`IsVulnerableToFriendlyFire`:**
   - a toy `WillowPlayerPawn` with the value 0.0: false; with 0.5: true;
   - a `WillowAIPawn`: false.
8. Each answer is write-free: #518's three counters are unchanged, apart from the base search's mode write.
9. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - the base search trace's flags;
   - `WillowCoopGameInfo.IsFriendlyFire`'s four unnamed parts;
   - whether AdjustDamage's stage 12 uses section 7's surface query;
   - the hit-region consumer's `AddDamageToHitRegion`.
3. **By fit:**
   - the collision component's offset in the floor query;
   - `WillowPhysicalMaterialProperty` as the surface rule's class;
   - `FriendlyFireDamageAccepted` against its base value;
   - `bCanBeInjured`'s place.
4. **For the hub:**
   - the base search's `NoLevel` stand-in (section 2);
   - the combat world's `WillowGameInfo` against the game's `WillowCoopGameInfo` (section 4).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. **`IsFriendlyFire`:** read `WorldInfo.Game`'s class and its `bIgnoreFriendlyFire` / `bIgnoreNPCFriendlyFire` in a
   normal game, and call `IsFriendlyFire(enemy, player)` and `(player, enemy)`.
2. **The swing's physics:** read a fresh enemy's `Base` and `Physics` right after its spawn, and the player's `Physics`
   before and after an enemy's melee hit.
3. **The momentum and surface:**
   - read `FleshMomentumModifier` and its siblings on a melee damage type;
   - read the surface a shot on a shielded and an unshielded enemy reports.
4. **`InGodMode`:** call `InGodMode()` on an enemy pawn, and on the player with `bGodMode` set.
