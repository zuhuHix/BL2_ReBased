## 2026-10-04: Sanctuary ambient citizens: census, assets, host perch cycle and node walks behind a flag

AI-assisted (Claude). Host and tools only; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/SANCTUARY_AMBIENT_NPCS.md`; native reading in `NATIVE_AMBIENT_NPC.md` (UNVERIFIED).

- **Census (our reader):** the civilians come from four population definitions in `Sanctuary_Combat` and two pawn
  archetypes, male and female Sanctuary Citizen. Their routes are decoded data: initial destinations, weighted next
  nodes, perch start/idle/stop clips, loop and lerp times, and Kismet scripted moves. Counts are in the record.
- **Real-game observations (SDK driver; Maya level 8 at mission Plan B):**
  - Saves were backed up and blocked in game. The game rewrote two files, which were restored, and the folders then
    compared identical.
  - 33 live citizens; 36 of 52 dens and encounters enabled, none of the 7 crowd dens.
  - A walking citizen's velocity read 150.
  - The town-wide patrols are Resistance fighters, not citizens: 94-95 uu/s by displacement, along the data's node
    circuits for 3 of 7 sampled.
- **Host:**
  - Both kinds are imported through the Marcus pipeline pattern: 112 bones and 42 clips each. Textures are bound by
    UModel's guess and the clip mapping is UNVERIFIED.
  - `-owambient=<manifest>` spawns them. Otherwise nothing spawns.
  - Stand-ins, all UNVERIFIED:
    - The live set is copied from one real-game moment.
    - Walking is in straight lines at 150 uu/s, with the floor taken from a trace.
    - A load balancer follows lane E's note.
    - There are no hats, hair or outfit variants, no Resistance fighters, and no talking or look-at.
- **Visual review:** an independent critic scored the host citizens 5/10 against real-game frames (mesh/outfit 4,
  poses 5, walk 6, scale 7). Main gaps:
  - bald identical heads;
  - no ink outline and washed-out colours;
  - the female reads as male;
  - a duplicated pawn at one perch;
  - several perch poses misaligned with their props.

  The real frames are matched by activity, not position: the population could not be held for close-ups.
- **Checks:**
  - `tools/test_ambient.ps1`: PASS (33 pawns, 33 reached a node, 3 walked, 24 at a perch).
  - Quest suite with ambient on: 79/0 and 11/0. One earlier first run failed check 61 on a Phaselock light reading; the
    repeat passed and the flake is with the Phaselock lane.
  - Quest suite without ambient: 79/0 and 11/0.
  - CTest 10/10; `verify_packages` 9/9.

