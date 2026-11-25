## 2026-11-25: The spawn and touch walk refuses invalid service bounds as Actor.Trace does

AI-assisted (Claude Code)

Lane WALK-INVALID-BOUNDS, with Claude Opus 5.5, the known gap decision 2026-11-25-18 recorded. A port policy that
tightens a check, Off only (the walks run in `ParityMode::Off`; Reference is unchanged). Every rule here is
UNVERIFIED in the running game. No sensitive file, no dependency, no bounds check of the reader changed.

- **Why:** `placement::actor_walk` (the spawn's spot search and the pickup touch's line check) left a live primitive
  out when its service bounds did not meet the query. Bounds with a NaN make every comparison false, and an inverted
  box meets nothing, so both were taken for a miss and the query could report clear. `Actor.Trace` refuses such bounds
  (review r1 of #633, item 2: "a primitive's bounds that are not finite and ordered (they cannot prove a miss)").
- **The rule:** `Bounds::is_sound` is true when every axis is finite and `min <= max`, `Actor.Trace`'s existing test,
  which now uses it too. In `actor_walk`:
  - only sound bounds that miss the region leave a primitive out before its filters are read;
  - a primitive the filters keep (the trace filter's unread rule would refuse it) with unsound bounds is refused with
    `Actor.Trace`'s text and record instead: `NotYet { NotRead }`, "<query prefix>: a primitive's bounds that are not
    finite and ordered (they cannot prove a miss) (the live actor primitive <actor>'s <part>)", on
    `NATIVE_ACTOR_TRACE.md`. That is the order `Actor.Trace` uses: its collide flags first, then the bounds;
  - a primitive the filters leave out is left out whatever its bounds, as before;
  - a stand-in part (decision 2026-11-25-18) is not ruled out by unsound bounds, as in `Actor.Trace`, and gets the
    stand-in refusal.
- **Tests (synthetic):** NaN and inverted bounds in the spawn's box test and in the touch's line walk. A primitive the
  filters keep is refused with `Actor.Trace`'s record and text, and nothing is made. One whose `CollideActors` is clear
  still lets the spawn be made. A stand-in part with unsound bounds gets the stand-in refusal. The existing walk tests
  pass unchanged.
- **Base:** stacked on fix/rust-unify-standin-rule `f12b4e99` (#683 rebased on integration candidate c15, plus the
  stand-in unification).
