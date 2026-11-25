## 2026-11-25: WillowInventory.GetInventorySpaceRequirement in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-GETINVENTORYSPACEREQUIREMENT, on main 3aaabe97. Coded from
NATIVE_INVENTORY_EQUIP.md's "Inventory space" row alone (clean room: no native code or analysis output was opened).
Every rule is UNVERIFIED in the running game.

- **What was decided:** `ow-inventory` binds `Engine.WillowInventory.GetInventorySpaceRequirement()` in Off, beside
  the other `WillowInventory` natives (`definition::space_requirement_of`). It asks the definition through
  `GetInventoryDefinition`'s rule (`inventory_definition_of`), so that rule's refusals come first. The answer then goes
  by the definition's exact class, as the row reads it:
  - `WillowGame.WeaponTypeDefinition` and `WillowGame.EquipableItemDefinition` take the default: **1**;
  - `WillowGame.MissionItemDefinition`: **0**.
- **What the row is too thin for, refused** (`NotYet { NotRead }`, each naming its question; an analyst job if a
  link needs it):
  - `WillowGame.UsableItemDefinition` answers by "its two low flag bits", but the row does not name the field that
    holds them;
  - any other definition class, including a subclass of a read class (an artifact, a shield or class mod under
    `EquipableItemDefinition`). The row gives the base's default but does not say which classes override it, so no
    subclass is assumed to inherit it;
  - an inventory with no definition.
- **Reads only.** Nothing is written, loaded or drawn. Reference binds nothing.
- **Tests:** synthetic (`definition/space_requirement_tests.rs`):
  - the two default classes give 1 and the mission item 0;
  - the usable item, a subclass, the bare item definition and no definition each give their `NotRead`;
  - an inventory class `GetInventoryDefinition` does not read gives its refusal;
  - Reference is unbound;
  - each answer leaves the heap's writes and the RNG unchanged.
- **The slice gate** (installed Off strict, under the machine lock, on main 3aaabe97 with this change): 5 ok / 9
  NotYet / 0 errors. The native is not reached on main: links 6 and 7 stop at `GetBoundingCylinder`'s skeletal bounds
  before `ValidTouch`.
- **Where link 6 goes next** (a throwaway local stack, never pushed: this change with the pickup world's game info,
  fix/rust-pickup-world-gameinfo 5deeefb6, and #707's first version, whose box was answered): link 6 runs `make` and
  `touch`. It passes this native, because the gun's definition is a `WeaponTypeDefinition` and answers 1. It then stops
  at the unbound `InventoryManager.InventoryActors(class,object,bool)`, an iterator native with no row in
  NATIVE_INDEX.md, so that is an analyst job.
- **Sensitive files:** none. `crates/ow-package` and `Cargo.toml` are unchanged.
- **Verified:** nothing in the running game.
