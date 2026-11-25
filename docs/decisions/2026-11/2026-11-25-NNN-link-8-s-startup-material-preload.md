## 2026-11-25: Link 8's Startup material preload

AI-assisted (Claude Code, Claude Opus 5.5). Lane LINK-8-STARTUP-MATERIAL-PRELOAD, on main ce76c5f8. A harness set-up
step, not a native rule. No real-game session was run. **UNVERIFIED in the running game.**

**Why.** With #702, link 8's `init` stopped at `BuildWeaponMaterial`'s parent-material refusal: the Material part's
`Material`, `Common_GunMaterials.Materials.AssaultRifle.Mati_BanditCommonAR` (an `Engine.MaterialInstanceConstant`),
was not loaded, and the native's preflight loads nothing (a lazy load is a write). SLICE_NPC_ASSETS.md records the
`Common_GunMaterials` materials as Startup exports, which the game loads at boot, so in the game the material is
resident before any weapon is built.

**What changes.** `ow-cli` `equip_run::preload_material_part_material`, in Off only. It loads the weapon's
`DefinitionData.MaterialPartDefinition.Material` through the store when the part is resident and the material is an
unloaded handle. It sits beside the muzzle-flash and gestalt preloads, in both `--equip-run` (`prepare_equip`) and
link 8's driver, after the new-mesh preload. As those do, it logs a `harness input:` line before it runs and its outcome
after (": loaded", or ": failed (...)" with the error kept); the slice-gate row's `harness` lists both. It loads only
that one object: not the part, nor the material's parent chain, nor any class. `BuildWeaponMaterial`'s refusal is
unchanged.

**The material classes** (the hub's extension, a second commit). With the material resident, `BuildWeaponMaterial`'s
step 4 refused next: it asks, with no load, for `Engine.MaterialInstanceConstant` and the weapon type's
`MaterialClass` to be ready (a defined layout and loaded code) and for that class's default object to be resident.
Engine and WillowGame script classes are loaded at boot in the game (startup and native script packages). So
`equip_run::preload_material_classes` follows the material preload, in Off, as a harness set-up load (not a native
stand-in; **UNVERIFIED** that the game has them resident at that point). It loads, each only when not already so:
1. `MaterialInstanceConstant`'s class code;
2. the `MaterialClass`'s class code;
3. that class's default object (`DefStore::class_default`).

Each is logged before and after, like the material preload. With no weapon type, or a `MaterialClass` of None, only
step 1 runs, and the native answers those cases itself. `BuildWeaponMaterial`'s refusals are unchanged.

**Tests** (`equip_run_material_tests.rs`, invented Engine and WillowGame packages):
- Off loads and logs, and a second call does nothing;
- no part, a part with no material, and a cold part: nothing, no line, no load;
- a failed load keeps its error;
- Reference does nothing;
- the class step loads both classes' code and the class default, logged, and a second call does nothing;
- with no `MaterialClass` only the family class loads, and Reference does nothing.

**The slice gate** (installed Off strict, release build, under the machine lock):
- **with the material preload alone:** 5 ok / 9 NotYet / 0 error, X 41.3%. Link 8 stopped at `init`, at
  BuildWeaponMaterial's class check, "the class Engine.MaterialInstanceConstant, not loaded (a class load is a
  write)";
- **with the class step too:** 5 ok / 8 NotYet / 1 error, `step share X = 5.783/14 = 41.3%`. Link 8 is 1 of 5 at
  `init`, now an **error at the unbound `MeshComponent.SetMaterial(int,object)`** (its note is #705, on main). So
  BuildWeaponMaterial answers past its preflight. The harness lines show `MaterialInstanceConstant`'s code loaded.
  Steps 2 and 3 logged nothing, so both were already satisfied. That fits the weapon type's `MaterialClass` being
  `MaterialInstanceConstant` itself, with its default object built by the material's load; this was not checked
  further.
