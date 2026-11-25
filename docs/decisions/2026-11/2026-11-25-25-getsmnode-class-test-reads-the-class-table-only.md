## 2026-11-25: GetSMNode's class test reads the class table only, never loading the blend class's code

AI-assisted (Claude Code, Opus 5.5). Lane GETSMNODE-CODE-LOAD, finding (c) of the ow-skills cold-class audit (#690),
stacked on fix/rust-cold-class-audit-skills. Off only (the module binds nothing under Reference). No game data and no
native analysis was used: the rule is NATIVE_SPECIAL_MOVE_PLAY.md's, unchanged.

- **The finding:** `SpecialMoveComponent.GetSMNode` looked the blend class up with `Vm::find_class`
  ("GearboxFramework.AnimNodeSpecialMoveBlend"). That loads the class's code when it is cold, and a load is a write.
  The audit's test saw `class_code_loads` move.
- **The rule kept:** the note answers the owner mesh's node named `SpecialMoveBlend` when it is an
  `AnimNodeSpecialMoveBlend`, and None otherwise. That is a class test, and it needs class layouts, not code.
- **The change:** the class is looked up by path in the class table (`classes.by_path`), with `is_child_of` as
  before. A node of the blend class, or of a subclass, has the class's layout by construction, since a class is only
  defined over a defined super class. So when the table has no such class, no node can be one, and the answer is the
  note's None, decided without a load.
- **Not a `NotYet`:** the hub's brief suggested a typed `NotRead` for a cold class. With the class table the answer is
  determined either way, so this lane answers it instead of refusing. That is the hub's to overrule.
- **Warm path:** unchanged. The existing `GetSMNode` tests pass as before.
- **Tests** (`special_move_tests.rs`, synthetic):
  - the audit's `cold_get_sm_node_loads_no_code` is un-ignored and passes: a node of a cold non-blend class gives
    None, with no class code loaded;
  - new, `a_cold_blend_class_node_is_answered_without_a_code_load`: a node of the cold blend class itself is answered
    and cached as when warm, its class still cold afterwards.

  Both fail with the old `find_class` lookup.
- **Not changed:** the node search reads node fields through the loading helper (`get`). A lazy node handle there is a
  separate question, and this lane leaves it.
- **Not verified:** anything in the running game.
