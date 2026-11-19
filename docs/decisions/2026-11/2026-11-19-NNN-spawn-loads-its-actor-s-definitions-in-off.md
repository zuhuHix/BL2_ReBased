## 2026-11-19: An Off spawn loads the definitions its actor's own properties point to

AI-assisted (Claude Code). Lane SPAWN-COMPLETE, part B (the hub narrowed the lane; part A, the Fire dummy's per-actor
components in `ow-mission`, went to another lane). There was no real-game session, so the rule is **UNVERIFIED** in
the running game. No dependency or crate edge was added, no bounds check was loosened, and Reference is unchanged.
The decision number is a placeholder until the hub assigns one.

- **The hub's ruling:** natives stay load-free; the spawn completes the actor before any script runs on it.
- **`ow-world` `actors.rs`, `load_own_definitions`, Off only.**
  - After the component plan has been applied, every unloaded definition handle that one of the new actor's own
    object-valued slots holds is loaded through the store. This happens before the actor joins the level list.
  - It is one level deep: the loaded definitions' own references are not followed. Arrays and structs are not walked.
  - Each load sets `loaded`, so a later refusal of the spawn is marked "stopped after partial writes", its kind kept
    (a lazy load is a write).
  - A handle whose load already failed is left alone, so a spawn that succeeded before does not fail on an old error.
  - A fresh load failure is the spawn's error, and the actor is freed, as a failure while making it already is.
  - `DefStore::is_unloaded` covers only the store's own definition handles; resource stand-ins never count.
- **Tests:** two in `world/tests.rs`, with an invented `BodyProbe` actor whose archetypes name the fixture's
  `ToyTarget`:
  - the definition is unloaded before the spawn, loaded after it, and the actor's `Body` is that definition;
  - a refusal after the load (the post-spawn encroachment check) is marked.
- **The slice:** unchanged.
  - The installed Off gate (with #522 merged and `Engine.IsSplitScreen()` bound false, both locally only) gave 1 ok /
    10 NotYet / 3 errors, **byte-identical** with and without this change.
  - Link 1 still stops in `maya:8` at `InitializePawnMeshes`' lazy-`BodyClass` `NotRead`, because Maya's body class
    is not a property the spawn copies. `WillowPawn.BodySelectClass(InBodyClass)` (script) assigns `BodyClass` from
    its parameter on the already-spawned pawn, then calls `ApplyBodyClassStartingValues`, whose first statement is
    `InitializePawnMeshes`.
  - So a spawn-time load cannot reach it. Where that load belongs instead (the caller that hands the body class to
    `BodySelectClass`, or a VM rule for an assignment of a definition handle) is a question for the hub.
