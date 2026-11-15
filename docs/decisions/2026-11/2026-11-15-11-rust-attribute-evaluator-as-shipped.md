## 2026-11-15: Rust attribute evaluator as shipped

AI-assisted (Claude Code). Lane W3-ATTREVAL of the Rust port fills `crates/ow-attr/src/{eval,expr}.rs` and
`crates/ow-attr/src/value_resolvers/`: the chain walks (`ResolveContext`, `ResolveContexts`, `GetValue` and
`GetBaseValue` with the override source tried first, `GetValueFromContext`), `EvaluateInitializationData`,
`EvaluateExpression(s)` and eleven value resolver classes. This is the one evaluator of architecture rule R2; it
replaces neither C++ evaluator yet (`src/progression.cpp`, `src/weapon_gen.cpp`), which stay the differential
references. Rules: NATIVE_ATTRIBUTES.md sections 1, 3, 6 and 11 and NATIVE_PROGRESSION.md section 1, read from native
code. **Every rule here is UNVERIFIED in the running game**; no real-game session ran. Hub decisions are marked "hub".

**Shipped.**

- Value resolvers registered by `value_resolvers::register` (what `AttributeSystem::standard` calls):
  `Engine.ObjectPropertyAttributeValueResolver` (the one writable class; its `storage` names a stack or a plain
  attribute), `GearboxFramework.ConstantAttributeValueResolver`, `SimpleMathValueResolver`,
  `ConditionalAttributeValueResolver`, `WillowGame.GlobalAttributeValueResolver`, `ManufacturerAttributeValueResolver`,
  `WeaponTypeAttributeValueResolver`, `PlayerSkillAttributeValueResolver` (through the new typed query below),
  `ResourcePoolStateAttributeValueResolver` and `AttributeSlotEffectAttributeValueResolver` (both `NotYet { NotRead }`:
  the pool-state test and the slot provider's answer are not read) and `BadassAttributeValueResolver`
  (`NotYet { LayerAbsent }`, the player profile). Every other class stays `NotYet { NotRead }`.
- `EngineQuery::PlayerSkill { definition }` (hub: granted, an additive variant of the `#[non_exhaustive]` enum): the
  skill object for the definition in the player's skill tree; the resolver reads its `Grade`, 0 for `Ok(None)`. The
  engine side answers `NotYet` until a later lane, so the resolver's result stays `NotYet`, through a typed query.
- The `Weapon_Is_<Maker>` name match of `weapon_gen.cpp` is replaced by `ManufacturerAttributeValueResolver`: the
  `D_Attributes.WeaponManufacturer.Weapon_Is_<Maker>` attributes are that resolver behind a weapon context.
- Definitions are read through `DefStore::ensure_loaded` (the W3-DEFS contract after its review r1, PR #307): only an `Origin::Resource` object can be an unloaded handle, so only those go to the store.
- Data drives the recursion, so each nested step enters a guard: a step already in progress with the same key, or
  nesting past 64 steps (`MAX_DEPTH`), is `AttrError::BadData`. Depth does not bound the work (a sub-definition named
  by all four formula terms costs about 4^n steps at depth n), so the guard also counts the steps of one outermost
  step and refuses the step past **10,000** (`MAX_WORK`) as `BadData`. Both budgets are Rust-only refusals of this
  project, not rules of the game; the C++ evaluators have neither. The guard is a per-thread stack, because resolvers
  re-enter through the public `AttributeSystem` methods, which carry no depth (the simulation is single-threaded,
  D18).
- Hub-granted test edits outside the lane's files: W2-ATTRAPI's `every_entry_point_is_not_ported_yet` no longer
  lists the eight entry points this lane fills; W2-VMAPI's `the_attribute_system_is_called_from_inside_a_native` now
  evaluates a synthetic attribute definition (a constant resolver, 2.5) from inside a native.

**Architecture 5.3 results.**

- `PowForm`: settled for the evaluator's fixtures. The re-expressed fixtures that take a power, the invented curve
  and health pool of `tests/mission_script_test.py`, pass under both forms. The others (the reward chain and skill
  points of that file, the scale and random-quantity rows of `tests/combat_test.py`, the `Weapon_Is_Acme` terms of
  `tests/weapon_gen_test.py`, the designer value of `tests/skill_stats_test.py`) take no power, so the form does not
  reach them; they run under the default. The live level curve (1,241 / 28,126 / 196,238 / 9,155,282) and health base
  (102.152, 212.6755, 638.886, 415,509.44) match under both forms, and both give the same bits under the two forms at
  every level 1..80 when run through definitions (`to_bits` loops). By 5.3's rule the default `DoubleThenSingle`
  stands. Real-game question
  21 stays open for the game's own form.
- `CombineForm`: nothing for this evaluator to settle. It reads a stack attribute's stored value and never combines
  a stack; W1-ATTRMATH's tests already show the VM 5F fixtures pass under the default `WeaponGen`, which the one stack
  this lane computes (the weapon_gen damage, 18.0) also uses. The recompute is W3-ATTREFFECTS's; question 22 stays
  open.

**Open numerics choice for the maintainer: `RoundingMode` 1 (`ATTRROUNDING_IntRound`).** The evaluator rounds it as
`weapon_gen.cpp` `Round()` does, `floor(x + 0.5)` (`ow_core::num::round_mode`), which architecture 5.3 names as the
attribute-initialization path's form. `progression.cpp` `restricted()` (the XP curve and health-base evaluator) calls
`std::round`, which rounds halves away from zero. The two differ only on negative halves (`-2.5` gives `-2` and `-3`)
and on the sign of a zero (`-0.3` gives `0.0` and `-0.0`); no fixture has such a value. `floor(x + 0.5)` is kept for
now. `NumForms` carries no rounding field; which form the game uses is UNVERIFIED.

**Random variance (hub: C++ behaviour, not a stand-in).** An enabled `RandomVariance` draws once from
`AttrWorld::rng(CrtStream::AttributeVariance)` and maps the draw as `progression.cpp` `randomBetween` does, in both
parity modes: `d = rand15 / 32768`; a real amount is `low + f32(d) * (high - low)`; an integer amount is
`ceil(low) + floor(d * span)` with `span = floor(high) - ceil(low) + 1` (both ends reachable), or `ceil(low)` when
the span is below 1. The bounds are evaluated before the draw. The record says only "a random amount between
`LowerBound` and `UpperBound` is added, integer-uniform when `bUseIntegerRandomization`"; the native mapping is
unread. UNVERIFIED.

**A definition with neither a formula nor a conditional keeps its base.** With neither `ValueFormula` nor
`ConditionalInitialization` enabled, no `BaseValueMode` step runs and the base stands (then the random variance,
scale, clamp and rounding apply), as `weapon_gen.cpp` does. `progression.cpp` refuses such a definition unless it is
random, and then combines `f = 0` by the mode; the two differ only for a non-zero base with mode 0. UNVERIFIED.

**Readings taken where the record or the two C++ evaluators are silent (each UNVERIFIED).**

1. The no-formula, no-conditional definition keeps its base (above).
2. Formula terms (in the order Multiplier, Level, Power, Offset), the clamp bounds and a conditional's values are
   evaluated with the same source and override as the data that names them.
3. Random variance uses `progression.cpp`'s mapping in both modes (above).
4. `SimpleMathValueResolver` follows the record, not `progression.cpp`: `Arg1Option` (0 the previous value, 1 the
   `Arg1Attribute` data) and a divide by `|arg| < 1e-8` gives 0 (the C++ tests an exact 0). Both data are evaluated
   with the context as their source, `Arg1Attribute` first; that order matters only when both draw random numbers.
5. `ConditionalAttributeValueResolver` does not consult `ValueExpressions.bEnabled`, as `progression.cpp` reads it
   (the flag is false in the shipped data that has entries; `tools/loot_pools.py` checks the reading against the
   editor's display shares).
6. `ObjectPropertyAttributeValueResolver` looks the property up over the context's whole class chain (real-game
   question 12); a property that is neither a stack nor a plain attribute reads 0 (the record); a property the class
   chain lacks is `NotYet { NotRead }` (the record is silent). The record gives "Modifier stack not found" for add
   and remove only, so a value read of a stack attribute whose companions are missing returns the stored number; the
   base read and `storage`, which need the companions, are `BadData` then.
7. The item's manufacturer is its `DefinitionData.ManufacturerDefinition`, matched by the manufacturer definition's
   object name (FName equality) against `AssociatedManufacturerName`; the native asks
   `WillowInventory.GetManufacturer()`, whose body is unread. The weapon type is
   `DefinitionData.WeaponTypeDefinition.WeaponType`. A context without `DefinitionData` is `NotYet { NotRead }`. The
   field and enum names were checked against the local reflection (names only).
8. An expression's right operand is not evaluated when the left one already made it false; this matters only for
   the random draw order.

Also as shipped: a None entry in a value chain is `NotYet { NotRead }` (the record says what a None entry does only
for add and remove); `AttributeDefinitionMultiContext` is `NotYet { NotRead }` in `ResolveContexts` and `GetValue`
(its expansion is not read); an out-of-enum byte is `BadData`.

**C++ stand-ins met, not reproduced.** `progression.cpp` matches `PlayThroughCount`, `NumberOfPlayers` and
`Targetable_Is_Champion` by path suffix, skips the GameInfo and Designer context resolvers, evaluates a
`DesignerAttributeDefinition`'s `BaseValue` instead of its chains, and implements the Noun, Random, Constraint,
Currency and AmmoDropWeight resolvers from unread bodies. The Rust evaluator runs the chains, and those resolver
classes are `NotYet { NotRead }`. No tracked test pins them against the Rust binary yet; the rows that depend on them
run through `--mission-run` and `--combat-run`, which W6 lanes gate.

**Passed to W3-ATTREFFECTS (hub).** `Storage` has no form for `BadassAttributeValueResolver`'s set-base (it writes the
rank, rounded half up), and `Option<Storage>` cannot tell "not an attribute" (`Refusal::NotAnAttribute`) from "not
writable".

**Open.** Real-data parity (the evaluator over the installed packages through `DefStore`) waits for the store's body.
Real-game questions 12, 21 and 22, and the rounding form above, are open.
