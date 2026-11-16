## 2026-11-16: Rust map load: Sanctuary's placed actors in a world, the native steps left to an analyst note

AI-assisted (Claude Code). Lane W5-MAPLOAD, step 2 of the slice plan (`docs/rust-port/SLICE_RUN_PLAN.md`, PR #369):
"Sanctuary's level actors in a `World`". This lane loads a map's placed actors into an `ow-world` `World` through the
definition store, and writes `WorldInfo.DeltaSeconds` each tick. The engine steps that bring a loaded map up for play
are native and were not read before. They are written up as an analyst note (`docs/verification/NATIVE_MAP_LOAD.md`)
and stay `NotYet` (clean room: the analyst does not implement). There was no real-game session. No dependency was added
and no bounds check was loosened; no parsing behaviour of an existing reader changed.

- **What was added.**
  - `crates/ow-world/src/world/mapload.rs` (+ tests), the module `ow_world::world::mapload`. It sits under `world`
    because lanes do not edit the crate's `lib.rs` (architecture section 2.3).
    - `read_level_list` reads a level's actor list from its map file.
    - `load_map(world, map)`:
      - places the persistent level's `WorldInfo` first (`Actors::place_world_info`), then its other listed actors
        in list order (`Actors::place`);
      - then the actors of the levels the `WorldInfo`'s `StreamingLevels` name, each from its index 1, so a streamed
        level's own `WorldInfo` is not placed (NATIVE_ENGINE_CORE.md, "Iterators");
      - loads each actor with `DefStore::resolve_ref` and a strict `load`. So each is the store's object for its
        path, with the prefix the store derives (26 for a placed actor, 4 for a collection actor: decision
        2026-11-16-55) and, in Off, its archetype;
      - does not place, but counts by class with its first reason, an actor that does not load or whose class is not
        an actor class; the load goes on;
      - returns a `MapReport`.
  - `World::tick` writes `WorldInfo.DeltaSeconds`, the clock's dilated delta, where the class declares it. Script
    reads it (the AI's actions, decision 2026-11-16-66). It is UNVERIFIED like `TimeSeconds`, because where the game
    advances its time fields is not read (NATIVE_ENGINE_CORE.md, "Not read yet").
  - `docs/verification/NATIVE_MAP_LOAD.md`, the analyst note.

- **The level's actor list (rule 4).** No record or reader held it, so it was recovered from the packages and checked
  with structural oracles over all ten Sanctuary level files.
  - The level is `TheWorld.PersistentLevel`, class `Engine.Level`.
  - Its untagged tail starts with an owner reference, which is the level itself in every file, then a count, then
    the references.
  - Item 0 is the file's `WorldInfo` in every file, and item 1 is that list's one null entry.
  - Every other item is an export whose outer is the level.
  - The level URL follows the list, starting with its protocol string `unreal`.
  - The exports under a level that the list leaves out are the model, its polys, the Kismet sequence and the model
    components, plus 8 blocking-mesh actors and one wiring mesh in two files.
  - `read_level_list` refuses any tail off that layout. Synthetic tests check each refusal: the owner, an item
    outside the level, a null item 0, a count past the tail, no URL after the list.

- **What stays `NotYet`** (`MapReport::pending`, three entries naming NATIVE_MAP_LOAD.md):
  - the begin-play pass over the placed actors (sections 2 and 2.1). The world has not begun play after a load;
  - the game info the engine spawns into `WorldInfo.Game` (section 3). The world's game info and replication info
    queries keep their `NotYet`;
  - the local player's play actor and the engine's local player list (section 4), which `Actor.LocalPlayerControllers`
    walks.

  They are `NotYet` in both modes. The C++ has no map load, so there is no Reference stand-in to keep (R3). The map
  load itself is the same in both modes; the definition store applies its own mode (archetypes in Off only).

- **The analyst note.**
  - It was read locally with `tools/ghidra/run.ps1` (read-only queries); the listings stay under ignored
    `local/analysis/`.
  - It records, in own words and with a confidence per item, the routine that brings a world up for play:
    - the time reset, three world-info floats that by position are, in public UE3 terms, the three time fields;
    - the one-time begin-play flag;
    - the per-level actor initialisation pass;
    - the level's two-pass begin-play routing, whose per-actor events are by convention `PreBeginPlay` and
      `PostBeginPlay`, not confirmed.
  - It records the game-info routine: nothing if a game info exists; else the configured `DefaultGame` or a URL
    class; spawned and stored in `WorldInfo.Game`.
  - It records the `SpawnPlayActor` native.
  - Its open items name what the implementer must confirm first: the vtable slots, the time-field mapping, and the
    class choice.
  - It was scanned for listing artefacts before handing over.

- **Proof.**
  - **Synthetic** (the placement test runs in both modes):
    - the persistent `WorldInfo` is first;
    - listed actors are placed in list order, and an unlisted actor under the level is not placed;
    - a streaming level is placed from index 1;
    - a non-actor is refused by class;
    - a streaming level no file holds is reported;
    - a second load into the same world is refused;
    - the three pending `NotYet`s are returned;
    - the tick writes `DeltaSeconds`;
    - the level-list refusals.
  - **Installed** (`mapload::tests::sanctuary_as_a_world_on_the_installed_game`, ignored by default; pc-hummer;
    output under ignored `local/p2/w5-mapload/`). Sanctuary_P loads in Off and in Reference alike, in about 17 s:
    - 2,363 actors placed, 0 refused, over the persistent level and its nine streaming levels;
    - per level, the placed count is the list minus its null entry (and, for a streaming level, its own
      `WorldInfo`).
    - **By class:** StaticMeshActor 310, DecalActor 293, WillowAIMoveNode 246, Perch 245, Emitter 188,
      BlockingMeshActor 162, WillowPopulationPoint 146, InterpActor 117, WillowPopulationOpportunityPoint 110,
      WillowInteractiveObject 89, PopulationOpportunityDen 49, StaticMeshCollectionActor 48, WillowTrigger 31,
      SkeletalMeshActor 29, and 51 more classes with fewer.
    - The singletons include WorldInfo, WillowCoopPlayerStart, DominantDirectionalLight and FastTravelStation. There
      are 11 WillowAIPawn.
  - **`--slice-run` in Off on today's main** (the Fire route):
    - `accept` and `range` are not ok;
    - `spawn` stops at `Behavior_AIHold` (`NotYet`, exit 3), as before.
    - The slice's mission bridge builds its own `WorldInfo` and refuses a world that already has actors, so it does
      not use the map load yet.
    - With the map world, the first stops would be this lane's three pending steps: the game info, the begin-play
      pass and the local player list, which `LocalPlayerControllers` needs.
    - Joining the bridge to the map world is the next step of the plan's sequence (W6 with H-WORLD), after an
      implementer codes NATIVE_MAP_LOAD.md.

- **UNVERIFIED:**
  - that every streaming level is loaded at map load (the game streams by class and Kismet);
  - that the lists concatenated in `StreamingLevels` order are the order the game's iterators visit;
  - the first-dynamic index, left at 0;
  - `DeltaSeconds` as the dilated delta;
  - everything in the analyst note.
