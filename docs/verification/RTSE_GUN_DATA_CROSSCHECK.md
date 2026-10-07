# Gun assembly data: package decode against a live-game dump (lane R1, 2026-10-07)

AI-assisted (Claude). The maintainer's separate project (a mod for the community SDK, running inside the live game) dumped the
gestalt definitions of the running game. This record compares that dump with what our own tools decode from the installed
packages. **Nothing from that dump is stored in this repository**: this file holds counts, agreement statistics and conclusions
in my own words, plus names of game objects and classes. The dump, the comparison scripts and their per-row output stay under
ignored `local/rtse/`. Every statement is `UNVERIFIED` unless it names the live dump or a live capture as the check.

## 1. What was compared

The dump covers 16 `GestaltSkeletalMeshDefinition` objects: all 9 item families (pistol, SMG, assault rifle, shotgun, sniper,
launcher, grenade, shield, relic) and the 7 `Remaster_` weapon variants. Per object it records the part table
(`GestaltInfos[0].Parts`: fragment name, material index, first index, triangle count), the part bounds, the socket mappings and
the sockets of the shared mesh. Our side decoded the same objects with a copy of `ow-package` (`--properties` with a local array
schema, `--properties-batch` for the sockets) from `Startup.upk` and, for the Remaster variants, `Mancana_StartupRemaster.upk`.
All 16 property streams were consumed exactly (no trailing bytes).

The existing local schema (`GestaltInfos`, `Parts`) decodes only the part table. Three more lines make the reader decode the
other three properties too (an array of bounds structs holding a nested bounds struct and vectors, and the socket mappings); no
reader change was needed. Floats were compared as single precision.

## 2. Result (counts only)

| object | package | Parts entries, live / ours | Parts exact | bounds exact | socket mappings exact | mesh sockets exact | decode failures |
|---|---|---|---|---|---|---|---|
| Artifact | Startup | 16 / 16 | 16 | 16 of 16 | none exist | none exist | 0 |
| Shields | Startup | 32 / 32 | 32 | 32 of 32 | none exist | none exist | 0 |
| AssaultRifle | Startup | 47 / 47 | 47 | 47 of 47 | 48 of 48 | 48 of 48 | 0 |
| Grenades | Startup | 15 / 15 | 15 | 15 of 15 | none exist | none exist | 0 |
| Launcher | Startup | 40 / 40 | 40 | 36 of 36 | 32 of 32 | 32 of 32 | 0 |
| Pistol | Startup | 59 / 59 | 59 | 59 of 59 | 60 of 60 | 57 of 60 | 0 |
| Shotgun | Startup | 45 / 45 | 45 | 45 of 45 | 40 of 40 | 40 of 40 | 0 |
| SMG | Startup | 46 / 46 | 46 | 46 of 46 | 52 of 52 | 52 of 52 | 0 |
| SniperRifle | Startup | 48 / 48 | 48 | 46 of 46 | 40 of 40 | 40 of 40 | 0 |
| Remaster AssaultRifle | Mancana_StartupRemaster | 47 / 47 | 47 | 47 of 47 | 48 of 48 | 48 of 48 | 0 |
| Remaster Grenades | Mancana_StartupRemaster | 15 / 15 | 15 | 15 of 15 | none exist | none exist | 0 |
| Remaster Launcher | Mancana_StartupRemaster | 40 / 40 | 40 | 36 of 36 | 32 of 32 | 32 of 32 | 0 |
| Remaster Pistol | Mancana_StartupRemaster | 59 / 59 | 59 | 59 of 59 | 60 of 60 | 60 of 60 | 0 |
| Remaster Shotgun | Mancana_StartupRemaster | 45 / 45 | 45 | 45 of 45 | 40 of 40 | 40 of 40 | 0 |
| Remaster SMG | Mancana_StartupRemaster | 46 / 46 | 46 | 46 of 46 | 52 of 52 | 52 of 52 | 0 |
| Remaster SniperRifle | Mancana_StartupRemaster | 46 / 46 | 46 | 46 of 46 | 40 of 40 | 40 of 40 | 0 |
| **Total** | | **646 / 646** | **646** | **636 of 636** | **544 of 544** | **541 of 544** | **0** |

"Parts exact" means the same entry at the same index with the same four fields; the entry order also agrees. The mesh sockets
are matched by socket name (the export order differs from the dump order in 6 of the 12 objects that have sockets); for each
the bone name, relative location, rotation and scale were compared, with the engine defaults (zero rotation, unit scale) read
for fields the cooked stream omits.

## 3. What this confirms (live dump as the check)

- The struct-array decode of `GestaltInfos` / `Parts` is right for every family: element count, order, name, material index,
  first index and primitive count, 646 of 646 entries, with the nested array-in-struct-in-array layout and exact byte
  consumption. Until now the part table had been checked only structurally (ranges tile the index buffer) and by six guns'
  triangle totals; this is a field-by-field check against the running game for all 9 families and the Remaster variants.
- The decode of the other three properties (bounds with nested structs and float vectors, socket mappings, sockets) is right
  too: 636 of 636 bounds equal in single precision, 544 of 544 mappings, 541 of 544 sockets (the 3 are section 5).
- The reader's reading of absent properties as defaults (a socket without a stored rotation or scale) agrees with the live
  object for 541 of 544 sockets.
- The part tables in the repository's local `local/gestalt/*.json` (the six weapon families the pipeline uses) equal the live
  tables: 6 of 6.

- A 17th gestalt definition exists in the installed packages that the dump does not contain (a DLC melee-axe type, 5 entries, decodes
  with exact byte consumption); it has no live check.

It does not confirm how the engine composes a gun from these tables, which fragments a given weapon draws (see section 6), or
anything about the meshes themselves.

## 4. The Remaster variants

- They exist in the installed packages: all 7 weapon variants are in `Mancana_StartupRemaster.upk`. There is no Remaster
  variant of the shield or the relic. They decode and agree with the live dump exactly (section 2).
- They are a different layout, not a copy: for all 7 families the fragment names are the same set as in the original table but
  the ranges, order and triangle totals differ, and the totals are larger (the remastered meshes carry more triangles). A
  Remaster table cannot be applied to the original UModel glTF export; `tools/filter_gestalt_gltf.py`'s section-total check
  would reject the mismatch.
- Which table the live game used was settled with our own earlier capture, not the dump: `local/realgame/orchC/gun_fragments.json`
  holds the live per-fragment triangle ranges of 7 spawned guns (6 distinct builds; 31 fragment ranges). Looked up in the two
  tables, **31 of 31 ranges match the original table (name and range) and 0 of 31 match the Remaster table**. So the original
  tables are what the game used in that session (this install, Remaster tables present). When the game uses the Remaster
  tables (a setting, a DLC state) is `UNVERIFIED`.
- The pipeline (UModel export of `Startup`, original tables) is consistent with that. No repository code refers to a Remaster
  gestalt.

## 5. Cooked against live

- Part tables and bounds: **no difference** (0 of 646 and 0 of 636). Unlike the 39 weapon-data objects of
  [NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md) section 7, the gestalt tables show no live-versus-cooked drift.
- Sockets: 3 of 544 differ, all on the original pistol mesh, all the scope eye socket of three scope fragments (Dahl, Torgue and
  Maliwan), location only, by at most about a unit and a quarter (mostly in the vertical offset). The Remaster pistol's cooked
  sockets equal the original pistol's cooked sockets exactly (60 of 60), so the live difference is not the Remaster data. Cause
  `UNVERIFIED`: a run-time adjustment of the shared mesh by the game, or the same unexplained load-time source as the weapon
  objects. It touches only the sight position for aiming down the sights; nothing in the repository uses these sockets yet.

## 6. Observations that came out of the comparison (own data, no live check unless stated)

- **Duplicate fragment names.** Three tables hold more entries than distinct names: the original and the Remaster launcher (one
  accessory fragment appears in 5 separate ranges) and the original sniper (one body fragment in 3 ranges). Bounds and socket
  data are per name, parts per range: that is why the launcher has 40 parts and 36 bounds and the sniper 48 and 46.
  `tools/filter_gestalt_gltf.py` keeps every range of a wanted name (checked: the launcher fragment gives 36 triangles over 5
  ranges, the sniper body 1802 over 3, the sums of the table ranges). `tools/slice_npc_assets.py::candidate_sections_gltf` builds
  `{name: part}` and would keep only the last range of a duplicated name. It is only used for the pistol (no duplicate there),
  so this is a latent hazard, not a current bug; not changed.
- **Tiling.** For all 9 original tables the ranges tile the index buffer with no gap and no overlap, starting at zero (the check
  RTSE's `dev/verify_gestalts.py` makes). For the 6 families that have a local UModel glTF the per-material totals equal the
  glTF's section sizes (6 of 6); shields, grenades and relics have no local glTF.
- **Material index.** Exactly one fragment in all 16 tables has a material index other than 0: the Vladof Var2 pistol body
  (original and Remaster). Every other fragment of every family is material 0. So the "material-index split" matters for one
  fragment of one family (the Infinity body variant); the other eight families are one section.
- **Part names against the table.** The 658 `WeaponPartDefinition` objects of `Startup.upk` decode with 0 failures; 447 name a
  fragment (560 names with the body variants), 211 name none. Every name of a part not called `*_None` exists in its own
  family's table. The only 2 names that do not (one pistol sight, one shotgun sight) belong to `*_None` placeholder parts. That
  is evidence for the `*_None` rule in `tools/weapon_recipe.part_fragments`, not proof that it is the game's own test.
- **Same rule independently.** The mod's own part-to-mesh function applies the same three steps as ours (the main name, the
  non-`None` additional names, nothing for `*_None`). That is a second implementation of the same inference, not independent
  evidence; its live mesh dump of one Jakobs revolver agrees with ours on the part names (6 of 6 part fields) and on the weapon
  type's gestalt and hidden-bone fields (3 of 3).
- **Origin.** The mod's per-part "origin" is the centre of that fragment's reference-pose bounds box. We already decode it
  exactly (636 of 636); it is not a pivot.

## 7. Coverage today, per family

| family | table decoded | local UModel glTF | rolled recipes (local) | hidden bones |
|---|---|---|---|---|
| Pistol | yes | yes | 20 | 2 of 8 types hide two bones |
| SMG | yes | yes | 2 | none |
| Assault rifle | yes | yes | 2 | none |
| Shotgun | yes | yes | 3 | all 5 types hide one bone |
| Sniper | yes | yes | 0 | none |
| Launcher | yes | yes | 0 | none |
| Grenade | yes | no | 0 | none found (no weapon type with a gestalt) |
| Shield | yes | no | 0 | not applicable |
| Relic | yes | no | 0 | not applicable |

- Rolling (`tools/weapon_recipe.py`) and the part-slot list are weapon-only; no code rolls shields, relics or grenade mods.
- Hidden bones: `tools/weapon_refresh_fragments.py` reads the two bone fields from the weapon type and removes triangles whose
  vertices are mostly dominated by those bones from the filtered glTF. Of the 33 weapon types in `Startup.upk`, 7 name bones
  (two pistol types two bones, five shotgun types one). In the original UModel export the affected geometry sits in three
  fragments: 404 triangles each in two pistol bodies and 140 in one shotgun body. Whether a triangle is cut when at least one,
  two or all three vertices are dominated by a hidden bone changes nothing there (808 of the whole pistol mesh and 140 of the
  shotgun in each case), so the mod's viewer rule (any vertex) and ours (two or more) give identical output. That the game hides
  a bone by this vertex-dominance test (and not by scaling the bone to nothing, which keeps partly weighted vertices) is
  `UNVERIFIED`; the mod's dump of the live mesh's bone list is an error string, so the live bone list is still missing.
- Sockets: our decode now has the per-fragment sockets (muzzle, eject port, front and rear sight, scope eye sockets, sight
  effect, elemental, laser) for the 6 weapon families, 541 of 544 equal to the live game. The host does not use them: the muzzle
  flash is still an estimated point ([WEAPON_VISUALS.md](WEAPON_VISUALS.md) section 7) although the barrel fragment's muzzle
  socket and the eject port are decoded data (`NATIVE_WEAPON_VISUALS.md` names the eject port as the shell-casing socket).

## 8. The mod as an oracle for the evaluator and the assembly (proposal, nothing run)

The mod can set one slot of a live weapon (write the part into the definition data, then rebuild the item through its own
initialisation) or its level (game stage and grade together), then read the resulting attributes and the built mesh. Our real-game
lane (`tools/real_game/`) already has the same ingredients: `spawn_definition` builds a weapon from exact parts, `weapon_record`
reads parts, names and the card, `live_overlay.py` supplies the live type values, and `golden_card_compare.py` joins them with
`weapon_stats.py`. The new use would be volume: instead of 69 hand-picked cards, a script enumerates legal part combinations from
`weapon_balance.py` (a few dozen per family, plus level sweeps) and for each builds the gun in memory (saves blocked, as in the
earlier sessions), reads the attributes the evaluator computes (damage, projectiles, fire interval, reload, magazine, accuracy,
element values; as numbers, not as the card's rounded text, which would settle the half-rounding cases left open in
[REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md)) and the mesh fragment ranges and hidden bones. The offline join then checks
`weapon_stats.evaluate` on the exact parts, `part_fragments` plus the decoded table against the live fragment ranges (as
section 4 did for 31 ranges) and the hidden-bone list, over hundreds of combinations including the families the slice has not
touched (sniper, launcher). Rebuilding one item in place is cheaper than spawning (no backpack growth) but has pitfalls the mod
documents in its own code (a weapon without a type crashes the game on rebuild, an unequipped weapon must be put away again, the
item id changes); spawning from a definition is the safer default for us. Real-game runs only when the orchestrator says so.

## 9. Open and `UNVERIFIED`

- When the live game uses the Remaster tables; whether the Remaster meshes should ever be exported.
- The cause of the 3 eye-socket differences.
- The game's own hide-bone test; a live bone list.
- That `*_None` is the game's own rule (only that no part outside that name pattern names a fragment missing from its family's
  table, and that the rule holds for the 31 live ranges).
- Everything about shields, grenades and relics beyond the table decode (no mesh export, no part roll, no live gun built).
- The dump covers one game session on one machine, and the tables are static data; a different install state could differ.

## 10. Gate

Nothing under `src/`, `tools/`, `tests/` or `CMakeLists.txt` was changed for this record; no sensitive file was touched; no
`DECISIONS.md` entry was written (a draft is in `local/rtse/handoff.md`). `ctest` and `tools/verify_packages.py` were therefore
not rerun. The only tracked file added is this one. Method scripts and per-row output: `local/rtse/` (ignored).
