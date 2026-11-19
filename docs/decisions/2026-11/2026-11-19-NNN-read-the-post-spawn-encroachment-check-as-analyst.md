## 2026-11-19: Read the post-spawn encroachment check as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-ENCROACHMENT-CHECK, analyst only (clean room; d2b6 implements). New
note `docs/verification/NATIVE_ENCROACHMENT_CHECK.md` and its NATIVE_INDEX row. The decision number is a placeholder
until the hub assigns one.

- **Sources:** `Borderlands2.exe`, through `tools/ghidra/run.ps1` read-only queries, one headless run at a time under
  the machine lock; field names from `tools/ghidra/class_layout.py`. Raw output stays under ignored
  `local/analysis/spawnspot/`. There is no bl-sdk session, so every rule is **UNVERIFIED**.
- **Read:**
  - the check runs only when the live actor's `bCollideActors` is set (otherwise no encroachment, no touch, no
    write);
  - its shape is the bounding cylinder;
  - the world's actor collision hash gives the overlaps;
  - an encroachment pass with skips (self, pending delete, based on the actor, a primitive that does not block by
    extent), then a per-class blocking test and the other actor's "is encroached" method. On encroachment the location
    is restored and the spawn destroys the actor and returns None;
  - a touch pass for the overlaps that do not block.
- **Off answers:**
  - `bCollideActors` clear passes with no write;
  - an empty overlap set passes with no write;
  - a service that cannot place a live candidate is `NotYet { LayerAbsent }`;
  - a non-empty set is `NotYet { NotRead }` (the per-class blocking test);
  - all are terminal after the actor was made.
- **Link 9:** the pawn spawns with only the `WorldInfo` live, so it passes when the `WorldInfo` has no colliding
  primitive, or refuses `LayerAbsent` under `NoLevel`.
