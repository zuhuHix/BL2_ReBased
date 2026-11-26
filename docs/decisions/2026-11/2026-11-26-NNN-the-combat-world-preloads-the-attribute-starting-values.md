## 2026-11-26: The combat world preloads the attribute starting values' definitions

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-SPAWN-SETBASEVALUE, on main c1058de. Clean room: implemented from
docs/verification/NATIVE_SPAWN_SETBASEVALUE.md section 3 (#811 at 3aedca6) alone; this lane did not read the
executable. No session was run. Every rule is UNVERIFIED in the running game.

- **What was decided** (Off only; Reference does nothing and logs nothing), in `ow-combat` `starting_values`, called
  from the combat world's set-up (`ow-cli` `combat::world`):
  - for every AI pawn archetype the run can spawn (`run::KINDS` with a pawn balance), a copy of the archetype's export
    (made as the spawn makes one, freed after) is walked: its `AIClass`, then the `AttributeParentClass` chain (each
    class loaded when it is a handle, after its declarations are checked), the starting values parents first and then
    the archetype's own list. Each entry's `Attribute`, and its `BaseValue`'s `BaseValueAttribute` and
    `InitializationDefinition` (read with the evaluator's own data reader), are loaded, with each attribute's
    `ContextResolverChain` and its `ValueResolverChain` up to the first None;
  - each load is logged as a harness input, "preload <path> (a SetBaseValue definition the game keeps resident;
    UNVERIFIED)", then its outcome. A failed load is logged and left, so `SetBaseValue`'s own refusal shows it later;
  - an undeclared field or one of another kind (`AIClass` and `AttributeParentClass` objects,
    `AttributeStartingValues` an array of structs, an entry's `Attribute` an object) is `VmError::Script`, decided
    before the class it is on loads. A class whose `AttributeStartingValuesNew` is declared and non-empty is
    `NotRead` ("the starting-value categories' layout"), decided before any definition loads (the note's open
    question 2 is unanswered, so the categories are not read);
  - **the hub's points on #811** (the fixed note to follow): (a) the archetypes are found at set-up from the run's own
    list, and a child's `AttributeParentClass` that is an unloaded handle is followed (loaded, logged, its lists
    read); (b) the walk covers what `SetBaseValue`'s preflight walks: every attribute it reaches (the destination, a
    base-value attribute, an expression operand) with its resolvers, and each initialization definition followed into
    the nested data its evaluation reaches (the enabled formula's four terms, else the enabled conditional's entries
    up to the first always-holding one and then `DefaultBaseValue`, the enabled variance bounds and range limits),
    recursively to the preflight's depth. A part that does not read as the evaluator reads it is left uncovered, and
    the native's preflight then refuses it with its own typed error; (c) the stop after `SetBaseValue` was the note's
    prediction; this lane's installed gate measured it (below);
  - a store that holds no such package (a toy world) has nothing to spawn and nothing to preload;
  - `SetBaseValue` is unchanged: its preflight still refuses an unloaded handle.
- **The note's open question 1** (set-up preload or a counted load in the spawn) was taken as the hub's brief says, a
  set-up preload, the threshold-attribute preload's shape. The note's later stops (sections 4 and 5) are not
  answered here: `ApplyGradeCustomizations` is #808's, and the rest need rulings or reads.
- **Tests** (an invented package, `starting_values_tests.rs`): the note's tests 1 to 6. They cover the walk (seven
  objects loaded, parents first, the value resolver after the None left, a second run loading and logging nothing),
  the archetype's own list, a mistyped list and a missing `AIClass` with nothing loaded, a non-empty category list
  refused before any definition loads (and an empty one passing), a failed load logged with the walk going on, and
  Reference doing nothing; and, for the hub's points, an unloaded parent class followed, and a nested formula term and
  conditional (an operand and a `BaseValueIfTrue` loaded; the entry after an always-holding one and the default not
  reached, not loaded). **Not tested:** test 7 (the effect through a spawn); the installed gate shows it.
- **Measured:** see the GATES row of this lane.
- **Verified:** the tests on invented data. **Not verified:** everything in the running game.
- **Sensitive files:** none.
