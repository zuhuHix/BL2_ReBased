## 2026-11-19: An Off spawn loads the definitions its actor's own properties point to

AI-assisted (Claude Code). Lane SPAWN-COMPLETE, part B (the hub narrowed the lane; part A, the Fire dummy's per-actor
components in `ow-mission`, went to another lane). There was no real-game session, so the rule is **UNVERIFIED** in
the running game. No dependency or crate edge was added, no bounds check was loosened, and Reference is unchanged.
The decision number is a placeholder until the hub assigns one.

- **The hub's ruling:** natives stay load-free; the spawn completes the actor before any script runs on it.
- **`ow-world` `actors.rs`, `load_own_definitions`, Off only.**
  - Every unloaded definition handle that one of the new actor's own object-valued slots will hold is loaded
    through the store. Since review r1 of #528 this runs on the source (class default or archetype) after the
    component plan and before the actor is made; the plan's components and inherited references are left out, since
    the actor holds their copies.
  - It is one level deep: the loaded definitions' own references are not followed. Arrays and structs are not walked.
  - A load is a write, so a later refusal of the spawn is marked "stopped after partial writes", its kind kept.
  - Every reference is required (review r1 of #528). A fresh load failure is the spawn's error, and a handle whose
    load already failed returns the same error again.
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

**Where Maya's body class gets loaded (hub ruling, option (a)).** The route, read from the installed script:
1. `WillowPlayerController.ApplyCharacterClassDefaults` asks the pawn data manager for the player class's
   `PawnArchetypePath` archetype (`FindAlreadyLoadedObject`).
2. It reads that archetype's `BodyClass` and passes it to `P.BodySelectClass`.
3. `BodySelectClass` assigns it, and `ApplyBodyClassStartingValues` calls `InitializePawnMeshes` first.

The only other script caller is `WillowPawn.PostBeginPlay`, with the pawn's own `BodyClass`. The game has the body
class resident because the whole streaming package is: `BodyClass_Siren` shares `GD_Siren_Streaming` with
`Pawn_Siren`.

- **`ow-cli` `cmd/combat.rs`, `load_same_package_references`:**
  - The harness's stand-in for the streaming load (`apply_class_inputs`, which preloads the archetype) now also loads
    the archetype's object references that live in the archetype's own package file. It goes one level deep, through
    the store, so the loads are the store's tracked ones.
  - Other packages' references and handles whose load already failed are left alone.
  - Each load is logged. It is an UNVERIFIED stand-in, like the preload it extends.
  - Natives stay load-free, and there is no VM assignment rule.
- **Test:** an invented archetype whose same-package reference is loaded and whose other-package reference is not.
- **The slice** (installed, Off, strict, with #522 merged and `Engine.IsSplitScreen()` bound false, both locally
  only): 1 ok / 10 NotYet / 3 errors.
  - Link 1 is 0 of 1 driver step. It now stops in `maya:8` at `InitializePawnMeshes`' `NotYet { LayerAbsent }`, "the
    body composition build (InstanceDataHelper.InitializeBodyComposition: component instancing and attachment to the
    pawn's mesh)": past the lazy-`BodyClass` `NotRead`, at the component layer the note names.
  - Links 3, 4, 5, 13 and 14 stop at the same place.

**Review r1 of #528 (Sol high).** All Off only; Reference unchanged; UNVERIFIED in game.
1. **A failed load keeps its kind.** The spawn's and the harness's definition loads convert a `DefError` with
   `VmError::from`, so a store `NotYet` stays `VmError::NotYet` (it was wrapped as `VmError::Def`, and the spawn's
   marker only matches `NotYet`). The harness (`apply_class_inputs`) reads the write tracker's counters when it
   starts and marks a `NotYet` from the preload, the same-package loads or the class select iff a counter moved.
2. **No copies left behind.** The actor's own definition loads now run before the actor is made, so a failed load
   leaves neither the actor nor its component copies. `apply_plan` frees the copies it made if it fails (making a
   copy or re-pointing), and the spawn frees the actor; nothing can fail after `apply_plan` returns, so the spawn
   keeps no copy list of its own (review r2 of #528).
3. **A cached failure refuses every time.** A handle whose load already failed is no longer skipped: the spawn
   returns the store's cached error, its kind kept. The reference is treated as required (the conservative choice;
   nothing shows it is optional). Review r2 of #528: the harness's same-package loads do the same, so a retry of
   `apply_class_inputs` after a failed load returns the cached error, unmarked, and never reaches the class select.
4. **The marker follows the counters.** The spawn reads #518's counters (`Heap::committed_writes`, `GameRng::draws`,
   `DefStore::load_transitions`, copied from `origin/fix/rust-483-followup` @ 211484a5 in their own commit) when it
   starts. In Off a `NotYet` is marked iff one moved, replacing the `loaded` flag.
   - A cold class default object the spawn builds is a write, so a refusal after it is now marked.
   - The retry of a cached failure moves no counter, so it is not marked.
   - A fresh failed load is a transition, so it is marked.
   - `crate::pools::Counters` and `after_counted_writes` hold the rule.
- **Tests:** three in `world/tests.rs` and two in `cmd/combat/harness_tests.rs`, all invented. The fixtures are an
  export whose archetype is itself (the store's `NotYet { NotRead }`, "its archetype chain is a cycle") and the
  archetypes that name it.
  - The failing spawn is marked and keeps its kind; nothing is left behind; the next spawn returns the same typed
    error, unmarked.
  - A cold class default marks a destroyed-template refusal; with the default warm, the same refusal is unmarked.
  - A template whose load fails is marked; its cached failure is not.
  - The harness: a failed same-package load keeps its kind and is marked; a class-select refusal is marked after a
    preload and unmarked without one.
