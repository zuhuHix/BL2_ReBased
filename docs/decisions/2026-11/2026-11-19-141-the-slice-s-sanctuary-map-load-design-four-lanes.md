## 2026-11-19: The slice's Sanctuary map load: design, four lanes

AI-assisted (Claude Code, Opus 5.5). Lane DESIGN-SLICE-MAPLOAD. The number NNN is a placeholder; the hub gives it.
Docs only: no code changed, no dependency added, and there was no real-game session.

- **What.** New design record: `docs/rust-port/SLICE_MAPLOAD.md`. It covers the smallest Sanctuary map load the slice's
  VM links need in Off:
  - the order of the pieces relative to the mission bridge's set-up;
  - what exists and what is new;
  - which links each piece unblocks;
  - the first new stop expected;
  - four lanes in dependency order.

  It supersedes the map-load part of SLICE_RUN_PLAN.md section 3.1.
- **Why.** No slice driver loads a map into its VM world today. The links measure the chain on hand-made worlds:
  - the bridge's own `WorldInfo`, GRI, tracker and controller;
  - for links 6 to 9, a lone `WorldInfo`.

  The map load (decision 2026-11-16-76) and the bring-up (decisions 2026-11-19-04 and 2026-11-19-20) exist, but
  nothing joins them to the slice. Three things are missing: the game info's class choice, the navigation list and the
  local player's play actor.
- **The decided shape** (a proposal for the hub):
  1. `World::new`, with the bridge's natives bound first;
  2. `load_map`;
  3. the navigation list;
  4. the game info spawned into `WorldInfo.Game`;
  5. `bring_up_for_play`, in which the script's `PreBeginPlay` makes the GRI and the mission tracker;
  6. the local player through `Login` and `PostLogin`;
  7. the bridge adopts the map's objects instead of making its own.

  The old hand-made route stays the default until the map route reaches as far on every link.
- **Lanes:**
  - ANALYST-MAPLOAD-NAVLIST-PLAYACTOR (analyst);
  - IMPL-MAPLOAD-GAMEINFO and IMPL-MAPLOAD-PLAYER (`ow-world`);
  - IMPL-SLICE-MAP-WORLD (`ow-mission` and `ow-cli`).
- **Evidence used:**
  - the port's code and records;
  - the installed script, read with `research/script_disasm.py` (listings under ignored `local/analysis/mapload/`, own
    words only here);
  - two `--def-dump` reads: `Sanctuary_P`'s `WorldInfo` has `DefaultGameType`, `Game`, `GRI` and
    `NavigationPointList` all None, and its player start has a None `nextNavigationPoint`;
  - the installed `DefaultGame.ini` (`DefaultGame` is `WillowGame.WillowCoopGameInfo`).
- **UNVERIFIED:** every game rule in the record. Its open questions:
  - the navigation list's source;
  - the game info class in the running game;
  - `SpawnPlayActor`'s call site;
  - grading on an incomplete bring-up, which is the maintainer's call;
  - streaming;
  - the bridge's remaining writes;
  - the director's placement.
- **Review r1 of #530 (Sol), folded in:**
  1. Lane 2 proves the class selection, the spawn and the typed bring-up stop only. Its installed test no longer
     expects a script-made GRI and tracker. The auto-state work (ANALYST-AUTO-STATE, then an implementer) is a named
     dependency of those assertions and of lane 4.
  2. The globals: on the map route the driver makes the world's one `WillowGlobals` before bring-up. This is a labelled
     harness input, with the installed globals definition. `ow-inventory`'s world getters answer for it; the bridge
     binds no getters of its own there. A call before the set-up is the getter's typed `NotYet { LayerAbsent }`.
  3. Lane 4 names its constructors and fixtures: `MissionScript::new` from `MissionBridge::new` and
     `FireMissionSlice::new`, the pawn accessor, and the old-route worlds. Its first PR is the adopt path plus
     `--combat-run`, with a test that it uses the possessed pawn.
  4. Lane 3 can be coded in parallel against a toy game info, and merges after lane 2, which owns `mapload.rs`'s
     wiring. The navigation work is scoped after lane 1 (9554).
  5. The play-actor steps follow NATIVE_MAP_LOAD.md section 4 in full: the player index and unique id before `Login`,
     the attachment before `PostLogin`, and the store on the local player. `add_local_player` is the iterator only.
     The call site and any fallback id stay UNVERIFIED.
  6. SLICE_RUN_PLAN.md section 3.1 and the lanes that cited W5-MAPLOAD now link to the design.
  7. The pickup row lists its globals, definition load and weapon-default preload. Q2 asks for three separate reads:
     the URL, the effective config and the resulting class.
- **Expected first new stop** on the map route: `GotoState('Auto')`, in the game info's `InitGame` and
  `PreBeginPlay`. This is taken from decision 2026-11-19-20's installed run with a harness `WillowGameInfo`, not
  re-run here.
