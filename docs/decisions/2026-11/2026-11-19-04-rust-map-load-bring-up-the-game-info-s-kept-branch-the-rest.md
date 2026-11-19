## 2026-11-19: Rust map load bring-up: the game info's kept branch, the rest NotYet with the note's open questions

AI-assisted (Claude Code). Lane MAP-LOAD-NATIVES, the fresh implementer of the W5-MAPLOAD clean-room pair (decision
2026-11-16-76). This lane coded from `docs/verification/NATIVE_MAP_LOAD.md` alone and opened no analysis output. There
was no real-game session, so every rule here is **UNVERIFIED** in the running game. No dependency was added, no bounds
check was loosened and no existing `NotYet` guard was relaxed. Reference is unchanged, because the C++ has no map load
(R3).

- **What was added.** `crates/ow-world/src/world/mapload/bringup.rs`, with tests, is the module
  `ow_world::world::mapload::bringup`.
  - `bring_up_for_play(world)` runs the rules the note settles and lists every other step as a `NotYet { NotRead }`
    naming the note. Today it changes nothing in the world.
  - `game_info(world)` is section 3's routine. When the persistent `WorldInfo.Game` is set, it returns that game info
    and does nothing else. The note rates that branch high confidence. The routine's other no-op case, a network or
    demo driver, cannot occur, because this world has no network layer. Otherwise it returns `NotYet` for the class
    choice. Without a placed `WorldInfo` it returns `NotYet { LayerAbsent }` naming the map load.
  - `begin_play_reach(world)` counts what section 2.1's routing would reach: pending deletion (skipped), not static
    (pass 1 reaches them) and static (open, see below). It also reports the `WorldInfo`'s `bBegunPlay`.
  - The map load's test helpers became `pub(super)`, and its synthetic `Engine.upk` gained `Actor.bStatic`, a
    `GameInfo` class, `WorldInfo.Game` and `WorldInfo.bBegunPlay`. `load_map` itself and its three `pending` entries
    are unchanged.

- **Kept `NotYet`, because the note leaves the rule open.** These are the questions for a follow-up analyst pass or a
  bl-sdk session:
  1. **The begin-play pass (section 2 step 4, and 2.1).**
     - The per-actor virtuals of pass 1, and pass 2's first call and second event, are "by convention `PreBeginPlay`
       and `PostBeginPlay`, not confirmed". The note's open items ask for them to be confirmed before implementing.
     - Pass 1 routes an actor that "is not static or carries a second flag". That flag is not named, so the case of a
       static actor is open.
     - The initialisation pass resets "a few per-actor fields", which are not named, and makes per-actor engine calls
       that are not identified.
     - Setting `bBegunPlay` and `bStartup` without the routing would mark every placed actor as having had events it
       never got. It would also switch `actors::spawn` to its after-play rules. So the flags are not set either.
  2. **The game info's class choice (section 3).**
     - The world info field that must be zero is not named, and neither is the engine check that selects
       `Engine.GameInfo.DefaultGame`.
     - The URL option parse was not read.
     - The spawn's location, rotation, owner and instigator are not given. The world spawn takes them, and
       `actors::spawn` reads them from a spawner.
     - The project also has no ini reader for `DefaultGame`. That would be a data seam, not taken here.
  3. **The rest of the bring-up (section 2, steps 1, 2, 3 and 5)**, all low confidence or not read:
     - the source of the time-reset flag, and the three fields checked against the `WorldInfo` class layout;
     - the URL options (the four world fields taken when `load` is absent);
     - the setup calls;
     - the per-level call after begin play.
  4. **The local player's play actor (section 4).** The routine `SpawnPlayActor` calls was not read. The local player
     list is already filled by the caller that makes the controller (`Actors::add_local_player`, decision
     2026-11-16-81).

- **Proof.**
  - **Synthetic tests**, three new ones, each run in Off and in Reference:
    - The reach counts: 4 actors, 1 pending deletion, 2 not static, 1 static. The test also checks that the bring-up
      changes no list and does not begin play, and that it returns five `NotRead` entries in the routine's order.
    - A game info that `WorldInfo.Game` already holds is returned and the field is left unchanged; four entries
      remain.
    - With no `WorldInfo` placed, `game_info` returns the map-load `NotYet`.
  - **Installed** (`bringup::tests::sanctuary_brought_up_on_the_installed_game`, ignored by default; pc-hummer, Off;
    output under ignored `local/p2/map-load-natives/`):
    - Sanctuary_P loads in about 7 s: 10 levels, 2,363 placed, 0 refused.
    - Routing reach: 1,198 actors not static, 1,165 static (open), 0 pending deletion. The 1,165 static actors are
      the part that hangs on the unnamed second flag.
    - **Begin-play events raised: 0** (`NotYet`).
    - **Game info chosen: none.** `WorldInfo.Game` is None after the load, so the class choice is `NotYet`.
    - The `WorldInfo` after the load: `bBegunPlay` and `bStartup` false; `TimeSeconds`, `RealTimeSeconds` and
      `AudioTimeSeconds` 0; `NetMode` 0. If the note's medium-confidence mapping holds, the step 1 time reset would
      change nothing on a fresh load.
    - Local players: 0.

- **`--slice-run` in Off and the map world: not yet.** The slice bridge still builds its own `WorldInfo` and refuses
  a world that already has actors (`ow-mission` bridge, "one bridge per fresh world"). Even joined, the map world has
  not begun play and has no game info. This lane does not change the bridge.

- **UNVERIFIED:** everything in the note, and the reach counts' reading of "static" as `bStatic`.
