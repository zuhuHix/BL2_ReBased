## 2026-11-19: Actor.Trace leaves out primitives the collision hash cannot admit

AI-assisted (Claude Code). Lane IMPL-TRACE-HASH-ADMISSION. The decision number is pending (the landing run numbers
it). Coded from docs/verification/NATIVE_ACTOR_TRACE.md section 5 alone, the amendment of #659 (42f558b9, lane
LINK9-NEXT-STOP-2, analyst d2b6), copied in as this branch's first commit. Clean room: this lane did not read the
native or write the note. Synthetic fixtures only.

- **What changes:** `ow-world` `world/actor_trace.rs`, the actor part of the walk (section 3.3, step 2). After a
  primitive and its owner pass the residency checks, and before its bounds are asked, the walk reads the primitive's
  `CollideActors` and then its owner's `bCollideActors` (the owner from `trace_filter::owner_of`, as before).
  - Either clear: the primitive is left out, and nothing more of it is read.
  - Both set: the walk goes on as before (T5 for missing bounds, out for disjoint bounds, T7 for meeting ones).
  - Each field must be declared as a bool on the object's class, else a `VmError::Script` naming the class.
  - `bAttached` is not used.
  - The change is read-only; it writes, loads, draws and interns nothing.
- **Verified:** host tests on invented packages cover the note's five tests: the primitive's flag clear, the owner's
  flag clear (both a miss where it was T5), both set with no bounds (T5), both set meeting (T7), and an int
  `CollideActors` (`VmError::Script`). Every answer leaves #518's three counters and the name table unchanged. The
  fixture's obstacles now set both flags, so the earlier T5 and T7 tests keep their meaning.
- **Not verified:** nothing in the running game; the rule is **UNVERIFIED**. This is the hash admission's necessary
  condition only, not the whole admission.
