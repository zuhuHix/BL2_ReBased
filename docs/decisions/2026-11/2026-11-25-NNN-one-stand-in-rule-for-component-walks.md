## 2026-11-25: One stand-in rule for the port's component walks

AI-assisted (Claude Code)

Lane UNIFY-STANDIN-RULE, with Claude Opus 5.5, the follow-up decisions 2026-11-25-16 and 2026-11-25-18 named. A
refactor with no behaviour change. No sensitive file, no dependency, no bounds check changed, and nothing about the
game is claimed.

- **Why:** two helpers walked a component's class chain with the same rule. They were
  `bounding_cylinder::primitive_class` (#681, the bounding cylinder's and the spawn extent's `Components` walks) and
  `trace_filter::part_class` (#683, the spawn, touch and `Actor.Trace` live collision walks). Each walk took
  `PrimitiveComponent` as a primitive, a chain through an engine-native stand-in class (decision 2026-11-15-08) as
  uncertain, and the end of a package chain as not a primitive. Two copies of one rule can drift apart.
- **What changed:** `primitive_class` now asks `part_class` and maps its answer. `Primitive` is yes, `Other` is no,
  and `Uncertain(class)` is the same `NotYet { NotRead }`, with the caller's prefix, `NATIVE_STAND_IN_CLASS`, the class
  path and the caller's record. Its text and record are unchanged. `part_class` is unchanged; its documentation now
  names both users. `primitive_class`'s stale decision link (`NNN`) now names 2026-11-25-16.
- **Tests:** every existing test of both walks passes unchanged. One new test pins that the two class tests agree,
  with and without a `PrimitiveComponent` id, for a primitive, a native stand-in, a package non-primitive and a class
  that is only declared. No object of a declared-only class can exist, but its class id can be asked about: both
  answer "not a primitive".
- **Base:** #683 rebased onto integration candidate c15 (`25425bb`), commit `794c6f9`; this branch stacks on it.
