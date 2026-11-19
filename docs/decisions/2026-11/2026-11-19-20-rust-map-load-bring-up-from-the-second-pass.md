## 2026-11-19: Rust map load bring-up from the second pass, and the travel countdown

AI-assisted (Claude Code). Lane MAP-LOAD-C, the implementer of MAP-LOAD-B's second analyst pass (decision 2026-11-19-13:
NATIVE_MAP_LOAD.md, second pass, and NATIVE_TRAVEL.md, read at 647c8685 on `research/rust-mapload-b`, identical to
main's copy after #398). It continues this lane's first pass (decision 2026-11-19-04, PR #388), and was rebased onto
main after #388 landed. It is coded from the notes alone; no analysis output was opened. There was no real-game
session, so every rule is **UNVERIFIED** in the running game. No dependency was added, no bounds check was loosened
and no existing `NotYet` guard was relaxed.

- **`WillowGameInfo.TravelPostCountdownInProcess()`** (`ow-world`, the world's pack, Off only; the C++ stubs it, so
  Reference keeps the stub). It answers `TravelPostCountdownTime > 0` and reads or writes nothing else. A class that
  declares no int field of that name is a typed error.
- **`mapload::bringup::bring_up_for_play(world, options)`** now runs what the second pass settles, in the note's order:
  - **Before anything changes, it refuses in two cases:**
    - `WorldInfo.Game` is not set. The class choice stays `NotYet`: the receiver of `SetGameType` in each branch is open,
      and no reader here reads the configured `DefaultGame`.
    - The caller passes `RouteMode::SkipStatic`. The value of the mode argument that the bring-up passes is open; only
      the stock "every actor" reading (`RouteMode::EveryActor`) runs.
  - **The time reset** of `TimeSeconds`, `RealTimeSeconds` and `AudioTimeSeconds` runs when the caller's `reset_time`
    says so; the flag the map load passes is open.
  - **Once:** nothing more runs when `bBegunPlay` is set.
  - **The initialisation pass** over every actor:
    - a network client's pass is `NotYet`;
    - an actor whose three `bLoadIfPhysXLevel` flags are not all set is `NotYet`, because the engine's physics level is
      not read. Such an actor is kept out of the routing;
    - otherwise, unless `bDeleteMe`, `LastRenderTime` becomes the most negative float and `Touching` is emptied;
    - not done: `PhysicsVolume` (volumes are not modelled, as in `actors::spawn`), the unnamed object flag, and the
      script-execution set-up virtual.
  - **Then** `bBegunPlay` and `bStartup` are set, the actor list begins play, and the game info gets `InitGame(Options,
    Error)` when it is not `bScriptInitialized`. The options are empty, because this load has no URL options.
  - **The routing.** An actor already `bScriptInitialized` gets nothing; this corrects the first pass, which had read
    that test as "pending deletion". An actor routes when it is not `bStatic`, or when it is static with
    `bRouteBeginPlayEvenIfStatic`.
    - Pass 1 gives each routing actor `PreBeginPlay`, in list order.
    - Pass 2 gives each one `PostBeginPlay`, then `SetInitialState` unless `bDeleteMe`, then the base search under the
      note's condition. The base search needs collision, so it is `NotYet`, as in `actors::spawn`.
    - A static actor that does not route only gets `bScriptInitialized`.
    - The engine calls without a script event (volumes, components, registrations, the extra virtual) are not run.
  - `bStartup` is cleared at the end.
  - **Failures are counted:** each failed step is recorded with the actor's class, that actor's later steps are
    skipped, and the pass goes on. `BringUp::complete()` is false when anything failed.
- **Still `NotYet { NotRead }`**, each named in the code and in `BringUp::pending`:
  - the `SetGameType` receiver;
  - the mode argument's value and the time-reset flag's value;
  - the URL options and the setup calls;
  - the flagged and second engine objects;
  - the per-level call;
  - the routing's engine calls;
  - `SpawnPlayActor`'s call site and where `GamePlayers` is filled.
- **Review r1 of #409:**
  - Every `WorldInfo` field the bring-up uses (`bBegunPlay`, `bStartup`, `NetMode`, the three time fields) is read
    before anything changes. A network client's world is now one `NotYet` before any change, instead of one failure
    per actor in the init pass.
  - `bStartup` is cleared on the error path too.
  - Pass 2 reads `bScriptInitialized` and whether the actor routes again, per actor, as the note tests them in each
    pass.
  - The counting rule (a failure is recorded and the pass goes on; an actor whose init-pass check or `PreBeginPlay`
    failed gets no later step) is labelled a project choice for measuring a map, UNVERIFIED. An incomplete bring-up
    cannot be redone: `bBegunPlay` is set, so the once test makes a second call a no-op. An actor the physics level
    filter could not decide ends neither script-initialised nor hidden nor destroyed, which the note does not describe.
  - An actor whose `PreBeginPlay` sent it into a state still gets `PostBeginPlay` and `SetInitialState`: the probe mask
    is not checked (UNVERIFIED for such an actor).
  - `pending` now also names the steps not run that it had left out: the flag the map load sets on the new game info,
    the time reset's two engine calls, the init pass's `PhysicsVolume`, unnamed object flag and script-execution set-up
    virtual, and `WillowGameInfo`'s `Hardcore` call (9 entries).
  - The installed counts are unchanged by these fixes (rerun on the rebased head).
- **Other changes:** `actors::needs_base_search` became crate-visible. The world test fixture gained the bring-up fields,
  `Probe.InitGame` (note 17) and a `WillowGameInfo` toy class. The map-load fixture gained the two routing flags.
- **Proof.**
  - **Synthetic tests** (7; the routing and reach tests run in both modes):
    - the reach under the second pass's reading, and a missing game info refused before any change;
    - the full event order: `InitGame`, then every routing actor's `PreBeginPlay` in list order, then each one's
      `PostBeginPlay` and `SetInitialState`. The test also covers a static actor only marked, a script-initialised one
      untouched, the flags, the time reset, `LastRenderTime`, and "once";
    - no time reset, and an initialised game info that gets no `InitGame`;
    - `SkipStatic` refused before any change;
    - counted failures: the physics filter, a self-destroying actor, and the base search;
    - the travel countdown at 0, 3 and -1 in Off, and unbound under Reference.
  - **Installed Sanctuary in Off** (`bringup::tests::sanctuary_brought_up_on_the_installed_game`, ignored by default;
    pc-hummer; counts under ignored `local/p2/map-load-c/`). It uses a **harness set-up, not the game's choice**: a
    `WillowGame.WillowGameInfo` from its class defaults is put in `WorldInfo.Game`. The world has the VM's core
    natives, as the slice and combat runs have.
    - **Before** (the first pass): 0 actors got any begin-play event.
    - **After:** 2,364 actors (2,363 placed plus the harness game info).
      - Reach: 1,802 route and 562 are static without the flag.
      - The init pass reset 2,097 actors.
      - Events raised: 1,100 `PreBeginPlay`, 597 `PostBeginPlay`, 0 `SetInitialState`. 321 static actors were marked,
        and 189 were destroyed by their own script during the events: 110 population opportunity points, 49 dens, 14
        mission pickup spawners, 9 pylon seeds and 7 others. Why the script destroys them was not traced.
      - `InitGame` did not complete.
      - 1,855 failures, by first stop:
        - 597 at `SetInitialState`, on `GotoState('Auto')`: the auto-state flag bit is not read (NATIVE_ENGINE_CORE.md).
          The same stop ends `InitGame` and the game info's `PreBeginPlay`;
        - 570 at `PreBeginPlay` on the unbound `Actor.SetPhysics`;
        - 267 at the init pass on the physics-level filter (241 static meshes, 26 skeletal mesh actors);
        - 188 at `PostBeginPlay` (the emitters) on the engine-native class `Core.BoolProperty`;
        - 117 at `PostBeginPlay` on the unbound `Actor.SetShadowParentOnAllAttachedComponents`;
        - 92 on the unbound `WillowGlobals.GetWillowGlobals`;
        - 10 on `Actor.ComponentList`;
        - 7 on `Pylon.OnPylonStatusChange`;
        - 2 on `ApexDestructibleActor.CacheFractureEffects`;
        - 1 each on the unbasing of a destroyed actor, the unbasing of the actors attached to a destroyed actor, and the
          `WorldInfo`'s `Actor.IsInPersistentLevel`.
    - About 6.5 s.
  - **`--slice-gate`, before and after** (before is 18bf9ea with `--slice-gate` applied): strict 0/5/9 and past-errors
    0/12/2 both times.
    - **Link 4 does not reach the travel countdown on this base:** it stops at `spawn:adult:7` on the replication info
      query.
    - With `fix/rust-fire-to-hit` (40200264) merged as well (a throwaway merge, for information only), the spawn passes
      that query and stops one step later on `TargetableAttributeValueResolver` in Off, which is not read. That is the
      same before and after, and an S-f `--combat-run` stops at the same spawn.
    - So no swing reaches `TravelPostCountdownInProcess` on any base available here. The native is proven by the
      synthetic test; its effect on link 4 waits for the Targetable resolver.
- **The next stops**, for the hub:
  - the auto-state flag bit (`GotoState('Auto')`, 598 stops);
  - `Actor.SetPhysics`, `SetShadowParentOnAllAttachedComponents`, `WillowGlobals.GetWillowGlobals`;
  - `Core.BoolProperty` as an engine-native class;
  - the physics level;
  - the game-info class choice;
  - the Targetable resolver for the swings.
