## 2026-10-23: Weapon part library: every gestalt mesh and skin in UE once, guns assembled from their parts at run time

AI-assisted (Claude Opus 5.5 orchestrating; Haiku sub-agents for the census, skin export, sample and tests; critic GPT-6-Luna).
Record: `docs/verification/WEAPON_PART_ASSETS.md`. No change to `src/`, `CMakeLists.txt` or package parsing.

- **Census.** `tools/weapon_part_census.py` reads every weapon part, weapon type and gestalt in the base and DLC packages
  (904 parts, 301 skins, 6 weapon gestalts with 279 fragments; every fragment is referenced). A live SDK census of the running
  game (`tools/real_game/scripts/weapon_visual_census.py`) agrees: the six fragment tables are identical, and the 814 parts
  loaded live have the same fragments and skin. 22 Torgue Material parts carry a `p_EmissiveColor` only in the running game (the
  documented live-versus-cooked data difference); the library takes the live value.
- **Extraction.** UModel 1590 exports all 301 skin MICs with their parent chains and textures (`tools/export_weapon_skins.py`,
  301 of 301 resolvable, inventoried; one root per skin because leaf names repeat across packages).
- **Library.** Each gestalt glTF is regrouped into one primitive (UE material slot) per fragment and imported once
  (`tools/weapon_library.py`, `host/ue5/import_weapon_library.py`); the paint reading of `import_weapon_paint.py` became one
  parameterised master material plus one instance per skin (300 of 301 imported; the BuzzAxe melee material is unsupported).
  The per-recipe meshes and materials stay as the fallback.
- **Assembly.** `OpenWillowWeaponAssembly` shows exactly the fragments a gun's parts draw on its type's library mesh, gives every
  slot one instance of the Material part's skin with the parts' vector parameters, and hides the type's bones. Used by the held
  gun, inventory preview, Maya's menu weapon and pickups. Item fields added: `Parts`, `WeaponType`, `Gestalt`, `Skin` (read from
  the recipe the generator already writes). `-owweaponprebuilt` forces the old path; an explicit `MeshPath` keeps it.
- **Confirmed in game (39 spawned guns, one per type x manufacturer plus 7 legendaries):** fragment names and triangle totals
  equal the host's for 39 of 39 (and the cooked prediction for all 210 sample guns), and each gun's material instance has the
  Material part's skin as parent and the parts' vectors (39 of 39), confirming the native BuildWeaponMaterial reading for them.
- **Visual (critic, 39 matched guns):** Inspect 7.32/10, first person 5.36/10 (round 1: 8.25 on six guns / 4.26). 9/10 not
  reached; the gaps are the shared first-person view model (sniper and launcher arm sets not imported) and the paint calibration
  (HDR zone colours clipped per channel under a stand-in light level), not the library. UNVERIFIED: the paint reading itself, the
  material on the pistol gestalt's second section, elemental first-person particles (not drawn).
- **Checks:** CTest 10/10, packages 9/9, Python 157 synthetic tests OK, inventory suite 49/49 (bounded recipe set) with assembled
  guns.

