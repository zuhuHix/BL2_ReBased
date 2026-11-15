## 2026-11-15: Rust context resolvers: recorded class paths only

AI-assisted (Claude Code). Lane W3-ATTRCTX of the Rust port fills `crates/ow-attr/src/context_resolvers/`: one
`ContextResolver` per `AttributeContextResolver` class that NATIVE_ATTRIBUTES.md section 5 reads, registered in
`context_resolvers::register`, which `AttributeSystem::standard` calls. No parsing behaviour changes. This file records
the choices a later lane should be able to find.

- **Class paths come from the repository only.** A resolver is registered under a class path the records, `src/` or
  `research/native_by_class.json` name. Five implemented resolvers have no recorded package: OffHandWeapon,
  PlayerController, WeaponResourcePool, EquippedInventory and Skill (`PACKAGE_NOT_RECORDED`). They are public and
  registered by nobody, so a chain naming one stays the registry's `NotYet { NotRead }`. A guessed package would have
  been an invented path. Once each package is read (a live read, or the code packages), each needs one
  `with_context_resolver` line.
- **WillowMind's rule is the C++ oracle's.** `WillowMindAttributeContextResolver` is not in the native notes. Its rule
  is taken from `src/combat.cpp` (`describe`) and `src/progression.cpp`: a `WillowMind` gives itself, a `WillowAIPawn`
  gives its `MyWillowMind`, anything else gives None.
- **Going past the C++ subset.** The C++ `AttributeContext` is a fixed set of fields read at every step of a
  chain, so its answers reproduce here for single-step chains and for chains that start with NoContextNeeded (which
  passes the source on); those are the `src/progression.hpp` pawn, mind and controller cases the tests pin. Where the
  C++ has no field for a source (a player controller's pawn, an AI mind's or AI pawn's controller), it answers None
  and the port follows section 5 instead. A later step of a longer chain may differ too: `[PlayerReplicationInfo,
  Controller]` from a player controller is the controller in the C++ and None here (the Controller step gets the
  replication info). A test pins each difference.
- **Candidate stand-in rows for architecture section 6.1** (proposed here; the hub edits ARCHITECTURE.md).
  `src/progression.cpp:85-87` treats two resolvers as always resolving and keeps the object the chain held before
  them; the port answers section 5 instead, and a test pins the difference
  (`where_the_cpp_keeps_the_previous_object_for_game_info_and_designer`). Both readings are UNVERIFIED.
  - `GameInfoAttributeContextResolver`: the C++ keeps the previous object; the port gives the game info. Owner:
    W4-ROLL.
  - `DesignerAttributeContextResolver`: the C++ keeps the previous object (a designer attribute keeps its base value;
    the loot's artifact weights read one that way); the port is `NotYet { LayerAbsent("ow-vm") }`, as the resolver is
    script. Owner: W6-LOOT.
- **What answers `NotYet`, and why.** Each names NATIVE_ATTRIBUTES.md section 5 unless said otherwise.
  - BalancedActor (`NotRead`): its interface class is not recorded, and the C++ has no such rule.
  - Designer and DesignerByName (`LayerAbsent("ow-vm")`): they are script.
  - StatusEffectChance (`NotRead`): not read in detail.
  - Inventory (`NotRead`): no record names the property of its "equipped only" flag.
  - Skill's own script body (`NotRead`, section 10).
  - The 11 classes in `UNREAD` (`NotRead`), each naming where it is known from: six from the notes' "Not read yet"
    list, `CurrentProficiencySkillAttributeContextResolver` from the section 5 table (named, body not read), and four
    (`Owner`, `GameReplicationInfo`, `AIComponent`, `InventoryDefinition`) from `research/native_by_class.json` only.
    `WillowGame.InstanceDataContextResolver` is in that census too but is left out: nothing records it as an
    attribute context resolver (its name does not say so, and its super class is not recorded).
- **Proposed `EngineQuery` additions** (contract changes for a later lane; not in code):
  - `BalancedActor`: a unit variant answering `Some(source)` when the source implements the balanced-actor
    interface, else `None`.
  - `ScriptContext { resolver: ObjRef }`: the VM runs a script resolver's `GetAttributeContext` on the source. One
    variant covers Designer, DesignerByName and Skill.
  - No status-effect proxy query: that resolver stays `NotRead` until its rule is read.
- **Engine lookups.** Pools and items go through `AttrWorld::engine_query`. A resolver definition is loaded through
  `DefStore::is_unloaded` and `DefStore::ensure_loaded` before its fields are read.
- **Verified:** synthetic-world Rust tests only (`context_resolvers/tests.rs`). The three chain rules per resolver
  (first None ends the chain, empty chain is None, override first) are checked through the evaluator's own walk
  (`AttributeSystem::resolve_context`, and `value_of` for the override, W3-ATTREVAL) on a synthetic definition.
- **UNVERIFIED:** every resolver rule in the running game. The stored chain order is kept, and whether it is the game's
  evaluation order stays UNVERIFIED (decision 2026-11-15-08).
