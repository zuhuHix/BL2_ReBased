## 2026-11-25: The bounding cylinder's per-component check passes for primitive components in Off (links 6 and 7)

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-COMPONENT-CHECK, slice links 6 and 7. Coded from
docs/verification/NATIVE_COMPONENT_SKIP_FLAG.md section 6.1 alone (#676, at its review r1 head e6ac5e9; decision
2026-11-25-11). Clean room: the analyst lane read the native; this lane did not open the executable, Ghidra, REA or
the analysis output. Synthetic fixtures only.

**What changes** (Off only; Reference is unchanged):
- `world::bounding_cylinder::component_check` passes when the entry's class chain, walked up through resident and
  defined layouts, reaches `Engine.PrimitiveComponent`. Main had the exact-class list `COMPONENT_CHECK_PASSES` and
  the "the component check of <class> (not read)" refusal (`COMPONENT_CHECK_OF`); both retire, as the note
  recommends.
- The check is the same on all three paths: the live receiver, the template receiver and the spawn extent
  (`placement::actor_rule`). Its place in each order (last, before the box) is kept. It reads only.
- **The guard:** a chain that is not resident or not defined, or that never reaches `Engine.PrimitiveComponent`, is
  `NotYet { NotRead }` (`CHAIN_NOT_READ`, on this note's record). Every path asks the check only of an entry its class
  test already found to be primitive, and a class cannot be defined over an undefined super. So no path reaches the
  guard today; a unit test pins it on a non-primitive object.

**What the evidence covers:**
- **Read** (by the analyst): the check answers true for all 93 native primitive classes, reading and writing nothing.
- **UNVERIFIED extrapolations** (the note's two, labelled so in the code): a script-only primitive class takes its
  native parent's answer, and no native primitive class lies outside the 93.

**Effect on links 6 and 7** (the note's prediction, not a run): the pickup's skeletal mesh component passes the check
and meets its box rule, "Actor.GetBoundingCylinder: a skeletal mesh component's bounds source" (`NotRead`,
NATIVE_COMPONENT_BOUNDS.md). On the spawn extent, the same entry meets step 8's `LayerAbsent`. The slice gate was not
run here (no install on this PC).

**Tests** (synthetic; every refusal and answer leaves the heap fingerprint, the RNG, #518's three counters and the
name table unchanged), `bounding_cylinder_tests.rs`:
- note tests 1, 2, 4 and 7: on a live entry, skeletal gives `SKELETAL_SOURCE`, static mesh the static layer's
  `LayerAbsent`, and particle and toy script-only subclasses of `PrimitiveComponent` and `CylinderComponent` give "the
  bounds update of <class>". A template gives refusal L. Not colliding, or with the bit set, gives no question. No text
  carries "the component check of";
- note test 3: a `BulletListenerComponent` meets its bounds rule; the cylinder box tests are unchanged;
- note test 6: a non-primitive entry is skipped at the class test;
- the guard: the check passes five primitive classes and refuses a non-primitive one with `CHAIN_NOT_READ`.

`placement_extent_tests.rs` covers note test 5: the skeletal entry now meets step 8's `LayerAbsent`, not the
component check. Note test 8 (Reference) is covered by the existing binding tests.

**Not verified:** anything in the running game; the two extrapolations; links 6 and 7's next stop (needs a runner
probe).
