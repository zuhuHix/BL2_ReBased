# Inventory context resolvers: `InventoryAttributeContextResolver` and `EquippedInventoryAttributeContextResolver` (2026-11-19)

AI-assisted (Claude Code), analyst lane ANALYST-INVENTORY-CONTEXT, decision 2026-11-19-59. Written in our own words.
No decompiler output, pseudo-code or listing structure is reproduced here, and the raw output stays outside the
repository. **Every rule is UNVERIFIED in the running game**, because nothing here was observed live. Section 6 lists
the bl-sdk live reads that would confirm each rule.

Sources, in the order they were used:

- **The packages.** Property names and kinds come from the cooked packages (`tools/ghidra/class_layout.py`, whose
  `Core.Object` oracle of 60 bytes holds).
- **The script.** `InventoryAttributeContextResolver.GetAttributeContext` is UnrealScript in `WillowGame.upk`, not a
  native. It was read with `research/script_disasm.py`, which decodes it exactly.
- **The executable**, for what the script does not settle. That is the `WillowInventoryManager.ItemActors` iterator
  the script calls, and `EquippedInventoryAttributeContextResolver`, which has no script override and is a C++
  virtual. Both were read with Ghidra 12.1.4 (`tools/ghidra/run.ps1`, read-only queries).

This note replaces the two rows in NATIVE_ATTRIBUTES.md section 5. The inventory row there was rated low; section 7
lists what changes. An implementer of `ParityMode::Off` should work from this note alone (docs/LANE_BRIEF.md, "Clean
room").

## 1. Where they sit

| Class | Declared properties | `GetAttributeContext` |
|---|---|---|
| `WillowGame.InventoryAttributeContextResolver` | `InventoryDefinition` (object, `WillowInventoryDefinition`), `bOnlyConsiderEquippedItems` (bool) | script, in `WillowGame.upk` |
| `WillowGame.EquippedInventoryAttributeContextResolver` | `EquipmentLocation` (byte, enum `WillowDeclarations.EEquipmentLoc`) | no script override: the native virtual of `AttributeContextResolver` |

**The equipped-only flag.** The packages name it `bOnlyConsiderEquippedItems`, the bool declared after
`InventoryDefinition`. This was the open item that made the Rust resolver answer `NotYet` (decision
2026-11-15-22, `crates/ow-attr/src/context_resolvers/inventory.rs`).

## 2. `InventoryAttributeContextResolver` (script)

**What it accepts.** The source is used in one of two ways, and anything else gives None:

1. **A controller.** The controller's `Pawn` is taken. If that pawn is a `WillowWeaponPawn` (a vehicle seat), its
   `Driver` is taken instead. The search below then runs on that pawn's `InvManager`, which must be a
   `WillowInventoryManager`.
2. **An inventory object.** If the search found nothing (or did not run) and the source is a
   `WillowInventory`, the answer is **the source itself**. This holds whatever its definition is: neither
   `InventoryDefinition` nor the flag is checked on this path.

**The other sources.** A pawn, a vehicle or any other actor is neither a controller nor an inventory object, so it
gives None. The resolver does not go from a pawn to its controller. A None source also gives None.

**The search** runs only on a controller source.

- **What it walks.** Only `WillowItem` objects, through `ItemActors(class'WillowItem', ...)`, with the iterator's
  own readied-only argument left at its default (false). Section 3 covers the iterator.
- **What a weapon does.** A `WillowWeapon` extends `Weapon`, not `WillowItem`, so a weapon is never found this way.
  A weapon is reached only as the source itself (rule 2).
- **The match.** An item matches when its `DefinitionData.ItemDefinition` is the same object as the resolver's
  `InventoryDefinition`. It is an identity test, not a class test and not a balance test.
- **The equipped-only flag.** When `bOnlyConsiderEquippedItems` is set, an item must also have `Inventory.bReadied`
  set to match. When the flag is clear, readied and unreadied items match alike. The flag reads `bReadied`, not
  the pawn's `EquippedItems` slots.
- **When it ends.** The loop stops at the first match. That item is the answer.

**Several matches.** The first one in `ItemActors` order wins, and the others are ignored. Section 3 gives that
order.

**No match.** The answer is None, unless the source is itself a `WillowInventory`. A controller is not, so a
controller with no match gives None.

**When the search stops early.** A controller with no pawn, a pawn with no inventory manager, or a manager that is
not a `WillowInventoryManager` gives None.

**A None `InventoryDefinition`.** The identity test would then match an item whose `ItemDefinition` is None. Whether
the game ever holds such an item is not known.

## 3. `WillowInventoryManager.ItemActors`, as this resolver uses it

This replaces "body not read in detail" in NATIVE_INVENTORY_EQUIP.md.

- **What it walks.** The manager's `ItemChain` (an `Inventory` reference), from its head, following each entry's
  `Inventory` field: the same "next" link the engine's weapon chain uses, but a separate chain. The weapons are not
  on it.
- **The order.** Each matching entry is given to the loop body in chain order. How entries are added to
  `ItemChain`, at the head or the tail, was not read, so "first" is the chain's first, and the order in which
  items join the chain is **UNVERIFIED**.
- **The class filter.** An entry is given only if its class is `BaseClass` or a subclass of it. A None `BaseClass`
  falls back to a class the engine looks up in `Engine`, which was not identified. This resolver always passes
  `WillowItem`.
- **The readied filter.** When `bOnlyReadied` is passed true, an entry also needs `bReadied`. This resolver does not
  use that argument (section 2).
- **A cap.** The walk counts the entries it skips over the whole iteration, not per step. After more than 100 skipped
  entries it ends, even if matching entries remain. With `WillowItem` as the base class almost nothing on the item
  chain is skipped, so this matters only for an unusual chain.

## 4. `EquippedInventoryAttributeContextResolver` (native virtual)

- **A None source** gives None.
- **An equipable item source.** If the source is a `WillowEquipAbleItem` or a subclass of it, the answer is the
  source itself. This holds whatever slot it is in, and whether it is equipped or not.
- **Any other source** is asked for its `IBodyPawn` interface, the same lookup the PlayerReplicationInfo resolver
  uses. Without the interface the answer is None. With it, the interface's pawn accessor gives a pawn. The answer is
  that pawn's `EquippedItems[EquipmentLocation]` (`WillowPawn.EquippedItems`, a static array of four
  `WillowEquipAbleItem`), or None when the accessor gives None or the slot is empty.
- **Which accessor.** `IBodyPawn` declares three: `GetAWillowPawn`, `GetAWillowPlayerPawn` and `GetAWillowAIPawn`. The
  native calls one of them by its place in the interface's C++ table, and how that table lines up with the script
  declaration order is not proven. `GetAWillowPawn` is the likely one, since the result is only used as a
  `WillowPawn`. **UNVERIFIED**: a player pawn source has to give its own `EquippedItems` slot (section 6).
- **Which sources implement `IBodyPawn`.** That decides which sources work: the pawn classes, and whether a
  controller does. It was not listed in this lane. **UNVERIFIED.**
- **The location is not range-checked.** The body indexes the four-entry array by the byte as given. The enum values
  above 3, if `EEquipmentLoc` has any, would read past it. An implementation should refuse a location of 4 or more
  as a data error rather than guess (AGENTS.md rule 5), as `ow-inventory`'s manager already does for its own slots
  (`EQUIPMENT_SLOTS`).
- **It reads the pawn, not the inventory manager.** NATIVE_ATTRIBUTES.md section 5 said "the item the source's
  inventory manager holds"; the slot array is on the pawn.
- **A weapon source** is not a `WillowEquipAbleItem`. It therefore goes through `IBodyPawn`, which a weapon is not
  expected to implement (UNVERIFIED), and so gives None.

## 5. For the implementer (Off)

These points follow from sections 2 to 4. They are not code.

- `EngineQuery::Inventory` (a definition and an equipped-only bool) does not carry the whole rule. A weapon or item source
  returns itself (rule 2), a pawn source gives None, and only a controller's pawn's `WillowItem` chain is searched,
  through a weapon pawn's driver when there is one. The Rust resolver can read the two properties and the source's
  kind itself, and ask the world only for the walk: a controller's `ItemChain` items, in chain order, first match by
  definition identity and, when the flag is set, `bReadied`.
- `EngineQuery::EquippedInventory` (a location byte) should answer from the source's pawn (`EquippedItems`), not from the
  inventory manager. An equipable item source returns itself before any query.
- Link 8 (`equip_gun`) stops in `init` on this resolver for a non-None source. What source it gets there was not
  traced in this lane. If it is the weapon, rule 2 answers the weapon itself. If it is the controller, a gun is
  never found and the answer is None, unless an item with that definition sits on the item chain.

## 6. bl-sdk live checks that would confirm each rule

Each check is a live read in a maintainer-approved session (docs/verification/REALGAME_GROUND_TRUTH.md). The checks
make a resolver object with `unrealsdk.construct_object`, set its properties and call its `GetAttributeContext` with a
None attribute. Saves stay byte-identical.

| Rule | Check |
|---|---|
| Flag name and kind | read `bOnlyConsiderEquippedItems` on an installed resolver default object |
| Controller source, first match by definition | the player controller as source, `InventoryDefinition` set to an equipped class mod's `DefinitionData.ItemDefinition`: the answer is that class mod |
| Flag reads `bReadied` | the same, with a backpacked (unreadied) copy as the only match: the answer is the copy when the flag is clear, None when it is set |
| Several matches | two items of one definition on the item chain: compare the answer with the order `ItemActors` gives in a script `foreach` |
| Weapon pawn driver | the controller while driving: the driver pawn's items are searched (the same answer as on foot) |
| Pawn source gives None | the player pawn as source: None |
| Inventory source returns itself | an equipped weapon as source, `InventoryDefinition` set to something else: the answer is the weapon |
| Equipped resolver, item source | a `WillowEquipAbleItem` as source: itself, at any `EquipmentLocation` |
| Equipped resolver, pawn source | the player pawn as source, each location 0 to 3: `EquippedItems[loc]` (settles which accessor) |
| Equipped resolver, controller source | the player controller as source: a slot or None (settles whether a controller has `IBodyPawn`) |
| Equipped resolver, weapon source | an equipped weapon as source: None |

## 7. Corrections to NATIVE_ATTRIBUTES.md section 5

- **`InventoryAttributeContextResolver`.** It is script, not a native lookup through an interface. Its accepted
  sources, the item-only walk, the `bReadied` flag and the inventory-source fallback are in section 2. The confidence
  is now medium to high for the script reading, and UNVERIFIED in the running game.
- **`EquippedInventoryAttributeContextResolver`.** The slot is the pawn's `EquippedItems`, reached through
  `IBodyPawn`, and an equipable item source returns itself (section 4). The confidence stays medium, because of the
  accessor question.
