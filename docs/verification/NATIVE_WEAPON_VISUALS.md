# Native weapon visuals: material build, glow, composite mesh, first-person placement, muzzle flash (2026-10-04)

AI-assisted (Claude). Behaviour notes from a local Ghidra reading of `Borderlands2.exe`, the installed script
(`research/script_disasm.py`) and installed class defaults / part data (`ow-package --properties`), under the policy in
[LEGAL.md](../LEGAL.md) ("Analysing the executable") and [NATIVE_ANALYSIS.md](../NATIVE_ANALYSIS.md). Nothing below is
a listing or pseudo-code. Companion to [NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md) (stats, part pick, names) and
Lane C's paint reading in `tools/weapon_paint_model.py` (the shader side). Raw output is under ignored
`local/analysis/E/`.

> **Retired host (2026-11-14).** Host runs in this record were made with the UE5 host, which left the tree on 2026-11-14 ([decision](../decisions/2026-11/2026-11-14-01-rust-replaces-the-ue5-host-engine-route-by-benchmark.md); its code is at main `08bcf3e`). They say what that host did and no longer count toward a gate. Evidence here that does not depend on the host (real-game captures, native and script reading, package checks, core results) keeps its status.

**Every rule here is `UNVERIFIED` in the running game.** "Read" means read from native code or script. The data values
quoted are those of the installed packages.

## 1. How a weapon gets its material (`WillowWeapon.BuildWeaponMaterial`, native)

Called from `WillowWeapon.InitMeshes` (and from `CloneAppearance`, `ClonePrimaryMesh` and the player stand-in's weapon
clone) with the weapon, its first-person mesh component and its `WeaponDefinitionData`. Read behaviour:

1. If the data has no `WeaponTypeDefinition`, no material (null).
2. **Parent material.** If the data's `MaterialPartDefinition` (slot 8) exists and has a `Material`, that
   `MaterialInstanceConstant` is the parent (for example `Common_GunMaterials.Materials.AssaultRifle.Mati_BanditCommonAR`
   for `Mat_BanditMade_1`; the part type enum is `WP_Material`). Otherwise the parent is whatever the mesh component
   currently has in material slot 0.
3. **Create** a new material instance constant of the class named by `WeaponTypeDefinition.MaterialClass` (a
   `ClassProperty` on `WillowInventoryDefinition`), owned by the weapon (a default outer is used when none is given), and
   set its parent to the material of step 2.
4. **Part parameters.** Walk the parts in slot order, skipping empty slots and the two name parts: Body, Grip, Barrel,
   Sight, Stock, Elemental, Accessory 1, Accessory 2, Material. For each part, apply its
   `WillowInventoryPartDefinition.MaterialVectorParameterValues` (array of `VectorParameterValue {ParameterName,
   ParameterValue (linear colour), ExpressionGUID}`) to the new instance by calling its vector-parameter setter with the
   name and the stored colour **unchanged**. A later part overwrites an earlier one with the same name.
5. Return the instance. **No scalar or texture parameter is set by this function**, and no other native code
   writes colours at spawn.

`InitMeshes` then assigns this one instance to slot 0 of the first-person mesh, the third-person mesh and every extra
slot mesh, so all views share it, and reads the instance's scalar named `WeaponTypeDefinition.GlowScaleMaterialParamName`
into `BaseGlowScale` (section 2).

**Data seen.** In the installed packages exactly 24 weapon parts carry `MaterialVectorParameterValues`, all with one
entry, `p_EmissiveColor`, on elemental parts. Linear values (not colour-managed, HDR above 1): on the assault rifle, Fire is
a strong red, Corrosive a strong green, Shock a strong blue. So the elemental glow colour is a
linear HDR vector parameter named `p_EmissiveColor` on the instance; the paint (zones, pattern, decal, reflection) lives
in the parent material instance chosen by the Material part (textures and its own parameters), not in this function.
**sRGB / linear:** vector parameters are `FLinearColor` and pass through unconverted; the texture sRGB flags are a
property of each texture (see `weapon_paint_model.py`). Elemental parts are the only part kind that overrides
anything here, so a UE5 importer needs: parent material = Material part's MIC; instance parameter `p_EmissiveColor` =
the elemental part's linear colour.

### 1.1 Amendment (2026-11-25): the Off answer for `BuildWeaponMaterial`

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-BUILDWEAPONMATERIAL (decision 2026-11-25-32), analyst only:
**another lane implements this section.** Raw output stays under ignored `local/analysis/bwm/`. **Every rule here is
UNVERIFIED in the running game.**

**Why.** With #678 (row 15's refill), link 8's `init` gets past `SetSkeletalMesh` and stops with an error at the
unbound native `WillowWeapon.BuildWeaponMaterial(object,object,weapondefinitiondata)`. Section 1 above (2026-10-04)
already reads its behaviour. This amendment re-reads it for the edge cases an Off port needs, and proposes the answer.

**Re-read** (Ghidra 12.1.4 headless, read-only, under the machine lock; field names from
`tools/ghidra/class_layout.py`). It agrees with section 1. In particular:
- **no `WeaponTypeDefinition`:** the answer is None, and nothing is created;
- **the parent:**
  - the `MaterialPartDefinition`'s `Material`, when both are set;
  - otherwise, when a mesh component was passed, that component's material 0 (a component virtual: by fit
    `GetMaterial(0)`, which for a skeletal component also falls back to its mesh's materials; not read here);
  - otherwise None;
- **the instance:** a new object of the class `WeaponTypeDefinition.MaterialClass`, with the weapon argument as its
  outer (a default outer when none is passed), no name and no template. **The class is not checked**: a None
  `MaterialClass` is read through unchecked;
- **its parent:** set through the material's own parent setter, a virtual (by fit `SetParent`; not read in detail);
- **the parts:** each of the eleven part slots in order (the two name parts end the walk) applies its own parameters to
  the instance, through a part virtual. That is section 1's `MaterialVectorParameterValues` step; the virtual was not
  re-read;
- **the answer:** the instance.

**Loads.** The native itself loads nothing. It reads the definition data's objects, which the weapon's set-up has made
resident.

**What follows in `InitMeshes`** (script): `WeaponMaterial` = the answer. Then `FirstPersonMesh.SetMaterial(0, …)` and
`ThirdPersonMesh.SetMaterial(0, …)`, then the instance's `GetScalarParameterValue(GlowScaleMaterialParamName)` into
`BaseGlowScale`, then `InitPartParticles`, `InitEffects` and `InitVisibleAmmoBones`.

#### 1.1.1 The proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `WillowGame.WillowWeapon.BuildWeaponMaterial` in `ParityMode::Off`, with this
section as record. Reference is unchanged.

**Preflight, write-free and loaded-only** (by name lookup on resident objects). A non-resident object it must read is a
`NotYet` of kind `NotRead`, "BuildWeaponMaterial: an object the store has not loaded (a load is a write)". A field of
the wrong kind is a `VmError::Script` naming it. In order:
1. `WeaponTypeDefinition` None: answer None. Nothing more is read;
2. **the parent:**
   - with the Material part set and its `Material` set: the parent is that `Material`;
   - with no such `Material` and a mesh component passed: a `NotYet` of kind `NotRead`, "BuildWeaponMaterial: the
     mesh component's material 0 (its material lookup is not read)";
   - with neither: the parent is None;
3. **the class:** `WeaponTypeDefinition.MaterialClass` None is a `NotYet` of kind `NotRead`, "BuildWeaponMaterial: a
   weapon type with no MaterialClass (the game constructs it unchecked)". A class that does not descend from
   `Engine.MaterialInstanceConstant` is the same kind of refusal, naming the class (only that family was seen);
4. **the parts' parameters:** for each part slot in order, skipping empty slots and the name parts, read the part's
   `MaterialVectorParameterValues` (an array of `ParameterName`, `ParameterValue`). Any entry at all is a `NotYet` of
   kind `NotRead`, "BuildWeaponMaterial: the material instance's vector-parameter setter (not read)". The setter's
   rule (update an existing name or append, and any render update) was not read, so the port must not write a guessed
   `VectorParameterValues`. Parts without entries change nothing.

**Commit** (only when the preflight passes):
1. create one object of `MaterialClass` with the weapon as its outer, no name and no template, through the VM's own
   object creation (as a script `new` does);
2. its `Parent` = the parent of step 2. This is by fit to the parent setter. Its render side is not ported, and
   nothing else is written;
3. answer the instance.

Creating the object is the first write. A refusal after it cannot happen here, because every check is in the
preflight.

**What link 8 would meet** (a prediction, not a run): it depends on the gun's definition data. With a Material part
whose `Material` is set and no part carrying `MaterialVectorParameterValues` (installed data: only the 24 elemental
parts carry any, section 1), the native answers a new instance. `InitMeshes` then reaches `SetMaterial`, the next
candidate stop, and after it `GetScalarParameterValue` on the instance. An elemental part instead gives the setter's
`NotRead`.

#### 1.1.2 Tests for the implementer

Synthetic, with `package()` / `tag()`. Each preflight refusal leaves #518's three counters and the name table
unchanged, and causes no class-code load (where #685 has landed):
1. definition data with no `WeaponTypeDefinition`: None, and nothing created;
2. a Material part with a `Material`, and a toy `MaterialClass` deriving from `MaterialInstanceConstant`: one new
   object of that class, its outer the weapon, its `Parent` that `Material`. The heap grows by one object, and nothing
   else changes;
3. no Material part, no mesh component: an instance with `Parent` None;
4. no Material part, a mesh component passed: the material-0 `NotRead`, nothing created;
5. `MaterialClass` None: the class `NotRead`, nothing created. A class outside the `MaterialInstanceConstant` family:
   the same refusal, naming it;
6. an elemental part with one `p_EmissiveColor` entry: the setter's `NotRead`, nothing created;
7. a lazy Material part, or a lazy `Material`: the "not loaded" `NotRead`, nothing loaded;
8. `MaterialVectorParameterValues` not declared as an array of that struct: `VmError::Script` naming it;
9. Reference: unchanged.

## 2. Glow scale and the firing impulse (native, per tick)

State on the weapon: `BaseGlowScale` (read once from the instance), `FinalGlowScale`, `GlowImpulseScale`,
`GlowImpulseDecayStartTime`, optional `GlowEffect` (a `WeaponGlowEffectDefinition` with a float curve) and
`GlowEffectStartTime`. Type settings (`WeaponTypeDefinition`, class defaults): `GlowScaleMaterialParamName`
`p_EmissiveScale`, `FiringGlowImpulse` 0.25, `MaxGlowImpulseScale` 5, `GlowImpulseDecayDelay` 0.2 s,
`GlowImpulseDecayRate` 3.5 per second.

* **On each shot** (from the native fire-effects routine): `GlowImpulseScale += FiringGlowImpulse`, capped at
  `MaxGlowImpulseScale`; `GlowImpulseDecayStartTime = now + GlowImpulseDecayDelay`.
* **Each tick** (needs a weapon type and a material): `Final = Base`. If the impulse is above 0: once the decay start time
  has passed, subtract `GlowImpulseDecayRate * dt` (floor 0); then `Final += Impulse * Final`, i.e. `Base * (1 +
  Impulse)`. If a glow effect is active: `t = now - GlowEffectStartTime`; past the last curve point the effect is
  dropped, otherwise `Final *= curve(t)`. If `Final` changed by 1e-8 or more, set the instance's scalar parameter
  `p_EmissiveScale` to `Final`.
* So the emissive of a gun pulses to about 1.25 x base after one shot, 1.5 x after two quick shots, and returns to
  base about 0.07 s per 0.25 impulse after the 0.2 s delay (3.5 per second).

## 3. Composite mesh and bones (`BuildCompositeMesh`, `HideBonesInMesh`)

* `BuildCompositeMesh(skeletalMeshComponent, WeaponDefinitionData)` (native): collects the mesh of each part slot
  (slot order as above, 11 slots, name parts yield nothing). Zero meshes -> none; exactly one -> that mesh; several ->
  a merged skeletal mesh. If the weapon type has a `GestaltMesh` (for example `Weap_Pistol.GestaltDef_Pistol`) the parts are
  instead turned into a gestalt (part names per part, `GestaltModeSkeletalMeshName` plus up to two additional names) and the component
  built with the gestalt data; the third-person mesh then copies the first-person gestalt data
  (`InitGestaltMeshDataFromOther`). Part mesh selection is `NongestaltSkeletalMesh` or the gestalt names.
* Script then calls `HideBonesInMesh` on both meshes: the type's `BoneToHideOnMesh` and `AdditionalBoneToHideOnMesh` are hidden.
  The third-person mesh scale is `ThirdPersonMeshScale`. Weapons whose body part has `bUseWeaponMelee` set mark the weapon
  as having a melee mode (`bUseWeaponMelee`).
* `VisibleAmmoBoneNames` (body part): bones matched on the first-person mesh to show/hide per remaining ammo.

## 4. First-person placement

* **Attachment.** `TimeWeaponEquipping` (script) takes the *arms* mesh of the player pawn (`WillowPlayerPawn.Arms`) and
  `AttachWeaponTo` attaches the weapon's first-person mesh to the socket named by
  `WeaponTypeDefinition.AttachmentData.FirstPersonAttachmentSocket` (class default **`Weapon`**; off-hand weapons use
  `FirstPersonOffHandAttachmentSocket`, default **`OffHandWeapon`**; third person uses the same two names on the pawn mesh).
  The position is therefore the arms skeleton socket plus the animated arms: `Weapon` is a socket on the arms
  skeleton, and the arm/weapon equip, fire, reload and put-down animations
  (`WeaponEquipAnimations`, `WeaponFireAnimations`, ...) move the hand.
* **View model sway.** Class defaults: `ViewModelLeadPivotName` `L_Weapon_Bone`, `ViewModelRotationOriginOffset` (150, 0, 0),
  `ViewModelRotationAmt` 3, `ViewModelTranslationAmt` 3, `LeadingSpeed` 6, plus `MaxPitchLead`/`MaxYawLead` and `BobDamping`
  per type. These drive the lag of the view model behind the camera (how they combine was not read).
* **`PlayerViewOffset`** exists per weapon type (Dahl pistol (20, 4, 2), Dahl assault rifle (12, 2, 2)); no use was found
  in the script placement path, so it may be a leftover of the engine's weapon class (`UNVERIFIED`; do not use it to place the
  mesh).
* **Foreground FOV.** `WillowPlayerController.UpdateForegroundFOV` sets `ForegroundFOV` to the held weapon's
  `FirstPersonMeshFOV` (45 on the pistol and assault rifle defaults) while the weapon is attached, else to
  `GlobalsDefinition.UnarmedFirstPersonFOV` (60), and sets `bForegroundFOV` when that value is above 0. The first-person
  mesh and the arms draw in the foreground depth group with their own projection field of view, independent of the world
  FOV. (Whether the number is horizontal or vertical, and the exact foreground clip planes, were not read; UE3
  convention is horizontal.)
* Weapons are hidden from the owner's third-person view and the arms from others (`ChangeOwnerVisibility`).

## 5. Muzzle flash, tracer, shell casing and fire effects

* **Components.** `AttachMuzzleFlash` (script) attaches the pre-created `FirstPersonMuzzleFlash`
  `ParticleSystemComponent` to the first-person mesh at `WeaponTypeDefinition.MuzzleFlashSocket` (class default
  `MuzzleFlashSocket`; the pistol and assault-rifle types override it with `Muzzle`), and a third-person copy to the
  third-person mesh. One extra flash per `AltMuzzleFlashSockets` entry of the barrel part (multi-barrel weapons) is
  created and attached to those sockets; `NumberOfMuzzleFlashes` = socket count + 1 and the routine cycles the
  current barrel index each shot. A `MuzzleFlashLight` point light (`MuzzleFlashLightTemplate` per type) is attached the same
  way.
* **Which particle system.** `WeaponTypeDefinition.MuzzleFlashPSTemplates` is an `EffectCollectionDefinition`: an ordered
  list of `{Expression (an attribute-expression evaluator on the weapon), ParticleEffect}` (for example the pistol default
  collection maps corrosive to the SMG corrosive flash and explosive to the pistol explosive flash, then defaults).
  The first entry whose expression holds is used (first-match order is an inference).
* **Duration.** `MuzzleFlashDuration` 0.33 s default; the flash is switched on at the shot and a timer stops it.
* **Tracer.** `TracerTemplate` with colour parameter `TracerColorParameterName` ("TracerColor") set to `TracerColor`
  (default 200,200,200,255) in `InitEffects`.
* **Shell casing.** A `ShellCasingPSCTemplate` component attached to the first-person mesh at `ShellCasingSocket`
  (default `EjectPort`); the body part can override template, socket and offsets (`bOverrideShellCasing`); off-hand
  weapons rotate it by `OffHandShellCasingRotOffset`.
* **The per-shot routine** (native, the weapon's fire-effects target; read only in outline): plays the firing sound for
  local players; advances the current-barrel index (modulo the number of barrels) and, for multi-barrel weapons, the
  alternate-flash index; computes the fire-animation duration as the largest of the interval-based duration the
  weapon reports, `MinFireAnimDuration` (0.1 s when unset) and three times a per-frame time value; plays the selected
  fire animation with that duration (arms and weapon mesh); spawns the muzzle effect at the muzzle socket through the
  pawn's effect objects; arms the `MuzzleFlashDuration` timer; handles the shell casing; and adds `FiringGlowImpulse` to the
  glow (section 2). The individual sub-steps and the view-shake call (`FireShake` is a per-type field) were not
  matched to calls.
* **Animation notifies.** First-person weapon clips carry `AnimNotify_UseBehavior` events that run behavior collections
  (for example `GD_Weap_Shared_Effects.BehaviorCol_Maliwan_PowerUp` / `PowerDown` on pistols and SMGs, and a bone
  visibility toggle for the Jakobs sniper rounds). Brand-specific effects such as the Maliwan charge glow are
  therefore data in the clips, not code.

## 6. UNVERIFIED and how to confirm

| Statement | Confirmation |
|---|---|
| Material instance = Material part's MIC as parent + elemental `p_EmissiveColor` | SDK trace of a spawned weapon's material instance parent and parameter list (`tools/sdk_trace`) for a corrosive, a fire and a non-elemental gun |
| Glow `1 + impulse` rule and constants | Trace `p_EmissiveScale` of the weapon instance while firing single shots and a burst |
| Attachment socket `Weapon` on the arms mesh; foreground FOV 45 | Trace the first-person mesh's attach socket and `PlayerController.ForegroundFOV` while a weapon is held |
| Muzzle flash template pick order | Trace the flash component's `Template` for an elemental gun |

## 7. What was not read

The merge algorithm for several part meshes (`BuildCompositeMesh`), the gestalt data layout, the body of the per-shot
routine beyond the outline, `AddWeaponBoneControllers`, the effect-collection expression evaluation order, and the
view-model sway maths.
