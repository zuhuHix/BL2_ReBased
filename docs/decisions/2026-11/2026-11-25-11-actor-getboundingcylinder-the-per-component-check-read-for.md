## 2026-11-25: Actor.GetBoundingCylinder: the per-component check, read for every component class

AI-assisted (Claude Code)

Lane ANALYST-SKELMESH-COMPONENT-CHECK, analyst only, with Claude Opus 5.5. The file number is provisional until the
maintainer gives one. Docs only: no code, no dependency, and no bounds check changed.

- **Why:** main's measured stop for links 6 and 7 (2026-11-25) is in `touch`, "Actor.GetBoundingCylinder: the
  component check of Engine.SkeletalMeshComponent (not read)". It is the refusal NATIVE_COMPONENT_SKIP_FLAG.md section
  5 gives a class whose check section 3 had not read.
- **What:** an amendment, section 6 of `docs/verification/NATIVE_COMPONENT_SKIP_FLAG.md`, and its NATIVE_INDEX row,
  updated. They come from a local read-only Ghidra reading under the machine lock and the superclass chains of the nine
  code packages. Raw output stays under ignored `local/analysis/skelcheck/`.
- **Findings** (UNVERIFIED in game):
  - the check is one virtual of the component's primary table. Each class's table was found by following its
    registration's constructor chain to the last table store;
  - the packages declare 96 descendants of `PrimitiveComponent` (itself included), found by hierarchy, not by name.
    93 are native, `SkeletalMeshComponent`, `StaticMeshComponent` and `ParticleSystemComponent` among them, and every
    one answers true, with the same folded body. The other three are script-only;
  - 52 of the packages' 64 non-primitive component classes were examined, and they answer false;
  - the split matches the package hierarchy without exception;
  - neither body reads or writes anything, so the check is a class constant;
  - the bounding cylinder asks its box routine for colliding components only.
- **Proposed Off answer:** the component check passes for every entry that reaches it, because only primitives
  reach it. For the script-only classes, and for any native primitive class outside the 93, that pass is an
  UNVERIFIED extrapolation (engine conventions, not read). `COMPONENT_CHECK_PASSES` and the `COMPONENT_CHECK_OF` refusal retire. An alternative that keeps a list of
  the 93 read classes is described and not recommended. Reference unchanged; tests listed.
- **On links 6 and 7 (a prediction, not a run):** the skeletal entry passes. It then meets the existing
  `NotRead` "a skeletal mesh component's bounds source" (NATIVE_COMPONENT_BOUNDS.md B.7), so the next analyst question
  is the skeletal bounds' source selector.
- **Clean room:** a different lane implements this note.
