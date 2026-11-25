## 2026-11-25: InventoryActors walks the inventory chain

AI-assisted (Claude Code)

Lane ANALYST-INVENTORY-ACTORS, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds
check changed.

- **Why:** link 6's next stop after the skeletal bounds (with #710 and #713) is the unbound
  `InventoryManager.InventoryActors`, in `WillowInventoryManager.HandlePickupQuery`.
- **What:** the new note `docs/verification/NATIVE_INVENTORY_ACTORS.md`, plus one NATIVE_INDEX row and one GATES row.
- **Evidence:**
  - `research/script_disasm.py`;
  - the `Engine.upk` export list;
  - a local read-only Ghidra reading under the machine lock;
  - the layout tool.

  Raw output stays under ignored `local/analysis/invactors/`.
- **Findings** (UNVERIFIED in game):
  - the iterator walks the manager's `InventoryChain` through each entry's `Inventory` link, in chain order;
  - it gives entries of `BaseClass` or below it (None means `Engine.Inventory`), and, with `bOnlyReadied`, only
    readied ones;
  - it examines at most 100 entries over the whole iteration, given or skipped, unlike `ItemActors`, which counts
    skips;
  - `ItemActors`'s unidentified None fallback is `Engine.Inventory` as well;
  - `WillowWeapon.DenyPickupQuery` is false, so link 6's loop denies nothing.
- **Proposed Off answer:**
  - an iterator beside `ItemActors`, with the VM's snapshot model;
  - no revisit refusal (the cap ends a looping chain);
  - tests listed.
- **Clean room:** a different lane implements this note.
