## 2026-11-25: Link 8's Startup material preload

AI-assisted (Claude Code, Claude Opus 5.5). Lane LINK-8-STARTUP-MATERIAL-PRELOAD, on main ce76c5f8. A harness set-up
step, not a native rule. No real-game session was run. **UNVERIFIED in the running game.**

**Why.** With #702, link 8's `init` stopped at `BuildWeaponMaterial`'s parent-material refusal: the Material part's
`Material`, `Common_GunMaterials.Materials.AssaultRifle.Mati_BanditCommonAR` (an `Engine.MaterialInstanceConstant`),
was not loaded, and the native's preflight loads nothing (a lazy load is a write).

**What is recorded** (review of #712):
- `Mati_BanditCommonAR` is **Startup export 23880**, read 2026-11-25 from the installed `Startup.upk`'s export table
  (`ow-package Startup.upk --exports`, a local read; no session);
- SLICE_NPC_ASSETS.md records one other gun material the same way, the pistol's
  `Common_GunMaterials.Materials.Pistol.Mati_MaliwanUncommon` as Startup export 23971. The same read gives 23971 for it;
- that the game loads the `Startup` package at boot is stated in decision 2026-11-25-36. No verification record reads
  it, so it stays **UNVERIFIED**, and so does the material being resident before a weapon is built.

**What changes.** `ow-cli` `equip_run::preload_material_part_material`, in Off only. It loads the weapon's
`DefinitionData.MaterialPartDefinition.Material` through the store when the part is resident, the material is an
unloaded handle, **and its export is in the `Startup` package**. The package comes from the store's own record, through
a new read-only `DefStore::handle_source`. A material from any other package is skipped with a logged line naming its
package and export, and `BuildWeaponMaterial`'s refusal stands. It sits beside the muzzle-flash and gestalt preloads, in both `--equip-run` (`prepare_equip`) and
link 8's driver, after the new-mesh preload. As those do, it logs a `harness input:` line before it runs and its outcome
after (": loaded", or ": failed (...)" with the error kept); the slice-gate row's `harness` lists both. It loads only
that one object: not the part, nor the material's parent chain, nor any class. `BuildWeaponMaterial`'s refusal is
unchanged.

**The material walk** (the hub's rulings, a third and a fourth commit). With SetMaterial bound (the stacked
IMPL-SETMATERIAL-AND-GLOW lane), the glow read walks the weapon material's `Parent` chain, and for a `Material` its
`Expressions`, refusing at an object that is not resident. Reverted probes on the installed run (2026-11-25) named:
- first the parent `Common_GunMaterials.MasterMaterials.Bandit.MasterMati_BanditCommon`, **Startup export 23841**;
- once that chain was loaded (its end, `Common_Materials.Weapons.Master_Gun`, is Startup export 22890), the expression
  `Common_Materials.Weapons.Master_Gun.MaterialExpressionVectorParameter_11`, **Startup export 23744**.

So after loading the Material-part material, the preload walks what the glow read may read, from that material:
- each visited object's `Parent`, and for an object that declares `Expressions` its entries in order (None entries are
  skipped, as the glow read skips them);
- an object is loaded **only if it is a Startup export** (`DefStore::handle_source`, the rule above); one already
  resident is visited with no load;
- a branch stops with a logged line at a None `Parent` and at the first object that is not a Startup export, which is
  left for the native to refuse;
- each object is visited once, and the walk stops at a total cap of 64 objects (logged if hit).

Each load is logged before and after; a failed load keeps its error. In the game `Startup` is resident from boot
(decision 2026-11-25-36's statement, UNVERIFIED), so this is the same **UNVERIFIED stand-in** as the material's.

**The material classes** (the hub's extension, a second commit). With the material resident, `BuildWeaponMaterial`'s
step 4 refused next: it asks, with no load, for `Engine.MaterialInstanceConstant` and the weapon type's
`MaterialClass` to be ready (a defined layout and loaded code) and for that class's default object to be resident.
Engine and WillowGame script classes are loaded at boot in the game (startup and native script packages; the hub's
reading, UNVERIFIED). So
`equip_run::preload_material_classes` follows the material preload, in Off, as a harness set-up load (not a native
stand-in; **UNVERIFIED** that the game has them resident at that point). It loads, each only when not already so:
1. `MaterialInstanceConstant`'s class code, then `Engine.MaterialExpressionScalarParameter`'s (a fifth commit, the hub's
   ruling). That second class is loaded only when the material's resident `Parent` chain holds a `Material` with
   expressions: the glow read tests expressions against it (NATIVE_INITMESHES_MATERIAL_GLOW.md section 4.2), and with
   the Startup walk loaded, link 8 stopped at that class check. It is another Engine script class, loaded at boot in
   the game (UNVERIFIED);
2. the `MaterialClass`'s class code;
3. that class's default object (`DefStore::class_default`).

Steps 2 and 3 run only for a `MaterialClass` whose layout is defined and that is a child of `MaterialInstanceConstant`
(review of #712). Any other class is skipped with a logged line and left to the native's own refusal:
`class_not_ready` while its code is cold, `outside_family` once loaded. On the installed run, steps 2 and 3 logged
nothing: the `MaterialClass` was already ready and its default resident by then. So those two steps are covered by the
synthetic tests only.

Each is logged before and after, like the material preload. With no weapon type, or a `MaterialClass` of None, only
step 1 runs, and the native answers those cases itself. `BuildWeaponMaterial`'s refusals are unchanged.

**Tests** (`equip_run_material_tests.rs`, invented Engine and WillowGame packages):
- Off loads and logs, and a second call does nothing;
- no part, a part with no material, and a cold part: nothing, no line, no load;
- a failed load keeps its error;
- Reference does nothing;
- the class step loads both classes' code and the class default, logged, and a second call does nothing;
- with no `MaterialClass` only the family class loads, and Reference does nothing;
- a material that is not a Startup export is skipped with a line, and nothing is loaded;
- a two-level Startup parent chain loads both parents, logged, and ends with a line;
- a parent that is not a Startup export stops the walk with a line and is not loaded;
- a Material with two Startup expressions has both loaded;
- a non-Startup expression is skipped with a line and stays unloaded, while the Startup one beside it is loaded;
- the scalar-expression class is warmed, logged, for a chain with a Material with expressions, and adds no line when
  already loaded; a chain with no expressions does not warm it;
- an out-of-family `MaterialClass` gets no class-code or default load, one skip line, and stays cold (the native's
  refusals for that case are tested in `ow-inventory`'s `weapon_material_tests.rs`).

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
