## 2026-11-25: The deferred reattach's owner branch

AI-assisted (Claude Code)

Lane ANALYST-DEFERRED-REATTACH-OWNER, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** link 1's `maya:8` stops at `REATTACH_OWNER`, the deferred reattach's owner branch, and links 3, 4, 5, 13
  and 14 are behind it.
- **What:** section 2.1 of `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md`, one NATIVE_INDEX row and one GATES
  row, from a local read-only Ghidra reading under the machine lock and the layout tool. Raw output stays under
  ignored `local/analysis/reattach/`.
- **Findings** (UNVERIFIED in game):
  - with an `Owner`, the native sets `bNeedsReattach`. It then runs the owner's component update at once only for an
    owner that is `bStatic` or `bTickIsDisabled`, not `bDeleteMe` and not being destroyed;
  - that update attaches, reattaches or refreshes the transform of every component of the owner, then runs each
    component's child update;
  - for any other owner, nothing more happens now. Maya's pawn, the pickup and the weapon are all ordinary owners by
    their class defaults.
- **Proposed Off answer:**
  - the owner branch reads the owner's three flags, loaded-only;
  - the immediate update keeps the existing `REATTACH_OWNER` refusal (review r1);
  - the ordinary owner is no refusal: only `bNeedsReattach` (and the setter's own field) is written;
  - Reference unchanged; tests listed, including `maya:8`'s call shape.
- **On link 1 (a prediction):** hook 2 no longer refuses there. Its plan, `bOwnerNoSee` and `bNeedsReattach`, must be
  applied to the made component.
- **Review r1 (#709):**
  - the two object-state flags are read as "not pending kill" and "not a template object", as the particle note has
    them. They are not modelled: the port refuses on the static path;
  - the refusal stays `REATTACH_OWNER` (`NotRead`), consistent with the sites that already apply the owner rule;
  - every owner-branch site is listed with its answer, and the changed tests are named;
  - hook 2's fallback goes and its plan is applied in the write phase. The render-hooks note is marked;
  - the class-defaults read is recorded, with the instance caveat.
- **Clean room:** a different lane implements this note.
