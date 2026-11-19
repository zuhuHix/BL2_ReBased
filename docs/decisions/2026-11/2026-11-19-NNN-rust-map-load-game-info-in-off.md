## 2026-11-19: Rust map load game info in Off: the configured default spawned into WorldInfo.Game, then the bring-up

AI-assisted (Claude Code). Lane IMPL-MAPLOAD-GAMEINFO, lane 2 of the map-load design (docs/rust-port/SLICE_MAPLOAD.md
sections 3 and 4, PR #530 on `research/rust-slice-mapload` at ecce435). It is coded from NATIVE_MAP_LOAD.md section 3
and the design alone (clean room); no analysis output was opened and the executable was not read. There was no
real-game session, so every rule is **UNVERIFIED** in the running game. No dependency was added, no bounds check was
loosened and no existing `NotYet` guard was relaxed. Reference is unchanged.

- **`mapload::game_info::spawn_game_info(world, input)`** (new, `ow-world`, Off only), the note's game info routine
  for the branch Sanctuary takes:
  - `WorldInfo.Game` already set: nothing done.
  - Refused before anything changes, each `NotYet { NotRead }`, unmarked: a `NetMode` other than standalone (the
    routine does nothing when a network or demo driver exists, which is not modelled); a world that has begun play; a
    set `DefaultGameType` and any `GAME=` option (both reach `SetGameType`, whose receiver is open; telling a loadable
    `GAME=` class from one that is not would need a load, which is a write); no configured default game.
  - `DefaultGameType` None and no `GAME=`: the configured default class (`[Engine.GameInfo] DefaultGame`, read from
    the installed `DefaultGame.ini` by `default_game_from_ini`, a small own reader) is looked up, checked against
    `WorldInfo.Game`'s declared class, spawned through `actors::spawn` with the `WorldInfo` as the spawner
    (UNVERIFIED: the engine's spawn has no spawning actor) and stored in `WorldInfo.Game`. A class that is not found,
    or a spawn that returns nothing, is the note's not-found text with the field left empty.
  - Whether the user's `WillowGame.ini` overrides `DefaultGame.ini` is open (the design's Q2).
  - Under Reference: `NotYet { NotPorted }` (the C++ has no map load).
- **`mapload::bringup::bring_up_map_for_play(world, input, options)`** (new): the route mode is checked first, then the
  game info routine, then the existing `bring_up_for_play`, so the game info's own script runs. `bring_up_for_play`
  and its refusals are unchanged; its `NotYet` for an empty `WorldInfo.Game` now names the map route.
- **The navigation list** is not built: where `WorldInfo.NavigationPointList` and `nextNavigationPoint` come from at
  load is open (the design's Q1, lane 1's note). It is the route's pending `NotYet { NotRead }`.
- **Marking.** The routine snapshots the heap's committed-write count and the definition store's load transitions
  before its first possible write (a lazy class load counts). A `NotYet` returned after either moved is marked
  "stopped after partial writes", its kind and record kept. The two counters, and the VM's `note_write` calls, are
  copied unchanged from #518 (`origin/fix/rust-483-followup` at 211484a: `ow-object` `heap.rs`, `ow-defs`
  `store.rs`, `ow-vm` `interp/aggregate.rs`, `interp/eval.rs`, `natives_core.rs`, `states.rs`) so that the rebase
  folds once #518 lands.
- **The world test fixture** gains a `WorldInfo.DefaultGameType` class property (and the `ClassProperty` import).
- **Tests** (invented packages): the ini reader and the `GAME=` parse; the configured default spawned, appended and
  stored, with no begin-play event; the second call a no-op; the six refused cases and Reference with nothing written
  or loaded; a missing class, an unfit class and a spawn refused for collision, the field left empty; the marking on
  the counters; the map route raising the game info's `InitGame` first and routing it with the map's actors.
- **Installed (ignored test, local only, under the machine lock).** It asserts the hub's scope (review r1 of #530):
  the class chosen (`WillowGame.WillowCoopGameInfo`, from the installed `DefaultGame.ini`), the spawn, `WorldInfo.Game`
  holding it, and the typed stop on its own path. That stop is `GotoState('Auto')` (the auto-state flag bit) in both
  its `InitGame` and its `PreBeginPlay`, each `NotYet` and marked "stopped after partial writes". The auto state is a
  separate lane. Reported, not asserted: `WorldInfo.GRI` holds a `WillowGameReplicationInfo` (made by script before
  the stop), and the GRI's `MissionTracker` stays None. 1,855 failures over the map, the same count as decision
  2026-11-19-20's harness run.
- **Not done here.** The navigation list (left as the `NotYet` above). `WillowGlobals.GetWillowGlobals` stays unbound:
  the game info's path does not reach it, and on Sanctuary it is the recorded stop of 92 `PreBeginPlay` events, first
  on `WillowGame.WillowInteractiveObject`. No handler is bound without a globals object behind it.
- **Gates.** The installed Off `--slice-gate`, strict, under the machine lock, on origin/main d52bcc7 and on this
  branch: 1 ok / 4 NotYet / 9 errors both times, every row byte-identical (no slice driver uses the map world yet).
