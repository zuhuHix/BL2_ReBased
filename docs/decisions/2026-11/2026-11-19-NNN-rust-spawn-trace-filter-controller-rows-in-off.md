## 2026-11-19: The spawn trace filter's per-component test, skip flag and controller rows, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-LINK9-TRACE-FILTER, slice link 9. Coded from
docs/verification/NATIVE_SPAWN_TRACE_FILTER.md alone, at its review-r1 head 206a18f4 (PR #502; clean room: lane
ANALYST-LINK9-TRACE-FILTER wrote it). The executable and the analysis output were not opened. Synthetic fixtures only.
Based on fix/rust-encroachment-check (4024bc6c, #503). The note's file and its pointers in the neighbouring records land
with #502.

- **What changes (`ow-world` `trace_filter::tested`, used by the spot search's actor walk).** Each read test only
  rejects, so a primitive any one of them rejects is not tested (note sections 3 and 7):
  1. **Terrain** stays Unsupported ahead of every other test, blocking or not (note section 7, step 3: what the terrain
     bit does beyond the hash was not read).
  2. **The owner:** the component's own `Owner` field when its class declares it and it is set, otherwise the walked
     actor whose `CollisionComponent` or `Components` holds it (`trace_filter::owner_of`). The port's spawn never sets
     the component field, so in practice the walked actor stands for it (UNVERIFIED).
  3. **The skip flag**, read as `BulletListener` on the live primitive (the note's inferred name, UNVERIFIED): set, not
     tested. A sweep that sets bit 30 would ignore it; the spawn's queries never do.
  4. **The blocking flag** for the query's kind, as before.
  5. **The per-component test:** the live primitive's `CollideActors` and the live owner's `bCollideActors`, both set.
     Otherwise not tested. It replaces the per-component `NotYet`.
  6. **The owner's row**, by exact class. `Engine.Controller`, `Engine.PlayerController`, `Engine.GamePlayerController`
     and `WillowGame.WillowPlayerController` now run the `Actor` row. No row is inherited. A row that rejects means not
     tested; this replaces the skip-flag `NotYet` that a blocking primitive used to give.
  - **`bAttached` is not read.** A clear `bAttached` is no answer on its own (note section 7, step 5).
- **What stays unread**, Unsupported only when every read test passes:
  - an owner class with no read row: "the trace filter of <class>", NATIVE_SPAWN_PLACEMENT.md;
  - a primitive every read test accepts: "the collision hash's admission beyond the per-component test", under
    NATIVE_SPAWN_TRACE_FILTER.md (`Unread::record`).
  - The walk's refusal now carries the unread rule's own record.
- **Marking.** The filter writes nothing and returns its refusals unmarked. In link 9 they come after the extent's
  loads and the driver's committed spawns, which mark them. Reference is unchanged: the spot search is Off only.
- **Not on link 9's path, left for a later lane:** NATIVE_SPAWN_EXTENT.md's Off step 6 now reads `BulletListener`, per
  section 7, step 6 of this note. An attached, colliding entry with it set would be skipped. One with it clear would
  pass to `Bounds`, which the extent rule does not read yet. `placement::extent` still answers its skip-flag `NotYet`
  there. Link 9's entries are unattached and never reach that step.
- **Tests.**
  - `trace_filter/tests.rs`:
    - the controller rows, and an unread subclass of them;
    - every read test rejecting in turn, with the hash admission when all pass;
    - link 9's controller cylinder with the class defaults' flags, not tested in all four spawn queries;
    - the owner field against the walked actor;
    - an unread owner class past the read tests, with terrain ahead of them.
  - `world/tests.rs`: the blocking-live-primitive walk now expects not tested for each rejecting test, and the hash
    admission or the owner class otherwise.
- **Checks.** `cargo test -p ow-world` 232 passed, 0 failed, 5 ignored; `-p ow-cli` 101 + 2 passed, 0 failed.
- **Installed `--slice-gate`, Off, strict, under the machine lock:**
  - before (#503): 0 ok / 5 not yet / 9 error; after: 0 ok / 4 not yet / 10 error.
  - Only link 9's row changed. It passes `spawn` (1 of its 2 steps) and stops in `cast` at "unbound native
    AttributeDefinition.GetValue(object,object,object)", a VM error. Before, it stopped in `spawn` at the trace filter
    on `WillowPlayerController_0`'s CollisionCylinder.
  - The third spawn's walk tests nothing, and its post-spawn encroachment check passes. Whether it passes by the gate
    or by an empty overlap set is not separated in the row.
- **UNVERIFIED.** Nothing here was checked in the running game. Still open, from the note:
  - the hash admission's scene condition and removal (open question 1);
  - the skip flag's name (2);
  - the live controller's flags at the cast (3);
  - the terrain bit (4).
