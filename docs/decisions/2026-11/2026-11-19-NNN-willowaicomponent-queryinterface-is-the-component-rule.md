## 2026-11-19: An AI component's `QueryInterface` is `ActorComponent`'s rule, and `PhaseLockOnHold` reads its holds

AI-assisted (Claude Code). Analyst lane ANALYST-WILLOWAICOMPONENT-QUERYINTERFACE-IFLAGPROVIDER (link 9).
Documentation only: no code changed, and the lane implements none of it (clean room). **Every rule is UNVERIFIED in
the running game.**

- **The note:** `docs/verification/NATIVE_WILLOWAICOMPONENT_QUERYINTERFACE.md`, with a `NATIVE_INDEX.md` row. It
  extends NATIVE_PAWN_AI_INTERFACE.md section 6, which now points at it.
- **What was read** (local Ghidra, read-only, under the machine lock; raw output under ignored `local/analysis/`):
  - `AIComponent` and `WillowAIComponent` use `ActorComponent`'s `QueryInterface` body: the plain answer, then the
    `Owner`'s. For `IFlagProvider` the component lists nothing, so the answer comes from its `Owner`, for a mind's
    component the mind, a flag store;
  - `WillowAIComponent.PhaseLockOnHold()` is true when the component's `PhaseLockHolds` list is not empty;
  - neither writes, loads or draws.
- **A port gap found:** the port's spawn leaves a spawned actor's components with `Owner` None (it does not run the
  attach step). A mind's AI component then answers none, and link 9's cast is blocked with no stop.
- **Proposed Off answer for a separate implementer:**
  - add the two AI component classes to `ow-combat`'s component rule's exact-class list;
  - a new read-only `PhaseLockOnHold` native, with its refusals;
  - for the hub to choose: a NotRead refusal when `Owner` is None and the component is not attached, recommended, or
    the faithful none.
  Reference is unchanged. The tests are in the note's section 4.
- **Local probe (never pushed):** the link 9 stack with #524 and a hand-merged #523, plus probe-only stubs of #589,
  #594 and this note. Installed Off strict `--slice-gate`: 2 ok / 11 NotYet / 1 error.
  - Link 9 ran both steps and is "no evidence", because the component's `Owner` is None.
  - With a probe-only stand-in for `Owner`, link 9 is an error at the unbound
    `AttributeDefinitionBase.ResolveContext(object)`, the next stop.
- **Live check (bl-sdk; not run, no session granted):** the note's last section.
