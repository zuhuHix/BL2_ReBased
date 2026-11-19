## 2026-11-19: Read the spawn trace filter on a controller as analyst

AI-assisted (Claude Code / Claude Opus 5.5). Lane ANALYST-LINK9-TRACE-FILTER, analyst only, for slice link 9. Every
two-digit number of 2026-11-19 is taken, so `NNN` is a placeholder until the maintainer rules on three-digit numbers;
the decisions check refuses it until then.

Stacked on `fix/rust-link9-extent` (#498, 40982583). There, link 9 stops in `spawn` (0 of 2 steps) at the spot
search's first box test, on "the trace filter of WillowGame.WillowPlayerController": the live controller's
`CollisionCylinder`.

The new own-words note `docs/verification/NATIVE_SPAWN_TRACE_FILTER.md` and one `NATIVE_INDEX.md` row record, as
behaviour only:
- **The controller classes** (`Controller`, `PlayerController`, `GamePlayerController`, `WillowPlayerController`)
  keep the `Actor` filter body, so the spawn's queries reject them (no `bWorldGeometry`, no bit 16).
- **The per-component test**, open question 4 of NATIVE_SPAWN_PLACEMENT.md: the primitive's `CollideActors`, and no
  owner or the owner's `bCollideActors`. Both walkers run it before the owner filter.
- **Every test in the walkers only rejects**, so their order does not change the outcome for a rejected primitive.
- **The skip flag** reads as `BulletListener`, inferred from the bool word's layout (UNVERIFIED).
- **The collision hash** admits a primitive at attach only if the per-component test passes, plus one scene condition
  that was not read.
- **For link 9:** the controller's class defaults have both collision flags clear, so the spawn's traces skip its
  cylinder. The first box test is then Clear in the `NoLevel` world, and the next stop is the post-spawn encroachment
  check.
- **Off answers:**
  - the four controller rows, as the `Actor` row;
  - the per-component test on the live flags;
  - the skip flag read as `BulletListener`;
  - for a primitive that passes every read test, `NotYet` (not read), "the collision hash's admission beyond the
    per-component test".

**Clean room.** Nothing is implemented here. The raw analysis stays under ignored `local/analysis/`. No real-game
session took place, so every rule is **UNVERIFIED**; the note lists four bl-sdk checks. No code, test, dependency or
parsing change.
