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
  probe on an unmerged base, never pushed). The proposed Off answer reads nothing it would have to load. It writes
  both out arguments only at the end. It refuses as follows:
  - a class whose body was not read: C;
  - a receiver whose template status the port cannot tell (`Export` or `Resource` origin): T;
  - a lazy-handle component: N;
  - a live actor's primitive entry, since the port tracks neither attachment nor bounds: L;
  - the unread skip flag: S, main's text.

  A template with no attached colliding entry answers 0 and 0.
- **Against NATIVE_SPAWN_EXTENT.md** (main's `placement::extent`):
  - main's reading wins on the skip flag, the exact-class refusal and a template's `bAttached`;
  - this note adds the pawn's template condition, on a re-read of the pawn body. For a template both give the same
    answer, so link 9 is unchanged.
- **No cylinder-bounds stand-in** is proposed. Its gain is small (only the half height matters, and
  `TouchPickupTrace` is still refused), and it would rest on rules that were not read.

Verified: nothing in the running game; no bl-sdk session was granted. Every rule is UNVERIFIED. Parsing is unchanged.
