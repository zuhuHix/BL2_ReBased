## 2026-11-19: GotoState('Auto') in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-AUTO-STATE, implementer. The decision number is pending from the hub
(three digits); NNN is a placeholder.

- **Sources:** `docs/verification/NATIVE_AUTO_STATE.md` section 5 alone (#540 at 7bcb12f; analyst lane ANALYST-AUTO-STATE,
  not this thread), with the hub's ruling from #540's review r1. No native code or analysis output was opened.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged:** no dependency was added and no bounds check was loosened.

**What changed** (`crates/ow-vm/src/states.rs`, Off only):
- **`auto_state`, the note's item 1:**
  - a class whose own state flags have bit 1 (value 2, `STATE_AUTO`) set gets no state, and `GotoState` stays silent as
    for `None`;
  - otherwise the chain is walked from the class to the root, each class's own declared states looked at, and the first
    class holding a state with the bit set gives that state.
- **The two typed refusals of item 2**, each `NotYet { NotRead }` with record NATIVE_AUTO_STATE.md:
  - more than one auto state in one class of the chain;
  - the census oracle broken, with the class bit set and an auto state in the chain, or (the hub's ruling) the class bit
    clear and none found: "GotoState('Auto'): class flags and state flags disagree".

  Both are raised before any write: `goto_state` resolves `Auto` before it makes the object's state frame.
- **Reference:** unchanged. This module's `GotoState` is bound outside Reference only, and the earlier approximation is
  kept for any other caller.
- **Not done here (item 3):** the state-code step on the map route. `World::set_state_step` is left alone; the
  map-load lanes (H6 owns `mapload.rs` and `bringup.rs`) install it when they build the world.

**Tests** (synthetic package):
- a class flagged auto (no state, silent, `EndState(None)` from a previous state);
- a class's own auto state;
- a child without one taking the parent's;
- a child's own auto state winning;
- a child's same-named non-auto state skipped for the parent's auto state export (not in installed data);
- the two refusals, with the heap unchanged;
- the auto state's `BeginState` raised inside `SetInitialState` after `PostBeginPlay`, and its `Begin` code run only at
  the next state-code step.

Two earlier tests are replaced, since their expectations (no state for a clear class bit with no auto state; a refusal
for a flagged state) are the old approximation.
