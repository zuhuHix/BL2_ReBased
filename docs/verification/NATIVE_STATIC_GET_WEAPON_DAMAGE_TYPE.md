# WillowWeapon.StaticGetWeaponDamageType on link 8 (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-STATIC-GET-WEAPON-DAMAGE-TYPE (decision 2026-11-19-NNN, number
pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the script, read with `research/script_disasm.py` (callers in the nine code packages);
- the installed packages through `ow-package --def-dump` and `tools/ghidra/class_layout.py` (names only);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock);
- a local, uncommitted probe of the installed slice gate (section 5).

There is no bl-sdk session and no recorded live evidence. Raw output stays under ignored `local/analysis/dmgtype/`.
This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every
rule here is UNVERIFIED** in the running game.

**What was already covered.** The rule was read from native code before (2026-10-23): `tools/weapon_recipe.py`'s
`native_damage_type`, cited by NATIVE_CACHE_WEAPON_CARD.md and WEAPON_BALANCE_DECODE.md. `ow-combat`'s
`hit::shot::weapon_damage_type` carries it as an internal helper for the shot pipeline. **Nothing binds the
script-callable native** on main or on any open head; that is why link 8 stops at it. There is no NATIVE_INDEX row
yet. This note re-checks the rule against the executable, places it on link 8, and gives the Off binding.

## 1. Callers (script)

`StaticGetWeaponDamageType` is a static native taking one `WeaponDefinitionData` struct and returning a
`WillowDamageTypeDefinition`. Its script callers are `WillowWeapon.ClientInitializeFromWeaponTypeDefinition`,
`WillowWeapon.DoesDamageType`, `WillowWeapon.SetElementalFrame`, `WeaponScopeGFxMovie.GetElementalName` and
`WillowInventoryManager.ServerIncrementWeaponPickedUpStats`.

**On link 8:** `ClientInitializeFromWeaponTypeDefinition` sets `InstantHitDamageTypeDefinitions[0]` to
`StaticGetWeaponDamageType(DefinitionData)`, between `InstantHitDamageTypes[0]` (the type's `InstantHitDamageType`)
and `InstantHitImpactDefinitions[0]` (the type's or the elemental part's `WeaponImpact`).

## 2. The rule (re-checked against the executable)

With `D` the definition data passed by value:
1. `D.WeaponTypeDefinition` None: the answer is **None**. The elemental part is not looked at.
2. Otherwise the answer starts as the weapon type's `DefaultDamageTypeDefinition`.
3. If `D.ElementalPartDefinition` is set and its `CustomDamageTypeDefinition` is set, that replaces it.

No other part is consulted. The native reads two fields of two definitions and writes nothing. It neither draws nor
loads anything of its own: a definition it reads must already be readable, and the damage type is returned as a
reference, loaded or not.

## 3. The slice gun (names only)

Link 8's weapon (`EQUIP_WEAPON`, the Bandit assault rifle) has no elemental part, so the answer is the type's
default. `GD_Weap_AssaultRifle.A_Weapons.WT_Bandit_AssaultRifle` has `DefaultDamageTypeDefinition`
`GD_Impact.DamageType.DmgType_Normal` (its `InstantHitDamageType` is `WillowDmgSource_MachineGun`). The probe found the
weapon type **already loaded** on entry, and the elemental part None.

## 4. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `WillowWeapon.StaticGetWeaponDamageType(weapondefinitiondata)` in Off;
Reference keeps the stub policy. Share the rule with `ow-combat`'s `weapon_damage_type`, but take its loading out
first: that helper loads its definitions, and the native must not.

**Preflight (write-free, loads nothing):**
1. The argument is a `WeaponDefinitionData` struct with `WeaponTypeDefinition` and `ElementalPartDefinition`
   object fields. Otherwise a `VmError::Script` naming the field.
2. `WeaponTypeDefinition` set and an unloaded handle: `NotYet { NotRead }`, "StaticGetWeaponDamageType: the weapon
   type definition is not loaded (a load is a write)".
3. `ElementalPartDefinition` set and an unloaded handle (checked only when the type is set, since step 1 of the rule
   answers None first): `NotYet { NotRead }`, "StaticGetWeaponDamageType: the elemental part definition is not loaded
   (a load is a write)".
4. A set definition whose class declares no `DefaultDamageTypeDefinition` (the type) or no
   `CustomDamageTypeDefinition` (the part), or declares it as a non-object: a `VmError::Script` naming the field.

**The answer:** section 2's rule, as an object (None included). The damage type is returned as stored and never
loaded.

**Writes and the counters:** none. The native neither writes nor loads, so its refusals never carry the "stopped
after partial writes: " prefix (#518's counters cannot move inside it).

## 5. What link 8 meets after it (local probe, uncommitted)

The probe ran on a local, unpushed merge of #558 (3a598e4c), #534 (8fc0420b), #552 (f17a9d60) and #572
(fix/rust-set-base-value 865b568d), with throwaway stand-ins:
- for `GetParticleEffect` (its handles loaded by the probe, the 5 unbound evaluators taken as false);
- for `GetExpLevelFromManufacturerData`, the interpolating path;
- for `SetBaseValue`'s unloaded destination, loaded by the probe, as the hub's harness preload now does;
- for this native, section 2's rule.

**Installed Off `--slice-gate` strict: 1 ok, 4 notyet, 9 error.** **Link 8 is 1 of 5 at `init`** (the driver's steps
are make, init, bases, equip and unequip; `make` passes). The stand-in answered `DmgType_Normal`, with the type
loaded and no elemental part. The first stop moved to the unbound **`SkeletalMeshComponent.SetSkeletalMesh(object,
bool)`**, the `FirstPersonMesh` reset in `ClientInitializeFromWeaponTypeDefinition`, right after
`InstantHitImpactDefinitions` and `FireOffset`.

## Open questions

1. **UNVERIFIED:** that no other part (barrel, body, and so on) can supply a damage type. The native reads only the
   elemental part; `tools/weapon_recipe.py` notes an "any part" variant that the tools use for names, which the
   native does not.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `StaticGetWeaponDamageType` (post) on a non-elemental Bandit assault rifle's build: expect `DmgType_Normal`.
2. The same on an elemental variant (for example a shock part): expect the part's `CustomDamageTypeDefinition`.
3. Read `InstantHitDamageTypeDefinitions[0]` on both weapons after the build, and compare.
