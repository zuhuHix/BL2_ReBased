## 2026-11-16: Rust Phaselock chains: Object.PathName and the player-controller context query

AI-assisted (Claude Code). Lane FIX-PHASELOCK-CHAIN takes Maya's Phaselock duration and cooldown chains past the two
stops decision 2026-11-16-62 left: the Core native `Object.PathName` and the world's handler for the player-controller
context query. No parsing behaviour changes. There was no real-game session and no native code was read: the
player-controller rule is the one NATIVE_ITEM_RULES.md already records, and `PathName` is a stock UE3 native. **Every
rule here is UNVERIFIED in the running game** unless a line says otherwise.

**Files.** `crates/ow-vm/src/path_name.rs` (new) with one registration line in `natives_core.rs` and the module in
`lib.rs`; `crates/ow-attr/src/context_resolvers/player.rs` and `player_tests.rs` (new), `balanced.rs` (the interface
test made a shared function), `mod.rs`, `tests.rs` (the test module); `crates/ow-attr/src/eval.rs` (`ensure_loaded`)
and `value_resolvers/tests.rs`; `crates/ow-world/src/query.rs` (the handler) and `world.rs` (its registration); the
correction in `docs/verification/NATIVE_ACTION_SKILL.md`.

**`Object.PathName(object)`** (`ow-vm`, both parity modes: the C++ VM has no binding for it, so there is no stand-in).

- The rule, public UE3 semantics and not a Gearbox rule: the names of the object's outer chain, outermost first,
  joined by `.`, except that `:` goes before an object whose outer is not a package while that outer's outer is one
  (the subobject delimiter). A package is an object of class `Core.Package`.
- The `:` case is what the recorded engine dump shows for a level's actors (`<Map>.TheWorld:PersistentLevel.<Actor>`,
  BLCMM_DUMP_CROSSCHECK.md). Its use for a data subobject (`...Att_X:Resolver_0`) follows the same rule and is
  UNVERIFIED: no record holds a live read of one. A plain definition (`GD_Siren_Skills.Misc.Att_Phaselock_Duration`)
  reads as the stock paths of the records.
- An object read from a package uses that package's export and import tables (the file's own package is not part of a
  cooked object's path); an object made at run time uses the VM's outer links; a package with no outer is its name.
  Any other object with no outer, and None, are `NotYet { NotRead }`: the game's transient package and levels are not
  modelled, and the answer for None is not read.
- The callers on the duration chain (`WillowPawn.GetInstancedDesignerAttribute`, `InstancedDesignerAttribute.Initialize`)
  store the string and compare it with the same call on the same definition, so the chain does not depend on the
  unconfirmed case.
- Test: `path_name::tests::the_path_name_joins_the_outer_chain_with_the_subobject_delimiter` (both modes).

**The player-controller context query** (`EngineQuery::PlayerController`, which `PlayerControllerAttributeContextResolver`
asks; `World::new` now registers a handler for it). The rule is NATIVE_ITEM_RULES.md's (read from the executable by an
earlier lane), implemented in `ow_attr::context_resolvers::player_controller_of` and run by `ow-world`'s handler:

| The source | The answer |
|---|---|
| implements `WillowGame.IPlayerBehavior` | the source |
| else implements `WillowGame.IPlayerMaster` | `NotYet { NotPorted }`: the controller its `GetPlayerMasterPlayerController` returns is not run here; no class of the base code packages reaches this row (the record names only `WillowPlayerController`, which implements both) |
| else | None |

The interface tests are the class index's (`implements_interface`, the test `BalancedActor` already used, now shared;
a native stand-in ancestry is `NotYet`, no store is `NotYet { LayerAbsent("ow-defs") }`). Behaviour change for both: a
store with no package that could hold the interface (an empty one) now has no implementer, as a store whose packages
lack the class already had, where it was a store error. Both interfaces were checked
as class exports of the installed `WillowGame.upk`, and `IPlayerMaster` declares one function,
`GetPlayerMasterPlayerController`. The C++ refuses this resolver, so both parity modes run the rule. Tests:
`player_tests::the_player_controller_rule_in_both_modes` (an invented `WillowGame.upk` with both interfaces),
`query::tests::the_player_controller_handler_runs_the_rule_and_refuses_other_queries`.

**A stand-in the store does not load** (`ow-attr` `eval.rs` `ensure_loaded`). After the store's load, a resource handle
that holds no field while its class has some (an object inside a class default object is one; the store leaves those
stand-ins, decisions 2026-11-16-37 and -40) is now `NotYet { NotRead }` naming it, where reading it was a raw heap error
("has no slot"). Behaviour change: every attribute read that reached such a stand-in gave `AttrError::Heap` and now
gives that `NotYet`. Test: `value_resolvers::tests::an_unloaded_definition_needs_the_store` (updated).

**NATIVE_ACTION_SKILL.md** called `GD_Siren_Skills.Misc.Cooldown_Phaselock` a designer attribute; the installed data
make it an `InventoryAttributeDefinition` with a player-controller context resolver and a constant value resolver
(decision 2026-11-16-62's finding). Corrected in place.

**Verified (automated, local, pc-hummer, on `origin/main` e002de50).** `cargo build --workspace --locked` clean; `cargo
test --workspace --locked` 46 binaries, 1,704 passed, 0 failed, 25 ignored; `cargo build --release -p ow-cli --locked`
clean; `tests/crate_edges_test.py` OK; `decisions_index.py --check` OK; `tests/rust_parity.d/combat.txt` 3 of 3. The
`ow-cli` comparisons with the C++ binary are unchanged, and W6-COMBAT's 15 installed steps give the same rows (Reference
20 of the C++'s 29, all equal; Off 0).

**The installed game** (W6-SKILLS' ignored installed test, both modes; outputs `local/p2/w6-skills/maya_installed*.txt`;
the comparison is with the Reference run, the C++ evaluator's reading, and the recorded values PHASELOCK_STOCK_DATA
gives: lock 5 at grade 0, cooldown 13):

| Chain, source | Reference | Off before this lane | Off after |
|---|---|---|---|
| `Att_Phaselock_Duration` and `LockDurationFormula`, the pawn | 5 and 5 | stops at `Object.PathName` | stops at the value resolver (below) |
| the same, the controller | 5 and 5 | None, 0 | None, 0 (the controller is no designer-attribute provider) |
| the cooldown pool's `BaseMaxValue`, the controller | 13 (was: stopped at the query) | stops at the player-controller query | **13**, as the record |
| the same, the pawn | 0 (was: stopped) | stops at the query | 0 (a pawn implements neither interface: no context, the constant stands) |

The cooldown maximum now resolves in Off, from the player's controller, where the game keeps the cooldown pool, and
equals the recorded 13. The duration in Off gets past `PathName` and the designer script and stops at the attribute's
value resolver, `Default__DesignerAttributeDefinition.ValueResolver`: a subobject of the designer class's default object,
which the definition store does not load, so its `PropertyName` cannot be read (`NotYet`).

**Open items.**

- The store loading subobjects of a class default object (W3-DEFS' rule, decisions 37 and 40): the next stop of the
  Off duration chain. With it, the duration's value would come from the instanced designer attribute's `Value`.
- `IPlayerMaster`'s call (`GetPlayerMasterPlayerController`) for a class that is an `IPlayerMaster` and not an
  `IPlayerBehavior`: none in the base packages; not checked in the DLC packages.
- A live read of `PathName` for a data subobject would confirm the `:` case.
