## 2026-11-19: Inventory context resolvers and ItemActors in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-INVENTORY-CONTEXT. Coded from
docs/verification/NATIVE_INVENTORY_CONTEXT.md alone (clean room: lane ANALYST-INVENTORY-CONTEXT wrote the note,
decision 2026-11-19-59). The executable and the analysis output were not opened. Synthetic fixtures only.

- **`InventoryAttributeContextResolver` (`ow-attr`).** The note says its `GetAttributeContext` is script (section
  2), so in `ParityMode::Off` the resolver runs the class's own script through the world
  (`AttrWorld::script_context`), a None source included. The rule is the script's, not restated in Rust. What the
  script reaches answers as the VM answers any call. Reference keeps what it did before this lane: `NotYet { NotRead }`
  for a source, None for a None source.
- **`WillowInventoryManager.ItemActors` (`ow-inventory`, Off only).** It is an iterator over the manager's
  `ItemChain`, walked from its head through each entry's `Inventory` link, in chain order (section 3).
  - An entry is given when its class is `BaseClass` or below it.
  - With `bOnlyReadied` set, the entry also needs `bReadied`.
  - Skipped entries are counted over the whole walk, and the walk ends after more than 100 of them.
  - A None `BaseClass` is `NotYet { NotRead }`, because the engine's fallback class is not identified.
  - A chain that comes back to an entry it already passed is refused as a data error. This is a Rust-only refusal:
    the note does not say what the walk does there.
  - The VM's iterator takes the whole list when the loop starts, as `AllParts` does, so a body that changes the
    chain mid-loop does not change what is visited (UNVERIFIED).
  - Reference leaves `ItemActors` unbound.
- **`EquippedInventoryAttributeContextResolver` (`ow-attr`, a native virtual).** In Off, section 4's rule:
  - A None source is None.
  - A `WillowEquipAbleItem` source, or one of a class below it, is answered as itself, at any location.
  - A source whose class implements no `WillowGame.IBodyPawn` by its package declaration is None. The interface's
    package was read from the installed `WillowGame.upk` export table; it is a name only.
  - A source that does implement it needs the accessor, so it gets the named `NotYet` "which IBodyPawn accessor the
    equipped resolver calls". Before that, a location of 4 or more is refused as a data error.
  - When it cannot be established whether the class implements the interface, the answer is its own named `NotYet`
    "whether the source class implements IBodyPawn" (review r2 of #445), never None. The cases are: no definition
    store, a class with no package export behind it, a missing package, and an engine-native stand-in on the
    ancestry.
  - Off asks no engine query. Reference keeps today's `EngineQuery::EquippedInventory`.
- **Writes.** None of the three writes anything, so nothing can refuse after a write.
- **Tests.**
  - `ow-attr` `context_resolvers/inventory_tests.rs`: six tests over an invented `WillowGame.upk`. Three older tests
    in `context_resolvers/tests.rs` now pin the Reference reading, or the Off script path.
  - `ow-inventory` `manager/tests.rs`: three `ItemActors` tests covering the order, the filters, the cap at 100 and
    101 skips, a None base class, a looped chain, and Reference.
- **Link 8 (`equip_gun`), installed, Off.**
  - Before, `init` stopped `NotYet` at this resolver's equipped-only flag. Now it gets past the resolver and stops
    at the unbound native `WillowInventory.InitializeAttributeSlots(bool)`. The gate grades an unbound native as an
    error, so strict goes from 0/14/0 to 0/13/1. The other 13 rows are byte-identical.
  - **The resolver's input on that run** (the note's open acceptance check) was read with a temporary local trace,
    which was not committed:
    - The attribute is `InventoryPartMonetaryValueModifierTotal`, reached from the weapon's monetary value.
    - Its `ContextResolverChain` holds this one resolver.
    - The source is the weapon (`WillowGame.WillowWeapon`), and the script answers the weapon itself: the note's
      inventory-source rule (section 2, rule 2).
    - `ItemActors` is not reached on this run, because the source is not a controller.
- **UNVERIFIED.** Nothing here was checked in the running game. Still open, from the note:
  - the `IBodyPawn` accessor;
  - which classes implement `IBodyPawn`;
  - the order in which items join `ItemChain`.

## Review r1 of #455

Four items, fixed as commits on the same branch.

- **Strict declaration lookup (inventory.rs).** The equipped resolver no longer uses the structural `implements`
  test, which fell back to the class's shape and dropped table entries it could not resolve. It uses a new
  `CodeIndex::implements_declared` in `ow-object` (`reflect.rs`), which has three outcomes: confirmed present,
  confirmed absent, or unreadable. It reads only package declarations: the interface tables on the class's ancestry,
  each entry resolved. An undecoded body, a class with a load error, or an entry that does not resolve gives
  "unreadable", and that maps to the named `NotYet` "whether the source class implements IBodyPawn (the interface
  tables on the ancestry of ... cannot be read)". Errors other than a missing class or package go up typed.
- **A None base class (manager.rs).** `foreach ItemActors(None, Inv)` was a data error. All three null forms the VM
  can pass (`Object(None)`, `Interface(None)`, `None`) now give `NotYet { NotRead }`. A script test passes the VM's
  literal None.
- **Typed errors across the attribute boundary (api.rs, ow-vm error.rs).** A VM error raised inside a resolver's
  script crossed `ow-attr` as `AttrError::World(text)` and came back as text. A new `AttrError::WorldTyped` carries
  the error itself, type-erased, and `From<AttrError> for VmError` turns it back into the original `VmError`. A step
  limit or an unbound native therefore reaches the caller unchanged. `NotYet` and definition errors still travel as
  they did.
- **A script `foreach` test (manager/script_tests.rs).** An invented manager runs `ItemActors` from assembled script
  through the VM's engine-iterator path. The chain is a weapon head the class filter skips, then two items, the first
  unreadied and the second readied. With the equipped-only flag clear the first item wins; with it set, the second
  wins; with neither readied the answer is None. The toy has no object-comparison native, so "matching" is the
  iterator's class filter, and the readied test is the script's.
- **Still UNVERIFIED in game**, as above.
