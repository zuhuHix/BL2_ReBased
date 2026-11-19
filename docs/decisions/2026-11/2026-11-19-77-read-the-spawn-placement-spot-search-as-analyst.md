## 2026-11-19: Read the spawn placement spot search as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SPAWN-PLACEMENT, analyst only (clean room). New record:
`docs/verification/NATIVE_SPAWN_PLACEMENT.md`.

- **Sources, in order:**
  - the records (NATIVE_ENGINE_CORE.md's spawn routine, steps 3 and 9, which left the spot finding and the
    encroachment check open);
  - the installed packages (`tools/ghidra/class_layout.py`, to name the flags);
  - `Borderlands2.exe`, through `tools/ghidra/run.ps1` read-only queries, one headless run at a time under the machine
    lock. The search was reached through `Actor.FindSpot`'s exec and the spawn routine that calls the same world routine.
- Raw output stays under ignored `local/analysis/spawnspot/`. No bl-sdk session was granted and no live evidence is
  recorded, so every rule is **UNVERIFIED**.
- **No code changes.** The port's spawn keeps its `NotYet` until a fresh implementer codes the rules from the note
  alone.

**What the note settles.**

- **When the search runs:** `bNoCollisionFail` false, and the template's `bCollideWorld`, or its
  `bCollideWhenPlacing` outside a network client.
- **What it tests:** a box of the template's collision cylinder extent against world geometry, with `bCollideComplex`
  choosing complex collision for the first test.
- **The steps:** the box at the spot; failure for a zero extent; the horizontal-slice step with its floor trace and
  height adjustment; the corner nudges. The constants are recorded.
- **What it changes:** only the location. A failure returns None before anything is made.
- **The encroachment check** runs after `PreBeginPlay` for every spawn without `bNoCollisionFail`, whatever
  `bCollideActors` says. A failure destroys the actor and returns None. Its internals are not read.
- **The relocation step** for a `bNoCollisionFail` spawn of an actor that collides with actors is not read.
- An Off answer list. Without a collision model, the search and the check stay a `NotYet` of kind `LayerAbsent`,
  never "free". A collision model with nothing near the spot answers "unchanged". A template without a cylinder
  collision component is a `NotYet` of kind `NotRead`.
- The port's shortcut that an actor without `bCollideActors` passes the encroachment check is not shown by this read;
  the note asks for it to stay labelled or become the same `NotYet`.
- The cross-reference: H4's host collision world (`crates/ow-host/src/movement/collide.rs`) as the later source of the
  tests, with the shape and geometry differences named.

**Open (in the note).** The encroachment check's internals, the relocation step, the bounding-cylinder rule, the trace
flag sets, and the net-mode comparison.
