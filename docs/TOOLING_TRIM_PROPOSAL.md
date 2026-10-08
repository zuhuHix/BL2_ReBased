# docs/TOOLING.md trim proposal (not applied)

Proposal only. Nothing in docs/TOOLING.md has been deleted. The maintainer approves each row before any edit.

Method: section sizes are the UTF-8 bytes of each `##` section (its `###` children included), in KiB (bytes / 1024), measured
mechanically on 2026-10-08. "Repeats" was judged from the section's own links, the dated DECISIONS entries it cites, and
how often its key terms appear in README.md, ROADMAP.md, DECISIONS.md and docs/verification/. A row is a candidate, not
a proof of duplication; the kept copy must be read before the cut.

Keep as-is: three headings are cited by name from other files, so renaming or removing them breaks those links.
`Tracing the real game's UI code` (CLAUDE.md, the menu-parity note), `Driving the real game` (docs/verification/REALGAME_GROUND_TRUTH.md),
and `External tools` (ROADMAP.md, anchor `#external-tools`). Those rows are marked "keep heading" below.

| Section (heading in docs/TOOLING.md) | KB | Kept copy of the same content | Suggested replacement one-liner |
|---|---|---|---|
| Material v1 and the first map loader (Phase 1) | 19.2 | docs/verification/MATERIAL_LEVEL_V1_VERIFICATION.md, COOKED_MATERIAL_VERIFICATION.md, GLACIER_PRIMARY_LAYER.md; DECISIONS.md 2026-09-23 entries. Keep the command blocks; cut the narrative. | "Material v1 and the first map loader: commands here; evidence and status in docs/verification/MATERIAL_LEVEL_V1_VERIFICATION.md." |
| Sanctuary Fire-mission slice data (stock values instead of stand-ins) | 11.2 | docs/verification/SLICE_WORLD_PLACEMENT.md, WEAPON_BALANCE_DECODE.md, PHASELOCK_STOCK_DATA.md, SLICE_AUDIO_CHAIN.md, SLICE_NPC_ASSETS.md, BEHAVIOR_DATA_DECODE.md; DECISIONS.md 2026-10-02, 10-04, 10-07. Keep the command blocks. | "Fire-mission slice data: the commands are below; what each stock value is, and how it was checked, is in the linked verification records." |
| BL2's HUD movie over UE5 (browser-overlay prototype) | 11.4 | docs/verification/INVENTORY_MOVIE_PROTOTYPE.md; DECISIONS.md 2026-09-26, 2026-10-04. Keep the commands. | "HUD movie over UE5: commands here; design and evidence in docs/verification/INVENTORY_MOVIE_PROTOTYPE.md." |
| Tools added 2026-10-01 and 2026-10-02 (one line each) | 5.9 | DECISIONS.md entries dated 2026-10-01 and 2026-10-02; docs/verification/INVENTORY_MOVIE_PROTOTYPE.md, NATIVE_MISSION_DISPATCH.md. This is a changelog that the decisions log already covers. | "Tools added on 2026-10-01 and 2026-10-02 are recorded with their evidence in DECISIONS.md under those dates." |
| Independent oracles: umodel and the game's own object dumps | 4.1 | docs/verification/BLCMM_DUMP_CROSSCHECK.md, UMODEL_CROSSCHECK.md (already linked from the section). | "Independent checks (UModel, OpenBLCMM dumps): see docs/verification/BLCMM_DUMP_CROSSCHECK.md and UMODEL_CROSSCHECK.md." |
| BSP texture axes against the editor's Polys exports | 4.4 | docs/verification/BSP_TEXTURE_AXES.md, BSP_TEXTURE_CALIBRATION.md (linked from the section). | "BSP texture axes: docs/verification/BSP_TEXTURE_AXES.md." |
| Reading the game's UnrealScript (bytecode disassembler prototype) | 4.0 | docs/verification/SCRIPT_BYTECODE_DISASM.md, SCRIPT_VM_PROTOTYPE.md, NATIVE_BYTECODE_OPCODES.md (CLAUDE.md already points at SCRIPT_BYTECODE_DISASM.md). Keep the research/script_disasm.py commands. | "Bytecode disassembler: commands here; format and evidence in docs/verification/SCRIPT_BYTECODE_DISASM.md." |
| Driving the real game (ground-truth captures) | 2.4 | docs/verification/REALGAME_GROUND_TRUTH.md (cites this heading), DECISIONS.md 2026-10-02. KEEP HEADING. | Keep the heading; cut only prose that restates REALGAME_GROUND_TRUTH.md. |
| Maya with the Infinity and Phaselock (host prototype) | 2.2 | ROADMAP.md Maya / Phaselock items; DECISIONS.md 2026-09-25 and 2026-10-07. | "Maya with Infinity and Phaselock (host prototype): status in ROADMAP.md; decisions in DECISIONS.md." |
| Verification status of the reader | 1.7 | README.md verification text; DECISIONS.md 2026-09-10. Check README before cutting; not confirmed. | "Reader verification status: README.md and DECISIONS.md 2026-09-10." |
| Sanctuary root BSP polygons | 1.7 | docs/verification/SANCTUARY_BSP_POLYGONS.md (already linked). | "Sanctuary root BSP polygons: docs/verification/SANCTUARY_BSP_POLYGONS.md." |
| Terrain floors | 1.7 | docs/verification/SANCTUARY_TERRAIN_BSP_HANDOFF.md (already linked); SANCTUARY_TERRAIN_BSP_REFRESH.md. | "Terrain floors: docs/verification/SANCTUARY_TERRAIN_BSP_HANDOFF.md." |
| Tracing the real game's UI code (golden files for menus) | 1.0 | docs/verification/REALGAME_GROUND_TRUTH.md; DECISIONS.md 2026-09-26. CLAUDE.md cites this heading. KEEP HEADING. | Keep the heading; cut prose only. |
| Sanctuary ambient NPCs (host, behind a flag) | 1.0 | docs/verification/SANCTUARY_AMBIENT_NPCS.md (no link in the section today, so add one); DECISIONS.md. | "Sanctuary ambient NPCs (host, behind a flag): docs/verification/SANCTUARY_AMBIENT_NPCS.md." |
| Sanctuary artifact trace | 0.5 | docs/verification/SANCTUARY_ARTIFACT_PASS.md (already linked). | Already a pointer; the section can merge into its verification link. |
| Bounded Sanctuary mover bridge | 0.6 | docs/verification/SANCTUARY_MOVER_PROTOTYPE.md (already linked). | "Bounded Sanctuary mover bridge: docs/verification/SANCTUARY_MOVER_PROTOTYPE.md." |

The candidate rows that are not "keep heading" sum to about 70 KiB of the 86.5 KiB file. The saving is real only for the
prose cut; command blocks stay, because the lanes run them.

Not proposed (no evidence gathered): External tools (3.9 KB; ROADMAP.md uses its anchor, so the heading stays; the UModel
text may overlap CLAUDE.md's external extraction policy, to check), Build and test (1.0 KB), Where things are (0.8 KB),
Repeatable inspection viewpoints (0.9 KB), Host engine probe (0.5 KB), `ow-package`: the package reader (4.6 KB; cites
DECISIONS.md), Independent oracles (see above).

Before applying any row: check that no other file cites the heading or its anchor (grep the repo, excluding local/ and
build/), and keep every command block that a tool or test instruction depends on.
