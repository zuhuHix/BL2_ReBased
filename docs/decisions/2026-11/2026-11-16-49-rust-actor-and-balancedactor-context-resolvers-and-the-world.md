## 2026-11-16: Rust Actor and BalancedActor context resolvers and the world's game info

AI-assisted (Claude Code). Lane FIX-ATTR-CONTEXTS closes the three gaps that stopped installed data after decisions
2026-11-16-37, -40 and -43: `GearboxFramework.ActorAttributeContextResolver` and
`Engine.BalancedActorAttributeContextResolver` in `ow-attr` (`crates/ow-attr/src/context_resolvers/`, registered in
`context_resolvers::register`, decision 2026-11-15-12's layout), and the game info of the Vm world in `ow-world`
(decision 2026-11-15-26). No parsing behaviour changes. **Every rule here is UNVERIFIED in the running game**; no
real-game session ran in this lane, and no native code was read (no clean-room note was needed).

**Class paths (installed code packages).** Read from the export tables: `ActorAttributeContextResolver` is a top-level
class of `GearboxFramework.upk` and `BalancedActorAttributeContextResolver` one of `Engine.upk`, both direct subclasses
of `Engine.AttributeContextResolver`; the balanced-actor interface is `Engine.IBalancedActor` (a top-level class of
`Engine.upk` whose super class is `Core.Interface`). NATIVE_CONTROLLER_HELPERS.md and NATIVE_INVENTORY_SORT.md already
name `IBalancedActor`; decision 2026-11-15-12's "its interface class is not recorded" is superseded by that.

**Where each rule comes from.** The records first, then the C++, then the script; the executable was not needed.

- **Actor: the class script.** NATIVE_ATTRIBUTES.md lists the class under "Not read yet", and the C++ evaluator refuses
  it ("unsupported attribute context resolver", `src/progression.cpp`). Its one function, `GetAttributeContext`, is
  script (it decodes exactly with `research/script_disasm.py`; the output stays under ignored `local/`). In own words:
  it returns the source cast to `Engine.Actor`. So an actor of any class is itself, and anything else, a None source
  included, is None. The port runs that cast without the VM (`context_resolvers::Actor`, in `actors.rs`); an ancestry
  through an engine-native stand-in class is `NotYet { NotRead }`, as every other `is_a` there.
- **BalancedActor: the record.** The class's `GetAttributeContext` is native (its function flags). NATIVE_ATTRIBUTES.md
  section 5 gives the rule (confidence medium-low): the source if it implements the balanced-actor interface, else None.
  The port (`context_resolvers::BalancedActor`, new file `balanced.rs`) asks the class index's interface test,
  `CodeIndex::implements`, the one `Vm::implements` and the VM's `InterfaceCast` use (the interface table of the class
  or of a super class names the interface or one derived from it). Following that contract: a class with no package
  export behind it implements nothing, and a "no" over an engine-native stand-in is `NotYet { NotRead }`. The test
  needs the code packages, so a world without a definition store is `NotYet { LayerAbsent("ow-defs") }`; a store whose
  packages hold no `Engine.IBalancedActor` has no implementer (None). The interface test itself is the C++'s
  (`Runtime::implements`), but the C++ has no rule for this resolver.
- **No Reference stand-in.** The C++ refuses both classes, so no tracked parity test pins a C++ reading of them
  (architecture R3). Both parity modes run the rules above, with the same answers.

**The game info (`ow-world`, the smallest additive seam).**

- `WorldQuery::follow_world_info(actors)` is new, and `World::new` calls it with the world's actor list. With no game
  info set (`WorldQuery::set_game_info`), the hooks answer the `Game` of the `WorldInfo` placed in that list. A set game
  info still wins. No `WorldInfo` placed, or a None `Game`, is still `NotYet { LayerAbsent }`. A `WorldInfo` class without
  an object property `Game` is `BadData`, and a borrowed actor list is an `AttrError::World`.
- Why `WorldInfo.Game`: it is where the engine keeps its game-info object, and where both combat worlds put one (the C++
  `CombatSystem::enableDeaths` in `src/combat_death.cpp`, and `ow_combat::death::enable_deaths`, which `Run::new` calls).
  NATIVE_ATTRIBUTES.md section 5 says the resolver answers "the game info" (confidence medium); that it is this object is
  UNVERIFIED in the running game.
- Decision 2026-11-15-12's candidate row stands: the C++ evaluator keeps the previous object at
  `GameInfoAttributeContextResolver` (`src/progression.cpp:87`) where the port gives the game info. In a Vm world the port
  now reaches the game info instead of stopping, which exposes one consequence below.
- No file of `ow-combat` or `ow-mission` changed.

**Verified (synthetic, AGENTS.md rule 9).** `ow-attr`: `actor_is_the_source_cast_to_actor` (the three chain rules, every
actor kind of the scene, a None source, the next step getting the actor, the native stand-in), and four tests in
`balanced_tests.rs` over an invented `Engine.upk` with an interface table: own table, a super class's table, a derived
interface, a plain class, a hand-defined class, no store, no interface class, a native stand-in, each in both modes where
the mode could matter. The registration test now lists Actor among the registered classes. `ow-world`: two query tests
(the fallback, no `WorldInfo`, a None `Game`, a set game info winning, the globals not following, bad data, a borrowed
list, a query that follows nothing) and one world test (`World::new` links the query to its actor list).

**Installed game (local output under `local/p2/fix-attr-contexts/`, no game data here).**

- **Sweeps.** The FIX-PLAYTHROUGH-OFF harness (every export of exact class `Engine.AttributeDefinition`, 708, and
  `Engine.AttributeInitializationDefinition`, 416; the offline world's player controller, then a bare `WillowMind`, as
  the source; Normal playthrough), before (`origin/main` 07f07b29) and after:

  | Mode, source | Attributes with a value | Init definitions that evaluate | Stops naming either class |
  |---|---|---|---|
  | Reference, controller | 328 to 329 (no context 169 to 184) | 170 to 194 | 20 + 38 to 0 |
  | Reference, mind | 306 to 307 (no context 165 to 180) | 168 to 192 | 20 + 38 to 0 |
  | Off, controller | 316 to 317 (no context 169 to 184) | 143 to 166 | 20 + 30 to 0 |
  | Off, mind | 294 to 295 (no context 165 to 180) | 141 to 164 | 20 + 30 to 0 |

  Every BalancedActor stop now evaluates: neither source implements the interface, so the chain gives no context. The
  init definitions that stopped at Actor (14 in Reference, 7 in Off) now stop one step later, at an object-property read
  of `ClonedForSharing` that neither source declares. That is an item's property, and these sources are not the
  objects those definitions are written for. A local check over one instance per class (both modes) gives the
  interface to `WillowPawn`, `WillowAIPawn`, `WillowPlayerPawn`, `WillowVehicle`, `WillowWeapon`, `WillowItem`,
  `WillowInteractiveObject` and `MissionDefinition`, and not to `Engine.Actor`, `Engine.Pawn`, the player controller,
  the mind, the player replication info or the game info. `MissionDefinition` is not an actor, so Actor answers None
  there. This agrees with NATIVE_PROGRESSION.md, which reads a mission's game stage through the balanced-actor
  interface. Decision 40's harness, which counted 7 + 1 such init stops, is not in this machine's tree, so its numbers
  are not rerun.
- **W6-COMBAT's 5 installed spawns** (its 15 steps, `--combat-run`):
  - Reference, before: all 5 stop at "the game info object of this world", 0 rows.
  - Reference, after: all 5 spawn, 20 rows. 18 equal the C++'s rows (W6-COMBAT's local C++ output of the same 15 steps):
    every health and max health, the three base values, both arrival values and the grenade hit on the dummy.
    `spawn:dummy:8` succeeds; the other four spawn and then stop at their loot setup (W6-LOOT's layer), as W6-COMBAT
    planned. Of the C++'s 29 rows, 9 are missing: the adult's hit and kill (the kill credit stops at W6-COMBAT's own
    `EffectiveNumPlayers` 0 item) and the loot rows (W6-LOOT).
  - 2 rows differ: `ratio.PrimalBeast.L5` and `ratio.PrimalBeast_Ranged.L5` are 0 here and 0.715 in the C++. The swing's
    AI damage scale reads the attribute `D_Attributes.GameProperties.NumberOfPlayers`, whose chain is the game info
    resolver and an object-property read of `EffectiveNumPlayers`. The port now reads it from the run's `WillowGameInfo`,
    which keeps its class default 0 because this run has no `players:` step. The C++ never reads that chain: it answers
    a fixed 1 for any condition attribute whose path ends in `NumberOfPlayers` (`src/progression.cpp:298`, a stand-in).
    With 0, no condition entry matches and the scale is 0. A diagnostic run with `players:1` after `player:8` gives no
    differing row (16 rows, all equal to the C++). Its swings then stop at
    `WillowGame.TargetableAttributeValueResolver` (the `Targetable_Is_Champion` condition, where the C++ stands in
    "not a champion"), and the adult's kill stops at the destroy routine's controller and player case.
  - Off: unchanged, all 5 spawns stop before any attribute, at the archetype reference
    `Default__WillowAIPawn.SpecialMoveComponent0` ("inside a class default object"), as decisions 37 and 40 found.
- **W6-LOOT's bullymong death drops:** not run, because W6-LOOT (PR #340) has not landed on `origin/main`.

**Open (outside this lane's files; not edited here).**

- The 2 differing ratio rows need one of two owners' calls. Either the run's game gets the player count the game would
  have (the login that sets `EffectiveNumPlayers`, or a `players:1` default in the run), or a Reference stand-in "a
  `NumberOfPlayers` condition operand is 1" in `ow-attr`'s condition evaluator, pinned and listed in architecture
  section 6.1. The second also needs a stand-in or a port for `TargetableAttributeValueResolver`
  ("not a champion").
- W6-COMBAT's kill credit with `EffectiveNumPlayers` 0 (decision 2026-11-15-35) is the same root. The destroy
  routine's controller and player case is the next stop after it.
- The `ClonedForSharing` stop is a harness-source artifact. A sweep with an item as the source would exercise those
  definitions.
- Real-game checks that would settle the rules: a live read of `GetAttributeContext` for both classes on a pawn, a
  weapon, an item and the player controller, and of which object `GameInfoAttributeContextResolver` answers.
