## 2026-10-04: Sanctuary ambient citizens round 2: heads, hair and hats from live pawns, ink line, perch root motion

AI-assisted (Claude). Host and tools only; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/SANCTUARY_AMBIENT_NPCS.md` section 8.

- **Causes of the round-1 defects:**
  - The "duplicate pawn" was two real neighbouring citizens (`Perch_66` and `Perch_140`, 240 uu apart) on the camera's
    line. The capture camera now rejects lines that pass within 130 uu of another pawn.
  - Lean, legs and squat: the stock perch clips carry root motion (the observed pawn-to-node offset matches the start
    clip's travel, e.g. 24.1 vs 25.8 uu at `Perch_66`), and the floor trace missed the real pawn height by more than
    10 uu on 19 of 33 pawns. The host now applies each clip's root travel when it ends and keeps the observed height
    for idle and held pawns.
- **Observed in the real game** (third SDK session, saves blocked, byte-identical afterwards): each live citizen's
  materials and attached static meshes are readable. 33 pawns carried 88 attachments (20 meshes, 8 head textures).
- **Host:** those heads and attachments, bone-attached; Maya's inverted-hull ink line and matte constants for the
  citizens. UNVERIFIED: the attachment transform (judged by eye), the hair tint stand-in, the outline thickness and the
  shader. Not done: `Master_NPC` zone colours, body garment variants, Resistance patrols, matched real-game close-ups.
- **Visual review:** an independent critic scored round 2 5.5/10 (round 1: 5), judged better than before, no
  overlapping pawns. Main gaps: the female still reads as male (face patch, hair), a mis-parented hat at one stop, scalp
  showing through blonde hair, one bald untextured head, the bang-on-wall fist not meeting the wall, a squat clipping
  a pipe.
- **Checks:** quest suite with ambient on 79/0 and 11/0, with ambient off 79/0 and 11/0; `test_ambient.ps1 -Shots`
  PASS (36 pawns, 36 reached a node); CTest 10/10; `verify_packages` 9/9.

