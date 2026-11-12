## 2026-10-23: Weapon part library, PR #58 review round 1

AI-assisted (Claude Opus 5.5; review GPT-6-Luna). Record: `docs/verification/WEAPON_PART_ASSETS.md` section 6b.

- **Part list:** the item uses PR #57's shape (ordered `{Slot, Part}` array with `FindPart`, `Material`, `GestaltMesh`) plus
  `WeaponType`; the assembler reads only these, so generated guns assemble once #57 lands (rebase pending, by the hub's order).
- **No partial guns:** a gun with no part list, an unknown part, skin or section material, or a fragment missing from the library
  mesh uses the pre-built mesh, and the reason is logged. Pure planning step with a synthetic automation test
  (`OpenWillow.WeaponAssembly`, `-game` mode).
- **Per-slot material, confirmed in game:** the pistol gestalt's second section shows the mesh's own `Mati_DahlUncommon` on every
  pistol read live (4 guns, Infinity included); the host now does the same for section-1 fragments.
- **Preview framing** from the shown fragments' bounds (offline from the glTF; host check against the imported bounds caught a
  mirrored axis and is now quiet). Reload clips are re-imported after a meshes import. Material instances are cached; the menu
  preload loads library meshes and skins (39 items, 46 assets, 424.5 ms); first show 50 ms, later 0.2-0.9 ms.
- **Oracle:** exact fragment-name sets plus triangles, explicit `fallback` / `not_shown` / `no_reference`, nonzero exit on any
  non-pass. 210/210 sample guns pass; 39/39 against the running game.
- **Checks:** CTest 10/10, packages 9/9, Python 164 synthetic tests OK, `OpenWillow.WeaponAssembly` and `OpenWillow.Skills` pass,
  in-engine gun shots (full sequence on the slice guns, quick on 210), inventory suite 49/49 (bounded recipe set).

