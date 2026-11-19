## 2026-11-19: Native map load second pass and travel countdown: analyst notes, all NotYet

AI-assisted (Claude Code, Opus 5.5). Lane MAP-LOAD-B plus TRAVEL-COUNTDOWN, **analyst only** (clean room,
docs/LANE_BRIEF.md). No crate code changed. The rules read here stay `NotYet` in Off until a fresh implementer codes
them from the notes. There was no real-game session: everything is **UNVERIFIED** in the running game.

**Tools.** Ghidra 12.1.4 on the analysed project, opened read-only: 8 `tools/ghidra/run.ps1` query runs and 2 runs of
a small local read-only GhidraScript under ignored `local/analysis/` (reads table entries, scans data for a code
pointer). Each run was one at a time, under the machine lock, after checking that no other process held the project.
Raw output stays only under ignored `local/analysis/mapload-b/`. Field names come from `tools/ghidra/class_layout.py`.
`WorldInfo` could not be laid out whole because of its map properties; every field used here lies before them, and
`WorldInfo.Game` falls where the bring-up stores the game info whatever size a map is given. Event names come from the
engine's start-up table of global names.

**What changed (docs only).**

- `docs/verification/NATIVE_MAP_LOAD.md`: a second pass over sections 2 to 4 and the open list. These are the items
  that unblock `bring_up_for_play` (decision 2026-11-19-04's questions).
  - **The begin-play event identities and order** (high for the events).
    - The routing's first per-actor virtual is the shared `Actor` routine that raises `PreBeginPlay`. The second is
      the one that raises `PostBeginPlay` and then, unless `bDeleteMe`, `SetInitialState`.
    - Each event is gated by the actor's state probe. On a fresh actor outside any state, both are raised (medium).
    - `WillowGameInfo`'s first slot holds an override: the shared routine, then one more engine call.
    - The slot identity is read from `WillowGameInfo`'s table. 262 tables in the executable hold the shared pair in
      the same two adjacent slots.
    - Before the routing, the game info's script `InitGame(Options, Error)` runs when the game info is not yet
      script-initialised.
  - **The flag that routes a static actor** is `bRouteBeginPlayEvenIfStatic` (high, named by the `Actor` layout).
  - **A correction.** The per-actor skip test is `bScriptInitialized`, not "pending deletion". A static actor that
    does not route only gets `bScriptInitialized` set.
  - **The actor initialisation pass.**
    - The server does nothing role-related; on a client, non-static, non-`bNoDelete` actors are destroyed and the
      others swap roles once.
    - The physics-level filter (`bLoadIfPhysXLevel0/1/2`) removes or hides actors.
    - It resets `LastRenderTime`, `Touching` and `PhysicsVolume`.
  - **The game info's class choice** (high for the field and the event, medium for the branch order). It reads the
    `GAME=` option and tests `WorldInfo.DefaultGameType`. With that set, or when the `GAME=` class loads, it raises the
    script `SetGameType(MapName, Options, Portal)` on a class default object. Otherwise it uses `DefaultGame` from the
    ini. The receiver per branch is open.
  - **The time reset**: the three fields are now named by the layout (high).
  - **The URL options**: without `load`, the world keeps a copy of the URL.
  - **Setup calls**: still not read.
  - **`SpawnPlayActor`**: on the server it gets the id from the local player's script `GetUniqueNetId`. It then calls
    the world's play-actor spawn: the game info's script `Login`, then the controller's index, player, `Role` and
    `RemoteRole`, then the game info's script `PostLogin`.
  - **Bring-up tail**: `StartMenuTimer` and a memory-leak command, both automated-testing only.
- `docs/verification/NATIVE_TRAVEL.md`, new: `WillowGameInfo.TravelPostCountdownInProcess()` answers
  `TravelPostCountdownTime > 0`.
  - The field is a script int with class default 0, so a fresh game info answers false. Installed script sets it to 3
    in `InitiateTravel` and to 0 in `ClearTravelData`.
  - The script native's code is folded with an unrelated native by the linker. The virtual it calls was resolved in
    `WillowGameInfo`'s table, read from that class's constructor.
  - This is link 4's stop at every `--combat-run` swing in Off (decision 2026-11-19-06).
- `docs/verification/NATIVE_INDEX.md`: three rows (the travel native, `LocalPlayer.SpawnPlayActor` and the engine's
  bring-up).

**What is still open** (in the note's "Open" list):

- the mode argument and the time-reset flag the bring-up passes (stock UE3 routes every actor; not confirmed);
- the receiver of `SetGameType` per branch;
- the unread engine calls;
- where the map load calls `SpawnPlayActor`, and where `GamePlayers` is filled;
- bl-sdk reads of the event order, the static actors that route, the game info class and the `Login` arguments.

**Not done.** No implementation (a different thread implements). No real-game session.

**Checks.** Docs and decision only. The gates and the scan of the notes for listing patterns are in the hand-back; the
notes hold no addresses, function names from the listing, pseudo-code or listing structure.
