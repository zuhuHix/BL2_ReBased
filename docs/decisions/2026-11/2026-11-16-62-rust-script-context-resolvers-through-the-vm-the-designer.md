## 2026-11-16: Rust script context resolvers through the VM: the designer resolver, its C++ stand-in, nine script classes

AI-assisted (Claude Code). Lane FIX-DESIGNER-RESOLVER gives `ow-attr`'s context resolvers whose rule is script a way
to run it: one additive method on the `AttrWorld` trait, implemented by the VM. It is the doc follow-up of W6-SKILLS
(decision 2026-11-16-42, contract item 2). No parsing behaviour changes. There was no real-game session and no native
code was read (no clean-room note was needed: every rule below is the installed script or the C++ reading). **Every
rule here is UNVERIFIED in the running game.**

**Files.** `crates/ow-attr/src/api.rs` (the trait method), `crates/ow-vm/src/api.rs` (its VM body) and
`ow-vm/src/interp/tests.rs`; `crates/ow-attr/src/context_resolvers/{mod.rs,pending.rs,tests.rs}`;
`crates/ow-attr/src/eval.rs` and `value_resolvers/{mod.rs,designer_tests.rs}` (the value-side stand-in); W6-SKILLS'
ignored installed test `crates/ow-skills/src/phaselock/installed.rs` (both modes, and the chains part).

**The seam.** `AttrWorld::script_context(resolver, attribute, source)` runs the resolver class's script
`GetAttributeContext(Attribute, AttributeContextSource)` and returns its object. The default answers
`NotYet { LayerAbsent("ow-vm") }`, which is what every world that runs no script gives (an offline world, the
roll's adapter `ow_items::RollWorld`, which is not changed and so answers that `NotYet` during a roll, unit-test
worlds). `ow-vm`'s `impl AttrWorld for Vm` finds the function by virtual lookup on the resolver's class and calls it with
the two arguments by parameter name, compared without case (the name table keeps the first spelling it interned;
review r2 of PR #360). A function that lacks either name is `NotYet { NotRead }`, never given a zero (reviews r1 and
r2): the engine binds by position, so an override with other names is valid script, but this binding does not guess
the order. A class with no such function, or a native one, is `NotYet { NotRead }`; an answer that is not an object
reference is bad data; whatever the script reaches answers as the VM answers any call. **Under a permissive stub
policy** (`LogAndZero`, which the CLI runs that use it set: the Reference runs, and the mover, census and VM tools in
both modes) an unbound native inside a context script gives its zero value, so the
chain may end as None and the attribute keep its constant; under the strict policy (`Refuse`) the same call is an
error, which reaches the caller as `AttrError::World`. Section 6.1 of ARCHITECTURE.md lists the two Reference-only
stand-ins of this lane. This is
the same pattern as `attribute_changed`, which already runs script through the world. No crate edge changes
(`ow-vm` already depends on `ow-attr`); `tests/crate_edges_test.py` passes.

**The designer resolvers** (`WillowGame.DesignerAttributeContextResolver` and `...ByName`).

- The rule is the class script (decoded locally with `research/script_disasm.py`; output under `local/` only). In own
  words: it asks the source for the designer-attribute provider interface, or, for an actor without it, the source's
  owner; when there is a provider and the attribute is a designer attribute, the answer is the provider's instanced
  designer attribute for that definition, else None. That agrees with NATIVE_ATTRIBUTES.md section 5.
- `ParityMode::Off`: the script runs. `ParityMode::Reference`, the plain form only: the C++ stand-in, which treats the
  class as always resolving and keeps the object the chain held before it (`src/progression.cpp:85-87`, the 6.1
  stand-in this lane was asked to keep). That step never ends the chain in the C++, even on null; so in Reference a None
  source falls back to the engine globals object, as `NoContextNeeded` does, and the value-side stand-in below still
  gives the `BaseValue` (review r1 of PR #360; test
  `a_designer_base_value_attribute_with_a_none_source_gives_its_base_value_in_reference`). The C++ has no reading of
  the `ByName` form (it refuses the class), so that form runs the script in both modes. In Off, and for `ByName`, a
  None source is None. The context resolver does not know whether it serves a read or a write, so in Reference the
  globals fallback also steers a set-base or a modifier write through a designer chain with a None source onto the
  globals object (`effects.rs`); the C++ has no such write, and no installed path reaches it (review r2 of PR #360).
- **The value-side stand-in, Reference only.** The C++ also answers a `WillowGame.DesignerAttributeDefinition`'s value
  as its own `BaseValue`, evaluated on the context (`src/progression.cpp:117`). Without it, the Reference context (the
  previous object) would be read by the definition's value chain, which reads the designer object, and the Reference
  answer would not be the C++'s. So `ow-attr`'s value fold does the same in Reference (`eval.rs`); Off reads the value
  chain. Test: `designer_tests::a_designer_attribute_is_its_base_value_only_in_reference_mode`.

**Nine more script classes.** Of the ten context resolver classes `ow-attr` listed as unread, nine have a script
`GetAttributeContext` in the installed code packages (decoded; flags Defined and Public, not native):
`Engine.ProjectileAttributeContextResolver`, `Engine.VehicleAttributeContextResolver`,
`GearboxFramework.PopulationMasterAttributeContextResolver`, `WillowGame.WillowInventoryManagerAttributeContextResolver`,
`WillowGame.WillowInteractiveObjectAttributeContextResolver`, `Engine.OwnerAttributeContextResolver`,
`Engine.GameReplicationInfoAttributeContextResolver`, `GearboxFramework.AIComponentAttributeContextResolver` and
`WillowGame.InventoryDefinitionAttributeContextResolver`. They are now registered as one `Script` resolver (the list
`SCRIPTED`) that runs the script through the world in both modes, a None source included: the C++ refuses all nine, so
there is no stand-in to keep. `WillowGame.CurrentProficiencySkillAttributeContextResolver` has no script function (the
record calls it native) and stays unread. `WillowGame.SkillAttributeContextResolver` has no script function either and
keeps its `NotYet`.

**Verified (automated, synthetic, AGENTS.md rule 9).**

- `ow-attr`: the designer forms in both modes (the script's call and answer, the C++ stand-in in Reference, the ByName
  form's script in both, a None answer ending the chain, a None source), the nine script classes in both modes, the
  C++ comparison test (Reference now gives the C++ answer), and the value-side stand-in in both modes.
- `ow-vm`: `a_script_context_resolver_runs_its_get_attribute_context` on an invented package (a script that returns its
  source, one that returns None, one that returns an int, a native function, no function), both modes.
- Gates (pc-hummer, on `origin/main` 40eb4fb4, after W6-COMBAT-RUN2): `cargo build --workspace --locked` clean;
  `cargo test --workspace --locked` 45 binaries, 1,652 passed, 0 failed, 22 ignored; `cargo build --release -p ow-cli --locked` clean;
  `tests/crate_edges_test.py` OK; `decisions_index.py --check` OK.

**The installed game** (W6-SKILLS' ignored installed test, extended to run in both modes and to evaluate the chains
through `ow-attr` from Maya's pawn and from her controller; outputs `local/p2/w6-skills/maya_installed*.txt`). There is
no C++ command that evaluates an attribute, so the comparison is with the C++ evaluator's reading of these two
definitions (`src/progression.cpp`) and with the values the records give (PHASELOCK_STOCK_DATA, decision 42).

- `GD_Siren_Skills.Misc.Att_Phaselock_Duration` (a designer attribute, base 5) and the Phaselock archetype's
  `LockDurationFormula`, which reads it:
  - Reference: 5 and 5, from the pawn and from the controller, and the game's own `EvaluateAttributeInitialization`
    on the pawn gives 5.0. The C++ reading gives 5 (the base); the record's lock time at grade 0 is 5.
  - Off, from the pawn: the designer script now runs. It finds the pawn's provider interface and calls the pawn's
    `GetInstancedDesignerAttribute`, which stops at the unread Core native `Object.PathName` (`NotYet`): the next
    blocker, as decision 42 found when it called that function directly. Off, from the controller: the controller is
    no provider and has no owner, so the script answers None and the formula keeps its constant, 0. That is the
    script's answer for that source, not a game number; the game's provider is the pawn.
  - Before this lane, both modes stopped at the designer resolver (`NotYet { LayerAbsent("ow-vm") }`).
- The cooldown maximum (`ActiveSkillCooldownPool_Siren.BaseMaxValue`, the attribute `Cooldown_Phaselock`): **not a
  designer attribute** in the installed data. It is an `InventoryAttributeDefinition` with a
  `PlayerControllerAttributeContextResolver` and a constant value resolver (NATIVE_ACTION_SKILL.md calls it a designer
  attribute; the data say otherwise, a record correction for a later docs pass). Its context resolver is native (no
  script function), and the world's handler for the player-controller query is not built ("the engine rule is not
  read"), so it stops there in both modes, before and after this lane. This seam does not reach it.

- W6-LOOT's bullymong run (`--combat-run`, decision 41's steps; the C++ prints 180 rows), Reference: before this lane
  30 rows (26 equal to the C++), after 33 (29 equal); the `rollpool` calls that stopped at the designer resolver now
  pass it, with the C++ answer. The 4 that differ are those decision 54 names: the two batch counts (each batch kill
  stops at `AmmoDropWeightAttributeValueResolver`) and the two `Pool_Money_1or2` item counts, which follow the random
  stream (the C++'s kills and rolls draw from it in a different order here). Off: 0 rows before and after (every spawn
  stops at the archetype reference `Default__WillowAIPawn.SpecialMoveComponent0`).

**Open items.**

- `Object.PathName` (Core): the Off duration chain stops there. Not in any record and not in the C++; reading it is
  native analysis (clean room) or a live read.
- The player-controller context query (`EngineQuery::PlayerController` in `ow-world`), which the cooldown maximum needs.
- `GameInfoAttributeContextResolver`: the C++ also keeps the previous object there; the port answers the game info in
  both modes (decision 2026-11-15-12's candidate row). Not changed here: the combat run's swings read
  `NumberOfPlayers` through it.
- The nine script classes' installed behaviour is not compared with anything (the C++ refuses them); their answers are
  the installed script's as run by the VM.
- `ow_items::RollWorld` does not forward `script_context`, so a roll over the VM answers `NotYet` at a script
  resolver; whether a roll may run script (it could draw from the VM's generators) is the same question as its
  `attribute_changed` rule.
- Decision number: the index proposed 58; 62 is used because the numbers between may be reserved by lanes in flight.
