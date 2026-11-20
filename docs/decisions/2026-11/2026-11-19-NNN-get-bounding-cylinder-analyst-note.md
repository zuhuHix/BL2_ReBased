## 2026-11-19: Read Actor.GetBoundingCylinder as analyst

AI-assisted (Claude Code, Opus 5.5).

Lane ANALYST-GET-BOUNDING-CYLINDER, analyst only; a separate lane implements it. Links 6 and 7 stop at `touch` on the
unbound `Actor.GetBoundingCylinder(float,float)`, and link 9's spawn extent refuses "the actor's bounding-cylinder rule".
The new own-words note `docs/verification/NATIVE_GET_BOUNDING_CYLINDER.md` and one `NATIVE_INDEX.md` row record:

- **What it answers.** Three implementations. The actor default turns the box of the actor's attached, colliding
  primitive components into a radius (the horizontal half-diagonal) and a half height, and gives 0 and 0 for an empty
  box. A pawn uses its `CylinderComponent` only as a template; a navigation point uses it always. It writes only its two
  out arguments, and loads and draws nothing.
- **The slice's pickup** holds a colliding `CylinderComponent` and a `SkeletalMeshComponent` in `Components` (a local
  probe, never pushed). The port computes no component `Bounds`, so the proposed Off answer refuses `LayerAbsent` (L)
  for a live actor with a colliding primitive left, answers 0 and 0 where nothing counts, and refuses `NotRead` (T)
  where the port cannot tell a pawn template from a live pawn.
- **A hub choice** is named, not taken: compute a cylinder's bounds from its radius and height, which would move links
  6 and 7 to `TouchPickupTrace` (refused `NotRead` in Off today).

Verified: nothing in the running game; no bl-sdk session was granted. Every rule is UNVERIFIED. Parsing is unchanged.
