## 2026-11-25: Actor.GetBoundingCylinder: where a skeletal mesh component's bounds come from

AI-assisted (Claude Code)

Lane ANALYST-SKELMESH-BOUNDS-SOURCE, analyst only, with Claude Opus 5.5. The number is left for the hub to
assign. Docs only: no code, no dependency, and no bounds check
changed.

- **Why:** after #677, links 6 and 7 stop in `touch` at "a skeletal mesh component's bounds source: not read".
- **What:** section 9 of `docs/verification/NATIVE_COMPONENT_BOUNDS.md`, with its NATIVE_INDEX row updated. It comes
  from a local read-only Ghidra reading under the machine lock, the layout tool (a local copy sizing `MapProperty`)
  and the installed class defaults. Raw output stays under ignored `local/analysis/skelbounds/`.
- **Findings** (UNVERIFIED in game; field names by fit):
  - the box routine reads the stored `Bounds`, which the skeletal bounds update writes;
  - that update returns early, keeping its old bounds, for a sleeping rigid body or `bSkipUpdateSkelPose`. It then
    decides visibility (`DetailMode`, `HiddenGame`, the owner's `bHidden`, `bIgnoreOwnerHidden`, `bCastHiddenShadow`);
  - it takes the first source that applies:
    - the mesh's stored bounds when the component is hidden or `bComponentUseFixedSkelBounds` is set;
    - the parent's fixed mesh bounds;
    - its own physics-asset bodies (with `bHasValidBodies`);
    - the parent's physics asset, or the parent's stored bounds;
    - the base rule with no mesh;
    - otherwise the mesh's stored bounds;
  - then come the per-poly bone boxes and `BoundsScale`, with nothing added;
  - the "stopped after partial writes" prefix is the pickup driver's, as section 2 says.
- **On links 6 and 7:** the port's component carries class defaults, without its template's mesh, so it takes the
  no-mesh base rule, an unbounded box. That is rule-faithful on a non-game input. A hub ruling is asked (option B
  recommended).
- **Proposed Off answer:** `entry_box`'s skeletal case becomes the ordered read-only rule. Unread parts and the
  kept-bounds cases are `NotRead`. The stored-bounds, bones and parent-bounds layers are `LayerAbsent`. The base rule
  is answered. Tests listed.
- **Clean room:** a different lane implements this note.
