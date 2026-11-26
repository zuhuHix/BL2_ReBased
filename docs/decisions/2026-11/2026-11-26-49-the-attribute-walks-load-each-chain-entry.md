## 2026-11-26: The attribute walks load each chain entry

AI-assisted (Claude Code, Claude Opus 5.5). Queue item 0ao, lane IMPL-RESOLVER-CHAIN-LOADS, on main d3be3e5. Clean
room: implemented from docs/verification/NATIVE_TARGETABLE_RESOLVER.md section 5.1 (#787, lane F) and the hub's
rulings on #787 alone; this lane did not read the executable. No session was run. UNVERIFIED in the running game.

- **What was decided**, in `ow-attr`'s `eval` (both parity modes: it is the evaluator's load, not a rule of a mode):
  - the context walk (`resolve_context`), the value walk (`fold_chain`) and `preflight_read`'s walk over both chains
    load each non-None entry before they read or dispatch it, through `ow-attr`'s own `ensure_loaded`
    (`load_entry`). By fit, the game's package load has the entries resident with their attribute (section 5.1); the
    port's store is lazy, so the walk does it;
  - **load-free scopes are unchanged:** `ensure_loaded` goes through `load_free_refusal`, so inside `load_free` (and
    in `preflight_read_resident`, which loads nothing) an unloaded entry is the existing `NotRead` naming it;
  - **a failed load** leaves the handle in place, with no new refusal point: the walk's residency check (`resident`)
    answers the store's `BadData` ("… failed to load") where the entry is reached;
  - **step 0 stays** in `TargetableAttributeValueResolver` (decision 2026-11-26-44) as a guard for a get reached
    outside the walks;
  - **the count of loads:** a read that runs `preflight_read` first loads the definition and each of its N unloaded
    chain entries, 1+N where it was 1, and #518's `load_transitions` rises by as many (the doc of `preflight_read`).
- **One existing expectation changed:** a slotless stand-in that the store does not hold as a handle, met in a
  context chain, was the heap's `SlotMissing` from the resolver's own read; the walk's load now refuses it first, as
  `ensure_loaded`'s existing stand-in refusal does (`NotRead`, "stays a stand-in … its fields cannot be read").
  `context_resolvers::tests::an_unloaded_resolver_definition_is_loaded_through_the_store` asserts that now.
- **Tests** (`ow-combat` `hit/attribute/chain_load_tests.rs`, an invented `WillowGame.upk` whose chain entries are
  subobject exports of the attribute): the note's rows 1 to 6. Rows 1, 2 and 5: through `GetValue` both entries are
  loaded and the get answers 0.0 on a pawn, 3 load transitions (the attribute and 2 entries); row 3: a resident
  attribute's handle refuses in a load-free scope with nothing loaded, and is loaded outside (the attribute loaded by
  a direct `DefStore::ensure_loaded`); row 4: a None context entry is skipped; row 6: a resolver whose payload is cut
  short is `BadData` at the value walk, the handle left failed, after the context walk ran. With `load_entry` a no-op,
  all four tests fail.
- **Measured** (installed, Off, strict `--slice-gate`, release build, under the machine lock), main d3be3e5 and this
  branch: 7 ok / 7 not ok, X 58.3% on both.
  - Links 3, 4, 5, 13 and 14 move from step 0's "a resolver the store has not loaded" (prefixed) to
    `WillowPawn.SetDefaultPhysics` "past its gates" (NATIVE_MIND_ROUTE.md section 1: the two unnamed pawn queries and
    the navigation-handle query), with no partial-writes prefix. That is the stop NATIVE_TARGETABLE_RESOLVER.md section
    7 predicted; #788's probe expected `WillowMind.RecalculateAttributeInitializedState` (unbound), which these links do
    not reach yet.
  - Link 6 (`UpdateAmmoCounts`' `NotRead`) and link 7 (the HUD's pickup targeting) are unchanged; links 1, 2 and 8 to
    12 stay ok.
- **Verified:** the tests on invented data. **Not verified:** anything in the running game.
- **Sensitive files:** none.
