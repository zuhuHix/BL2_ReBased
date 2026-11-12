## 2026-10-04: Guns: parts and card stats from the running game, a tone-mapper-aware gun material, first-person foreground FOV

AI-assisted (Claude). Host, tools and tests; no change to `src/`, `CMakeLists.txt` or package parsing. Numbers and
paths: `docs/verification/WEAPON_VISUALS.md`.

- **Real-game session** (SDK driver, saves backed up, blocked and byte-identical afterwards, guns in memory only):
  six exact-part guns captured in first person and in the Inspect view; live part mesh lists and material instances read.
- **Part assembly (confirmed in game):** the host used only the main gestalt mesh. Adding the body-variant meshes and
  drawing nothing for `*_None` parts makes all six fragment lists equal the live part mesh names, with exactly equal
  triangle totals. The Jakobs pistol's hidden-bone triangles (moon clip, bullet) are cut. Not confirmed: that the
  `*_None` name is the game's own test.
- **Colour, round 1:** with the right meshes the existing paint model was largely right (Maliwan colours within 0.82x
  median and 2.7 degrees of hue of the real Inspect view). The material became an Unlit evaluation of the base pass's
  structure with stand-in lights (UNVERIFIED) and the part's elemental emissive vectors.
- **Colour, round 2 (cause):** the oversaturation, crushed blacks, red-for-orange accents and hot Infinity shroud came
  from UE5's film tone mapper acting on the Unlit output (scene-linear 0.18 displays 0.03, 1.0 displays 0.51; measured
  with a grey ramp in the material). The material now applies the inverse of the measured curve (a 28-point table used
  per channel; an approximation) and clips per channel. Median linear ratio host/real against the Inspect view:
  Maliwan 1.06, Jakobs 0.69, Infinity 1.01, SMG 1.16, rifle 1.05, shotgun 0.86 (round 1: 0.77, 0.72, 1.06, 0.93,
  1.08, 0.64).
- **First person:** the game draws arms and gun with a foreground FOV of 45 (read through the SDK; world FOV 77.55
  with the config's 90). UE 5.8's first-person FOV now does the same by default; making it the default is a host
  choice, `-owfpfov=0` opts out. The Phaselock hand effects follow it (rounds 12-13). The SMG, rifle and shotgun were
  held in the pistol clips; their own clip sets are imported and chosen by weapon type. The weapon socket and its
  90-degree yaw, and the arms mesh and material, are the game's own data.
- **Shotgun cards (lane D's report):** 4.4/9 and 3.7/13 came from stats stored by an older evaluator and from the
  cooked weapon type differing from the running game's. Re-evaluated on the live overlay: 4.1/10 and 3.5/14, matching
  the game's Inspect card for the exact parts. Where the live values come from is still unexplained.
- **Mission pistol:** the quest lends the recipe's own corrected mesh where it is imported, with the earlier rolled
  sample as the fallback (one block in `OpenWillowQuest.cpp`).
- **Visual review:** independent critics scored round 1 7.2/10 and round 2 6.8/10; round 2 was judged closer than round
  1 on all six Inspect pairs and on five of six first-person pairs (one tie). Main gaps: guns about 20% large with too
  little cant in first person, the Infinity shroud still too pink, the fire pistol's barrel too light, no ink outlines.
- **Checks:** CTest 10/10; `tests/weapon_paint_test.py` 34 OK, `tests/weapon_recipe_test.py` 14 OK,
  `tests/weapon_stats_test.py` 28 OK; quest suite with the foreground FOV default 79/0 and 11/0. Inventory suite in the
  final pass.

