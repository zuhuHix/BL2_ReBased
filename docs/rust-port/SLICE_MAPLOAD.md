# The slice's Sanctuary map load: the smallest load the 14 links need in Off (2026-11-19)

AI-assisted (Claude Code, Opus 5.5). Lane DESIGN-SLICE-MAPLOAD. A design record, not a verification record: it says
what the Rust port already has for loading Sanctuary into a slice world, what is missing, in what order the pieces
must run, and proposes the implementer lanes. There was no real-game session. Nothing here was observed in the running
game, so every game rule named below is **UNVERIFIED** unless a line says where it was confirmed. The hub schedules the
lanes; the maintainer decides what section 7 lists.

**Base:** `origin/main` d52bcc7. Open PRs this design leans on: #513 (`WorldInfo.AllNavigationPoints` in Off), #499 and
#501 (link 1's login checks and the engine object), #522 (`InitializePawnMeshes`). **Sources:**
- the port's code: `ow-world` `world/mapload.rs` and `world/mapload/bringup.rs`, `ow-mission` `bridge.rs` and
  `slice.rs`, `ow-inventory` `globals.rs`, `ow-cli` `cmd/combat.rs`, `cmd/slice.rs`, `cmd/inventory/equip_run.rs` and
  `cmd/slice_gate.rs`;
- the records: NATIVE_MAP_LOAD.md (sections 1 to 4), NATIVE_WILLOW_GLOBALS.md, and decisions 2026-11-16-76,
  2026-11-19-04 and 2026-11-19-20;
- the installed script, read with `research/script_disasm.py` (the game info's begin-play, login and restart
  functions). The listings stay under ignored `local/analysis/mapload/`; only own-words behaviour is here;
- two installed values read with `ow-package --def-dump` into the same folder: `Sanctuary_P`'s `WorldInfo` and its
  player start;
- the installed `DefaultGame.ini`.

This record replaces the map-load part of SLICE_RUN_PLAN.md section 3.1 (its proposed lane W5-MAPLOAD). SLICE_RUN_PLAN.md
now links here instead.

**Review r1 of #530, folded in.** Seven changes:
1. Lane 2's installed test no longer expects a script-made GRI and tracker. Both wait on the auto-state work.
2. Section 3 now says who makes the globals before bring-up.
3. Lane 4 names its constructors and fixtures, and its first PR is bounded.
4. Section 4 now gives lane 3's dependency on lane 2: coding order, merge order and who owns the module wiring.
5. Section 3 now has the full play-actor steps of NATIVE_MAP_LOAD.md section 4.
6. SLICE_RUN_PLAN.md is updated.
7. The pickup row is corrected, and the Q2 evidence is split into three reads.

## 1. Where the slice's worlds come from today

No slice link loads a map into its VM world. The drivers build their worlds by hand:

| Links | Driver | World |
|---|---|---|
| 1, 3, 4, 5, 13, 14 | `combat-run` | `combat::world`, then `combat::MissionBridge::new`, which calls `MissionScript::new` (`ow-mission` `bridge.rs`). That makes a new `WorldInfo`, `WillowGameReplicationInfo`, `MissionTracker`, `WillowPlayerController`, PRI and `WillowGlobals` from class defaults, puts the globals in `WorldQuery`, and binds the bridge's getters. `new` refuses a world that already has actors. The player's pawn comes from `MissionScript::player_pawn`, which makes a `WillowPlayerPawn` from class defaults when its cache is empty. Link 1's scope says "no map spawn" (decision 2026-11-16-54). |
| 2 | `sanctuary-walk` | The host's own scene load and collision (`walk::run`), not a VM world. It already passes. |
| 6, 7 | `pickup`, `pickup-aimed` | `equip_run::pickup_world`, on top of the equip run's world, which has a hand-placed `WorldInfo`. Its own set-up does three things: it makes a `WillowGlobals` from class defaults; it loads the installed globals definition (`GD_Globals.General.Globals`) into its `TheGlobalsDefinition` and records the globals with `WorldQuery::set_globals`; and it preloads the weapon class's default object (decision 2026-11-19-94). `ow-inventory`'s globals getters answer from that object. |
| 8 | `equip-run` | `equip_run::gate_world`: the hand-placed `WorldInfo` only. |
| 9 | `phaselock-cast` | `slice_gate::phaselock_world`: the same. |
| 10 to 12 | `slice-run` | `cmd/slice.rs` builds `ow_mission::bridge::world`, then `FireMissionSlice::new` (`ow-mission` `slice.rs`). That calls `MissionScript::new` directly, so it is the same fresh-world bridge as links 1 and 3 to 5, plus the waypoint, the director and the dummy. |

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
| `Actor.LocalPlayerControllers` | **Exists** as a hook (`Actors::add_local_player`). It is only the controller iterator, not the local player's engine state | decision 2026-11-16-81 |
| The world's globals getters, from `WorldQuery`, `NotYet { LayerAbsent }` without one | **Exists** | `ow-inventory` `globals.rs`, decision 2026-11-19-94 |
| `WorldInfo.AllNavigationPoints` | **Open PR** #513 (walks `NavigationPointList`) | `world/nav_points.rs` on #513 |
| **The game info**: class choice and spawn into `WorldInfo.Game` (NATIVE_MAP_LOAD.md section 3) | **New**. Only the "already has one" branch is coded; the bring-up test uses a harness game info | to add in `mapload/` |
| **`WorldInfo.NavigationPointList`** at load | **New**, and not read: where the list comes from is open (section 5, Q1) | scoped after lane 1 |
| **`GotoState('Auto')`**: which state flag bit marks an auto state | **Not read** (NATIVE_ENGINE_CORE.md). It stops the game info's own `InitGame` and `PreBeginPlay` (decision 2026-11-19-20) | lane ANALYST-AUTO-STATE, then an implementer |
| **The GRI and the mission tracker** | **Script**, once a game info exists, its `PreBeginPlay` runs, and `GotoState('Auto')` is answered (section 3, step 5) | the installed script |
| **The local player's play actor**: `GamePlayers`, `LocalPlayer.SpawnPlayActor`, the world's play-actor spawn calling `Login` and `PostLogin` (NATIVE_MAP_LOAD.md section 4) | **New**. `Login`, `PostLogin` and `RestartPlayer` are script; the native around them is not bound | to add in `mapload/` |
| **The mission bridge on a map world** | **New**. The bridge must use the map's objects instead of making its own | `ow-mission` `bridge.rs` and `slice.rs` |

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

1. **`World::new`** with the driver's packs, as today. The bridge's natives are bound now, by path, for their own
   package (`register_native_for`, #519), except the two globals getters (below).
   - Natives that the bridge scopes to "its objects" (the tracker's, the controller's) resolve those objects from the
     world when called, not from objects the bridge made. Before the bridge adopts (step 7), a call to one of them is
     a typed `NotYet { LayerAbsent }`, naming the bridge's adoption. It never answers from an object the bridge would
     otherwise have made.
2. **The globals** (review r1 of #530, item 2), before any script runs. On the map route, the driver's set-up makes the
   world's one `WillowGlobals`, from class defaults:
   - its `TheGlobalsDefinition` is the installed definition `GD_Globals.General.Globals`, loaded here;
   - it is recorded with `WorldQuery::set_globals`;
   - in Off, the pawn data manager's globals set-up runs then, as `combat::MissionBridge::new` runs it today.

   This is a **harness input**, labelled as one. Where and when the engine makes its globals object was not read. The
   definition is the configured `DefaultGlobalsDefinitionName` (NATIVE_WILLOW_GLOBALS.md section 2). The pawn data
   set-up's timing is NATIVE_PAWN_DATA.md's open question 1.
   - **Who answers `GetWillowGlobals` and `GetGearboxGlobals`:** on the map route, `ow-inventory`'s globals pack, the
     world's getters, which answer from `WorldQuery`. The bridge does not bind its own getters on this route: it reads
     the world's globals, and so does the pickup route.
   - On the old routes nothing changes. `MissionScript::new` makes the globals and binds its own getters, and the pickup
     route keeps `pickup_world`.
   - **A call before this step**, for example from a bring-up run with no globals set-up, gets the world getter's
     `NotYet { LayerAbsent }`. It is never None: None would let `PostBeginPlay` skip work silently (decision
     2026-11-19-94). So a missing set-up shows as a typed stop.
3. **`load_map(Sanctuary_P)`** (exists). This places `WorldInfo_2` first, then the persistent and streaming levels'
   actors.
4. **The navigation list** (new). It fills `WorldInfo.NavigationPointList` and each point's `nextNavigationPoint` before
   any script runs, so #513's iterator walks real points. Scoped once lane 1 says where the list comes from (section
   4).
5. **The game info** (new). NATIVE_MAP_LOAD.md section 3 for the branch Sanctuary takes:
   - `DefaultGameType` is None, and the slice URL has no `GAME=` option, so the class is the configured
     `DefaultGame`.
   - In that branch the note says the choice ends there, with no `SetGameType`. The script's own `SetGameType`
     (`GameInfo`'s; neither Willow class overrides it) returns the receiver's class anyway.
   - The game info is spawned through `actors::spawn` and stored in `WorldInfo.Game`.
   - The other two branches stay `NotYet`: a set `DefaultGameType`, and a loadable `GAME=` class. Both need the
     `SetGameType` receiver, which is open.
   - The world has not begun play, so the spawn raises no begin-play events (NATIVE_ENGINE_CORE.md, "Actor.Spawn").
6. **`bring_up_for_play`** (exists). `InitGame` runs before the routing, with an empty options text. The game info is in
   the actor list, so in the routing:
   - its `PreBeginPlay` runs the script that spawns the replication info class into `WorldInfo.GRI`;
   - `WillowGameInfo`'s override then spawns the `MissionTracker` into the GRI's `MissionTracker`.
   - Both spawns happen after `bBegunPlay`, so they run their own begin-play events.

   **These events stop today.** On decision 2026-11-19-20's installed run, `InitGame` and the game info's
   `PreBeginPlay` both stopped at `GotoState('Auto')`. So nothing in this step makes a GRI or a tracker until the
   auto-state work lands (ANALYST-AUTO-STATE, then an implementer). Until then, the map route ends here with that typed
   stop.
7. **The local player** (new; NATIVE_MAP_LOAD.md section 4, all of it; review r1 of #530, item 5):
   1. **The engine's local-player list:** one local player is put in `GamePlayers`. Where the engine fills it is open
      (Q3).
   2. **`LocalPlayer.SpawnPlayActor(URL, out Error)`**, on the server:
      - it parses the URL text and adds a `Name=<name>` option when the local player's name is known;
      - it finds the player's index in `GamePlayers`;
      - it asks the local player's script event `GetUniqueNetId` for the player's id. With no online subsystem, the
        answer the port uses is lane 3's to label. Any fallback id is UNVERIFIED;
      - it calls the world's play-actor spawn with the local player, the remote role, the URL, the id, the error string
        and the index.
   3. **The world's play-actor spawn:**
      - it joins the URL options and asks the game info's `Login(Portal, Options, UniqueId, out ErrorMessage)` for a
        controller. In the script, `Login` finds a start through `FindPlayerStart` and spawns the controller there;
      - when one comes back, it sets the controller's net player index;
      - it **attaches the local player to the controller**, before anything else runs on it;
      - it sets `Role` to the authority value and `RemoteRole` to the role passed in;
      - **then** it raises `PostLogin(NewPlayer)`. `WillowGameInfo`'s `PostLogin` runs, and the stock `PostLogin`
        reaches `RestartPlayer`. That calls `FindPlayerStart` again, then `SpawnDefaultPawnFor` and `Possess`;
      - it returns the controller, or None; with None, the error string is the answer.
   4. **`SpawnPlayActor` stores the returned controller as the local player's actor.**
   5. **Then the port's hook:** `Actors::add_local_player(controller)`, so `LocalPlayerControllers` yields it. This
      gives the iterator only. It is not the attachment of step 3 or the store of step 4, and does not replace them.

   Where the map load calls `SpawnPlayActor` is open (Q3). The lane calls it once after the bring-up, as a labelled
   harness call site, UNVERIFIED.
8. **Adopt** (the bridge's second half), once the player exists. The bridge takes:
   - the `WorldInfo` from the world, the GRI from `WorldInfo.GRI`, and the tracker from the GRI's `MissionTracker`;
   - the controller from `LocalPlayerControllers`, its PRI from the controller, and its possessed `Pawn`;
   - the world's globals from `WorldQuery`.

   It then does what the C++ bridge did and the script does not do:
   - the mission data validation reply;
   - the tracker's hooks;
   - the playthrough record, while no save is loaded.

   It makes none of those objects. On this route the player-pawn accessor returns the controller's possessed `Pawn`. It
   refuses, typed, when there is none, instead of making one from class defaults.

Today the bring-up refuses unless step 5 has run, and the bridge refuses a world with actors. Both refusals stay for
the old route. The map route is a new entry point, and the old one stays the default until the map route reaches at
least as far on every link (section 6).

## 4. Proposed lanes, in dependency order

| # | Lane | Kind | Owner | Depends on | Unblocks |
|---|---|---|---|---|---|
| 1 | ANALYST-MAPLOAD-NAVLIST-PLAYACTOR (9554) | analyst (clean room) | NATIVE_MAP_LOAD.md, new sections 5 and 6 | none | the navigation list; lane 3's open call site |
| 2 | IMPL-MAPLOAD-GAMEINFO (H6) | implementer | `ow-world` `world/mapload/game_info.rs` (new); `bringup.rs`; the `mapload.rs` module wiring | none for its first PR | lane 3's merge; with the auto-state work, links 4 and 10 to 12 |
| — | ANALYST-AUTO-STATE, then an implementer | analyst, then implementer | NATIVE_ENGINE_CORE.md or its own note; `ow-vm` `states.rs` | none | the game info's `InitGame` and `PreBeginPlay`, so the GRI and the tracker; lane 2's installed assertions past the stop; lane 4 |
| 3 | IMPL-MAPLOAD-PLAYER | implementer | `ow-world` `world/mapload/player.rs` (new) | coding: none; merge: after lane 2 | link 1 (the map spawn); links 6 to 9 (the map's local player) |
| 4 | IMPL-SLICE-MAP-WORLD | implementer | `ow-mission` `bridge.rs` and `slice.rs`; `ow-cli` `cmd/combat.rs`, `cmd/slice.rs`, `cmd/slice_gate.rs` | lanes 2 and 3, the auto-state implementer, #519 | every VM link measured on the map world, one driver family per PR |

**Lane 1, analyst (9554).** From the executable and the level data, in own words:
- **Q1:** where `NavigationPointList` and `nextNavigationPoint` come from at load. Three candidates: data in the
  level's untagged tail after the URL, a rebuild over the placed points, or each level's list joined onto the
  persistent one when it is added. Also which classes join, and in what order across streamed levels.
- **Q3:** where the map load calls `SpawnPlayActor`, and where `GamePlayers` is filled.

Hand-back: the note's new sections with confidence levels, no implementation. **The navigation work is scoped from that
answer**, not before:
- if the list is read from the level's data, a new reader with package oracles (rule 4);
- if it is a rebuild, an own-words rule.

It is a small item of its own, or added to lane 2 if the hub prefers. Until then, #513's iterator walks the empty list,
as it does today.

**Lane 2, the game info** (H6; from NATIVE_MAP_LOAD.md section 3, clean room). It **proves class selection, the spawn
and the typed bring-up stop, and nothing further**. Contents:
- The class choice for the "`DefaultGameType` None, no `GAME=`" branch: the configured `DefaultGame`, read from the
  installed `DefaultGame.ini` by a small own reader. The user's `WillowGame.ini` may override it (section 5, Q2).
- The spawn into `WorldInfo.Game`, through `actors::spawn`.
- `NotYet` for the other branches.
- `bring_up_for_play` then takes a map whose game info the load chose, so the harness game info stays only in the
  installed test of decision 2026-11-19-20.
- **The spawn API:** `mapload::game_info::spawn_game_info(world, options)` (or the name lane 2 picks), returning the game
  info. It exists once lane 2's first PR lands.
- **The module wiring:** lane 2 owns `mapload.rs`'s `mod` lines and `MapReport::pending`.

Tests, on a synthetic map package built with the `package()` / `tag()` helpers:
- the branch taken;
- the refused branches;
- an ini without the key, as `NotYet`;
- the spawn order.

The installed Sanctuary test (ignored by default) asserts three things: the class chosen, the game info in
`WorldInfo.Game`, and that the bring-up stops typed at `GotoState('Auto')` in the game info's `InitGame` or
`PreBeginPlay`, recorded as the stop, not as a pass. It does **not** assert a GRI or a tracker. That assertion moves to
the auto-state implementer's PR, or to a follow-up after it.

**Lane 3, the local player** (from NATIVE_MAP_LOAD.md section 4 and lane 1). Contents:
- one local player in `GamePlayers`;
- `LocalPlayer.SpawnPlayActor` bound in Off, with the world's play-actor spawn: all of section 3, step 7, in that
  order;
- `add_local_player` last;
- `GetUniqueNetId`'s answer, as lane 1 gives it, or a labelled `NotYet`.

**Coding order and merge order** (review r1 of #530, item 4):
- **Coding:** lane 3 can start before lane 2 lands. Its synthetic tests put a toy game info in `WorldInfo.Game`
  themselves, as decision 2026-11-19-20's tests do, and read it through the existing `bringup::game_info`.
- **Merging:** lane 3 lands after lane 2. It rebases onto lane 2's module wiring and adds its `mod player;` line there,
  so the two lanes never both edit `mapload.rs`'s wiring in flight.
- **Lane 1:** where lane 1 has not settled `SpawnPlayActor`'s call site, lane 3 keeps its labelled harness call site.

Tests:
- a toy game info whose `Login` spawns a controller at the toy start;
- a toy `PostLogin` that records what it sees, which **asserts the attachment**: the controller's local player is set
  when `PostLogin` runs, along with the net index and the roles;
- the local player's actor is the controller after the call;
- a `Login` that returns None, giving the error string, with no attachment and no `PostLogin`;
- the play actor in `LocalPlayerControllers`;
- Reference unbound.

**Lane 4, the slice on the map world** (review r1 of #530, item 3). The constructors it changes:
- `MissionScript::new` (fresh world only), reached from `combat::MissionBridge::new` (`--combat-run`) and from
  `FireMissionSlice::new` (`--slice-run`, through `cmd/slice.rs` and `ow_mission::bridge::world`);
- the pawn accessor, `MissionScript::player_pawn` (`pawn_for`), which makes a pawn when its cache is empty.

The fixtures that stay on the old route until their own PR: `equip_run::pickup_world` and `gate_world`,
`slice_gate::phaselock_world`, and the slice's waypoint, director and dummy (`FireMissionSlice::new`).

- **First PR, bounded:** the bridge's bind and adopt split (section 3, steps 1 and 8), as a new adopt constructor next
  to `MissionScript::new`, plus **one driver, `--combat-run`** (links 1, 3 to 5, 13 and 14), behind a `--map <name>`
  input.
  - **Proof that it uses the possessed pawn:** a synthetic map world whose toy `Login` and `RestartPlayer` spawn and
    possess a pawn. The adopted bridge's `player_pawn` must be that same object (the controller's `Pawn`), and no
    `WillowPlayerPawn` may be allocated by the bridge. With no possessed pawn, the accessor must refuse, typed.
  - The old route's tests stay unchanged, byte for byte.
  - The installed `--combat-run` with `--map Sanctuary_P` reports its first stop. Until the auto-state work lands, that
    is expected to be the bring-up's typed stop, before the player exists.
- **Later PRs, one family each:**
  - `--slice-run`: `FireMissionSlice::new` on the adopt path, with the waypoint, the director and the dummy (Q7);
  - the pickup, equip and phaselock worlds, with the globals of section 3, step 2;
  - the slice gate's option to run the VM links on the map world. Each row adds a harness line with the map, the
    bring-up's failure count, and its first stop on the game info's and the player's own path.

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
  `WillowGame.ini` override `DefaultGame.ini`? The resulting class alone cannot tell these apart, so they need three
  separate reads (section 5, last paragraph).
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
  mission playthrough, the PRI list and the authority role. The same goes for the globals set-up of section 3, step
  2. The question is whether the game makes these during start-up or login, from the save, or elsewhere. The answer
  decides what lane 4's adopt step and the globals set-up may still write. Any write they keep is a labelled harness
  input.
- **Q7. The director.** The bridge copies Marcus from his archetype. Whether Sanctuary places him (in a streamed level,
  or spawned by Kismet or population) is not checked. It matters for links 10 to 12 on the map world, in lane 4's
  `--slice-run` PR.

**What a bl-sdk session would settle** (when the maintainer grants one). For Q2, three separate reads:
- the URL Sanctuary was loaded with (its options);
- the effective `[Engine.GameInfo] DefaultGame` value the running game holds after the config merge;
- the class of `WorldInfo.Game`.

Also:
- the `WorldInfo.NavigationPointList` head and its length after load (Q1);
- `LocalPlayerControllers`, the local player's actor and the player's start after load;
- the order of `PreBeginPlay`, `PostBeginPlay` and `SetInitialState` for the game info and the GRI.

## 6. Acceptance

- Every lane runs `cargo build --workspace` and `cargo test --workspace`, and the installed Off `--slice-gate` under
  the machine lock, before and after.
- On the default route, rows must not change.
- Each of lane 4's PRs reports the map route's rows for its driver family next to them.
- The map route becomes the default only when, on every link, it reaches the same driver step or a later one. That
  switch is a hub decision, with the maintainer's OK for link 1's scope text (decision 2026-11-16-54).

## 7. For the maintainer

- Q4, whether a map world with an incomplete bring-up may grade a link.
- Changing link 1's scope from "the mission bridge's player from class defaults; no map spawn" to the map spawn, when
  lane 4's `--combat-run` PR is ready.
- A bl-sdk session for the reads in section 5, when convenient. None is needed to start lanes 1 to 3.
