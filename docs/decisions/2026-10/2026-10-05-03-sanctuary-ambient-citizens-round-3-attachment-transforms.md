## 2026-10-05: Sanctuary ambient citizens round 3: attachment transforms, zone colours, the grey stand-ins

AI-assisted (Claude). Host and tools (`OpenWillowAmbient.cpp/.h`, `ambient_npc_assets.py`, `ambient_npc_attach_editor.py`,
`prepare_ambient_world.py`, the real-game `ambient_npcs.py` script); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/SANCTUARY_AMBIENT_NPCS.md` section 9.

- **Hats, hair and gear:** their placement lives on the attached mesh component itself (translation, rotation, scale),
  not on the attachment entry; rounds 1-2 ignored it. Observed on live pawns through the SDK script, now applied.
- **Zone colours:** a host material mixes the diffuse with the pawn's own zone colours through its light map and zone
  mask. The mixing formula is applied by analogy with the weapon master reading and is `UNVERIFIED` for the NPC master.
- **Grey citizens in the 2026-10-04 showcase:** the first version of that material failed to compile (a linear sampler
  given an sRGB default), so UE drew the default material. Fixed; the shot tour log has no compile failure and 35 of
  35 pawns render textured.
- **Visual review:** a blind A/B critic judged round 3 a little closer than round 2 (about 5.9 against 5.6 per stop;
  the female now reads as female). Main gaps: packs float beside the shoulder, some heads read as flat discs, the
  palette is warmer than the game's night-lit blue-grey, the wall-bang and kick idles do not read, weak face cues.
- **Checks:** `tools/test_ambient.ps1 -Seconds 60` PASS (35 pawns, 35 reached a node, 24 at a perch); quest suite with
  ambient on and off 79/0 and 11/0; CTest 10/10.

