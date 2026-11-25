# A particle component's colour parameter: SetColorParameter (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-PSC-SETCOLORPARAMETER (decision 2026-11-25-54), the analyst half
of the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/psccolor/`.
This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every
rule here is UNVERIFIED in the running game.**

**Why.** Link 8 (`equip_gun`, `init`) now gets past `SetMaterial` and the glow read
([NATIVE_INITMESHES_MATERIAL_GLOW.md](NATIVE_INITMESHES_MATERIAL_GLOW.md)). It stops at the unbound
`ParticleSystemComponent.SetColorParameter(name, color)`.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **Existing records:** `native_index_find.py` has no row for `SetColorParameter` or `SetFloatParameter`.
  [NATIVE_SPAWN_PICKUP_PARTICLES.md](NATIVE_SPAWN_PICKUP_PARTICLES.md) step 8 reads the sibling `SetVectorParameter`
  in full. [NATIVE_PARTICLE_ACTIVATION.md](NATIVE_PARTICLE_ACTIVATION.md) does not touch the parameters.
- **The script** (`research/script_disasm.py`):
  - the call is in `WillowWeapon.InitEffects`, which `InitMeshes` calls after `InitPartParticles`. It is the function's
    only statement: `TracerParameterTemplate.SetColorParameter(WeaponTypeDefinition.TracerColorParameterName,
    WeaponTypeDefinition.TracerColor)`;
  - `SetColorParameter` is a native final function of `ParticleSystemComponent`. It is also reached from `Emitter`,
    `WillowReplicatedEmitter`, `WillowPhysicalMaterialProperty` and the behavior-parameter helpers; none of those is on
    a slice path.
- **The native:** Ghidra 12.1.4 headless, read-only, under the machine lock. The exec and the routine it calls were
  read, and `SetVectorParameter`'s exec beside it for comparison.
- **Layouts:** the local copy of `tools/ghidra/class_layout.py` gives `ParticleSysParam` (64 bytes), with `Name`,
  `ParamType` and `Color` exactly where the routine reads them. `ParticleSystemComponent.InstanceParameters` sits at
  the same constant shift as the component's other fields. The enum `EParticleSysParamType`, read from `Engine.upk`'s
  export, lists `PSPT_None`, `PSPT_Scalar`, `PSPT_ScalarRand`, `PSPT_Vector`, `PSPT_VectorRand`, `PSPT_Color`,
  `PSPT_Actor`, `PSPT_Material`, `PSPT_MAX`. So the value the routine writes, 5, is `PSPT_Color`.
- **Link 8's data**, with `ow-package --def-dump ... --cooked <CookedPCConsole>` from the installed packages (output
  under ignored `local/analysis/psccolor/`):
  - `GD_Weap_AssaultRifle.A_Weapons.WT_Bandit_AssaultRifle`: `TracerColorParameterName` `TracerColor`, `TracerColor`
    R 200, G 200, B 200, A 255;
  - `WillowGame.Default__WillowWeapon`: `TracerParameterTemplate` is its subobject `TracerParameterParticleSystem`, a
    `ParticleSystemComponent`;
  - that subobject: `InstanceParameters` empty, `Template` None, `bAttached` false, `Owner` None.

## 2. What SetColorParameter does

Its arguments are a name and a `Color` (four bytes). On the receiving component:
1. **a None name:** nothing;
2. otherwise, the component's `InstanceParameters` is scanned in order. **The first entry whose `Name` is the name and
   whose `ParamType` is `PSPT_Color` (5)** gets `Color` := the colour. Nothing else of that entry changes, and it ends
   there;
3. **with no such entry:** one zeroed entry is appended, with `Name` := the name, `ParamType` := 5 and `Color` := the
   colour.

An entry with the same name but another type is not matched: a new colour entry is appended beside it.

**Nothing else.** The routine writes no other field of the component. It does not reattach, mark a transform update,
touch the emitter instances or call any render-side code. It is the same shape as `SetVectorParameter` (type 3,
`Vector`; NATIVE_SPAWN_PICKUP_PARTICLES.md step 8), with type 5 and `Color` instead. The running emitters read the
parameter later, on their own ticks. **So no part of this native is render-only state, and none needs a `LayerAbsent`
answer.**

**Purpose** (by fit): the weapon type's tracer colour is stored on a parameter-holder component, and the weapon's
tracers take their parameters from it later.

## 3. The Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `ParticleSystemComponent.SetColorParameter(name, color)` in Off, under the
key the stop names. Reference keeps the stub policy.

**Preflight** (read only, no load, nothing interned):
1. **The receiver None:** the script's usual null-context answer, as for any native call on None.
2. **The name:** None (the name table's `None`): nothing, and no other read. This comes before the receiver's load
   check, because the game does nothing with a None name and reads nothing of the receiver.
3. **A lazy receiver:** a store handle not loaded yet is `NotYet { NotRead }`, "SetColorParameter on a component not
   loaded at set-up (a load is a write)".
4. **The declaration:** the receiver's class declares `InstanceParameters` as an array of a struct with `Name` (a
   name), `ParamType` (a byte) and `Color` (a `Color` struct). Otherwise `VmError::Script` naming the field. Each
   element read must hold those kinds, else `VmError::Script`.
5. **The plan:** the index of the first element with that `Name` and `ParamType` 5, or an append. An appended element
   is the struct's zero value with the three fields set. Build the whole new array value in the plan, as
   `pickup_particles.rs` does for `SetVectorParameter`, and check it fits the declaration.

**Write:** one write, `InstanceParameters` := the planned array. Nothing else.

**On link 8.** The receiver is the weapon's `TracerParameterTemplate`. In the game, the spawned weapon holds its own
instanced copy of the class default's `TracerParameterParticleSystem`, by the engine's component instancing. The port's
answer is the same rule on whatever the field holds. The implementer should check, in the slice harness, that the field
is the weapon's own copy and that the class default's subobject is left unchanged. If it is not the weapon's own
copy, that is a harness or instancing gap, not a rule of this native. With the class default's values, the call
appends one entry: `TracerColor`, type 5, colour (200, 200, 200, 255).

**What link 8 would meet next** (a prediction, not a run), in `InitMeshes` after `InitEffects`:
1. `InitVisibleAmmoBones` (script): for each name in the body part's `VisibleAmmoBoneNames`,
   `FirstPersonMesh.MatchRefBone` (not indexed). With an empty list it calls nothing;
2. then the `ExtraSlots` loop. The list is filled by `InitExtraMeshes` from the owner pawn's body class, so it is
   empty for a weapon with no pawn owner (by the script; link 8's world not run). For each slot with a mesh:
   `InitGestaltMeshDataFromOther` (indexed) when the weapon type has a `GestaltMesh`, `SetSkeletalMesh` (indexed),
   `SetScale` (indexed) and `SetMaterial` (indexed);
3. then `InitMeshes` returns.

## 4. Tests for the implementer

Synthetic, with `package()` / `tag()`. Each refusal leaves #518's three counters and the name table unchanged:
1. an empty `InstanceParameters`: one element appended, `Name` the name, `ParamType` 5, `Color` the colour, every
   other field zero;
2. an element with the same name and type 5: its `Color` replaced in place, the array length unchanged, its other
   fields kept;
3. an element with the same name and type 3 (a vector): not matched, a colour element appended after it;
4. two type-5 elements with the same name: only the first changes;
5. a None name: nothing written;
6. `InstanceParameters` missing, not an array, or with an element missing `Color` or holding a wrong kind:
   `VmError::Script` naming it, nothing written;
7. a lazy receiver: the not-loaded `NotRead`, nothing loaded. With a None name and a lazy receiver: nothing, no
   refusal;
8. the colour's four channels (B, G, R and A) copied as given, alpha included;
9. no other field of the component changes (`bNeedsReattach`, `bNeedsUpdateTransform` and `Template` included);
10. a toy weapon whose `InitEffects` runs on its own instanced parameter component: the class default's component is
    unchanged;
11. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above. The routine's rule is read whole. The type value is confirmed by the enum export.
2. **Not read:** how the weapon's tracers later read the parameter holder (render and effects side, off the slice).
3. **Not run:** whether the port's spawned weapon holds its own copy of `TracerParameterParticleSystem`.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After a Bandit assault rifle is spawned, read its `TracerParameterTemplate.InstanceParameters`. Expect one entry:
   `TracerColor`, `PSPT_Color`, (200, 200, 200, 255).
2. Read `WillowGame.Default__WillowWeapon.TracerParameterParticleSystem.InstanceParameters`. Expect it still empty.
