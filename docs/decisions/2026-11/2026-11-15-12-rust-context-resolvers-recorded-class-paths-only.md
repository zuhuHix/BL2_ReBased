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
- **Going past the C++ subset.** The `src/progression.hpp` pawn, mind and controller cases reproduce. Where the C++
  `AttributeContext` has no field for a source (a player controller's pawn, an AI mind's or AI pawn's controller), the
  C++ answers None and the port follows section 5 instead. A test pins the difference.
- **What answers `NotYet`, and why.**
  - BalancedActor, Designer and DesignerByName (`NotPorted`): they need an interface test or a provider lookup that
    `EngineQuery` does not have yet.
  - StatusEffectChance (`NotRead`): not read in detail.
  - Inventory (`NotRead`): no record names the property of its "equipped only" flag.
  - Skill's own script body (`NotRead`).
  - The 11 classes in `UNREAD` (`NotRead`).
- **Engine lookups.** Pools and items go through `AttrWorld::engine_query`. A resolver definition is loaded through
  `DefStore::is_unloaded` and `DefStore::ensure_loaded` before its fields are read.
- **Verified:** synthetic-world Rust tests only (`context_resolvers/tests.rs`). The three chain rules per resolver
  (first None ends the chain, empty chain is None, override first) are checked with a test-only walker written from
  NATIVE_ATTRIBUTES.md section 1, because the evaluator's walk (W3-ATTREVAL) had not landed.
- **UNVERIFIED:** every resolver rule in the running game. The stored chain order is kept, and whether it is the game's
  evaluation order stays UNVERIFIED (decision 2026-11-15-08).
