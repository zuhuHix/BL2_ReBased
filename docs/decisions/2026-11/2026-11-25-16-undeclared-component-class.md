## 2026-11-25: A Components entry whose class runs through an engine-native stand-in is NotRead, not skipped

AI-assisted (Claude Code)

Lane UNDECLARED-COMPONENT-CLASS, with Claude Opus 5.5, from #677's review (L3, pre-existing on main). A port policy
that tightens a check; every rule here is UNVERIFIED in the running game. No sensitive file, no dependency, no bounds
check changed.

- **Why:** `Actor.GetBoundingCylinder`'s `Components` walk (`bounding_cylinder.rs`, the live receiver and the
  template path) and the spawn extent's `Actor` rule (`placement.rs`) keep an entry only when its class derives from
  `Engine.PrimitiveComponent`, and skip the rest silently. An entry whose real class chain the port does not know
  would then be left out of the cylinder with no `NotYet`, while the game might count it.
- **What the review named, and what can happen:** the review named an entry whose class is only declared (its package
  not loaded, no super class). The object space makes no object of a declared-only class (`alloc` and
  `alloc_stand_in` refuse it), and a class is defined only after its super class, so that case cannot reach the walk.
  The case that can is the **engine-native stand-in class** (decision 2026-11-15-08): a class with no package export
  is defined directly under `Core.Object` and marked native, so the `PrimitiveComponent` test answers no although its
  real chain is not known. The VM's casts already answer `NotYet` for such a chain (`interp::is_child`).
- **The rule** (`bounding_cylinder::primitive_class`, used by both walks): the chain is walked from the entry's class
  up. Reaching `PrimitiveComponent` is yes. Reaching an engine-native stand-in class first is `NotYet { NotRead }`
  ("... a Components entry whose class runs through the engine-native stand-in class (decision 2026-11-15-08), so
  whether it is a primitive is not known: <class>"), on `NATIVE_GET_BOUNDING_CYLINDER.md` for the bounding cylinder
  and `NATIVE_SPAWN_EXTENT.md` for the spawn extent. It comes before any other check of the entry and is read only.
  The end of a chain of package classes is no, and that entry is skipped as before.
- **Only the class-test lines changed**, so the change rebases over #677.
- **Tests (synthetic):** both walks refuse a native stand-in entry with `NotRead` naming it, with the heap, the RNG,
  #518's counters and the name table unchanged. A package class that is not a primitive is still skipped, and a
  primitive takes its usual path; the existing tests of both rules pass unchanged.
- **Measured** (zuhuPC, Linux, release builds under the machine lock, Aspyr install): installed Off strict
  `--slice-gate` on main 4b3e4df and on this change (on bf9eea7, before the rebase onto 4b3e4df) gives the same
  result and first stop on all 14 links. Cause A
  (lower-case package names, #679) still stops most links on that install, so this covers links 2, 6, 7, 8 and 9 only.
  Not measured on a Windows install.
