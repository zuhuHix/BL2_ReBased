## 2026-11-19: Rust map load local player in Off: the local-player spawn through Login and PostLogin

AI-assisted (Claude Code). Lane IMPL-MAPLOAD-PLAYER, lane 3 of the map-load design (docs/rust-port/SLICE_MAPLOAD.md,
final at fee80eb on `research/rust-slice-mapload`, #530). It is coded from NATIVE_MAP_LOAD.md sections 4 and 6
(section 6.3 at 43a75d9c on `research/rust-mapload-navlist`, #536) and the hub's binding corrections, in a clean room:
this lane did not analyse those notes, and no analysis output was opened. It is stacked on lane 2
(`fix/rust-mapload-gameinfo`, #537). There was no real-game session, so every rule is **UNVERIFIED** in the running game.
No dependency was added, no bounds check was loosened and no existing `NotYet` guard was relaxed. Reference is unchanged.

- **`mapload::player::spawn_play_actor(world, player, input)`** (new, `ow-world`, Off only). It is one routine: the
  engine's local-player spawn, which the map load calls directly for each local player after the bring-up. The
  script's `LocalPlayer.SpawnPlayActor` reaches the same routine. In this port the harness calls it; the call site is
  UNVERIFIED, and the script wrapper is not bound.
  - **The preflight writes nothing.** It refuses, unmarked, each as a `NotYet`:
    - not Off (`NotPorted`);
    - no game info in `WorldInfo.Game`;
    - a network client;
    - a `GetUniqueNetId` answer the port does not model (anything but the empty id);
    - no single-player remote role stated (the caller states it; the engine's value is not read);
    - a local player that already has a play actor.

    A player missing from the `GamePlayers` list given is a data error.
  - **In order:**
    1. the player's index in `GamePlayers`, and the empty id (the parameter's zero value);
    2. `Login(Portal, Options, UniqueId, out ErrorMessage)` on the game info, with the URL text's joined options and
       its portal (no `Name=` option is added);
    3. on the returned controller, `NetPlayerIndex`, then the attachment (`Player` = the local player), then `Role`
       (authority) and `RemoteRole`;
    4. `PostLogin(NewPlayer)`;
    5. the local player's `Actor` = the controller;
    6. last, on success only, `Actors::add_local_player`.

    The controller's four fields are read before the first write to it. The `Login` lookup runs after the counters'
    mark, because it can load the function.
  - **`Login` returning None.** With an error text it is the load's failure, `PlayActorOutcome::LoginFailed`, the text
    unchanged. What the map load then does is not read (by stock convention the load fails). With an empty text it is
    `NotYet { NotRead }`, "Login returned None without an error": the hub's wording, where the note says "the map load
    after a play-actor spawn that gave no actor and no error". Either way nothing after `Login` runs.
  - **Marking.** A refusal after script has run (inside `Login` or `PostLogin`, or after them) is marked "stopped after
    partial writes" when one of #518's counters moved, keeping its kind and record. A preflight refusal is never
    marked.
- **`make_local_player(world, class)`**: a local player from the class defaults, standing in for what
  `CreateInitialPlayer` makes at engine start-up, before any map. It is a harness input; `ControllerId` stays 0, the
  note's port answer.
- **The world test fixture** gains `LocalPlayer.Actor`, `PlayerController.NetPlayerIndex` and a `ToyGame` actor class
  with native `Login` and `PostLogin`. The tests bind those natives in place of the game's script.
- **Tests** (invented packages):
  - the URL's options and portal;
  - the success order: `Login`'s arguments; at `PostLogin`, the attachment, the net index and both roles already set,
    and the local player's `Actor` not yet set; after it, `Actor` set; the controller at the toy start, and the
    world's local player;
  - `Login` None with an error (the outcome, the text unchanged) and with an empty one (the `NotYet`, marked only when
    `Login` wrote), with no attachment, no `PostLogin` and no local player;
  - the six preflight refusals, unmarked, with nothing written and `Login` not called;
  - Reference.
- **Installed (ignored test, local only, under the machine lock).** The test makes one local player of the configured
  class (`Engine.LocalPlayer`, from the installed `BaseEngine.ini`) before the map. It then runs the map route on
  Sanctuary (lane 2's `WillowGame.WillowCoopGameInfo`, with 1,855 bring-up failures) and the local-player spawn with the
  URL text `Sanctuary_P`, using a harness remote role of 1. The spawn gets through its preflight into `Login`, whose
  script stops at the unbound native `WillowGameInfo.IsEditor()`. That is a plain error, not a `NotYet`, so it is not
  marked. There is no play actor and no local player. `IsEditor` is #508's lane.
- **Also in this branch (review r2 of #537, nits):** the game info's not-found stop uses
  `ow_core::notyet::mark_partial_writes`, and its test asserts the unmarked text when no lazy load ran. A marked
  not-found case was not added: in the synthetic store a package load moves none of #518's three counters.
