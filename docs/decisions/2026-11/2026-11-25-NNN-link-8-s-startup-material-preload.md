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

**Tests** (`equip_run_material_tests.rs`, an invented package):
- Off loads and logs, and a second call does nothing;
- no part, a part with no material, and a cold part: nothing, no line, no load;
- a failed load keeps its error;
- Reference does nothing.

**The slice gate** (installed Off strict, release build, under the machine lock): 5 ok / 9 NotYet / 0 error,
`step share X = 5.783/14 = 41.3%`. Link 8 is 1 of 5 steps. Its preload lines show the material loaded. **Its new stop
is `init` at BuildWeaponMaterial's step 4, not SetMaterial:** "the class Engine.MaterialInstanceConstant, not loaded
(a class load is a write)", marked "stopped after partial writes" by the run's earlier writes. Loading the material
defines that class's layout but not its code, and the preflight asks for both. Whether the harness should also warm
that class (it is resident at boot in the game, by the same Startup reading, UNVERIFIED) is the hub's call. It is not
done here.
