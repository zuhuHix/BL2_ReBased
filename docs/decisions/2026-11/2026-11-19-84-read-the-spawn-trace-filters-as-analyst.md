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
  blocking kind (`BlockNonZeroExtent` for a box, `BlockZeroExtent` for a point); then the owner's filter by class; then
  the primitive's own test.
- **The spawn's queries pass no source actor,** so the blocking narrowing rejects every actor that needs it.
  - **Box tests (8902):** the BSP and world-geometry actors only. Movers are rejected, and pawns, volumes and other
    actors are not selected. Complex collision applies in template mode.
  - **Sweeps (8326):** the BSP, world-geometry actors and movers. Simple collision.
- **What a headless collision service must answer** for each query, and when it answers Unsupported.
- **Port's Off choice, UNVERIFIED:** two unread component conditions are taken as not excluding.

**Open (in the note).** The primitive skip flag (bit 10; its name is unresolved by the layout tool), one further
per-component test, and what the terrain bit does beyond entering the actor hash.
