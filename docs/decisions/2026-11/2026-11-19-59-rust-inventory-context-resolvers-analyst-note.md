## 2026-11-19: Rust inventory context resolvers: analyst note (NATIVE_INVENTORY_CONTEXT.md)

AI-assisted (Claude Code). Analyst lane ANALYST-INVENTORY-CONTEXT; the hub gave number 59. Link 8 (`equip_gun`) stops
at step `init` on `InventoryAttributeContextResolver`, which answers `NotYet` because its equipped-only flag was not
recorded (decisions 2026-11-15-22 and 2026-11-19-39). NATIVE_ATTRIBUTES.md section 5 rated that resolver low. This
lane writes the own-words note `docs/verification/NATIVE_INVENTORY_CONTEXT.md`. Parsing does not change, and no code
changes: another thread implements from the note (docs/LANE_BRIEF.md, "Clean room"). Nothing here was checked in the
running game.

- **Sources.** The packages (`class_layout.py`) and the script (`research/script_disasm.py`) come first. Ghidra 12.1.4
  read-only queries (`tools/ghidra/run.ps1`) cover only what neither settles. Raw output stays under ignored
  `local/analysis/inventory-context/`. No bl-sdk session was granted, so nothing was read live.
- **Found:**
  - The flag is `bOnlyConsiderEquippedItems` (packages).
  - `InventoryAttributeContextResolver.GetAttributeContext` is script, not native. It searches a controller source's
    pawn (a weapon pawn's driver) for the first `WillowItem` on the item chain whose `ItemDefinition` is the
    resolver's `InventoryDefinition`, and readied too when the flag is set. With no match, a `WillowInventory`
    source is returned itself, and anything else gives None.
  - `ItemActors` walks the manager's `ItemChain` in chain order.
  - `EquippedInventoryAttributeContextResolver` returns a `WillowEquipAbleItem` source itself, and otherwise the
    `IBodyPawn` pawn's `EquippedItems` slot. The earlier note said the inventory manager's.
- **Records changed:** NATIVE_ATTRIBUTES.md section 5 (both rows), NATIVE_INVENTORY_EQUIP.md (`ItemActors`),
  NATIVE_INDEX.md (the `ItemActors` row, and a new row for the equipped resolver).
- **Review r1 of #445** (documentation only):
  - the note records what the Rust path establishes for link 8: the monetary value starts the chain from the
    weapon, and each resolver gets the previous one's answer. The resolver's final input stays link 8's acceptance
    check;
  - Off answers a named `NotYet` for a source that implements `IBodyPawn`, until the accessor is settled, and for a
    source whose interface list cannot be read. None is kept for an established absence;
  - the live checks are called outcome checks, with a case that tells the possibilities apart for each open
    question;
  - the `ItemActors` summary in NATIVE_INVENTORY_EQUIP.md is replaced.
- **UNVERIFIED** in the running game: every rule. Also open: the order in which items join the item chain, which
  `IBodyPawn` accessor the equipped resolver calls, and which classes implement `IBodyPawn`. The note's section 6
  lists a bl-sdk live read for each rule.
