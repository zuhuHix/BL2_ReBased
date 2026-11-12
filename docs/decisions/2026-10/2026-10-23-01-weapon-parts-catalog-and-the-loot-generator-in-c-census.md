## 2026-10-23: Weapon parts catalog and the loot generator in C++ (census, native gaps, exact port, live checks)

AI-assisted (Claude Opus 5.5 orchestrating; Opus and Haiku sub-agents; review GPT-6-Luna). Record:
`docs/verification/WEAPON_PARTS_CATALOG.md`; rules in `docs/verification/NATIVE_WEAPON_RULES.md` section 12. No package
parsing behaviour changed (`src/package.cpp` and `src/container.*` untouched).

- **Census.** `tools/weapon_catalog.py` over every installed package (2,010 files, 315 with weapon data) and
  `tools/real_game/scripts/weapon_parts_census.py` in the running game: 369 weapon balances, 908 parts, 89 weapon types, 22
  manufacturers, 876 name parts, 2,014 item pools, packages and OpenBLCMM agreeing except 7 remaster gestalts and one
  pool path. Catalog under ignored `local/weapons/catalog/`; counts only in the record.
- **Rules read from native code** (UNVERIFIED in game unless stated): the value function (type base modifier, part and
  name-part product, truncation, currency cap), the slot value ((grade - 1) * per-grade, max then min clamp), the
  part-weight hook (relics only), the grade choice and stage cap (opt-in `stage_cap`), the status card chance through the
  damage type's presentation remap (slag prints 0.1 higher), and name-part expressions (all must hold;
  `WeaponDamage_Is_<Element>` checks the weapon's damage type, confirmed live on one gun). REA's `open_binary` could not
  use the existing analysis database and its analysis calls failed, so the reading used `tools/ghidra/run.ps1`.
- **Generator.** `weapon_recipe.roll()` now defaults to `CrtRand` (the MSVC `rand()`), our seeding; the slice tools pin
  Python's generator so pre-rolled slice guns are unchanged. `tools/weapon_gen_manifest.py` writes a line-based manifest
  (base game `Startup.upk` only: 249 balances; DLC not yet); `src/weapon_gen.{hpp,cpp}` (in `ow-core`) and `ow-weapongen`
  reproduce `roll()` + `evaluate()`: 2,919,834 fields over 29,880 balance rolls, 7,680 pool rolls and 6,795 pool guns,
  **0 mismatches** (floats bit for bit), uncapped and with a stage cap of 50.
- **Host.** `OpenWillowWeaponGen.{h,cpp}`: manifest load (missing = off), conversion to `FOpenWillowWeaponItem`, new
  shared item fields `Parts` (slot order), `Material`, `GestaltMesh` for the weapon-assembly lane, `ow.RollGun`, and the
  quest turn-in reward rolled from the stand-in's own source pool (fallback: the pre-rolled stand-in).
- **Live** (real game, saves blocked and unchanged): 40/40 spawned generated guns carry the generated balance, parts,
  level, prefix, title (after the expression rule) and value; part frequencies of two common balances consistent with the
  weights (2 x 400 rolls per session, two sessions); level = stage and the name rule on 1,600 of 1,600 game rolls. Read
  stats of 44 spawned guns: 366 of 484 values equal to the generator bit for bit on cooked data, 26 explained by runtime
  data, **92 not triaged** (some look like read errors); no card captured. The Dahl rifle 9.5 % above its card (Weapons 3)
  is runtime data: the live overlay reproduces it exactly.
- **Checks.** CTest 11/11 (new `weapon-gen-synthetic`: hand-worked synthetic manifest plus parser robustness);
  `verify_packages.py` all match; Python unit tests (weapon_recipe 20, weapon_stats 42, weapon_gen 13, loot_pools,
  weapon_catalog 18, gen_confirm 15); golden cards 69 / 69 / 68 of 69, 0 unsupported (was 67 / 67 / 66 with 2 unsupported:
  an empty card presentation entry made the evaluator raise). In engine: host against `ow-weapongen` 245 fields, 0
  differences; `tools/test_inventory_actions.ps1` 49/49; the quest self-test stops before turn-in on this machine (Marcus
  not imported), so pickup of a generated reward is not checked.
- **Sensitive file:** `CMakeLists.txt` (adds `src/weapon_gen.cpp` to `ow-core`, the `ow-weapongen` executable and the
  `weapon-gen-synthetic` test; verified by the build and CTest).

