## 2026-11-25: WillowWeapon.BuildWeaponMaterial in Off (link 8)

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-BUILDWEAPONMATERIAL, link 8's next stop after #678 (`init` stopped at
the unbound `WillowWeapon.BuildWeaponMaterial(object,object,weapondefinitiondata)`). The branch starts from
origin/integration/2026-11-25-c15 at 25425bb. Coded from docs/verification/NATIVE_WEAPON_VISUALS.md section 1.1 alone,
at #698's review r1 head 5797b5a (decision 2026-11-25-32). The note's commit is not in this branch. Clean room: the
analyst lane read the native; this lane did not open the executable, Ghidra, REA or the analysis output. Synthetic
fixtures only. Off only; Reference binds nothing.

**What changes:** `ow-inventory` `equip::weapon_material` binds the native in Off (keep-existing by key), beside
`StaticGetWeaponDamageType` in the equip keys.

**The preflight** is write-free and loaded-only. It reads by name on resident objects; a field of the wrong kind is a
`VmError::Script` naming it. In the note's order:
1. no `WeaponTypeDefinition` answers None;
2. a None weapon is `NotRead` (the default outer);
3. the parent: the Material part's `Material` (each must be resident). With no such material and a mesh component
   passed, it is the material-0 `NotRead`; with neither, None;
4. the class:
   - `MaterialClass` None is `NotRead`;
   - `MaterialClass` and `Engine.MaterialInstanceConstant` must each be ready with no load (`globals::ready_class`:
     a defined layout and loaded code), and the class's default object must already be resident, found by a
     load-free scan of the heap for its default origin. Otherwise `NotRead` naming the class;
   - a class outside the `MaterialInstanceConstant` family is `NotRead`;
   - the instance's `Parent` must be declared as an object reference that can hold the parent;
5. the nine non-name part slots (`WEAPON_PART_FIELDS`, empty ones skipped): `MaterialVectorParameterValues` must be
   an array of a struct with `ParameterName` and `ParameterValue`, and any entry is the vector-parameter setter's
   `NotRead`.

**The commit:** one object of `MaterialClass` through `instantiate_from_defaults` (the class default's values), with
the weapon as its outer, as a script `new` does. Then its `Parent` = the parent, **UNVERIFIED by name** as the note
labels it: the parent setter's other effects were not read, and its render work is not ported. The answer is the
instance. Every check is in the preflight. A refusal after the creation would be marked "stopped after partial
writes" by #518's counters and the class-code loads (#685).

**Tests** (`equip/weapon_material_tests.rs`, synthetic; each preflight answer or refusal leaves #518's counters, the
class-code loads, the name table and the heap unchanged): the note's tests 1 to 12.
- 2 is link 8's call shape: a mesh passed and a Material part's material, with no refusal; one new instance whose
  outer is the weapon and whose `Parent` is the material.
- 3 also covers a Material part whose `Material` is None.
- 5 and 10 also cover a ready class whose default object is not yet built.

**Effect on link 8** (predicted, not run): with a Material part's material and no part carrying vector parameters,
the native answers an instance. `InitMeshes` then reaches `SetMaterial`, the next candidate stop. An elemental part
gives the setter's `NotRead`. The installed `MaterialClass` must be ready, with its default object resident, by then.

**Not verified:** anything in the running game; the parent setter's other effects; whether the installed material
class's default object is resident when link 8 gets there.

**Review r1** (Opus high, on #702):
- **`MaterialClass`'s readiness before the family check.** A declared-only class has no super chain, so it now gets
  `class_not_ready` instead of `outside_family`. The order is readiness (layout and code), then the family, then the
  default object, so a class outside the family is still named as such without a default object being required.
  Test 10 gains the declared-only case. It fails with the old order.
- **The default-object check.** The store exposes no load-free query of its class-default cache, so the heap scan
  stays. A comment records why it agrees with that cache: outside tests only `ow_object::cdo`'s builder allocates
  `Origin::Default`, and it records each object it builds there.
- **Test 2** also asserts that the store's load count is unchanged and that the default object is the same handle.
  The tests file and this decision both say tests 1 to 12.

**Link 8 on the runner (JOB 21)** stops at this native's not-loaded refusal for the Material part's `Material`
(`Mati_BanditCommonAR`); the refusal stays. SLICE_NPC_ASSETS.md records a `Common_GunMaterials` material as an export
of the `Startup` package, which the game loads at boot. So in the game the material is resident before any weapon is
built. In the port, the matching stand-in is a harness preload of the definition's Material-part material at link 8's
set-up, like the muzzle-flash and gestalt preloads in `ow-cli` `equip_run.rs`. That is not part of this native.
