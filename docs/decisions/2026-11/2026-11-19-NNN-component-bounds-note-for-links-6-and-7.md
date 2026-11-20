## 2026-11-19: Component bounds: an own-words note for links 6 and 7

AI-assisted (Claude Code, Claude Opus 5.5). Lane ANALYST-COMPONENT-BOUNDS, an analyst lane; a separate lane
implements. The new record is `docs/verification/NATIVE_COMPONENT_BOUNDS.md`, with a NATIVE_INDEX row. The natives were
read locally with Ghidra 12.1.4, read-only, and the script with `research/script_disasm.py`. Raw output stays under
ignored `local/analysis/component_bounds/`. No bl-sdk session was run, so **every rule is UNVERIFIED in the running
game.** No code changes.

**What the note settles:**
- **(a) When and how `Bounds` is computed.**
  - **When:** a primitive's attach sets `bAttached`, then updates the transform, then the bounds. A deferred reattach
    (for example from `SetCylinderSize`) recomputes them.
  - **The base rule** is an unbounded box.
  - **A cylinder** is an axis-aligned box (R, R, H) at its world position, with sphere radius √(2R² + H²).
  - **A static mesh** transforms the mesh's stored bounds by `LocalToWorld`, adds 1, and scales by `BoundsScale`.
  - **A skeletal mesh** uses its physics asset over the current bone transforms, or the mesh's bounds. Only its
    branches were read.
- **(b) What `GetBoundingCylinder` reads.** It uses the counted boxes' union only. With one counted box the origin does
  not matter.
- **(c) The partial-writes mark.** It is the pickup driver's, from `make`'s gun creation. The touch path itself writes
  nothing before the call (script read), so no refusal can usefully move ahead of the mark.
- **The probe.** Throwaway, on main df0970ec. The live pickup's `SkeletalMeshComponent` now has `CollideActors` set,
  so the counted set would be the cylinder and the skeletal mesh. A stand-in past refusal L moves links 6 and 7 to
  `DroppedPickup.TouchPickupTrace`, which #624's note reads.

**The proposal, for the hub:**
- an attach record (not `bAttached` writes);
- on a live receiver, a `CollideActors` skip before the attachment question;
- the skip flag read as `BulletListener`. This is a choice: keeping refusal S is the alternative;
- the cylinder box computed at query time;
- static and skeletal mesh bounds `LayerAbsent`. The skeletal layer is the one #599's `SetSkeletalMesh` names.

So links 6 and 7 would move from "attachment unknown" to the bone-transform layer, still in `touch`.

**Gates.** Docs only. The leak gate is clean. Installed Off strict `--slice-gate` on main df0970ec: 4 ok / 10 NotYet / 0
errors. Links 6 and 7 pass `make` (1 of 3, and 1 of 4) and stop in `touch` at refusal L. This is what the host did,
not parity.
