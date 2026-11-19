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
