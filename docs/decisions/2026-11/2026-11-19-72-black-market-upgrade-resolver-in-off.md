## 2026-11-19: The black-market upgrade resolver in Off from the analyst note

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-BLACKMARKET-RESOLVER, implementer. This lane coded
`WillowGame.BlackMarketUpgradeAttributeValueResolver` from `docs/verification/NATIVE_BLACKMARKET_UPGRADES.md`
(analyst lane ANALYST-BLACKMARKET-RESOLVER, decision 2026-11-19-63, at its review-r2 head on #451) alone. It opened no
analysis output, Ghidra or REA. That note lands with its own branch, which has to land first. There was no real-game
session: every rule stays **UNVERIFIED** in the running game. No dependency was added, no bounds check was loosened
and no existing `NotYet` guard was relaxed.

- **`ow-attr`, `value_resolvers/blackmarket_upgrade.rs` (new), Off only.** Value and base value are one rule, and the
  chain's previous value is not used. The steps:
  - **The controller check:** the context is accepted when it is a `WillowPlayerController` or a subclass. When the
    class is not declared, every context is accepted.
  - **The index**, computed whether or not the context was accepted. With no upgrade, -1. A warm cache gives its value,
    with no globals lookup. A cold cache plans the assignment of every non-None listed upgrade's position; the
    effective index comes from that pending plan (the last occurrence of a duplicate, -1 when unlisted), not from a
    reread of the field.
  - **The result:** with an accepted context and an index, the controller's level at that index, after the array is
    zero-filled to index + 1 when it is shorter. Otherwise 0.0.
- **The write order** (the hub's ruling, reviews r1 and r2 of #451): every check comes before either write. Then the
  cache assignments are committed, then the zero-fill, then the entry is read.
  - A refusal leaves the cache fields and the array as they were.
  - The note's two `NotYet { NotPorted }` names are answered when the object to write is a stand-in that holds no
    properties, before any write.
  - A world without engine globals gives `AttrWorld::globals()`'s `NotYet`, before any write. A supported lookup that
    finds no definition answers 0.0 with nothing written.
  - Bad data covers a cache field that is not an int, a globals list that is not objects, and controller levels that
    are not ints.
- **The globals definition (the port's reading, UNVERIFIED).** The note says the definition is reached through the
  engine globals object. `AttrWorld` gains `globals_definition()`, an additive method with a default. It reads the
  globals object's `DefaultGlobalsDefinitionName` (the installed class default holds `GD_Globals.General.Globals`) and
  loads it through the world's store. An empty name is "none found"; a missing store is its `NotYet`. The VM world uses
  the default. The mission bridge finds the same object by path.
- **Review r1 of #458:**
  - Both arrays' declarations are checked by element kind before either write, so an empty array of the wrong kind
    is refused too.
  - A grown length past the object model's array bound (`ow_object::decode::MAX_ARRAY_LENGTH`) is bad data, and the
    grown array is reserved fallibly, both before the cache commits.
  - A listed upgrade is loaded for writing: a real load failure keeps its error, and a stand-in left by a load (or
    one that is not a handle) is the named cache-write `NotPorted`.
  - `EvalCx` counts committed resolver writes, and the shared fold makes a later `NotYet` terminal, prefixed once
    ("stopped after partial writes"), its kind kept. `ow-combat`'s `GetValue` wrapper no longer adds the prefix twice.
  - `ValueResolver::available` (a default) lets Reference refuse this class at the preflight, as an unregistered
    class did.
  - `AttrWorld::globals_object` (and `WorldHooks::globals_object`) tell "no globals object" (`Ok(None)`, 0.0) from
    "no globals support" (`NotYet { LayerAbsent }`). The world query service answers `Ok(None)` until the map load
    sets the object.
- **Review r3 of #458:**
  - Every exit of the shared fold goes through the partial-write rule, a None value-chain entry included.
  - The write scope belongs to each outer evaluation: the outermost fold resets it and nested folds share it, so a
    reused `EvalCx` does not carry one read's writes into another.
  - With a store, every listed upgrade is loaded through the store first. A load that succeeds but leaves a stand-in
    is the named cache-write `NotPorted`; a real load error comes back unchanged. Both are pinned with a synthetic
    package.
  - Reference's refusal names the resolver's class as the object space spells it, as the registry did.
- **Reference:** unchanged. Get, get-base and the writes answer the same `NotYet { NotRead }` an unregistered class
  gave; the C++ has no rule for this class.
- **Verified (automated, local, pc-hummer):**
  - 14 `ow-attr` tests on a synthetic world cover the note's cases: cold, warm (no lookup), rejected context, no
    upgrade, no definition found, no globals support, unlisted, duplicates (and growth from the pending index), bad
    data before any write, a stand-in controller, Reference, the default globals lookup, and the registry;
  - `cargo test --workspace` 2,169 / 0 / 46 and crate edges 19 OK.
- **Installed, Off:** `--slice-gate` strict 0/14/0, byte-identical before and after on the same base (main 74c7b0ac).
  Links 1, 3, 4, 5, 13 and 14 stop at `maya:8` on `WillowPlayerController.InitPlayerSkillTree`. That native is unbound
  and runs in the stock `OnPlayerClassChange` (since #444) before the ammo maxima are read. So this resolver is not
  reached on main yet. It was the stop on a merge before #444's review rounds (decision 2026-11-19-58).
- **UNVERIFIED:** every rule above, and the class of the context (the note's inference).
