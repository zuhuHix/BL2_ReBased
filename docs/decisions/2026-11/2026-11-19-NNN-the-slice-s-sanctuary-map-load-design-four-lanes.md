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
- **Expected first new stop** on the map route: `GotoState('Auto')`, in the game info's `InitGame` and
  `PreBeginPlay`. This is taken from decision 2026-11-19-20's installed run with a harness `WillowGameInfo`, not
  re-run here.
