## 2026-11-19: Read the spawn trace filters as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SPAWN-TRACE-FILTERS, analyst only (clean room). It extends
`docs/verification/NATIVE_SPAWN_PLACEMENT.md` section 7: the filter paragraph of review r2 of #459 is replaced, along
with the "filters are unread" term of section 3, open question 4, and step 5 of section 6.

- **Sources:** `Borderlands2.exe`, through `tools/ghidra/run.ps1` read-only queries and the local read-only table
  reader, one headless run at a time under the machine lock. Read: the world's box and sweep routines, the actor
  collision hash's two walkers, and the per-actor filter of `Actor`, `Pawn`, `Volume`, `InterpActor` and `KActor`.
  The flags were named with `tools/ghidra/class_layout.py`.
- Raw output stays under ignored `local/analysis/tracefilters/`. There is no bl-sdk session and no recorded live
  evidence, so every rule is **UNVERIFIED**.
- **No code changes.**
- **The base:** the branch is stacked on `research/rust-spawn-placement` (#459, accepted, not yet landed), since the
  paragraph it replaces exists only there. It is rebased onto main once #459 lands.

**What the note now says** (values in decimal).

- **The bits read:**
  - 1 pawns; 2 movers; 4 the level's BSP; 8 volumes; 16 other actors;
  - 32 projectile targets only; 64 blocking only (through a test on the source actor); 128 world-geometry actors;
  - 512 stop at the first hit; 1024 one result (added by the sweep); 8192 terrain (entry mask only);
  - 131072 complex collision.
- **How a query runs:** the BSP first; then the actor hash (for a sweep only under the mask 8347); then each primitive's
  blocking kind (`BlockZeroExtent` for a zero-extent box test and a point sweep, `BlockNonZeroExtent` otherwise); then
  the owner's filter by class; then the primitive's own test.
- **The spawn's queries pass no source actor,** so the blocking narrowing rejects every actor that needs it.
  - **Box tests (8902):** the BSP and world-geometry actors, a world-geometry `Volume` without `bPawnsOnly` included.
    Movers are rejected; pawns, other actors and the other volumes are not selected. Complex collision applies in
    template mode.
  - **Sweeps (8326):** the same, plus movers. Simple collision.
- **What a headless collision service must answer** for each query, and when it answers Unsupported.
- **Off policy (since review r3 of #465):** nothing unread is assumed. Any query whose answer depends on an unread
  point is Unsupported: the per-component test, the skip flag, an unread class filter, terrain, or the BSP and actor
  comparator.

**Open (in the note).** The primitive skip flag (bit 10; its name is unresolved by the layout tool), one further
per-component test, and what the terrain bit does beyond entering the actor hash.

**Review r1 of #465 (four findings, fixed in the note).**
1. The query summaries apply the class rules per owner class, in a table for box tests and sweeps: a world-geometry
   `Volume` without `bPawnsOnly` is tested, and movers follow their own row (no bit 128 shortcut), so sweeps test them
   with or without `bWorldGeometry`. Section 2 says the sweeps also test movers.
2. One Off policy: a candidate with the unread skip flag makes the query Unsupported (so a `NotYet` of kind
   `NotRead`); the further per-component test is taken as passing; any terrain-specific effect of bit 8192 is
   ignored. All UNVERIFIED.
3. The box-test contract: `BlockZeroExtent` for a (0, 0, 0) extent, `BlockNonZeroExtent` otherwise (read); the
   effective `complex` mode passed explicitly; a primitive without per-polygon collision tested simple (the game's
   answer) versus collision the service lacks (Unsupported); "Hit" throughout.
4. Sweep hit selection (read further): BSP first, then actors walked up to the BSP hit plus 5 units, the smallest
   actor fraction kept, then the two sorted by an unread comparator (taken as the smaller fraction). Exact ties with
   different locations or normals are Unsupported. After the actor is made, Off rolls nothing back.

**Review r3 of #465 (four findings, fixed in the note).**
1. **No guessed filters.** Any candidate primitive (the unread per-component test, the skip flag), an owner class
   whose filter was not read, a `Terrain` primitive, or a sweep with both a BSP hit and an actor hit makes the query
   Unsupported, which section 6 turns into a `NotYet` of kind `NotRead` naming the filter. Off can answer a box test
   the BSP blocks, a query with no candidate primitive, and a BSP-only sweep.
2. **Ties kept to the end.** Every hit at the minimum fraction is kept through the final choice, and a tie with
   different locations or normals is Unsupported. When the BSP hit plus 5 units reaches past the end point, the actors
   are walked only to the end point (the scale is capped at 1).
3. **Every `NotYet` after the actor is made is terminal,** `PreBeginPlay`'s own included: prefixed "stopped after
   partial writes: ", kind and record kept, writes left in place.
4. **This summary** now matches the addenda, and the walker points at the extent-specific blocking rule.
