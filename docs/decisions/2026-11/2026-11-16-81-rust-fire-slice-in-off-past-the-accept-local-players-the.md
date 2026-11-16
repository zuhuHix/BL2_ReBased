## 2026-11-16: Rust Fire slice in Off past the accept: local players, the lent weapon, play in editor

AI-assisted (Claude Code). Lane FIX-SLICE-OFF-1 (SLICE_RUN_PLAN.md, the critical path). Number 81 was given by the hub.
It carries `--slice-run` in `ParityMode::Off` past its first stops with three fixes, and writes the analyst note
`docs/verification/NATIVE_SLICE_OFF_STOPS.md` for the unread Gearbox natives behind the next ones. No dependency, no
CLI flag. There was no real-game session; every rule here is **UNVERIFIED** in the running game. Reference
(`OW_PARITY=reference`) is unchanged: it keeps the C++ in each place (R3).

- **1. `Actor.LocalPlayerControllers`** (the stop at `accept`). A stock UE3 iterator, with an own-words note already in
  NATIVE_ENGINE_CORE.md ("Actor.LocalPlayerControllers"): a None `BaseClass` yields nothing; otherwise the engine's local
  players' controllers in player index order, filtered by class. The hub's decision 2026-11-15-34 had kept it refused;
  the hub lifted that for this lane.
  - `ow-vm` (`natives_core`): the iterator, registered outside Reference only, through the same `ActorSource` service
    as `AllActors` and `DynamicActors`, with a new walk `ActorWalk::LocalPlayers`. Without a world it is
    `NotYet { LayerAbsent }`.
  - `ow-vm` (`interp::iter`): every installed site calls it by `FinalFunction` (it has no native index), and the
    interpreter bound only numbered iterator natives, so the loop fell to the stub site even with the iterator
    registered. Outside Reference a `FinalFunction` call of a native with a registered iterator now binds too;
    Reference keeps the C++ (numbered natives only).
  - `ow-world` (`actors`): the local player list (`Actors::add_local_player`, `local_players`; the walk skips destroyed
    controllers). Who logs a local player in is the engine's and not read; the caller that makes the controller adds
    it.
  - `ow-mission` (`bridge`): the bridge adds its player controller as the one local player.
- **2. The lent weapon's region-stage condition** (the next stop at `accept`, `NotYet { NotRead }` since decision 34).
  The rule is recorded in NATIVE_MISSION_SCRIPT_BRIDGE.md: inside Active the mission weapon is granted only when the
  weapon's mission objective belongs to this mission and the mission has a `GameStageRegion`; inside Complete it is
  removed when the mission has a `MissionWeapon`. Off now follows that (`ow-mission` tracker); Reference keeps the C++,
  which grants and removes on the objective test alone. On the installed Fire mission both give the same events.
- **3. `WorldInfo.IsPlayInEditor`**, a stock UE3 native (public UnrealScript semantics: true only inside the editor's
  Play In Editor session): false, since the shipped game never runs there; `ow-world`, outside Reference only.
- **Analyst only (clean room).** This lane read `MissionTracker.GetObjectivesProgress`, `AIComponent.ActivateEvent` and
  `Trigger.TriggerDetachSprites` in the executable and wrote NATIVE_SLICE_OFF_STOPS.md in its own words. It does not
  implement them: they stay unbound, so Off's gameplay stub policy refuses them (an error line in the slice's run), as
  decision 2026-11-16-50 does for unread natives. Nothing was implemented from `local/analysis/`. The note's
  `ActivateEvent` walks `AIDef.NodeList[i].Action`, which bears on how part 2 of the AI port holds its action objects
  (decision 2026-11-16-66): an open item for the hub.
- **Verified.**
  - Synthetic tests in both modes: the iterator (order, class filter, None base, no world, unbound under Reference),
    a `foreach` over a `FinalFunction` iterator (walked in Off, the stub site under Reference), `IsPlayInEditor` (false
    in Off, unbound under Reference), the lent weapon in Off with and without a region (and the Reference rule), and
    the bridge's controller as the one local player.
  - **The base moved under this lane.** On `origin/main` feda4cd (after PR #370, decision 2026-11-16-70) Off
    `--slice-run` stops before its first step, on main and on this branch alike: the definition store's `NotYet` for
    the archetype `WillowGame.Default__WillowAIPawn.GearboxDialogComponent0` ("no package holds it"). That stop is
    #370's area (`ow-defs`), not this lane's; it is reported to the hub. The step-by-step evidence below was taken on
    the base this lane started from, 733a244 (before #370), with the same three fixes.
  - Installed `--slice-run`, Off, the verify tool's steps through `range` (before is main 733a244):
    - before: `accept` and `range` fail (status 0); errors `Actor.LocalPlayerControllers`, `TriggerDetachSprites`,
      `AIComponent.ActivateEvent`, the region-stage `NotYet`;
    - after: every step through `range` succeeds (status Active, the lent weapon granted, the kickoff dialog and set,
      the range objective, the second set); errors left `TriggerDetachSprites`, `AIComponent.ActivateEvent`,
      `MissionTracker.GetObjectivesProgress`, `Object.Localize`.
    - The whole run still stops at `spawn` on `Behavior_AIHold` on this branch: PR #375 (decision 74) removes that stop.
      With #375 merged locally (a throwaway merge, for information only), all 22 steps run, the mission completes
      (status 3, the weapon removed, the reward), `confirmreward` fails, and `tools/verify_fire_event_order.py` gives 27
      of 33 rules in Off (12 of 33 with #375 alone). The errors left there: `TriggerDetachSprites`,
      `AIComponent.ActivateEvent`, `GetObjectivesProgress`, `Object.Localize` (twice) and
      `GearboxGFxMovie.SingleArgInvokeS`.
  - Reference: 33 of 33 Fire rules, as before, on 733a244 and on feda4cd; `tests/mission_script_test.py` prints the
    same as main in both modes, on both bases.
- **The next stops**, all unread or UI: the three natives of the note (a fresh implementer), `Object.Localize` (a stock
  native that needs the game's localization files read: a data seam, not taken here) and the GFx calls (the UI layer,
  W7-UI). Which of the UI calls may answer a labelled no-op in Off before W7-UI is the hub's or the maintainer's call
  (SLICE_RUN_PLAN.md, FIX-MISSION-OFF).
