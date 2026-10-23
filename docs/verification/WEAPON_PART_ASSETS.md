# Weapon part assets: every part mesh and skin in UE, and guns built from their parts at run time (2026-10-23)

AI-assisted (Claude; implementation sub-agents on Claude Haiku, reviews and the visual critic on a Codex model). This record
holds rules, counts and paths into ignored `local/`; everything decoded from the game (meshes, textures, dumps, captures) stays
there. Companion records: [WEAPON_VISUALS.md](WEAPON_VISUALS.md) (lane C/E: the fragment rule, paint calibration, first-person
placement) and [NATIVE_WEAPON_VISUALS.md](NATIVE_WEAPON_VISUALS.md) (native material build, composite mesh, hidden bones).
**Every rule below is `UNVERIFIED` unless a line says how it was confirmed.**

## 1. What the game does (the part system, visual side)

A Borderlands 2 gun is a weapon type plus up to nine parts (Body, Grip, Barrel, Sight, Stock, Elemental, Accessory 1/2,
Material). Each weapon type (pistol, SMG, assault rifle, shotgun, sniper rifle, launcher) has one *gestalt* skeletal mesh that
holds every part of every manufacturer; a fragment table (`GestaltSkeletalMeshDefinition.GestaltInfos`) names each fragment's
triangle range. A gun draws only the fragments its parts name: a part's `GestaltModeSkeletalMeshName` plus its non-`None`
`AdditionalGestaltModeSkeletalMeshNames`, and nothing for `*_None` parts (WEAPON_VISUALS.md section 2: confirmed on six guns;
here re-checked on the sample of section 5). The paint is one material instance: parent = the Material part's skin MIC, plus the
parts' `MaterialVectorParameterValues` (`p_EmissiveColor`); the weapon type hides `BoneToHideOnMesh` bones.

## 2. Census

Tools: `tools/weapon_part_census.py` (cooked packages through `ow-package`, base game and every DLC `Compat/Content` package)
and `tools/real_game/scripts/weapon_visual_census.py` (the running game, `find_all`, the RTSE way), compared by
`weapon_part_census.py --live`. Output: `local/wpa/census/visual_census.json`, `local/realgame/wpa/live_visual_census.json`.

| | count |
|---|---|
| Weapon gestalt meshes | 6 (all in `Startup.upk`); 279 distinct fragments (Pistol 59, SMG 46, AR 47, Shotgun 45, Sniper 46, Launcher 36) |
| Other gestalts (not guns) | 4: artifacts, shields, grenades, Krieg's BuzzAxe (DLC) |
| Weapon part definitions | 904 unique paths (base and DLC): Body 167, Barrel 205, Grip 45, Sight 47, Stock 34, Elemental 50, Accessory 44, Material 310, unknown 2 |
| Parts per gestalt | Pistol 113, SniperRifle 118, SMG 79, AR 90, Shotgun 87, Launcher 73; 336 draw nothing (material parts and `*_None`); 7 BuzzAxe parts belong to the non-gun gestalt; 1 ambiguous (`Scorpio_Elemental_Slag`, a vehicle weapon) |
| Skins (Material part MICs) | 301: Pistol 72, Shotgun 52, AR 49, SMG 48, Sniper 46, Launcher 33, BuzzAxe 1 |
| Weapon types | 68 (35 name a weapon gestalt; 7 hide bones) |
| Fragments never referenced by any part | 0 |

Live against cooked (2026-10-22 session, title screen, saves blocked and byte-identical afterwards except the `.bak` the game
rewrites at start, which was restored): the six weapon fragment tables are **identical** (name, material index, first index,
triangles for every range). Of 904 cooked parts, 814 were loaded live; for all 814 the fragment names and the Material match. 22
Torgue Material parts carry a `p_EmissiveColor` in the running game that the cooked parts do not have (the documented
live-versus-cooked data difference, REALGAME_GROUND_TRUTH.md); the library uses the live values for those (49 parts carry live
vectors in all). Skins: 270 of 301 loaded live, all with the same MIC; weapon types: 35 of 68 loaded live, hidden bones equal.
The cooked-only parts and skins are most likely ones the session never loaded (DLC packages); not checked.

## 3. Extraction (UModel / UE Viewer build 1590)

Benchmark and batch (`tools/export_weapon_skins.py`, `C:\Users\yorad\Tools\UEViewer\umodel.exe`, compiled 2022-12-23, official
gildor2/UEViewer build): `umodel -path=<BL2 install root> -game=border -export -png -out=<root> <Package> <MIC> MaterialInstanceConstant`
per skin. About 2 s per skin; 301 attempted, **301 exported and resolvable**, 0 failed (8 min wall clock, 4 workers). Every
chain ends at a Material (293 at `Master_Gun`, 8 at the Anemone DLC's `Master_Gun_Anemone`); 211 have an intermediate `MasterMati_*`
MIC, exported by UModel with the leaf. Leaf names are not unique (4 leaves name 8 skins across packages), so each skin exports to
its own root; UModel writes a DLC MIC's textures into the base package folder, so the tool copies them beside the MIC. A DLC
package needs `-path` at the install root (pointing at the DLC Content folder loads nothing). 2,904 PNGs with 111 distinct
contents (82 MB); the per-skin layout takes 13 GB because of the copies (not deduplicated). Inventory:
`local/external/umodel/weapon-skins/inventory.json`. The six gestalt meshes are the existing UModel glTF exports of
2026-10-01 (`local/external/umodel/gestalt/Startup/SkeletalMesh3`), checked against the fragment tables section by section.

## 4. The library in UE (`tools/weapon_library.py`, `host/ue5/import_weapon_library.py`)

- **Meshes:** each gestalt glTF is regrouped so every fragment is its own primitive with its own material named after the
  fragment; vertices, skin and nodes are unchanged, and the triangle total and each fragment's count equal the table (checked
  by the tool). Imported once each as `/Game/OpenWillow/Weapons/Library/<Type>/SK_Gestalt_<Type>`: **6 of 6 imported**, slot
  names equal the 279 fragment names, none missing.
- **Skins:** `prepare_weapon_paint.py` (extended: `prepare_recipe` prepares a skin without a recipe file; a DLC skin reads its
  static parameters and textures from its own package and the base defaults from `Startup.upk`) prepares every skin; the importer
  builds one master material (`M_OW_GunMaster`, the same paint reading as the per-recipe material with every value a parameter,
  `import_weapon_paint.build_master`) and one `MaterialInstanceConstant` per skin. **300 of 301 prepared and imported**; the
  BuzzAxe material (a melee weapon, its static channel pick selects three channels) is unsupported. Textures are imported once
  each, linear, and decoded in the shader by their installed sRGB flag.
- **Reload clips:** the weapon reload clips lane E converted (pistol, SMG, assault rifle) are also imported onto the library
  skeletons (`import_weapon_reload_anims.py`, library target); shotgun, sniper and launcher clips do not exist yet (as before).

Coverage per type (imported / failed / unsupported):

| type | mesh | fragments | skins imported | unsupported |
|---|---|---|---|---|
| Pistol | imported | 59/59 | 72/72 | 0 |
| SMG | imported | 46/46 | 48/48 | 0 |
| Assault rifle | imported | 47/47 | 49/49 | 0 |
| Shotgun | imported | 45/45 | 52/52 | 0 |
| Sniper rifle | imported | 46/46 | 46/46 | 0 |
| Launcher | imported | 36/36 | 33/33 | 0 |
| (BuzzAxe, not a gun) | not imported | - | 0/1 | 1 |

## 5. Run-time assembly (`OpenWillowWeaponAssembly.{h,cpp}`)

`OpenWillowWeaponAssembly::Show(component, item)` puts the item's library gestalt on the component, shows only the sections
whose slot is one of the fragments its parts draw (from the library's part table; the recipe's `gestalt_fragments` when a part is
not in the table), gives every slot one dynamic instance of the Material part's skin with the parts' vector parameters in native
slot order, and hides the weapon type's bones (`HideBoneByName`, instead of cutting their triangles from a per-recipe mesh). It
is used for the held gun, the inventory preview (side and Inspect), Maya's menu weapon (and its ink outline, which copies the
visibility) and pickups. The pre-built per-recipe mesh stays as the fallback: a gun with an explicit `MeshPath` (the quest's lent
pistol), a gun whose type has no library mesh, and `-owweaponprebuilt` (A/B).

Item fields added (the part list, read from the recipe the generator writes): `Parts` (slot -> part path), `WeaponType`,
`Gestalt`, `Skin`. Nothing else in the recipe format changed.

## 6. Correctness

### Sample

`tools/weapon_assembly_sample.py` rolls every base-game balance once (seed 1, game stage 30; 242 balances, 0 roll failures) with
the existing `weapon_recipe.roll` and `weapon_stats` (generation code unchanged) and keeps: **32 guns, one per weapon type x
manufacturer** (32 of the 33 cells; the Torgue pistol cell is missing because `weapon_stats` raises on a `None` entry in a
part's presentation list, a generator-side issue reported to the generation lane), **78 base-game legendaries/uniques**, and **100
skin-sweep guns** (a matrix gun of the same type with only its Material part replaced): 210 guns, 209 of the 301 skins shown.
The 92 skins not shown are used only by DLC material parts, whose balances live in DLC packages the sample does not roll.

### Triangle oracle (game triangles = host triangles)

Host side: the assembly logs the LOD0 triangles of the sections it shows (`OpenWillowWeaponAssembly::ShownTriangles`); six
host runs (`tools/run_weapon_assembly_shots.ps1 -Quick`, the backpack holds 39 guns per run) showed all 210 guns. Game side, two
sources (`tools/weapon_assembly_oracle.py`):

| reference | guns | result |
|---|---|---|
| prediction: the cooked fragment table summed over the gun's fragments (the table equals the live one, section 2) | 210 | **210 pass** |
| live: the spawned gun's own `GestaltData` (fragment names and triangle ranges of its first-person mesh), read from the running game | 39 (32 matrix + 7 legendaries, one per type) | **39 pass**; the fragment name sets are equal too (39/39) |

So the fragment rule of section 1 holds on 39 live guns covering every weapon type and manufacturer, not only the six of
WEAPON_VISUALS.md (**confirmed in game** for those 39; the rule is still a reading, not native code). Hidden bones are not part of
this count (the game hides them by bone scale too).

The same session read each live gun's first-person material instance: for **39 of 39** its parent is the Material part's skin
MIC and its vector parameters are exactly the parts' `MaterialVectorParameterValues` in slot order (Torgue Material parts:
`p_EmissiveColor` from the live part data). This **confirms in game** the native BuildWeaponMaterial reading of
NATIVE_WEAPON_VISUALS.md section 1 for these guns, which the assembly follows.

### Matched frames and critic

Real frames: `tools/real_game/weapon_part_session.ps1 -Mode Capture -Inspect` (Maya level 8, Crimson Raiders HQ, 1280x720;
saves backed up, blocked in game and compared afterwards, the only change the `.bak` the game rewrites at start, restored; the
save's own backpack emptied in memory so each spawned gun is the only backpack weapon). Each gun is spawned from its recipe's
parts, held (identity checked through the SDK before the frame) and inspected. Host frames: the same recipes, first person in
Sanctuary and the inventory preview's side view. Pairs: `local/wpa/review_r2/pairs/` (39 Inspect + 39 first person).

Independent critic (Codex GPT-6-Luna, max effort, pairs only), 0-10 per gun:

| round | Inspect / side | first person | notes |
|---|---|---|---|
| 1 (2026-10-23) | 8.25 (6 slice guns, lane C's real Inspect frames) | 4.26 (39 guns) | first person scored with the scene's blue light; launchers cropped |
| 2 | **7.32** (39 guns: pistol 7.56, shotgun 7.40, SMG 7.35, sniper 7.33, AR 7.23, launcher 6.93) | **5.36** (pistol 6.08, SMG 5.65, shotgun 5.58, AR 5.55, sniper 4.77, launcher 4.15) | overall 6.34; no wrong-gun pair found |

**The 9/10 target was not reached.** Between the rounds: real Inspect frames for all 39 guns, larger side-view framing for
launchers and sniper rifles, and the critic told that the first-person blue cast is the HQ's light (the host gun shading is
scene-independent by design, WEAPON_VISUALS.md section 4). The library material was also checked against the per-recipe one it
replaces: the six slice guns render alike through both (`-owweaponprebuilt` A/B, `local/wpa/ab_material.jpg`), so the remaining
paint differences come from the shared paint reading and its calibration, not from the library rewrite.

Remaining, as the critic ranked them, with what was found:
1. First-person size/position per type (AR, SMG, shotgun read small or low; sniper and launcher use the pistol arms set because
   their arms clips were never imported). This is the first-person view model (WEAPON_VISUALS.md sections 10-15), unchanged here.
2. Some guns read flatter / more side-on in first person (same cause).
3. Maliwan violet accents too luminous (host `#A2AAFA` on a sniper): the elemental/Maliwan emissive term; how the game scales it
   after lighting is `UNVERIFIED`.
4. Torgue checker coverage looks broader on some parts (pattern scale/placement reading, `UNVERIFIED`).
5. Yellow/orange finishes shift warmer (Hornet real `#EDE372`, host `#D7983B`): the skin's zone colours are HDR (A midtone about
   3.9, 3.1, 0.7) and the material clips per channel, so the hue depends on the light level; the stand-in light level was calibrated
   on six guns (WEAPON_VISUALS.md section 8). Not re-tuned here: a global change moves every gun.

## 6b. PR #58 review round 1 (2026-10-23)

- **Part list shape.** The item now carries PR #57's shape: an ordered `TArray<FOpenWillowWeaponPart> {Slot, Part}` with
  `FindPart`, `Material` (skin MIC), `GestaltMesh`, plus `WeaponType` (object path). The assembler reads only these, so guns the
  generator makes assemble the same way as recipe guns once #57 lands (rebase pending).
- **No partial guns.** `OpenWillowWeaponAssembly::Plan` (pure; automation test `OpenWillow.WeaponAssembly`, run in `-game` mode)
  refuses a gun with no part list, a part, skin or section material the library does not know, a fragment the library mesh
  lacks, or parts that draw nothing; `Show` then uses the pre-built mesh and logs `fallback to the pre-built mesh <path>
  (<reason>)`. The host runs below logged no fallback.
- **Every material slot, read live** (4 guns, 2026-10-23): slot 0 is the gun's own instance; the pistol gestalt's slot 1 holds
  the mesh's own `Mati_DahlUncommon` on every pistol, including the Infinity, which is the gun that draws a section-1 fragment
  (`Pistol_Body_Vladof_Var2`). **Confirmed in game** for those guns; the host now gives fragments from section 1 that skin
  (without the parts' vectors) and every other fragment the gun's instance. The other five gestalts have one section.
- **Preview framing** uses the shown fragments' bind-pose bounds (computed offline from the glTF; the host checks that their
  union equals the imported mesh's bounds, which caught a mirrored axis in the first mapping; now 0 warnings on six types).
- **Reload clips survive a meshes import:** `import_weapon_reload_anims.py` records what it put on the library skeletons
  (`local/weapon_library/reloads.json`) and `tools/import_weapon_library.ps1` re-imports them after a meshes import (checked).
- **Oracle** compares exact fragment-name sets as well as triangles, reports `fallback`, `not_shown` and `no_reference`
  (incomplete) per gun, and exits 1 on any non-pass. Live references: **39 of 39 pass** with exact name sets.
- **Load cost:** the menu preload (`OpenWillowMayaHUD`) now loads each item's library mesh and skin
  (`OpenWillowWeaponAssembly::Preload`): 39 items, 46 assets in 424.5 ms at level start. After it, `Show` takes 50 ms the first
  time in a run (first gun, render state) and 0.2-0.5 ms for other first shows; repeated shows 0.2-0.9 ms. Material instances
  are cached by skin, parameters and capture mode (shared; `Configure` resets the glow scalar on each show).

## 6c. PR #58 review round 2 (2026-10-23)

- **Per-component instances:** material instances are cached per component (and skin, parameters, capture mode), owned by
  the component, so the held gun's glow (`SetGlow` writes `OW_Emissive`) cannot light a pickup or preview of the same gun.
- **Complete guns only:** `Plan` also requires Body, Grip, Barrel and Material parts (every one of the 210 sample guns, all six
  types, has them; Sight and Stock are present too but may be `*_None` parts) and a weapon type the census knows; a missing
  or unknown one falls back with its reason.
- **Section materials strict:** a type without a fragment-section table, a fragment without a section, or a non-zero section
  without a mapped material falls back (logged); a section material that fails to load falls back too, before the component
  changes. `tools/weapon_library.py index` warns when it writes no live section materials.
- **Oracle:** an empty index, or one that does not hold `--expect N` guns, exits 1.
- **Reload record** paths are repository-relative (resolved at import).
- **`-owgunnolook`** clears the previous gun's override materials before leaving the imported ones alone.
- Checks: `OpenWillow.WeaponAssembly` (12 planning checks and 11 refusal cases) and `OpenWillow.Skills` pass; the meshes
  import restored the three reload clips from the relative record; gun shots (full sequence on the 6 slice guns, quick on the
  39 live-set guns): oracle 39/39 with `--expect 39`, no fallback, no bounds warning; CTest 10/10, packages 9/9.

## 6d. Generated guns (after PR #57, 2026-10-23)

Eight guns rolled in the host with `ow.RollGun` from pools (base `All` pools: common, legendary, rare seed 2; one rare pool per
weapon type) were shown by the gun shots (`-owgunexec`): all eight assembled from their parts with no fallback (3-4 ms cold), and
for all eight the shown fragment set and triangles equal the generator's own parts (`ow-weapongen`) through the library table and
the cooked gestalt tables. After the rebase: `OpenWillow.WeaponAssembly` passes, oracle 39/39 (`--expect 39`), inventory suite
49/49 (bounded recipe set), CTest 12/12, packages 9/9.

## 7. Checks

- Python (synthetic): `tests/weapon_part_census_test.py` 34, `tests/weapon_library_test.py` 28,
  `tests/weapon_assembly_oracle_test.py` 18, `tests/weapon_assembly_sample_test.py` 17, `tests/weapon_paint_test.py` 38,
  `tests/weapon_recipe_test.py` 14, `tests/weapon_view_model_test.py` 13, `tests/weapon_reload_anims_test.py` 2 (needs numpy); all OK.
- CTest 10/10; `tools/verify_packages.py` 9/9 packages match.
- In engine: `tools/test_inventory_actions.ps1 -Items <bounded recipe set>` **49/49** with every gun assembled (one per weapon
  type). With the full local demo library it gives 24 / 3 / 22 (pass / fail / not run): that library fills the 39-slot backpack,
  the same result HUD_PARITY.md records, not this change.
- UE import: 6 of 6 meshes, 300 of 301 skins, reload clips for three types; the master material compiles (SM5) after three
  fixes found by the first host runs (an sRGB default texture on a linear sampler, float4 swizzles on float3 inputs).

## 8. UNVERIFIED and open

- The fragment rule on guns beyond the 39 live-checked ones; the `*_None` test as the game's own.
- Live-checked guns are base-game only; DLC skins are imported and resolvable but not compared with the running game.
- The paint reading itself (WEAPON_VISUALS.md); the master material is an exact rewrite of the per-recipe material, not a new reading.
- Elemental first-person particles (`FirstPersonParticle` on elemental parts) are not drawn.
- Skin manufacturer and rarity labels in the census are read from names.
