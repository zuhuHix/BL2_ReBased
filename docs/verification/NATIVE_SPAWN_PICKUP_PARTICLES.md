# WillowPickup.SpawnPickupParticles: the pickup's particle effect (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-SPAWN-PICKUP-PARTICLES (decision 2026-11-19-NNN, number
pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the installed script, read with `research/script_disasm.py` (every script function of the nine code packages searched
  for the native, `PickupParticles` and the interact-particle functions);
- the installed packages: `tools/ghidra/class_layout.py` for field names, types and order, and the value names of
  `EItemRarity` from `WillowGame.upk`;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock). The
  native's exec step only calls one of the pickup's virtuals. That virtual was found through `WillowPickup`'s
  constructor and read, together with its callees, each matched to the script native that calls the same code;
- two local probes of the installed slice gate on a merge that is never pushed (section 5).

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/spp/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**Builds on** NATIVE_ATTACH_MESH_TO_PICKUP.md (part 1: `Actor.AttachComponent`, `SetHidden` and the deferred reattach;
part 2 section 6.4: this native's refusals) and its implementation #520 (`fix/rust-attach-mesh-to-pickup-2`,
`crates/ow-inventory/src/attach_mesh.rs`), NATIVE_PICKUP_RB_MINIMAL.md (#574), NATIVE_PICKUP_TARGETING.md
(`GetPickupParticleOffset`) and NATIVE_INVENTORY_DEFINITION.md (`GetInventoryDefinition`).

**What existing particle work covers.** `tools/native_index_find.py Particle` lists only `GetPickupParticleOffset`.
That rule is reused here, through the same reads (section 4, step 4).
`EffectCollectionDefinition.GetParticleEffect` (#562's note, #573's binding) picks a weapon's muzzle-flash template from
a collection. This native neither calls it nor shares its code, and no particle stand-in exists in the tree. So none of
this native is covered already, except the offset.

**Why it matters.** With #574's answer for `InitializePickupForRBPhysics` stood in, links 6 and 7 stop at `make` on
#520's refusal of this native (`NotYet`, kind `LayerAbsent`: "WillowPickup.SpawnPickupParticles: the pickup's particle
effect", layer "a particle-effects layer (the port has none)"). Their 7 driver steps pass none.

## 1. Where it is called, and with what (question: inputs)

The native has no arguments. It is called by script in three places, all in `WillowPickup`:
- `SetInteractParticles(PSys)`: `PickupParticles` := `PSys`, `bNetDirty` := true, then this native;
- `ReplicatedEvent('PickupParticles')` (a network client);
- through `SetInteractParticles` from `TryToSetInteractParticles(bTurnParticlesOn)`. When the pickup is not
  `bIsMissionDirector`:
  - on, and `PickupParticles` is None and `bPickupable`: `SetInteractParticles` of the globals definition's
    `PartPickup`;
  - off, and `PickupParticles` is set: `SetInteractParticles(None)`.

`TryToSetInteractParticles(bPickupable)` is called by `SetPickupability`, which `InitializeFromInventory` calls with its
`bEnablePickup`. **On links 6 and 7** the driver initialises the pickup with pickup enabled, so the native runs with
`PickupParticles` = the globals definition's `PartPickup`. `DroppedPickup.SetPickupParticles` is a separate script
function that this native does not use. On its guarded path (a template is given and the net mode is not a dedicated
server) it makes a new component, attaches it and activates it.

**What the native reads:**
- on the pickup: `PickupParticles`, `PickupParticleComponent`, `InventoryRarityLevel`, `Inventory`, its rotation (for
  the offset);
- on the inventory's definition (`GetInventoryDefinition`): `PickupFlagOffset` (through `GetPickupParticleOffset`) and
  `LootBeamColorOverride`;
- on the globals definition: `RarityLevelColors` (through the script natives `GetRarityForLevel` and
  `GetRarityColorForLevel`);
- for one rarity only, an item-set definition it loads by name (section 2, step 4);
- on the chosen template (`ParticleSystem`): `WarmupTime` and `LODSettings`.

## 2. What it does, in order (question: what it makes, attaches and writes)

1. **No `PickupParticles`:** the pickup's `DestroyPickupParticles()` (a script native on the same class, its virtual not
   read), and nothing else. This is #520's `PARTICLES_REMOVAL`.
2. **No `PickupParticleComponent` yet:**
   1. a new `ParticleSystemComponent` is constructed with the pickup as its outer, with no name and no template object
      (by fit, the class global it passes is `ParticleSystemComponent`'s; the field is declared as that class). It is
      stored in `PickupParticleComponent`;
   2. its `Translation` := `GetPickupParticleOffset()`. That is NATIVE_PICKUP_TARGETING.md's rule: the definition's
      `PickupFlagOffset` turned by the pickup's rotation, zero without a definition. A non-zero offset also sets the
      pickup's `bHasPickupParticleOffset`;
   3. `AttachComponent(it)` on the pickup (part 1's rule);
   4. if `PickupParticleComponent` is still None, the native ends there.

   When a component is already there, none of step 2 runs: it is reused, and its `Translation` is not refreshed.
3. **The globals definition** G, through `WillowGlobals` (possibly none).
4. **The template:** normally `PickupParticles`. When G is present and `G.GetRarityForLevel(InventoryRarityLevel)` is
   `RARITY_Rainbow` (7), the native instead uses a field of the item-set definition
   `GD_AnemonePackageDef.ItemSetDef_Anemone`, if that field is set. The native looks that object up and **loads it by
   name** the first time, then keeps it. The field was not fitted by name.
   `GetRarityForLevel` answers the `RarityRating` of the first `RarityLevelColors` entry whose `MinLevel`..`MaxLevel`
   holds the level, else `RARITY_Unknown`.
5. **`SetTemplate(template)`** on the component, the script native's code (part of it read; see below).
6. On the component: `AbsoluteTranslation` := false, `AbsoluteRotation` := true, `AbsoluteScale` := false, and
   `Rotation` := zero.
7. **The colour:** white (1, 1, 1) to start.
   - With G: `G.GetRarityColorForLevel(InventoryRarityLevel)`. That is the first matching entry's `Color`, else opaque
     white. Each of R, G and B becomes a float in 0..1 (byte × 1/255).
   - Then, when the inventory's definition has a `LootBeamColorOverride` that is not all zero (the four bytes, alpha
     included), its R, G and B replace that colour the same way.
8. **`SetVectorParameter(name, colour)`** on the component, the script native's code (read in full). The parameter
   name is a fixed name in the executable, read as the text "RColor" (UNVERIFIED). In `InstanceParameters`:
   - the first entry with that `Name` and `ParamType` `PSPT_Vector` (3) gets `Vector` := the colour;
   - otherwise a zeroed entry is appended with `Name`, `ParamType` 3 and `Vector`.

   Nothing else is touched. A None name would do nothing.
9. **`SetHidden(false)`** on the component (the entry `PrimitiveComponent.SetHidden` calls; part 1's rule: nothing
   when `HiddenGame` is already false).
10. **The deferred transform update** on the component (by fit): `bNeedsUpdateTransform` := true. Then:
    - without an owner, and with the component `bAttached`, the component's own transform update (not read);
    - with an owner that is `bStatic` or `bTickIsDisabled`, and not `bDeleteMe`, the owner's component update (not
      read; the same entry and the same tests as part 1's deferred reattach);
    - otherwise nothing more.

**`SetTemplate`, as far as it was read (step 5).** A None template sets `Template` to None and ends there. For a
template, on the game's path:
- the template is post-loaded if needed;
- `bIsViewRelevanceDirty` := true;
- the emitter instances are reset (an engine step, cleared when the template changed);
- `bWasCompleted` := false;
- `Template` := the template;
- `WarmupTime` := the template's `WarmupTime`.

Then the system is either **activated** or **initialised**:
- activated (`ActivateSystem(false)`) when `bAutoActivate` is set, or when it was already `bIsActive` and not
  `bWasDeactivated`;
- otherwise initialised.

After that, `bAcceptsLights` := the `bLit` of the template's `LODSettings` entry for the component's `LODLevel`, or
false when there is none. Last comes part 1's deferred reattach, but only when the template changed or the component
has a render proxy. The emitter reset and the post-load were not read.

**The activation, read for its script-visible writes (hub ruling 2 on #581).** It writes script-visible state, so it is
not render-side only. On the component, in order:
- the LOD level, set on the game's path when the camera's choice differs;
- `bJustAttached`, only when its argument is set (it is not here);
- `EmitterDelay` := the template's emitter delay. When a template flag asks for it, this is **a random value between
  the template's low and high delay**: a draw from the engine's float random source (which stream was not settled);
- `AudioEventDelay` := the template's;
- `bWasCompleted`, `bSuppressSpawning` and `bWasDeactivated` cleared, then `bIsActive` set;
- the deferred transform update (step 10's rule), or just `bNeedsUpdateTransform` when the component had no render
  proxy;
- `EmitterInstances` built, or each emitter reset and re-initialised (engine);
- `bHasBeenActivated` set;
- on the first activation, a LOD flag set, `LODMethod` set and the LOD level set;
- with a non-zero `WarmupTime`, the system is **ticked in steps until the warm-up time is reached**, then `WarmupTime`
  := 0. That is particle simulation, which may spawn and draw;
- `AccumTickTime` := 0, then a final engine step, then a time stamp from the current time.

**The initialisation**, the other branch, was read only as far as its script-visible edge. It is an engine step (its
body not read). When the component is `bAttached`, it then writes `AccumTickTime` := 0, and it activates only when
`bAutoActivate` is set, which this branch excludes.

**Loads and random numbers:**
- **Loads:** one only, the item-set definition for a `RARITY_Rainbow` pickup (step 4). `SetTemplate` post-loads a
  template that needs it; a loaded template needs none.
- **Random numbers:** the native's own steps draw none. The activation draws when the template asks for a random
  emitter delay, and its warm-up ticks may draw.

**Field names.** Every field named in this note was fitted by name from the installed packages' layouts. The fits
themselves stay under ignored `local/analysis/spp/`. All are UNVERIFIED.

## 3. What links 6 and 7 read of it

**The slice's inputs** (the first probe, section 5; loaded-only reads, on both links and the decoy):
- `bIsMissionDirector` is clear and `PickupParticleComponent` is None, so step 2 runs and makes a new component;
- `PickupParticles` is a `ParticleSystem` handle that the port has **not loaded**;
- `InventoryRarityLevel` is 0, which is not `RARITY_Rainbow`, so the template is `PickupParticles`, with no load by
  name;
- the gun's definition has a zero `PickupFlagOffset` (so `Translation` is zero and `bHasPickupParticleOffset` is not
  written) and an all-zero `LootBeamColorOverride` (no override). The colour is the rarity entry's.

**What the drivers read, and whether this native's writes reach it:**
- `touch` (`DroppedPickup.Pickup.Touch`, `ValidTouch`): the toucher, `Physics`, `Instigator`, `Velocity`, the trace
  between the toucher and the pickup's `Location` raised by its bounding cylinder, and `PickupQuery`. The native writes
  none of these. The bounding cylinder is the actor's (`Actor.GetBoundingCylinder`, not read here), and the particle
  component is not the `CollisionComponent`. That it does not count is UNVERIFIED until that native is read.
- `look` (link 7's targeting): `bHasPickupParticleOffset` (the targeting computes the same offset and sets the same
  flag itself), `Rotation`, `LifeSpan`, `Inventory`, and the height of the bounding box of the pickup's components.
  The new component joins `Components`, so a world with component bounds could see it. The headless world has no
  component bounds and refuses there (NATIVE_PICKUP_TARGETING.md), so the slice does not read it.
- `key` and the `taken` check: the controller's choice, the inventory chain, the pickup's destroyed state, the
  decoy's `Inventory` and `bPickupable`. None of them comes from this native.

**So nothing links 6 and 7 read depends on the particle effect.** The only write they could see is the component in
`Components`, through component bounds, which the headless world does not have. A minimal answer would be enough for
the drivers. But the activation (or the initialisation) writes script-visible state, so it cannot be ruled into the
empty particle layer, and it stays a refusal (section 4).

## 4. A proposed Off answer, for the implementer

It replaces #520's `PARTICLES` refusal. The particle system's render-side work (emitter instances, spawning,
rendering) is left out as an empty particle layer, the same shape as the empty scene and physics layers ruled for
`AttachComponent` and #574 (UNVERIFIED, a hub ruling). **Neither the activation nor the initialisation falls under that
ruling.** Both write script-visible state (section 2), so each keeps its own refusal (E and F).

**Harness inputs** (hub ruling 1 on #581; UNVERIFIED, logged like the globals preload). The pickup world's set-up
preloads, before `make`:
- the globals definition's `PartPickup` template;
- the `ParticleSystemComponent` class, its default object, and the declarations this answer reads: the component
  fields of section 2, `ParticleSysParam`'s `Name`, `ParamType` and `Vector`, and `ParticleSystemLOD`'s `bLit`.

In the game these packages are loaded already. Every lookup below reads only what is loaded, through the class table
and the definition store, and never through a path that loads.

**Preflight, before the first write.** It is write-free, load-free and draw-free. It runs in the native's order, and
the first refusal met wins.
1. **The pickup's declarations:** `PickupParticles`, `PickupParticleComponent`, `InventoryRarityLevel`, `Inventory`,
   `Rotation` and `bHasPickupParticleOffset`. A missing or wrongly typed one is a data error.
2. `PickupParticles` None: #520's `PARTICLES_REMOVAL`, unchanged.
3. **The component C:**
   - when `PickupParticleComponent` is set, C is that object. It must be a `ParticleSystemComponent`, else a data
     error. Its own fields are the inputs below;
   - when it is None, nothing is allocated yet. The plan takes C's inputs from the preloaded class default object. If
     the class or its default object is not loaded: refusal G.
4. **The attach plan, only for a new C,** from the actor guards and the default's values, never from an allocated
   object:
   - the pickup destroyed or `bDeleteMe`: nothing is attached, and part 1's guards apply. The native's later steps
     still run on C with no owner;
   - no `SceneLayer` in the world: #520's `ATTACH`;
   - the default's `bAttached` set: #520's `ATTACH_DETACH`;
   - otherwise the plan is part 1's writes, with C's owner then the pickup.

   The offset comes from the same reads `GetPickupParticleOffset` makes: `inventory_definition_of` (write-free), the
   definition's `PickupFlagOffset` and the pickup's `Rotation`, turned as NATIVE_PICKUP_TARGETING.md says. No method
   lookup is involved.
5. **The rating and the colour:**
   - G is the globals definition the pickup world prepared;
   - the rating is the `RarityRating` of the first `RarityLevelColors` entry holding `InventoryRarityLevel`, else
     `RARITY_Unknown`. `RARITY_Rainbow`: refusal A;
   - the colour is planned from the same entry's `Color`, and the definition's `LootBeamColorOverride` when it is not
     all zero, as section 2, step 7.
6. **The template T** is `PickupParticles`. Not loaded: refusal B. With the preload, it is loaded on the slice. T's
   `WarmupTime` and `LODSettings` are read; "changed" means T differs from C's `Template` (None for a new C).
7. **Activation or initialisation**, from C's `bAutoActivate`, `bIsActive` and `bWasDeactivated` (the default's for a
   new C): activation is refusal E, initialisation is refusal F.
8. **The deferred steps.** O is C's owner after step 4's plan. Three requests come in the native's order, each with
   the same owner tests (read in #574 for the shared entry):
   1. **SetTemplate's reattach**, requested only when T changed or C has a render proxy (the port has none). With O
      set: refusal C1 when O is `bStatic` or `bTickIsDisabled`, and not `bDeleteMe`. Without O: #520's
      `REATTACH_SCENE` when C's `Scene` is not zero;
   2. **`SetHidden(false)`'s reattach**, requested only when C's `HiddenGame` is set. The same tests and texts as 8.1.
      This native does not use #520's `SetHidden` preflight, which refuses any owner;
   3. **the transform update**, always requested. With O set: refusal C2 under 8.1's owner test. Without O: refusal D
      when C is `bAttached`.
9. **The destinations' declarations:** `Translation`, `Rotation`, `AbsoluteTranslation`, `AbsoluteRotation`,
   `AbsoluteScale`, `Template`, `WarmupTime`, `bIsViewRelevanceDirty`, `bWasCompleted`, `bAcceptsLights`,
   `bNeedsReattach`, `bNeedsUpdateTransform`, `HiddenGame` and `InstanceParameters` (an array of `ParticleSysParam`).
   A mismatch is a data error.

**Writes, after the preflight passes.** They are applied from the plan, with no helper that can refuse:
1. a new C, if planned (the port's object construction, as script `New` makes one), then `PickupParticleComponent` :=
   C, `Translation` := the offset, `bHasPickupParticleOffset` := true when it is not zero, then the planned attach
   writes;
2. `bIsViewRelevanceDirty` := true, `bWasCompleted` := false, `Template` := T, `WarmupTime` := T's. That is as far as
   it gets today, since step 7 always refuses. With E and F answered later: `bAcceptsLights`, then `bNeedsReattach`
   when 8.1 requests it;
3. the absolute flags and `Rotation` := zero;
4. the `InstanceParameters` update with the planned colour;
5. `HiddenGame` := false and `bNeedsReattach`, when 8.2 requests it;
6. `bNeedsUpdateTransform` := true.

No loads and no draws. **A late `NotYet`** can come only from a step this plan missed. It keeps its kind and record,
and gets the "stopped after partial writes" prefix only when this native's own write, load or draw counters moved. The
pickup driver marks the earlier `make` writes separately.

**Typed refusals.** The record is `docs/verification/NATIVE_SPAWN_PICKUP_PARTICLES.md` unless one is named.

| # | When | Kind | Text |
|---|---|---|---|
| A | the rating is `RARITY_Rainbow` | `NotRead` | "WillowPickup.SpawnPickupParticles: the Rainbow-rarity template, loaded by name from an item-set definition" |
| B | T is not loaded | `NotRead` | "WillowPickup.SpawnPickupParticles: a particle template that is not loaded (a load is a write)" |
| C1 | a requested reattach, with O `bStatic` or `bTickIsDisabled` and not `bDeleteMe` | `NotRead` | #520's `REATTACH_OWNER`, "the deferred reattach: the owner's component update", record `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md` |
| C2 | the transform update, with the same owner test | `NotRead` | "the deferred transform update: the owner's component update" |
| D | the transform update, with no O and C `bAttached` | `NotRead` | "the deferred transform update: the component's own transform update" |
| E | the activation | `NotRead` | "ParticleSystemComponent.ActivateSystem: the system's activation (script-visible writes, a possible random draw and warm-up ticks)" |
| F | the initialisation | `NotRead` | "ParticleSystemComponent.SetTemplate: the system's initialisation (not read)" |
| G | the component class or its default object not loaded | `NotRead` | "WillowPickup.SpawnPickupParticles: the particle component class, not preloaded" |

#520's `PARTICLES_REMOVAL`, `ATTACH`, `ATTACH_DETACH` and `REATTACH_SCENE` keep their texts and record.

**On the slice:** a new C, a rating that is not Rainbow, and the template preloaded. So the first refusal is E or F,
whichever C's class default `bAutoActivate` decides. That default was not read here. The stock engine sets it, so E is
the likely one (UNVERIFIED). That makes `ActivateSystem`'s script-visible writes the next read for links 6 and 7.

## 5. The probe and the next stop

**The probe** was a local merge of origin/main d52bcc7 with #520 (`fix/rust-attach-mesh-to-pickup-2` 4d0539b), never
pushed. It ran `--slice-gate`, installed, Off and strict, under the machine lock. Two stand-ins were used:
- `InitializePickupForRBPhysics` answered with no write (a stand-in for IMPL-PICKUP-RB, which #574 ruled);
- this native answered with no write, after a report of section 3's inputs.

**Result:**
- **Links 6 and 7 pass `make` and stop at `touch`**, with an `error`: the unbound native
  `Actor.GetBoundingCylinder(float,float)`, which `ValidTouch`'s trace location needs. Link 6 passes 1 of its 3
  steps (`make`); link 7 passes 1 of its 4 (`make`).
- The gate as a whole: 1 `ok` (link 2), 4 `notyet` (links 9-12), 9 `error`:
  - links 1, 3-5, 13 and 14 at `GameEngine.GetOnlineSubsystem`;
  - links 6 and 7 at `GetBoundingCylinder`;
  - link 8 at `CacheWeaponCard`.

**What that shows, and what it does not.** With both natives stood in by no-write answers, the observed stop is `touch`
(`Actor.GetBoundingCylinder`). No probe ran either proposed answer or the `PartPickup` preload. With the section 4
answer and the preload, the expected stop is refusal E (or F) at `make` (section 4, "On the slice"). That is
UNVERIFIED until an implementation runs, and so is any stop after it.

**`SkeletalMeshComponent.SetAnimTreeTemplate`** (19d5's stop on the #573 stack) **does not come first on this base.**
On origin/main with #520, `make` reaches `InitializePickupForRBPhysics` and, with the stand-ins, passes. It does not
stop at `SetAnimTreeTemplate`. That stop comes from the #573 stack, which this base does not include.

## Open questions

1. **Not read:**
   - `DestroyPickupParticles`;
   - the bodies of the activation's engine steps and of the initialisation;
   - `SetTemplate`'s emitter reset and its post-load;
   - the random stream of the activation's delay draw;
   - the component's own transform update and the owner's component update;
   - the item-set definition's field (step 4);
   - the component class default's `bAutoActivate`.
2. **UNVERIFIED:** the parameter name "RColor"; the class the new component is constructed from (fitted from the field's
   declared class).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `WillowPickup.SpawnPickupParticles` on a weapon drop and record, before and after:
   - on the pickup: `PickupParticleComponent` and `bHasPickupParticleOffset`, and its `Components`;
   - on the component: `Template`, `Translation`, `Rotation`, `AbsoluteTranslation`, `AbsoluteRotation`,
     `AbsoluteScale`, `InstanceParameters` (names, types and vectors), `HiddenGame`, `bNeedsUpdateTransform`,
     `bNeedsReattach`, `WarmupTime`, `bAcceptsLights`, `bIsActive`, `bWasCompleted`, `bIsViewRelevanceDirty`, `Owner`.

   Expect section 2's values, and the parameter's name.
2. Call it a second time on the same pickup with another `PickupParticles`. Expect the same component, with its
   `Translation` unchanged and `InstanceParameters` updated in place, not appended.
3. A pickup whose definition sets `LootBeamColorOverride`, and one at a level whose `RarityLevelColors` entry is
   `RARITY_Rainbow`. Record the template chosen and the colour.
4. `FRand` before and after against a seeded expectation: the native itself should not advance it.
