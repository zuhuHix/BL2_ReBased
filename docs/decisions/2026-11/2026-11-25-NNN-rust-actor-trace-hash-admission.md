## 2026-11-25: Actor.Trace in Off leaves out primitives the collision hash cannot admit

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-TRACE-HASH-ADMISSION, slice link 9. The file number is a placeholder
until the landing run numbers it. Coded from docs/verification/NATIVE_ACTOR_TRACE.md section 5 alone (the analyst
amendment that landed with #659, decision 2026-11-19-265, including its r1 points). Clean room: this lane did not read
the native and did not write the note. Synthetic fixtures only.

- **What changes:** `ow-world` `world/actor_trace.rs`, the actor part of the walk (the note's section 3.3, step 2, as
  section 5 amends it). After a primitive and its owner pass the residency checks (T1 still comes first), and before
  its bounds are asked, the walk reads the primitive's `CollideActors`, then its owner's `bCollideActors` (the owner
  from `trace_filter::owner_of`, as before).
  - Either clear: the primitive is left out and nothing more of it is read. Its bounds are never asked, so a clear
    flag wins over bounds that meet the swept box: a miss, not T7.
  - Both set: the walk goes on as before (T5 for missing bounds, out for disjoint bounds, T7 for meeting ones).
  - **Terrain is not skipped:** when the walked actor or the owner is a terrain, the flags are not applied, and
    meeting bounds are T7 naming the terrain handling, as before.
  - Each flag must be declared as a bool on the object's class; otherwise a `VmError::Script` naming the class.
  - `bAttached` and `BulletListener` are not read.
  - Read-only: it writes, loads, draws and interns nothing.
- **Verified (host only):** tests on invented packages cover the note's eight tests, each leaving #518's three
  counters and the name table unchanged: the primitive's flag clear, and the owner's flag clear, each a miss where it
  was T5; flags clear with meeting bounds, a miss; a clear flag with an unresident owner, T1 naming the owner; a
  terrain's primitive with the flags clear and meeting bounds, T7 naming the terrain handling; both set with no bounds,
  T5; both set and meeting, T7; an int `CollideActors`, `VmError::Script`. The fixture's obstacles now set both flags,
  so the earlier T5 and T7 tests keep their meaning.
- **Not verified:** nothing in the running game; the rule is **UNVERIFIED**. It is the hash admission's necessary
  condition only, and it rests on the note's own inference (open question 4): that Trace's check walks the same
  collision hash with the same per-component test as the spawn's walkers. A primitive whose class does not declare a
  flag at all takes the same `VmError::Script` path as the int case; no fixture covers it, since every toy primitive
  inherits the declaration.
