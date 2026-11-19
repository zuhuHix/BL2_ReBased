## 2026-11-19: the map route's follow-ups (review r1 of #556)

AI-assisted (Claude Code, Opus 5.5). Lane MAP-ROUTE-FOLLOWUP, implementer. The decision number is pending from the hub
(three digits); NNN is a placeholder.

- **Base:** fix/rust-slice-map-world at 07b1ffda (#556, on #547).
- **Scope:** the five findings the review of #556 parked. Off only: every change is behind the adopted bridge or in
  argument parsing, so Reference and the default route are unchanged.
- **No real-game session:** **UNVERIFIED** in the running game.
- **Unchanged:** no dependency was added and no bounds check was loosened.

**What changed** (`crates/ow-mission/src/bridge.rs` and its tests, plus `ow-cli`'s `--map` parsers):
1. **The globals definition on the map route** is the adopted globals' own `TheGlobalsDefinition`, not a fresh
   lookup of `GD_Globals.General.Globals` in the mission's package. Unset, it is `NotYet { NotRead }`; so is a
   definition handle the store has not loaded. Nothing is loaded.
2. **A refusal after adopt's first write** carries the "stopped after partial writes" mark once, with its kind kept.
   adopt itself has no natural refusal after that write, because the validation reply's script run records its
   failures, so a test-only injection point raises one there.
3. **`SetGameStage` on the map route** compares against the controller's possessed `Pawn`, read at the call. The old
   route keeps its cached pawn.
4. **The director and the waypoint on the map route.** No object is made for either:
   - `place_marcus` refuses typed, before any write, because the map's own Marcus (placed, or spawned by its
     population) is not identified yet;
   - `place_waypoint` takes the actor the map load placed from that export, identified by its export origin, without
     running `PostBeginPlay` again (the bring-up already routed it). A map that did not place it is a typed refusal.
5. **`--map` parsing** for `--combat-run` and `--slice-gate` moved into two small functions, `map_option` and
   `parse_args`, each with a test: the name and the steps, a missing name, the option's place, and Reference.

**Tests:**
- ow-mission: 213 passed / 0 failed / 1 ignored, with four new adopt tests;
- ow-cli: the two parse tests;
- workspace (`-j 2`): 2,474 passed / 0 failed / 48 ignored.

**The installed Off `--slice-gate`** (strict, under the machine lock, pc-hummer), on this head and on the base
07b1ffda:
- **default route:** ok 1, NotYet 4, error 9 of 14 on both, every row identical. Link 1 is at `maya:8` on the unbound
  `GameEngine.GetOnlineSubsystem()`;
- **`--map Sanctuary_P`:** ok 1, NotYet 4, error 9 of 14 on both, every row identical. Link 1 runs 0 steps and stops
  at `setup` on the unbound `WillowGameInfo.IsEditor()`, before adopt;
- no link changes. These follow-ups act after adopt, so the slice cannot reach them until the map route's setup
  passes.

**Review r1 of #571:**
1. **The waypoint:** on the map route, `place_waypoint` checks for the adopted world before any package load. It
   matches the world's placed actors by their own export origins and the package metadata the store already holds,
   and loads nothing. An uncached or unknown package is the typed refusal, and so is a missing export (it was a store
   error or `false` before).
2. **The globals definition:** on the map route, the adopted globals' `TheGlobalsDefinition` is read on every call and
   never cached. A field cleared later, or pointed at an unloaded handle, is the typed refusal, with nothing loaded.
   The default route keeps its cache.
3. **Tests:** both cases are added to the adopt tests. The workspace (`-j 2`) passes 2,474 / 0 / 48 ignored.
