## 2026-11-19: GetBoundingCylinder's template test walks the outer chain

AI-assisted (Claude Code). Lane FOLLOWUP-BOUNDING-CYLINDER-TEMPLATE, a follow-up to #595 (IMPL-GET-BOUNDING-CYLINDER).
The decision number is pending (the landing run numbers it). Stacked on #595's head 9de51c43, because #595 is not
on main yet. The executable and the analysis output were not opened, and the lane did not analyse #590's note. Synthetic
fixtures only.

- **Why:** a recheck of #595 found that `ow-world` `world/bounding_cylinder.rs` decided "template or live" (step 2 of
  NATIVE_GET_BOUNDING_CYLINDER.md) from the receiver's own heap origin, so every run-time object was live. VM `new`
  can give a run-time object any outer, a class default among them. The note says the game tests the receiver and
  its outer chain for the class-default and archetype flags; the port's assumption that run-time objects have no
  template outers does not hold.
- **What changes:** the template test now walks the receiver and its outers, read-only (heap reads only, no load,
  no write, no draw):
  - a class default anywhere on the chain: a template;
  - otherwise an export or a resource stand-in on the chain, the receiver included: refusal T, as before for an
    export receiver, because the port keeps no archetype flag;
  - otherwise (only run-time objects): live.

  The two places that use the answer (the pawn rule, and the actor default's `bAttached` test) are unchanged.
  `placement::extent` is untouched.
- **Verified:** host unit tests on invented packages:
  - a pawn `new`ed into a class default, and one two outers up, get the pawn rule's cylinder; an actor `new`ed into a
    class default reads its entries' `bAttached` and meets the skip flag, with no refusal L;
  - a run-time object under only run-time outers is live (L); one with an export on its chain is T;
  - a run-time object whose outer is a really lazy store handle is T, and the handle stays unloaded and not failed
    (review r1 of #627);
  - an exported non-pawn with a resident primitive is T (both entry shapes), and without one answers the actor
    default.

  Every answer and refusal leaves the heap fingerprint (classes, slots, `is_unloaded` and `is_failed` per object,
  and the name table's size) and the RNG state unchanged.
  #518's counters (`committed_writes`, `load_transitions`, `draws`) were not in #595's base. With #595 and this
  change on main, lane FOLLOWUP-627-COUNTERS added that assertion, with the name table's length, to every refusal
  and answer of the module's tests; no counter moves.
- **Not verified:** nothing is checked against the running game, so the rule is **UNVERIFIED** in game. Whether an
  export on the chain is an archetype stays unread (refusal T).
