# The slice's Sanctuary map load: the smallest load the 14 links need in Off (2026-11-19)

AI-assisted (Claude Code, Opus 5.5). Lane DESIGN-SLICE-MAPLOAD. A design record, not a verification record: it says
what the Rust port already has for loading Sanctuary into a slice world, what is missing, in what order the pieces
must run, and proposes the implementer lanes. There was no real-game session. Nothing here was observed in the running
game, so every game rule named below is **UNVERIFIED** unless a line says where it was confirmed. The hub schedules the
lanes; the maintainer decides what section 7 lists.

**Base:** `origin/main` d52bcc7. Open PRs this design leans on: #513 (`WorldInfo.AllNavigationPoints` in Off), #499 and
#501 (link 1's login checks and the engine object), #522 (`InitializePawnMeshes`). **Sources:**
- the port's code: `ow-world` `world/mapload.rs` and `world/mapload/bringup.rs`, `ow-mission` `bridge.rs`, `ow-cli`
  `cmd/combat.rs` and `cmd/slice_gate.rs`;
- the records: NATIVE_MAP_LOAD.md (sections 1 to 4), and decisions 2026-11-16-76, 2026-11-19-04 and 2026-11-19-20;
- the installed script, read with `research/script_disasm.py` (the game info's begin-play, login and restart
  functions). The listings stay under ignored `local/analysis/mapload/`; only own-words behaviour is here;
- two installed values read with `ow-package --def-dump` into the same folder: `Sanctuary_P`'s `WorldInfo` and its
  player start;
- the installed `DefaultGame.ini`.

This record supersedes the map-load part of SLICE_RUN_PLAN.md section 3.1 (proposed lane W5-MAPLOAD). Most of that
proposal has since landed.

## 1. Where the slice's worlds come from today

No slice link loads a map into its VM world. The drivers build their worlds by hand:

| Links | Driver | World |
|---|---|---|
| 1, 3, 4, 5, 13, 14 | `combat-run` | `combat::world` plus the mission bridge (`MissionScript::new`): a new `WorldInfo`, `WillowGameReplicationInfo`, `MissionTracker`, `WillowPlayerController`, PRI and globals, made from class defaults. `new` refuses a world that already has actors. Link 1's scope says "no map spawn" (decision 2026-11-16-54). |
| 2 | `sanctuary-walk` | The host's own scene load and collision (`walk::run`), not a VM world. It already passes. |
| 6, 7 | `pickup`, `pickup-aimed` | `equip_run::pickup_world`: a hand-placed `WorldInfo` only. |
| 8 | `equip-run` | `equip_run::gate_world`: the same. |
| 9 | `phaselock-cast` | `slice_gate::phaselock_world`: the same. |
| 10 to 12 | `slice-run` | The mission bridge's world, as in links 1 and 3 to 5. |

Today's links do not need the map to reach their current stops. For example, #513 shows link 1 passing
`FindPlayerStart`'s walks over an empty list, because no point is placed. The map matters for what the gate claims:
- **Link 1:** Maya spawns at `WillowCoopPlayerStart_0`, through the game info's own login and restart script. Today
  she is a pawn from class defaults.
- **Links 4 and 10 to 12:** the replication info and the mission tracker are the ones the game info's script makes.
  Today they are the bridge's stand-ins.
- **Links 6 to 9 and 13 and 14:** the local player, the level and its stations are the map's.

Without the map, the gate measures the chain on a world the game never builds.

## 2. What already exists and what is new

| Piece | State | Where |
|---|---|---|
| The level actor lists, persistent and streaming, placed in list order, `WorldInfo` first | **Exists**: 2,363 actors placed and 0 refused, about 17 s | `mapload.rs`, decision 2026-11-16-76 |
| The map's own `WorldInfo` as the world's | **Exists** (`Actors::place_world_info`) | same |
| Bring-up for play: time reset, init pass, `bBegunPlay`, `InitGame`, the two-pass routing (`PreBeginPlay`, `PostBeginPlay`, `SetInitialState`) | **Exists**, but refuses to start until `WorldInfo.Game` is set | `bringup.rs`, decisions 2026-11-19-04 and 2026-11-19-20 |
| `Actor.LocalPlayerControllers` | **Exists** as a hook (`Actors::add_local_player`) | decision 2026-11-16-81 |
| `WorldInfo.AllNavigationPoints` | **Open PR** #513 (walks `NavigationPointList`) | `world/nav_points.rs` on #513 |
| **The game info**: class choice and spawn into `WorldInfo.Game` (NATIVE_MAP_LOAD.md section 3) | **New**. Only the "already has one" branch is coded; the bring-up test uses a harness game info | to add in `mapload/` |
| **`WorldInfo.NavigationPointList`** at load | **New**, and not read: where the list comes from is open (section 5, Q1) | to add in `mapload/` |
| **The GRI and the mission tracker** | **Script**, once a game info exists and its `PreBeginPlay` runs (section 3, step 5) | the installed script |
| **The local player's play actor**: `GamePlayers`, `LocalPlayer.SpawnPlayActor`, the world's play-actor spawn calling `Login` and `PostLogin` (NATIVE_MAP_LOAD.md section 4) | **New**. `Login`, `PostLogin` and `RestartPlayer` are script; the native around them is not bound | to add in `mapload/` |
| **The mission bridge on a map world** | **New**. The bridge must use the map's objects instead of making its own | `ow-mission` `bridge.rs` |

Read from the installed data (local reads, values only):
- `Sanctuary_P`'s `WorldInfo_2` has `DefaultGameType` None, `Game` and `GRI` None, `NetMode` 0, nine
  `StreamingLevels`, and a None `NavigationPointList`.
- `WillowCoopPlayerStart_0` has a None `nextNavigationPoint`, with `bStatic`, `bNoDelete`, `bEnabled` and
  `bPrimaryStart` set.
- So neither the list head nor the links are in the tagged properties. Either the engine builds them at load, or they
  come from data outside the tags (section 5, Q1).
- `DefaultGame.ini` `[Engine.GameInfo]` names `WillowGame.WillowCoopGameInfo` as `DefaultGame`.

## 3. The order: the map world, then the bridge

The bridge's hand-made objects become the map's own, so the order of the bridge's set-up changes. The proposed
sequence for a slice driver (one `World`, Off):

1. **`World::new`** with the driver's packs, as today. The mission bridge splits in two (section 4, lane 4):
   - **Bind** before anything runs. The bridge's natives are bound by path, for their own package
     (`register_native_for`). That includes `WillowGlobals.GetWillowGlobals`, the bring-up's 92 stops in decision
     2026-11-19-20.
   - **Adopt** after the player exists.
   - Natives scoped to "the bridge's objects" resolve those objects from the world when called, not from objects the
     bridge made.
2. **`load_map(Sanctuary_P)`** (exists). This places `WorldInfo_2` first, then the persistent and streaming levels'
   actors.
3. **The navigation list** (new; lane 1 says where it comes from). It fills `WorldInfo.NavigationPointList` and each
   point's `nextNavigationPoint`, before any script runs, so #513's iterator walks real points.
4. **The game info** (new). NATIVE_MAP_LOAD.md section 3 for the branch Sanctuary takes:
   - `DefaultGameType` is None, and the slice URL has no `GAME=` option, so the class is the configured
     `DefaultGame`.
   - In that branch the note says the choice ends there, with no `SetGameType`. The script's own `SetGameType`
     (`GameInfo`'s; neither Willow class overrides it) returns the receiver's class anyway.
   - The game info is spawned through `actors::spawn` and stored in `WorldInfo.Game`.
   - The other two branches stay `NotYet`: a set `DefaultGameType`, and a loadable `GAME=` class. Both need the
     `SetGameType` receiver, which is open.
   - The world has not begun play, so the spawn raises no begin-play events (NATIVE_ENGINE_CORE.md, "Actor.Spawn").
5. **`bring_up_for_play`** (exists). The game info is in the actor list, so in the routing:
   - its `PreBeginPlay` runs the script that spawns the replication info class into `WorldInfo.GRI`;
   - `WillowGameInfo`'s override then spawns the `MissionTracker` into the GRI's `MissionTracker`.
   - Both spawns happen after `bBegunPlay`, so they run their own begin-play events.
   - `InitGame` runs before the routing, with an empty options text.
6. **The local player** (new; NATIVE_MAP_LOAD.md section 4). One local player is put in `GamePlayers`. The world's
   play-actor spawn then asks the game info's `Login` for a controller:
   - the script finds a start through `FindPlayerStart` and spawns the controller there;
   - the native sets the role and the net index and raises `PostLogin`;
   - `WillowGameInfo`'s `PostLogin` runs, and the stock `PostLogin` reaches `RestartPlayer`;
   - `RestartPlayer` calls `FindPlayerStart` again, then `SpawnDefaultPawnFor` and `Possess`.

   The controller goes to `Actors::add_local_player`.
7. **Adopt** (the bridge's second half), done once the player exists. The bridge takes:
   - the `WorldInfo` from the world, the GRI from `WorldInfo.GRI`, and the tracker from the GRI's `MissionTracker`;
   - the controller from `LocalPlayerControllers`, and its PRI from the controller.

   It then does what the C++ bridge did and the script does not do:
   - the mission data validation reply;
   - the tracker's hooks;
   - the playthrough record, while no save is loaded.

   It no longer makes any of those objects.

Today the bring-up refuses unless step 4 has run, and the bridge refuses a world with actors. Both refusals stay for
the old route. The map route is a new entry point, and the old one stays the default until the map route reaches at
least as far on every link (section 6).

## 4. Proposed lanes, in dependency order

| # | Lane | Kind | Owner | Depends on | Unblocks |
|---|---|---|---|---|---|
| 1 | ANALYST-MAPLOAD-NAVLIST-PLAYACTOR | analyst (clean room) | NATIVE_MAP_LOAD.md, new sections 5 and 6 | none | lanes 2 and 3 |
| 2 | IMPL-MAPLOAD-GAMEINFO | implementer | `ow-world` `world/mapload/game_info.rs` (new) and `bringup.rs` | lane 1 only for the navigation list part | links 4 and 10 to 12 (GRI and tracker from script); link 1's `Login` |
| 3 | IMPL-MAPLOAD-PLAYER | implementer | `ow-world` `world/mapload/player.rs` (new) | lanes 1 and 2, #513 | link 1 (the map spawn); links 6 to 9 (the map's local player) |
| 4 | IMPL-SLICE-MAP-WORLD | implementer | `ow-mission` `bridge.rs`; `ow-cli` `cmd/combat.rs`, `cmd/slice_gate.rs` | lanes 2 and 3 | every VM link measured on the map world |

**Lane 1, analyst.** From the executable and the level data, in own words:
- **Q1:** where `NavigationPointList` and `nextNavigationPoint` come from at load. Three candidates: data in the
  level's untagged tail after the URL, a rebuild over the placed points, or each level's list joined onto the
  persistent one when it is added. Also which classes join, and in what order across streamed levels.
- **Q3:** where the map load calls `SpawnPlayActor`, and where `GamePlayers` is filled.
- If no lane owns it yet, also the auto-state flag bit (`GotoState('Auto')`). It is the bring-up's largest stop, and it
  also ends `InitGame` and the game info's `PreBeginPlay` (decision 2026-11-19-20).

Hand-back: the note's new sections with confidence levels, no implementation.

**Lane 2, the game info** (from NATIVE_MAP_LOAD.md section 3 and lane 1, clean room). Contents:
- The class choice for the "`DefaultGameType` None, no `GAME=`" branch: the configured `DefaultGame`, read from the
  installed `DefaultGame.ini` by a small own reader. The user's `WillowGame.ini` may override it (section 5, Q2).
- The spawn into `WorldInfo.Game`.
- `NotYet` for the other branches.
- With lane 1's answer, the navigation list.
- `bring_up_for_play` then takes a map whose game info the load chose, so the harness game info stays only in the
  installed test.

Tests, on a synthetic map package built with the `package()` / `tag()` helpers:
- the branch taken;
- the refused branches;
- an ini without the key, as `NotYet`;
- the spawn order;
- the list built in the rule lane 1 gives;
- the installed Sanctuary test (ignored by default) reports the class and the GRI and tracker the script made.

**Lane 3, the local player** (from NATIVE_MAP_LOAD.md section 4 and lane 1). Contents:
- one local player in `GamePlayers`;
- `LocalPlayer.SpawnPlayActor` bound in Off, with the world's play-actor spawn: `Login`, then role, net index and
  `PostLogin`;
- `add_local_player`;
- an empty `GetUniqueNetId` answer, or `NotYet` if lane 1 leaves it open.

Tests:
- a toy game info whose `Login` spawns a controller at the toy start, with `PostLogin` recorded;
- a `Login` that returns None, giving the error string;
- the play actor in `LocalPlayerControllers`;
- Reference unbound.

**Lane 4, the slice on the map world.** Contents:
- The bridge's bind and adopt split (section 3, steps 1 and 7).
- A `--map <name>` input on `--combat-run` and `--slice-run`.
- A slice-gate option that runs the VM links on the Sanctuary map world. Each row adds a harness line with the map,
  the bring-up's failure count, and its first stop on the game info's and the player's own path.

Tests:
- the adopt path on a synthetic map world, with the same tracker natives answering;
- the old route unchanged, byte for byte, on the existing tests;
- the installed gate run both ways, under the machine lock, with every row compared.

Lanes 2 and 3 can run in parallel once lane 1 lands, as long as lane 3 starts from lane 2's spawn API. Lane 4 is last.

**The first new stop expected.** Once lane 2 spawns the game info, the bring-up reaches the game info's own events. On
decision 2026-11-19-20's installed run (a harness `WillowGameInfo`), `InitGame` and the game info's `PreBeginPlay`
both stopped at `GotoState('Auto')`, the auto-state flag bit. So the first new stop on the map route is expected
there, before the GRI exists. It is UNVERIFIED that `WillowCoopGameInfo` stops at the same place. Past it, the
recorded next stops are `Actor.SetPhysics`, `SetShadowParentOnAllAttachedComponents`, `Core.BoolProperty` as an
engine-native class, and the physics level filter. Those are on other actors, not on the player's path.

## 5. Open questions (UNVERIFIED until settled)

- **Q1. The navigation list.** It is None in the tagged data of `Sanctuary_P` (section 2). Is it built at load or read
  from untagged data? How do the streamed levels' points join, and in what order? Which placed classes are
  `NavigationPoint` subclasses? `Sanctuary_P` and `Sanctuary_Dynamic` hold player starts, station exit points,
  destinations, pylons, pylon seeds and path target points; the lane counts which of them qualify. (Lane 1.)
- **Q2. The game info class.** Does the travel URL into Sanctuary carry a `GAME=` option? Does the user's
  `WillowGame.ini` override `DefaultGame.ini`? A bl-sdk read of `WorldInfo.Game`'s class in Sanctuary settles both.
- **Q3. The play actor.** Where the map load calls `SpawnPlayActor`, and where `GamePlayers` is filled
  (NATIVE_MAP_LOAD.md, Open). (Lane 1.)
- **Q4. Grading an incomplete bring-up.** Decision 2026-11-19-20 counted 1,855 failures over Sanctuary's actors, and
  `BringUp::complete` is false for any failure. The question is whether a link may pass on a map world whose bring-up
  is not complete, if every failure is off that link's path. This design proposes: such a link is reported with the
  failure count as a harness line and keeps its own stop. That changes what the gate means, so it is the maintainer's
  call.
- **Q5. Streaming.** Whether every level `StreamingLevels` names is loaded at map load. This is the map load's
  existing UNVERIFIED.
- **Q6. What the bridge still adds.** Some of the bridge's hand-made state is neither script nor a save: the empty
  mission playthrough, the PRI list and the authority role. The question is whether the game makes these during login,
  from the save, or elsewhere. The answer decides what lane 4's adopt step may still write. Any write it keeps is a
  labelled harness input.
- **Q7. The director.** The bridge copies Marcus from his archetype. Whether Sanctuary places him (in a streamed level,
  or spawned by Kismet or population) is not checked. It matters for links 10 to 12 on the map world, after lane 4.

**What a bl-sdk session would settle** (when the maintainer grants one): Q2, the class of `WorldInfo.Game`; the
`WorldInfo.NavigationPointList` head and its length after load (Q1); `LocalPlayerControllers` and the player's start
after load; the order of `PreBeginPlay`, `PostBeginPlay` and `SetInitialState` for the game info and the GRI.

## 6. Acceptance

- Every lane runs `cargo build --workspace` and `cargo test --workspace`, and the installed Off `--slice-gate` under
  the machine lock, before and after.
- On the default route, rows must not change.
- Lane 4 reports the map route's rows next to them.
- The map route becomes the default only when, on every link, it reaches the same driver step or a later one. That
  switch is a hub decision, with the maintainer's OK for link 1's scope text (decision 2026-11-16-54).

## 7. For the maintainer

- Q4, whether a map world with an incomplete bring-up may grade a link.
- Changing link 1's scope from "the mission bridge's player from class defaults; no map spawn" to the map spawn, when
  lane 4 is ready.
- A bl-sdk session for the reads in section 5, when convenient. None is needed to start lanes 1 to 3.
