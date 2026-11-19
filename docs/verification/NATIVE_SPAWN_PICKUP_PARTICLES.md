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
That rule is reused here as it stands, through `pickup_targeting::particle_offset`.
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
function (a new component per call, attached and activated) that this native does not use.

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

Then the system is either activated (`ActivateSystem(false)`, when `bAutoActivate` is set, or when it was already active
and not deactivated) or initialised (an engine step, not read). After that, `bAcceptsLights` := the `bLit` of the
template's `LODSettings` entry for the component's `LODLevel`, or false when there is none. Finally, when the template
changed or the component has a scene proxy, comes part 1's deferred reattach (the same owner tests). The activation
and initialisation bodies, the emitter reset and the post-load were not read.

**Loads and random numbers:**
- **Loads:** one only, the item-set definition for a `RARITY_Rainbow` pickup (step 4). `SetTemplate` post-loads a
  template that needs it; a loaded template needs none.
- **Random numbers:** the native draws none. The activation (not read) starts the emitters, and particle spawning may
  draw, but that is the particle system's own work.

**Field fits:**
- exact: `PickupParticleComponent` and `PickupParticles`; `RarityLevelColors` and `RarityLevelColor`'s fields;
  `LootBeamColorOverride`; `ParticleSystem.WarmupTime` and `LODSettings`;
- with the pickup's +16 shift after `RBState`: `InventoryRarityLevel`;
- with the component shifts #563 and #574 used: `AbsoluteTranslation`, `AbsoluteRotation`, `AbsoluteScale`,
  `Translation`, `Rotation`, `bAcceptsLights`, `Template`, `bIsViewRelevanceDirty`, `bWasCompleted`, `bAutoActivate`,
  `InstanceParameters`, `WarmupTime` and `LODLevel`;
- the component's `bNeedsUpdateTransform`, `bAttached` and `Owner`, and the owner's `bStatic`, `bTickIsDisabled` and
  `bDeleteMe`, at the offsets part 1 used.

All are UNVERIFIED.

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
`Components`, through component bounds, which the headless world does not have. A minimal answer is enough for the
slice.

## 4. A proposed Off answer, for the implementer

It replaces #520's `PARTICLES` refusal. The particle system's own work is left out: emitter instances, activation,
spawning and rendering. That is an empty particle layer, the same shape as the empty scene and physics layers ruled for
`AttachComponent` and #574. **That is the hub's ruling to make**, and the steps below say where it applies.

**Preflight, write-free, before the first write.** It uses loaded-only reads, in this order:
1. The declarations of `PickupParticles` and `PickupParticleComponent`. When `PickupParticles` is None: #520's
   `PARTICLES_REMOVAL` (`NotYet`, kind `NotRead`, "WillowPickup.SpawnPickupParticles with no particle system: the
   particles' removal", record `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md`), unchanged.
2. When `PickupParticleComponent` is set, it must be a `ParticleSystemComponent`, else a data error. When it is None:
   `ParticleSystemComponent` must resolve, and `AttachComponent`'s own preflight must pass (part 1: the world's
   `SceneLayer`). The offset is `ow_inventory::pickup_targeting::particle_offset`, which writes nothing.
3. G as the pickup world's set-up prepared it (#574's dependency on `globals_preflight`). `InventoryRarityLevel`'s
   rating from G's `RarityLevelColors`: the first entry holding the level, else `RARITY_Unknown`. That is the same walk
   as `ow_inventory::globals::rarity_bands`, reading `RarityRating`.
4. `RARITY_Rainbow`: refusal A.
5. The template is `PickupParticles`. When the definition store has not loaded it: refusal B. A loaded template's
   `WarmupTime` and `LODSettings` declarations are checked.
6. The owner tests of the two deferred steps. The owner is the pickup when step 2 attaches; otherwise it is the
   component's `Owner`. Refusal C applies when the owner is `bStatic` or `bTickIsDisabled`, and not `bDeleteMe`.
   Without an owner and with `bAttached`: refusal D. Without an owner and with a non-zero `Scene`: #520's
   `REATTACH_SCENE`.
7. The activation (`SetTemplate`, when `bAutoActivate` is set, or when the component is active and not deactivated):
   refusal E, **unless the hub rules the activation into the empty particle layer**. On the slice, a new component's
   `bAutoActivate` is its class default. This lane did not read that value, so the implementer checks it.

**Writes, in the native's order:**
1. With no component: a new `ParticleSystemComponent`, with the pickup as its outer, through the port's object
   construction (as script `New` makes one). Then `PickupParticleComponent` := it, `Translation` := the offset, and
   `bHasPickupParticleOffset` := true when the offset is not zero. Then #520's `attach_component`.
2. `SetTemplate`'s field writes: `bIsViewRelevanceDirty` := true, `bWasCompleted` := false, `Template` := the template,
   `WarmupTime` := its `WarmupTime`. Then the activation, empty under the ruling. Then `bAcceptsLights` := the `bLit`
   of `LODSettings[LODLevel]`, or false when `LODLevel` is out of range. Then, when the template changed,
   `bNeedsReattach` := true (part 1's deferred reattach; the owner case is preflight step 6).
3. `AbsoluteTranslation` := false, `AbsoluteRotation` := true, `AbsoluteScale` := false, `Rotation` := zero.
4. The colour (section 2, step 7), then the `InstanceParameters` update of section 2, step 8.
5. `SetHidden(false)`: #520's rule, which writes nothing when `HiddenGame` is already false.
6. `bNeedsUpdateTransform` := true.

No random draws and no loads.

**Typed refusals** (record `docs/verification/NATIVE_SPAWN_PICKUP_PARTICLES.md` unless named):

| # | When | Kind | Text |
|---|---|---|---|
| A | the rating is `RARITY_Rainbow` | `NotRead` | "WillowPickup.SpawnPickupParticles: the Rainbow-rarity template, loaded by name from an item-set definition" |
| B | the template is not loaded | `NotRead` | "WillowPickup.SpawnPickupParticles: a particle template that is not loaded (a load is a write)" |
| C | the owner is `bStatic` or `bTickIsDisabled`, and not `bDeleteMe` | `NotRead` | "the deferred transform update: the owner's component update" (and #520's `REATTACH_OWNER` for the reattach) |
| D | no owner, and the component is `bAttached` | `NotRead` | "the deferred transform update: the component's own transform update" |
| E | the activation runs, without the hub's ruling | `LayerAbsent` | "ParticleSystemComponent.SetTemplate: the system's activation", layer "a particle-effects layer (the port has none)" (#520's `PARTICLES_LAYER`) |

**Refusal B stops the slice.** The slice's template is a handle the port has not loaded. Two ways past it are open,
and choosing between them is the hub's call:
- the pickup world's set-up preloads the globals definition's `PartPickup`, as it preloads the globals. That is an
  UNVERIFIED harness input, and in the game the package is loaded;
- or the Off answer reads `WarmupTime` and `LODSettings` from the definition store without loading. The port has no
  such path today.

The first is the smaller change.

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

**So the next stop after this native** (with the RB answer and the template preload) is `Actor.GetBoundingCylinder`
at `touch`. Without the preload it is refusal B at `make`.

**`SkeletalMeshComponent.SetAnimTreeTemplate`** (19d5's stop on the #573 stack) **does not come first on this base.**
On origin/main with #520, `make` reaches `InitializePickupForRBPhysics` and, with the stand-ins, passes. It does not
stop at `SetAnimTreeTemplate`. That stop comes from the #573 stack, which this base does not include.

## Open questions

1. **Not read:** `DestroyPickupParticles`; `SetTemplate`'s activation and initialisation, its emitter reset and its
   post-load; the component's own transform update; the owner's component update; the item-set definition's field
   (step 4).
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
