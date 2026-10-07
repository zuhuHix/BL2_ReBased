# Native combat groundwork: pool references, pawn health glue, damage pipeline objects, kill notification, kill experience, allegiance (2026-10-07)

AI-assisted (Claude), analyst lane G24. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Nothing in this note was confirmed in the running game.
Script behaviour was read with `research/script_disasm.py` (the local WillowGame listing and a local Engine listing);
field names were matched to the native code with `tools/ghidra/class_layout.py` and a local copy of it that sizes a map
property (the same copy G23 used); data values come from the installed packages (`ow-package --exports/--properties`
through the existing `tools/weapon_recipe.py` decoder). One data read needed a different method, described in
section 3.4: class defaults of attribute properties are reported as "unsupported" by the property decoder, so the raw
floats were read from the cooked default object at the offsets the decoder lists. Raw output lives under ignored
`local/analysis/G24/` and in the Ghidra project copy's out folder.

Scope: what the combat "groundwork" swap needs before the stock hit chain can run on VM pawns (swap plan
`local/p2/S1/combat_swap_plan.md` sections 4 and 6): the pool reference glue, the pawn health natives, the damage pipeline
object pool, the open items of [NATIVE_DAMAGE_DEATH.md](NATIVE_DAMAGE_DEATH.md) (DD) section 1, what the mission tracker
does when a pawn dies, the kill experience amount, and the allegiance natives. Overlaps: G23
([NATIVE_LEVEL_UP_ATTRIBUTES.md](NATIVE_LEVEL_UP_ATTRIBUTES.md), pool rebase and level-up; not redone), G11 (DD),
C1 (mission tracker, [NATIVE_MISSION_DISPATCH.md](NATIVE_MISSION_DISPATCH.md)), G3 (engine core).

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| ResourcePoolManager.CreatePool (called through the class) | `native function ResourcePoolReference CreatePool(out ResourcePoolManager Mgr, Actor ActorOwner, ResourcePoolDefinition PoolDefinition, optional bool bReinitializeExistingPool)` | every pool (health, shield, exp, cooldown, ammo, accuracy) | high | UNVERIFIED |
| ResourcePoolManager.CreatePoolAtIndex / InternalCreatePoolAtIndex | `CreatePoolAtIndex(out ResourcePoolManager Mgr, Actor ActorOwner, ResourcePoolDefinition PoolDefinition, int Index, byte PoolGUID)` returning `ResourcePool`; `InternalCreatePoolAtIndex(ResourcePoolDefinition PoolDefinition, int Index, byte PoolGUID)` | the helper under CreatePool | high | UNVERIFIED |
| ResourcePoolManager.DeletePoolByReference / DeletePool / DeletePoolAtIndex / DeleteAllPools | `DeletePoolByReference(out ResourcePoolReference PoolReference)`, `DeletePool(ResourcePoolManager Mgr, ResourcePoolReference PoolReference)`, `DeletePoolAtIndex(int Index)`, `DeleteAllPools(ResourcePoolManager Mgr)` | pawn death (health pool), class switch | high (DeleteAllPools, DeletePoolAtIndex), medium (the two by-reference forms) | UNVERIFIED |
| ResourcePoolManager.HasResourcePoolForResourceDefinition / GetResourcePoolForResourceDefinition / CreateResourcePoolManagerIfNecessary | `HasResourcePoolForResourceDefinition(ResourceDefinition Resource)`, `GetResourcePoolForResourceDefinition(out ResourcePoolManager Mgr, Actor ActorOwner, ResourceDefinition Resource)`, `CreateResourcePoolManagerIfNecessary(out ResourcePoolManager Mgr, Actor ActorOwner)` | lookups by resource | high | UNVERIFIED |
| Controller.GetResourcePoolForResourceDefinition | `native function ResourcePoolReference GetResourcePoolForResourceDefinition(ResourceDefinition Resource, optional bool bCreatePoolIfOneDoesntExist)` | ammo, accuracy pools (lazy creation) | medium-high | UNVERIFIED |
| Controller.ApplyCharacterClassStartingValues (virtual; WillowPlayerController override) | `native function ApplyCharacterClassStartingValues(CharacterClassDefinition inCharacterClass)` | creates the player's pool set; fixes slot order | high for the pool part | UNVERIFIED |
| Actor.IsResourcePoolValid | `native function bool IsResourcePoolValid(out ResourcePoolReference PoolReference)` | health, shield, exp, cooldown checks | high | UNVERIFIED |
| Actor.GetResourcePoolCurrentValueOrDefault | `native function float GetResourcePoolCurrentValueOrDefault(out ResourcePoolReference PoolReference, float DefaultValue)` | pipeline reads of health and shield | high | UNVERIFIED |
| Pawn.SetHealthResourcePoolDefinition | `native function SetHealthResourcePoolDefinition(out ResourcePoolManager ResourcePoolManager, ResourcePoolDefinition Definition)` (virtual, not overridden by WillowPawn) | creates Maya's and the dummy's health pool | high | UNVERIFIED |
| WillowPawn.SetShieldResourcePoolDefinition | script, not native | shield pool | high (script) | UNVERIFIED |
| Pawn.GetHealth / SetHealth / GetMaxHealth / SetMaxHealth | `native function float GetHealth()`, `SetHealth(float NewHealth)`, `float GetMaxHealth()`, `SetMaxHealth(float NewMaxHealth)` | every hit | high | UNVERIFIED |
| Actor.GetDamagePipeline / ReleaseDamagePipeline | `native function DamagePipeline GetDamagePipeline(optional DamageTypeDefinition InDamageTypeDef, optional ImpactDefinition InImpactDef, optional bool inCanHitFriendly, optional Object InHitObject)`, `ReleaseDamagePipeline(DamagePipeline Pipeline)` | every hit that does not pass a pipeline | high | UNVERIFIED |
| Actor.GetPipelineDamageType / GetPipelineImpactDefinition | `native function DamageTypeDefinition GetPipelineDamageType(DamagePipeline Pipeline)`, `ImpactDefinition GetPipelineImpactDefinition(DamagePipeline Pipeline)` | every hit | high | UNVERIFIED |
| WillowGlobals pipeline pool (virtual acquire and release; also WillowGlobals.GetDamagePipelineObject / ReleaseDamagePipelineObject) | native virtuals of the globals object | under the two rows above | high | UNVERIFIED |
| DamagePipeline.Initialize / ResetToDefaults | `native function Initialize(DamageTypeDefinition, ImpactDefinition, bool, Object)`, `ResetToDefaults()` (virtual) | under GetDamagePipeline | high | UNVERIFIED |
| WillowDamagePipeline.AdjustDamage, open items only | see DD section 1 | every hit | high for the three items closed here | UNVERIFIED |
| MissionTracker.NotifyPawnDied | `native function NotifyPawnDied(WillowPawn KilledPawn, Controller InstigatedBy, class<DamageType> DamageTypeClass, DamageTypeDefinition DamageTypeDefinition, IDamageCauser DamageCauser, bool bKilledByCritical, optional TraceHitInfo HitInfo)` | kill objectives (nil effect on the Fire mission) | high for the three effects, medium for the filter | UNVERIFIED |
| MissionTracker.NotifyActorDied | `native function NotifyActorDied(Actor DeadActor)` | defend-target bookkeeping (nil on Fire) | high | UNVERIFIED |
| WillowExperiencePipeline.CalculateExperiencePointsForKill | `static native function float CalculateExperiencePointsForKill(WillowPlayerController Killer, Actor KilledActor, bool bFatalBlowWasCriticalHit, class<DamageType> FatalBlowDamageSource, DamageTypeDefinition FatalBlowDamageTypeDefinition)` | kill XP (off the Fire gate) | high for structure, medium for the combat-multiplier helper | UNVERIFIED |
| WillowExperiencePipeline.AwardCombatExperienceToParty | `static native function AwardCombatExperienceToParty(float ExperiencePoints, WillowPlayerController Killer, Actor KilledActor)` | kill XP | high for structure, medium for the display part | UNVERIFIED |
| Pawn.IsEnemy / IsFriendly / IsNeutral / GetOpinion (ITargetable) | `bool IsEnemy(Object anObject)`, `IsFriendly`, `IsNeutral`; `byte GetOpinion(Object Other)` | `WillowGameInfo.Killed` event choice | high for the generic rule, low for the WillowPawn override | UNVERIFIED |

Not native (no note needed): `TeleporterDestination.GetNextExitPoint` is Engine script (it runs on the VM unchanged; its rule is
in [SLICE_WORLD_PLACEMENT.md](SLICE_WORLD_PLACEMENT.md)). `WillowPlayerReplicationInfo.GetCurrencyOnHand` / `AddCurrencyOnHand` are already
covered in [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) and were not re-read.

## The result in one paragraph

A pool reference is a small value: a pointer to the pool manager, the pool's slot number (0 to 15), a one-byte generation
number (the "GUID"), and a cached pointer to the pool. It is valid only while the manager's slot holds a pool whose own
generation byte equals the reference's, so deleting a pool and creating another in the same slot invalidates old references. Pools
are created in the **lowest free slot**; a resource (for example "health") can have only one pool per manager. A player's
manager is filled in a fixed order by the character class: the class's resource list in array order (Maya: seven ammo pools,
slots 0 to 6), the accuracy pool, then experience, action-skill cooldown and melee cooldown; the health pool and the shield pool
come later, when the pawn gets its defaults. `Pawn.GetHealth` returns the health pool's current value when the pawn's reference resolves
and its plain health variable otherwise; `SetHealth` and `GetMaxHealth` follow the same rule, and `SetMaxHealth` only ever
writes the plain variable. Damage pipeline objects come from a pool owned by the globals object: first free entry, reset to
class defaults, marked in use; release only clears the mark.

## 1. Pool references and the manager

### 1.1 Data layout (names; computed offsets agree with the registered class sizes)

- **ResourcePoolReference** (struct, 12 bytes): `PoolManager` (object), `PoolIndexInManager` (byte), `PoolGUID` (byte), `Data`
  (the cached `ResourcePool`). The **invalid reference** is: manager None, index 255, GUID 255, data None. Every native
  that fails to produce a pool returns or leaves that value.
- **ResourcePoolManager** (an Actor, a `ReplicationInfo`): `ResourcePools[16]` (object array, 16 slots), `ResourcePoolMap` (a
  map from `ResourceDefinition` to a slot number, 60 bytes in the in-memory layout), `NextPoolGUID` (byte), and replicated
  mirrors (identities, current, min and max values per slot; the replication side was not read). Registered size equals the
  computed size, which is the oracle for the offsets.
- **ResourcePool**: `PoolGUID` (byte), `Definition`, `AssociatedProvider`, and the value fields listed in
  [NATIVE_LEVEL_UP_ATTRIBUTES.md](NATIVE_LEVEL_UP_ATTRIBUTES.md). `ResourcePoolDefinition.Resource` names the
  `ResourceDefinition`; `ResourceDefinition.DefaultResourcePoolClass` is the class instantiated for a pool of that resource and
  `ResourceDefinition.DefaultResourcePoolDefinition` is the definition used for lazy creation (section 1.4).
- **Pawn**: `HealthVar`, `HealthMaxVar` (plain floats), `HealthPool` (a `ResourcePoolReference`). `WillowPawn.ShieldArmor` is the
  shield pool reference (despite its name, the shield pool). `Controller.ResourcePoolManager` is the controller's manager;
  `Controller.AccuracyPool`, `OffHandAccuracyPool`; `WillowPlayerController.ExpPool`, `SkillCooldownPool`,
  `MeleeSkillCooldownPool`.

### 1.2 How a reference is resolved

Used by every native below. "Resolve" means:

1. No manager: not valid, and the cached pool is cleared to None.
2. Slot number 16 or more: not valid, cache cleared.
3. The slot's pool is read. It is stored in the reference's cache **before** the generation check, then, if the slot is empty or the
   pool's `PoolGUID` differs from the reference's GUID byte, the cache is cleared again and the reference is not valid. A match
   leaves the cache set and the reference valid.

So validity is exactly "slot non-empty and GUID byte equal"; the stored index and GUID are authoritative, the cache is only a
shortcut. References are compared by value only through this rule; the manager's `ResourcePoolMap` is not consulted.

### 1.3 ResourcePoolManager.CreatePool(out Mgr, ActorOwner, PoolDefinition, bReinitializeExistingPool)

- **Reads:** the manager (if any), its Role byte, its 16 slots, each slot pool's `Definition` and that definition's `Resource`,
  `NextPoolGUID`, and whether a game world exists.
- **Does (outcomes, in order):**
  1. The result starts as the invalid reference. With no `PoolDefinition` the call ends there.
  2. The manager is made if necessary (section 1.5). If there is still no manager, or the manager's Role is not authority (value 3), the result is the invalid reference.
  3. All 16 slots are scanned in order. A used slot "matches" when its pool's `Definition` is the same object as `PoolDefinition`, or the two definitions
     share the same `Resource` object. The first empty slot is remembered.
  4. **A match and `bReinitializeExistingPool` false:** nothing changes and the result is the invalid reference (a second pool
     for the same resource is refused; callers check first with `GetResourcePoolForResourceDefinition`).
  5. **A match and `bReinitializeExistingPool` true:** the pool's script function `Reinitialize` runs (name read as that; its
     FName was not resolved). The new GUID is taken from `NextPoolGUID` (the value before the increment), `NextPoolGUID` is incremented, and the result
     is manager, matched slot, **that new GUID**, cached pool None. The pool's own `PoolGUID` is **not** changed, so this result does not
     validate against the pool (a quirk read from the code, **medium confidence**; it happens when the pawn's health pool reference is stale but the
     controller's manager still holds a health pool).
  6. **No match, an empty slot exists:** GUID taken as in step 5, then `CreatePoolAtIndex` (section 1.4) creates the pool in the
     **lowest empty slot**; the result is manager, slot, GUID and the new pool as the cache. If the pool could not be created the result stays invalid.
  7. **No match and all 16 slots used:** an on-screen debug message (5 seconds) "Too many ResourcePools" naming the owner and the definition is queued
     (presentation) and the result is the invalid reference.
- **Calls other natives / script:** `CreatePoolAtIndex`; the pool's script `Reinitialize`.
- **Constants:** 16 slots; GUID counter is a byte starting at 0 (class default), so the first pool of a manager has GUID 0, the next 1, and so on. At wrap (after 255 creations) the counter
  restarts near 0; not reachable in the slice.
- **Edge cases:** `ActorOwner` None is allowed once a manager exists. A manager passed in `Mgr` is used as is (the player's is the controller's field).
- **Implementer checklist:**
  1. First pool in an empty manager lands in slot 0 with GUID 0 and the reference is valid.
  2. A second call for the same resource without reinit returns the invalid reference and changes nothing.
  3. After `DeletePoolAtIndex(i)` the next created pool takes slot i (lowest free), with the next GUID, and old references to slot i are invalid.
  4. 17th distinct resource returns the invalid reference.
  5. The manager must carry Role 3 or nothing is created.
- **Open:** the FName of the called script function (read as `Reinitialize`, no arguments, which matches the zero-argument call).

### 1.4 CreatePoolAtIndex / InternalCreatePoolAtIndex (manager virtual, same body)

- **Does:** with a non-None definition, an index below 16 and an **empty** slot: creates a pool object of the class named by `PoolDefinition.Resource.DefaultResourcePoolClass`,
  stores it in the slot, sets its `PoolGUID` byte to the given GUID, then, if the manager is authoritative, runs the pool's script function
  `InitializeAuthoritative(PoolDefinition, manager.Owner)` (read as that name; arguments match: definition, then the provider to associate), otherwise only sets
  the pool's `Definition`. It then records `Resource -> Index` in `ResourcePoolMap` and returns the pool. Any refusal returns None.
- **`AssociatedProvider`** (the context of the base-maximum formula, an open item of G23) therefore is the **manager's `Owner` actor**: for a controller's
  manager that is the controller (the manager is spawned with the controller as owner), for a pool made through `Pawn.SetHealthResourcePoolDefinition` it is still
  the manager's owner, the controller, because the manager is the controller's.
- **Pool start state** is in the script `InitializeAuthoritative` (runs on the VM): current value from `StartingValue`, then the minimum if `StartWithMinValue`, the maximum if
  `StartWithMaxValue` (and `PoolIsNowFull`), then clamped; `UpdateLastValues` and `ResetIdleTimer`.
- **Implementer checklist:** index and slot refusal rules above; the map entry is what `GetResourcePoolForResourceDefinition` and `DeletePoolByReference` use.

### 1.5 CreateResourcePoolManagerIfNecessary and the manager lookups

- **CreateResourcePoolManagerIfNecessary(out Mgr, ActorOwner):** when `Mgr` is None, `ActorOwner` is non-None with Role 3, and a world exists, spawns a
  `ResourcePoolManager` actor owned by `ActorOwner`, checks it really is of that class, and stores it in `Mgr`; otherwise nothing. The shared first step of
  `CreatePool`, `GetResourcePoolForResourceDefinition` and the controller lookup.
- **GetResourcePoolForResourceDefinition(out Mgr, ActorOwner, Resource):** makes the manager if necessary (and Mgr is None); with a manager, looks `Resource` up in
  `ResourcePoolMap`; if found and the slot is in range the result is manager, slot, the slot pool's `PoolGUID` (255 when the slot is empty) and the cache is filled by the resolve rule;
  otherwise the invalid reference. No creation.
- **HasResourcePoolForResourceDefinition(Resource):** that lookup plus the validity rule, as a bool.
- **Controller.GetResourcePoolForResourceDefinition(Resource, bCreate=true):** result starts invalid. With a `Resource`, and a manager or `bCreate`: the controller's manager is made if
  necessary and the lookup above is tried; **if the controller drives a vehicle that provides pools, that provider's lookup is tried next** (read from a virtual
  call on the controller's pawn, vehicle path not needed for the slice); and if still no valid pool and `bCreate` is true, `CreatePool(manager, controller,
  Resource.DefaultResourcePoolDefinition, no reinit)` is called and its result returned. This is how ammo and other lazily needed pools come to exist.
- **Implementer checklist:** lookup by resource uses the map, not a scan; creation uses the resource's default definition, owner is the controller.

### 1.6 Deletion

- **DeletePoolAtIndex(i)** (manager virtual): for a used slot below 16: removes the pool's `Resource` from the map, runs the pool's script function (read as `ShutDown`,
  which zeroes its values and clears `Definition` and `AssociatedProvider`), empties the slot and marks the manager as needing a network update. The pool object is not otherwise destroyed.
- **DeleteAllPools(Mgr):** for a manager with Role 3, `DeletePoolAtIndex` for every slot 0 to 15 in order.
- **DeletePoolByReference(out PoolReference) / DeletePool(Mgr, PoolReference):** with a manager that has Role 3: the reference must resolve (same rule as section 1.2; an uncached reference is resolved
  first and a stale one deletes nothing). The pool's `Definition` and `Resource` must be non-None. The slot to delete is the one the **map gives for that resource** (normally
  the reference's own slot), by `DeletePoolAtIndex`. `DeletePoolByReference` then **clears the reference it was given to the invalid value in every case**
  (so `Pawn.HealthPool` becomes invalid when the health pool is destroyed); `DeletePool` takes the reference by value and leaves the caller's copy alone.
- **Implementer checklist:** after deleting the health pool by reference the pawn's `HealthPool` is the invalid reference and `GetHealth` falls back to `HealthVar`; the freed slot is reused by the next creation.

### 1.7 Actor.IsResourcePoolValid and GetResourcePoolCurrentValueOrDefault

- **IsResourcePoolValid(ref):** the resolve rule of section 1.2; also leaves the cache filled (valid) or cleared (invalid) in the **caller's** reference.
- **GetResourcePoolCurrentValueOrDefault(ref, Default):** if the reference resolves, the pool's `CurrentValue` (and the cache is filled); otherwise the cache is cleared and `Default` is returned.
  No clamping, no `MaxValue` involved.
- **Implementer checklist:** returns the stored current value of the pool exactly; the default only when the reference does not resolve (including a manager of None).

### 1.8 Pawn.SetHealthResourcePoolDefinition(out Mgr, Definition)

- **Reads:** the pawn's Role, its `HealthPool`, `Controller`, the pool maximum.
- **Does:** only for a pawn with Role 3. If `HealthPool` is not valid: `CreatePool(Mgr, pawn.Controller, Definition, reinit **true**)` and the returned reference (manager, slot, GUID, cached pool) is stored
  into `HealthPool`. Then, if `HealthPool` is valid after that, **`HealthMaxVar` is set to the pool's effective `MaxValue`**. A pawn that already has a valid health pool is not given a second one; its `HealthMaxVar` is just
  refreshed. It does not set the current value (the pool's own start rules do, `StartWithMaxValue` for the player and enemy pools in the data).
- **Callers:** `Controller.ApplyCharacterClassDefaults` (script) passes the controller's manager and `CharacterClass.HealthPoolDefinition`; `WillowPawn.SetHealthResourcePoolDefinition` (script) calls it and then copies the
  new reference into the player's replication info; `WillowVehicle` has its own.
- **Implementer checklist:** the pawn gets a pool in the controller's manager in the lowest free slot; `GetMaxHealth` equals the pool maximum afterwards; calling it twice does not create a second pool; the pawn must have Role 3.
- **Open:** the case "stale reference but the manager still has a health pool" (section 1.3 step 5): under that case this native leaves `HealthPool` invalid and `HealthMaxVar` unchanged.

### 1.9 WillowPawn.SetShieldResourcePoolDefinition (script, for completeness)

Script only: with Role 3, if `ShieldArmor` is not valid, `ShieldArmor := CreatePool(Mgr, pawn.Controller, Definition)` (no reinit), then the replication info gets a copy. The
call sites are `WillowPlayerController.ApplyCharacterClassDefaults` (player class shield pool) and the mind's version (AI class).

### 1.10 Slot order: the answer to the question left open by NATIVE_LEVEL_UP_ATTRIBUTES section 2

Rule: **lowest free slot, first come first served; one pool per resource.** The creation order that matters is the one the class produces:

1. `WillowPlayerController.SwitchPlayerClass` calls `ApplyCharacterClassStartingValues(CharacterClass)` (virtual). The base (Controller) part first deletes **every** pool of the controller's manager (`DeleteAllPools`),
   then creates, in order: each entry of the class's `ResourcePools` array (array order), then `AccuracyPoolDefinition`, then `OffHandAccuracyPoolDefinition`, each only if set; the accuracy references are stored into the controller's `AccuracyPool` and `OffHandAccuracyPool`.
2. The player-controller override then, **only with Role 3** and a `PlayerClass`, creates and stores `ExperiencePoolDefinition` into `ExpPool`, `SkillCooldownPoolDefinition` into `SkillCooldownPool` and
   `MeleeSkillCooldownPoolDefinition` into `MeleeSkillCooldownPool`, then calls one parameterless script function on the controller (name not resolved).
3. When the pawn gets its defaults (`WillowGameInfo.SetPlayerDefaults` calls `ApplyCharacterClassDefaults`): `Controller.ApplyCharacterClassDefaults` creates the **health** pool (section 1.8) and reinitialises the two accuracy pools if valid; the
   `WillowPlayerController` version then creates the **shield** pool and ends with `RecalculateAttributeInitializedState` (G23).
4. Pools not in the class lists (other ammo, grenade resources) appear on first use through `Controller.GetResourcePoolForResourceDefinition(.., true)`.

For Maya's data (class `GD_Siren.Character.CharClass_Siren`, read from the package) this gives: slots 0 to 6 the seven ammo pools in the class order (Combat_Rifle, Combat_Shotgun, Grenade_Protean, Patrol_SMG, Repeater_Pistol,
Rocket_Launcher, Sniper_Rifle), slot 7 accuracy, slot 8 experience, 9 action-skill cooldown, 10 melee cooldown, then 11 health and 12 shield when the pawn exists. The class has no off-hand accuracy pool in the data (the property is unset), so no slot is spent on it.
A dead player pawn's health pool is deleted by `Pawn.Died` (DD section 9), so the respawned pawn's health pool takes the freed slot again (11 in this layout) unless a lazily created pool took it.
This order is a **reading of the code and the data, not an observation**. A cheap live check exists (read `ResourcePoolManager.ResourcePools[i].Definition` on Maya's controller); G23's statement that the order does not
change any rule it read stays true: no pool reads another, and only iteration order (`AllResourcePools`, used by the save code) is visible.
The AI mind's `ApplyCharacterClassStartingValues` (WillowMind override) was **not read**; for an AI pawn the base list creation is presumably the same.

## 2. Pawn health natives (extends DD section 4; read it first)

- **Pawn.GetHealth():** the pool's `CurrentValue` if `HealthPool` resolves (resolved on a copy; the pawn's own cache is not updated), else `HealthVar`. No clamping.
- **Pawn.SetHealth(v):** if 0 <= v < 1 the value becomes exactly 0 (**negative values are not changed**). Then, if `HealthPool` is valid (the pawn's own reference is resolved and cached), `ResourcePool.SetCurrentValue(v)` on the pool (DD section 4 and G23 section 4 for its clamp and gates);
  otherwise `HealthVar := v`. `HealthVar` is **not** written while the pool is valid.
- **Pawn.GetMaxHealth():** takes **no argument** (DD section 4's "or the base maximum when asked" does not apply to this native): the pool's **effective** `MaxValue` if the reference resolves, else `HealthMaxVar`.
- **Pawn.SetMaxHealth(v):** writes `HealthMaxVar` only. It never touches the pool, so it has no effect on `GetMaxHealth` while the pool resolves.
- **Where `HealthMaxVar` is refreshed:** `Pawn.SetHealthResourcePoolDefinition` (section 1.8) and script (`OnHealthPoolMaxValueModified`, not read). After a level-up the pool's maximum changes (G23) but `HealthMaxVar` is not updated by anything read here.
- **Implementer checklist:** `GetHealth` = pool current when valid; `SetHealth(0.5)` stores 0; `SetHealth(-3)` is passed on as -3 (the pool clamps to its minimum); `GetMaxHealth` = pool effective max; `SetMaxHealth` is a plain field write.
- **Open:** the replicated mirrors (`LastHealthVar`, `LastHealthMaxVar`) and when the owning client reads `HealthVar`.

## 3. The damage pipeline object and the three open items of DD section 1

### 3.1 Actor.GetDamagePipeline / ReleaseDamagePipeline / GetPipelineDamageType / GetPipelineImpactDefinition

- **Pool:** the globals object (`WillowGlobals`, reached through the engine's global pointer) owns `DamagePipelinePool`, an array of `WillowDamagePipeline` objects.
  `GetDamagePipeline` asks the globals object for a pipeline (the same call as `WillowGlobals.GetDamagePipelineObject`); with no globals object it returns None.
- **Acquire, in order:** the first array entry whose `bInUse` flag is clear is taken, reset with its virtual `ResetToDefaults` (every script property is restored to the class default value and the list
  `TakingRadiusDamageOnHitRegions` is emptied), and `bInUse` is set. If no entry is free a **new** `WillowDamagePipeline` is created (no reset needed), appended, and marked in use: the pool has no limit.
- **Then `GetDamagePipeline` fills four fields** (this is `DamagePipeline.Initialize`): `DamageTypeDef` from `InDamageTypeDef`, `ImpactDefinition` from `InImpactDef`, `bCanHitFriendly` from `inCanHitFriendly`, `DirectHitObject` from
  `InHitObject`. It does nothing else (the summary, penetration count, source times and every Willow field keep their reset values).
- **ReleaseDamagePipeline(Pipeline):** with a pipeline and a globals object, clears `bInUse` and nothing else (the contents stay until the next acquire resets them).
- **GetPipelineDamageType(Pipeline)** returns `DamageTypeDef`, **GetPipelineImpactDefinition(Pipeline)** returns `ImpactDefinition`; both return None for a None pipeline. Note these are the
  base `DamagePipeline` fields; the `WillowDamageTypeDef` field of `WillowDamagePipeline` is a separate cast copy made by AdjustDamage's setup (DD section 1).
- **Implementer checklist:** a VM that makes a fresh default-initialised pipeline object per `GetDamagePipeline` call and discards it at release is equivalent as long as nothing holds a pipeline across calls (the script releases it in the same function that acquired it, DD section 1 edge cases);
  `DamageTypeDef` read back must be exactly the object passed in; release must tolerate being given None.
- **Open:** the reset also skips two groups of properties by flag bits (not identified); no script property of the pipeline is known to be in them.

### 3.2 The real order of the stages inside one pass (correction to DD section 1)

DD lists the stages by the order of their summary fields. The per-pass routine calls them in this order (the summary field each writes is in brackets; all single precision; each "reduction" is value before minus value after):

1. AI damage scale (`AIDamageScaleReduction`)
2. **Level difference** (`ExpLevelDifferenceReduction`) (DD had this fourth)
3. **Damage source** (`DamageSourceReduction`) (DD had this second)
4. Instigator damage type (`InstigatorDamageTypeReduction`)
5. Backstab and unsuspecting-target factors (no field; skill-event tags)
6. Hit region lookup, then **hit region and critical** (`HitRegionReduction`)
7. Projectile reflection (`ProjectileReflectionReduction`)
8. First and last shot in clip (no field)
9. **Shield** (`ShieldReduction`)
10. **Intrinsic armor** (`IntrinsicArmorReduction`) (DD called this "protection timer reduction", section 3.3)
11. Damage surface (`DamageSurfaceReduction`)
12. Hit region cap (`HitRegionCapReduction`)
13. Averted death (no field)
14. Protection timer cap (`ProtectionTimerReduction`)
Then the victim's `NotifyDamageTaken` gets the summary and the instigating player controller gets its damage-dealt callback.

The product of the multiplicative stages does not depend on order, so DD's final numbers stand; the **per-field values of a live summary do**: compare field by field in this order.

### 3.3 `RecipientDamageTypeReduction` and `IntrinsicArmorReduction`

- **IntrinsicArmorReduction:** the summary field written by the stage DD section 1 called "ProtectionTimerReduction (stage 11)". It uses the victim as `IDamageable` (not the protection-timer interface): the
  victim's `GetIntrinsicArmor()` (for a `WillowPawn` this returns the pawn's `IntrinsicArmor` attribute value) is X, and damage becomes `damage / (1 + X * 0.01)`; the field stores before minus after. The stage runs only when the victim implements `IDamageable`.
  `IntrinsicArmor`'s base is set from `PlayerClass.BaseArmor` at every level change (G23 section 6); `BaseArmor` is not set in the data for Maya's class or for the dummy's class, so X is 0 unless gear or skills change the attribute (no effect).
  The summary field named `ProtectionTimerReduction` is written only by the last stage (the cap of DD section 1 item 15).
- **RecipientDamageTypeReduction:** **no stage of the per-pass routine writes it**; it stays 0 in every summary this chain produces (the recipient's damage type modifier is applied in the damage surface stage and recorded in
  `DamageSurfaceReduction`). A live summary should show 0 there.

### 3.4 The critical hit base value

DD section 1 stage 7 and its Open list say the stored base of `CurrentInstantHitCriticalHitBonus` is 0. That was an artefact of the property decoder, which reports attribute properties as unsupported with an empty value. Reading the raw
floats of the cooked `Default__WillowPlayerController` object (at the offsets the decoder lists, each tagged property's value is the float after its 24-byte tag; the pattern is confirmed by the other attribute defaults in the same object coming out as 1.0 for
identity modifiers) gives:

| Attribute (WillowPlayerController class default; value and base value are equal) | Default |
|---|---|
| CurrentInstantHitCriticalHitBonus | **2.0** |
| CurrentMeleeCriticalHitBonus | **2.0** |
| ExpScaleForCriticalHitKills | 1.0 |
| AttackUnsuspectingTargetDamageModifier, MeleeAttackTargetFromBehindDamageModifier, AttackInjuredMeleeDamageModifier, TargetOverMinHealthGunDamageMultiplier, FirstShotInClipBonusModifier, LastShotInClipBonusModifier | 1.0 each |
| the six `Received<Source>HealingModifier` attributes | 1.0 each |

So a critical hit multiplies by 2.0 at the class default, before the modifier stack of that attribute (skills and class mods; Maya at stage 8 has none by default). **UNVERIFIED in game**; a live read of
`CurrentInstantHitCriticalHitBonus` on Maya's controller confirms it in one call.

### 3.5 The AI attacker's extra level factor (stage 2)

Stage 2 runs only when there is an instigating controller **and** the victim is a `WillowPawn` (no pawn victim, no stage: it leaves the damage alone). It first gives the table scale of DD (player table when the attacker is a player controller, AI table otherwise; position `min(difference, table length) - 1`, higher or lower entry, 1.0 when levels are equal). For an **AI attacker** two more things happen,
and both depend on the game's current **playthrough index** (0 first playthrough, 1 second, 2 third, as used by NATIVE_PROGRESSION section 2):

- **Table replacement, index 2 or more only:** the balance modification entry chosen by the attacker's effective level (the last `BalanceModifiers` entry of the playthrough's `BalanceModifierDefinition` whose `MinEffectiveLevel` is at most that level, the same selection
  rule as NATIVE_PROGRESSION section 2) supplies its own `AIDamageScaleByLevelDifference` table, indexed the same way; when the attacker's level is above the victim's the scale is that entry's higher-level value, when below its lower-level value, when equal the
  scale from the globals table is kept. Below index 2 the scale from the globals table is used unchanged.
- **Enemy damage multiplier, index 2 or more only:** the same balance modification entry's `EnemyDamageMultiplier`, or `BadassEnemyDamageMultiplier` when the attacker's targetable interface reports a badass, multiplies the damage (and is part of the stage's field).

On playthroughs 1 and 2 (index 0 and 1) both are the identity, so **for the slice the stage is just the globals table of DD**. The attacker's "effective level" comes from a balanced-actor interface slot (read as the game-stage level, not read further, **medium**). Both extra factors are part of this stage, so
their effect is inside `ExpLevelDifferenceReduction`. A player attacker gets only the table scale.

### 3.6 Implementer checklist for the pipeline groundwork

- run the stages in section 3.2's order; critical multiplier base 2.0; armor stage `damage / (1 + 0.01 * IntrinsicArmor)`; `RecipientDamageTypeReduction` stays 0;
- AI-attacker extra factors are identity below playthrough index 2;
- pipeline objects: acquire gives a reset object with four fields set, release only clears the in-use flag.

## 4. MissionTracker.NotifyPawnDied and NotifyActorDied

### 4.1 NotifyActorDied(DeadActor)

- **Reads:** the tracker's `DefendMissions` array (entries: the objective and the defend `Target` actor), `ActiveMission`, `DefendTarget`.
- **Does:** finds the **first** defend entry whose target is `DeadActor`. If none, nothing happens. If found: the mission is the entry objective's outer object (cast to `MissionDefinition`);
  if that mission is the tracker's `ActiveMission`, the tracker's `DefendTarget` record is cleared (objective None, percent 0); if the mission's `DefendMissionSetting` is **Failable** (`EDM_Failable`; the enum is None 0, NoFail 1, Failable 2) the mission status is set to **Failed** (5) through the tracker's status setter
  (`SetMissionStatusImpl`, C1); the entry is removed. Called from the death of an interactive object (script, `CleanupKilledInteractiveObject`) and as step 1 of `NotifyPawnDied`.
- **Effect on the Fire mission:** none (no defend entries).
- **Implementer checklist:** pure bookkeeping over `DefendMissions`; no effect without an entry.

### 4.2 NotifyPawnDied(KilledPawn, InstigatedBy, DamageTypeClass, DamageTypeDefinition, DamageCauser, bKilledByCritical, HitInfo)

Called by `WillowPawn.HandleHealthDepleted` (DD section 9: arguments are the pawn, the instigating controller, the damage source class, `GetPipelineDamageType(pipeline)`, the causer, `bWasLastDamageACriticalHit` and the hit info) before the engine's death handling, and by the script `Behavior_Killed` path (with no damage class and no causer).

- **Does (in order), for a non-None pawn:**
  1. `NotifyActorDied(KilledPawn)` (section 4.1).
  2. **The pawn's own objective:** if the pawn has `MissionObjectiveToUpdateOnDeath` set: if that objective's mission has a record, the tracker's `CanUpdateObjective` passes (mission Active or RequiredObjectivesComplete, the objective is in the active set and in
     the mission's list, progress below `ObjectiveCount`; C1 section B1), and either the objective has no kill restrictions (`KillRestrictions.bUseKillRestrictions` false) or the filter of section 4.3 passes, then the objective is **enqueued** for update with bit 0 (the queue and its drain are `UpdateObjective` of C1 section B1: plus one to
     progress, events, completion).
  3. **Kill-stat objectives:** if the pawn has a controller whose `CharacterClass` is a `WillowCharacterClassDefinition`: every objective registered with the tracker under the class's **`KilledStatID`** name (skipped when the name is None) is looked up, a copy of that list is walked in order,
     and each objective is enqueued with bit 0 if it has no kill restrictions or passes the filter (here **without** the `CanUpdateObjective` pre-check; the update itself ignores objectives that are not updatable, B1). Then the same is done for the name `AllegianceKilledStat` of the pawn's `Allegiance` object (only when the pawn has an allegiance).
- **The dummy:** `CharClass_TargetDummy` has `KilledStatID` = `STAT_PLAYER_KILLS_PSYCHO` in the data, so killing the dummy would progress an active "kill Psychos" objective keyed by that stat; the Fire mission is not keyed by it.
- **4.3 The kill filter** (`MissionObjectiveDefinition.KillRestrictions`, struct `KillMissionData`), applied only when `bUseKillRestrictions` is set; all of these must hold: (a) `DamageCauserType` if non-zero equals the kill's causer type (derived from the causer; not read further, **low**);
  (b) `DamageType` (the element enum) if non-zero equals the `DamageType` enum of the pipeline's damage type definition; (c) `bCriticalHit` requires `bKilledByCritical`, `bNotCriticalHit` requires it false; (d) if `HitRegionName` is set, the hit region found for `HitInfo` on the dead pawn has that name (`bNotHitRegion` inverts: the kill must **not** be in that region; a missing region passes the inverted test and fails the plain one); (e) `bMissionWeapon` requires that
  the weapon behind the causer (resolved through projectile, instigator or equipped weapon; **low confidence**) is the mission weapon linked to this objective.
- **Implementer checklist:** `NotifyPawnDied` = defend bookkeeping, then the pawn's own objective, then the two stat-keyed lists; each update goes through the existing enqueue (re-entrancy safe); with no matching objectives nothing observable happens (as in the Fire mission).
- **Open:** how the stat-name to objectives map is filled (assumed: objectives register under their `StatId` when they become active; not read); the causer type derivation; the exact mission-weapon comparison.

## 5. Kill experience

The script path (read, DD section 9): `WillowGameInfo.Killed` picks the killer (the killing `WillowPlayerController`, or a minion's `PlayerMaster`, or none), then, for a non-human victim, calls
`CalculateExperiencePointsForKill(Killer, KilledPawn, KilledPawn.bWasLastDamageACriticalHit, DamageType class, GetPipelineDamageType(pipeline))` and `AwardCombatExperience(Killer, KilledPawn, total)`. `AwardCombatExperience` (script) returns at once if the victim `WillowAIPawn` already has
`bAwardedExperienceForKill`; otherwise it sets it and calls `AwardCombatExperienceToParty(total, Killer, KilledPawn)`. With no killer, the calculation still runs when the victim's class has `bAwardKillsToAllPlayers`.

### 5.1 WillowExperiencePipeline.CalculateExperiencePointsForKill (the amount for the whole party, before the split)

Single precision. In order:

1. **Base.** Evaluate `GlobalsDefinition.BaseEnemyExperienceFormula` with the **killed pawn** as context (NATIVE_PROGRESSION section 1). Data (read from the packages): the base value is the killed pawn's `ExperiencePointMultiplier` attribute (via `GD_Balance_Experience.Attributes.Attribute_ExperienceMultiplier`); the definition
   `Init_BaseEnemyExperience` is "scales the base value" with the formula `Multiplier x (Level^1 + 4)` where Multiplier is `Init_EnemyExperience_PerPlaythrough` (conditional on the playthrough count: **10** for playthrough 1, **11** for playthrough 2, otherwise 10) and Level is the pawn's `ExpLevel`; range restriction **minimum 10**; scale constant 1.
   So `base = max(10, ExperiencePointMultiplier x M x (ExpLevel + 4))` with M = 10 on playthrough 1. (Offset inside the multiplier as in NATIVE_PROGRESSION section 1.) The multiplier attribute comes from the AI class and is one of the `GD_AI_Balance.XP.XPMultiplier_*` formulas, each a constant
   (formula multiplier x (1^1 + 0)): 01_Chump **0.5**, 02_Normal 0.75, 03_NormalHigh 1.0, 04_Tough 1.25, 05_ToughHigh 1.5, 06_Badass 3, 07_SuperBadass 10, 08_UltimateBadass 20, 09_Boss 40. The Fire dummy is the Chump tier (from G11's read of its class; the starting-value list was not decoded here).
2. **Vehicle adjustment.** Skipped with no killer, no victim or no globals. Otherwise: if the **victim** is a vehicle (a pawn virtual, read as "is a vehicle") the amount becomes **0** when the vehicle has no occupant and is unchanged when it has one; if the victim is not a vehicle and the
   killer's controller is using a vehicle, the amount is multiplied by `GlobalsDefinition.VehicleVSCharacterExpModifier` (not set in the data); in every other case (on-foot killer, non-vehicle victim) it is unchanged. (**medium**; not relevant to the slice.)
3. **Critical and damage-type multipliers.** Multiply by (a) the killer's `ExpScaleForCriticalHitKills` attribute (class default **1.0**) when the kill was critical and there is a killer, else 1; (b) the `Multiplier` of the **first** `DamageTypeExpModifiers` entry whose `DamageSource` class is the kill's damage source class or one of its ancestors
   (entries with no class never match); (c) the `Multiplier` of the first entry whose `TypeDefinition` is exactly the kill's damage type definition. Both lists are **empty in the data** (not present on `GD_Globals.General.Globals`), so (b) and (c) are 1.
- **Result** (not rounded here): `xp_total = base x vehicle x crit x source x type`. With no `GlobalsDefinition` the base is 0.
- **Implementer checklist:** `xp_total = max(10, mult x 10 x (level + 4))` on playthrough 1 for an on-foot kill, times 1; no rounding in this native; level and multiplier come from the killed pawn only.

### 5.2 WillowExperiencePipeline.AwardCombatExperienceToParty(ExperiencePoints, Killer, KilledActor)

Does nothing without a world and a game replication info. With them:

1. **Party weights.** N = `GameInfo.EffectiveNumPlayers` (1 with no game info). The globals table `ExpAwardWeights` is indexed by **position N - 1** (not by its `Players` field); a position outside the table gives bonus 0 and weight 1. The killer's share is `(KillerExpBonus + ExpWeight) x ExperiencePoints`, every other player's share is `ExpWeight x ExperiencePoints`.
   Data: N=1: bonus 0, weight 1; N=2: 0.04, 0.9; N=3: 0.05, 0.85; N=4: 0.06, 0.8. So alone: 100 percent; two players: killer 94 percent, partner 90 percent; three: 90 and 85; four: 86 and 80. A share goes to **every** player in the game replication info's player list, not only those who fought.
2. **For each player replication info in list order** whose owner is a `WillowPlayerController` (others are skipped): start from the killer's share if this controller **is** `Killer`, else the other share (with no killer everyone gets the other share);
   a. **level-difference scale** with the **player controller's `ExpLevel`** against the killed actor's `ExpLevel` (balanced-actor interface): equal levels or a missing level source leave it unchanged; otherwise the entry at position `min(|difference|, table length) - 1` of `ExpScaleByLevelDifference` multiplies
      by its **`HigherLevelEnemyExpScale`** when the killed actor's level is above the player's, by its **`LowerLevelEnemyExpScale`** when below. Data (6 entries, differences 1 to 6, the last also covers larger differences): higher 1, 1.03, 1.06, 1.09, 1.12, 1.15; lower 0.9, 0.7, 0.4, 0.15, 0.05, 0.01;
   b. **the player's combat multiplier** `m` (below) multiplies the result;
   c. the product is **truncated toward zero to an integer**;
   d. that integer is passed to the controller's `ExpEarn(amount, EES_XPCombat (1), EXPERIENCE_Combat (1))` (the existing native, bound in the VM's progression chain), and a floating "+N" notification is built for the player from the amount (presentation, not read in detail, **low**).
3. **The combat multiplier `m`** is the same helper that scales mission XP (NATIVE_PROGRESSION section 2, "playthrough multiplier"), with "kind 1" meaning combat: on playthrough index 2 or more it is the `XPGainedFromCombatMultiplier` of the last `BalanceModifiers` entry whose `MinEffectiveLevel` is at most the player's level (kind 2, quests, uses `XPGainedFromQuestsMultiplier`);
   otherwise, if the player's level is at least the level cap and a playthrough-3 modifier exists, `ModifierToXPGainedTowardsNewLevelsInEarlierPlaythroughs[playthrough index]`; otherwise **1.0**. **For the slice (playthrough 1, level below the cap) `m` is 1.0.**
- **Order of effects:** the party loop runs in replication-list order; each player's `ExpEarn` call (experience pool, level-up) happens inside the loop, one player at a time.
- **Worked example, UNVERIFIED (formula only):** Maya level 8, killed Chump pawn level 8, playthrough 1, one player: `base = max(10, 0.5 x 10 x (8 + 4)) = 60`, no level scale, share 60, `m` = 1, **ExpEarn(60)**. Same pawn with Maya at level 9: 60 x 0.9 = 54. Pawn level 10 with Maya at 8: 70 x 1.03 = 72.1, truncated to **72**. A kill by one of two players: killer 0.94, partner 0.90 of the total before each one's scales.
- **Implementer checklist:** compute `xp_total` once (5.1); per player: share by N, level scale from the two levels, `m`, truncate, `ExpEarn(.., 1, 1)`; the once-per-victim guard is script; the amount is an integer per player, never a float.
- **Open:** the exact identity of the "level cap" test input and of the playthrough index source (the same helper as the mission XP, so the same open items as NATIVE_PROGRESSION); the floating-number notification; `ExpEarn`'s own side effects are in the progression notes.

## 6. Allegiance: Pawn.IsEnemy / IsFriendly / IsNeutral / GetOpinion

- **The three tests are thin:** each asks the pawn's `GetOpinion(Other)` and compares the result: `IsEnemy` is "opinion is Enemy", `IsFriendly` "is Friendly", `IsNeutral` "is Neutral". The opinion enum (`PawnAllegiance.EOpinion`) is **Enemy 0, Neutral 1, Friendly 2**.
- **Pawn.GetOpinion(Other) (generic rule):** a pawn with no `Allegiance` object answers **Neutral**. Otherwise let A be this pawn's allegiance and B the other object's allegiance (the other's targetable interface supplies it, or the other object itself if it is an allegiance definition; no allegiance gives **Neutral**). Then:
  1. B is A: A's `SelfOpinion`;
  2. else the first entry of A's `MyOpinions` whose allegiance is B: that entry's opinion;
  3. else the first entry of B's `OtherOpinions` whose allegiance is A: that entry's opinion;
  4. else if B has `bForceAllOtherOpinions`: B's `ForcedOtherOpinion`; else A's `DefaultOpinion`.
- **WillowPawn override** (a minion or commanded-target layer on top, low confidence): the pawn answers Enemy for one remembered target actor (a pawn field, read as the actor it was told to attack), Friendly for another remembered actor (read as its master), and in one further case where the other
  pawn has a master and that master's friendliness check on this pawn is true it also answers Enemy (read from the code, direction not understood, **low**); otherwise the generic rule. None of this applies to the Fire dummy or to Maya without a minion.
- **Implementer checklist (slice):** with allegiances as data objects, opinion = the four-step rule above; `WillowGameInfo.Killed` (script) uses `Killer.Pawn.IsEnemy(KilledPawn)`, then `IsFriendly`, then `IsNeutral`, to pick the killer and killed skill events (11/14, 12/15, 13/16).
- **Open:** the data of the player and dummy allegiance objects (which `GD_AI_Allegiance` entry the dummy uses and the player's); the master-related layer.

## Not read yet

- The `WillowMind` override of `ApplyCharacterClassStartingValues` (AI pool creation order) and the unnamed script function the player-controller override calls at its end.
- The FName identities of the three pool script functions called from native (read as `InitializeAuthoritative`, `Reinitialize`, `ShutDown` from their arguments and effect).
- The vehicle provider path of `Controller.GetResourcePoolForResourceDefinition`; `WillowVehicle` pool natives; the replicated mirrors of the manager.
- The kill filter helper that derives the damage causer type; the multimap that keys objectives by stat name and its population; the mission-weapon comparison.
- `ExpEarn` body and the floating XP notification; the exact source of the playthrough index and the level cap in the XP multiplier helper.
- The `WillowPawn` allegiance override details; the player and dummy allegiance data.
- Not native, so not read: `TeleporterDestination.GetNextExitPoint` (script). Already covered elsewhere: `WillowPlayerReplicationInfo.GetCurrencyOnHand` / `AddCurrencyOnHand` (NATIVE_CONTROLLER_HELPERS).

## Corrections to earlier notes (listed, not applied)

1. DD section 1, stage list and order: stage 2 (level difference) runs before stage 3 (damage source) and stage 4 (instigator damage type) in the real call order (section 3.2 here); final damage is unaffected, per-field summary values are.
2. DD section 1 stage 11 and the "Constants" paragraph: the stage that divides by `1 + 0.01 x X` writes **`IntrinsicArmorReduction`**, with X the victim's `IntrinsicArmor`; it is not the protection-timer interface. `RecipientDamageTypeReduction` is never written. The "Open" line for these two items is closed.
3. DD section 1 stage 7 and Open ("the stored base is 0 ... must come from a modifier applied at start-up"): the class default of `CurrentInstantHitCriticalHitBonus` and `CurrentMeleeCriticalHitBonus` is **2.0** (section 3.4); the zero was the decoder's empty value. The crit base is no longer open.
4. DD section 1 stage 4 ("an additional level-based factor ... **not read**"): read (section 3.5); identity below playthrough index 2.
5. DD section 4: `Pawn.GetMaxHealth` takes no argument and always returns the effective maximum; `SetMaxHealth` writes only the plain variable; `SetHealth` leaves negative values unchanged (only [0, 1) becomes 0).
6. DD section 9 "Experience natives (structure only, not read)" and the Summary row "low (structure only)": replaced by section 5 here (formula, multipliers, level scale, split, rounding).
7. NATIVE_LEVEL_UP_ATTRIBUTES section 2 "Open: the slot order that CreatePool produces" and "Not read yet: what sets a pool's AssociatedProvider": answered in sections 1.4 and 1.10 (lowest free slot; the provider is the manager's owner).
8. `local/p2/S1/combat_swap_plan.md` sections 4 and 6 list `TeleporterDestination.GetNextExitPoint` as a native without a note: it is script and runs on the VM; the pool-reference natives and the pipeline getters now have this note.
9. NATIVE_INDEX: the rows for `WillowExperiencePipeline.CalculateExperiencePointsForKill / AwardCombatExperienceToParty` (currently "low (structure only)") should read medium-high with this note as the target; new rows are in `local/p2/G24/index_rows.md`.
