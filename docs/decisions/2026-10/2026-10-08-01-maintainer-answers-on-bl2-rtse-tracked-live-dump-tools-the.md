## 2026-10-08: maintainer answers on BL2_RTSE, tracked live-dump tools, the hostile enemy and the Remaster tables

AI-assisted (Claude) record of answers the maintainer (zuhu) gave in conversation on 2026-10-08.

- **BL2_RTSE** (https://github.com/zuhuHix/Bl2_RTSE) is the maintainer's own project. Its licence and provenance question is settled: it is theirs, and the
  README now links it. That is a statement about ownership; any code taken from it still gets a note in the commit that brings it in. Nothing has been copied
  so far.
- **Tools that read live dumps may be tracked**, as long as they are tools: scripts and the code that reads and compares. The dumps and any values
  taken from the game still stay under ignored `local/` and never go into a tracked file, commit or doc.
- **Hostile enemy for the Phase 4 gate: a bullymong**, built on the VM from the game's own data (Three Horns bullymong archetype), not the dummy plus a stand-in
  attacker. The Fire dummy is still the first VM pawn for the stock hit chain (its real health is 778.658 with a floor of 1.0, see lane L5), as a stepping stone.
- **Remaster tables** (`Mancana_StartupRemaster.upk`): the maintainer has no view on them yet. Still open; nothing is decided, and no record is written.

