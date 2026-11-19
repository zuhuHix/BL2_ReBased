# Native and script rules of a controller's and a pawn's destroy: the kill's unlinking (draft, 2026-11-19)

AI-assisted (Claude Code), analyst lane ANALYST-ACTOR-DESTROY-CONTROLLER (decision 2026-11-19-83; SLICE_RUN_PLAN.md
section 3.1, lane 4). Sources:
- the records: NATIVE_ENGINE_CORE.md ("Actor.Destroy", the destroy routine) and NATIVE_ACTOR_DESTROY.md (the
  routine's controller and player case at its entry);
- the installed script, read with `research/script_disasm.py`;
- the class defaults, read with `ow-package --object-dump`;
- `Borderlands2.exe` locally, with the project's Ghidra tooling (read-only queries under the machine lock).

No recorded bl-sdk evidence covers this path. Raw output stays under ignored `local/analysis/`, and this note is in our
own words: no decompiler output, pseudo-code or listing structure. Field names come from the cooked packages.
**Every rule here is UNVERIFIED** in the running game. The live checks at the end would confirm them. This lane does
not implement them; a fresh implementer codes the Off rules from this note alone.

**Why it matters.** The kill in slice links 3 and 4 ends with the dead bullymong's mind destroying itself. The port's
`ow_world::actors::destroy` refuses every controller in Off (the controller case waits for an implementer), which is
where the kill stops after 2 of its 9 rows.

## Summary

| Question | Answer | Confidence |
|---|---|---|
| What destroys the mind on a kill | its own `Controller.PawnDied`, reached from the pawn's `Dying` state | high (script) |
| Which part of the destroy routine is the controller's own | one class-specific world-drop step, run right after `Destroyed()` | high |
| What the world drops | the controller from `WorldInfo.ControllerList`; the pawn, at its own later destroy, from `WorldInfo.PawnList` | high |
| What the game info drops | nothing for an AI mind (`Logout` runs only for a player) | high (script and defaults) |
| What else a mind's destroy destroys | its `ResourcePoolManager`, its `InteractionProxy`; its rule engine is told and dropped | high for the order, not read for the rule-engine call |

## 1. How the kill reaches the controller's destroy (script)

The dying pawn's `Dying` state starts. `WillowPawn`'s own `Dying.BeginState` replaces `Pawn`'s, and does not call it.
It sets the dead flags, collision and life span, starts the corpse timers, and as its last step, when the pawn has a
`Controller`, calls `Controller.PawnDied(self)`.

For a bullymong the controller is its `WillowMind`. In order:
1. **`WillowMind.PawnDied`:**
   - tells the game info's skill manager of the death;
   - fires its `Died` event rule;
   - removes the pawn from the radar if it was added (`bPawnAddedToRadar`);
   - clears its AI component's target;
   - notifies the combat music manager (`EnemyTargetChanged`);
   - cleans its spawn family (`CleanSpawnFamily(None)`);
   - then calls `GearboxMind.PawnDied`.
2. **`GearboxMind.PawnDied`:** clears its AI component's state (`ClearState`), then calls `Controller.PawnDied`.
3. **`Controller.PawnDied(P)`:**
   - nothing when `P` is not its `Pawn`;
   - fires the Kismet `SeqEvent_Death` class event;
   - aborts each of its latent actions, then empties `LatentActions`;
   - with a `Pawn`: moves itself to the pawn's `Location`, calls the pawn's `UnPossessed`, and sets `Pawn` to None;
   - then: a player (`bIsPlayer`) goes to state `Dead` unless the game has ended; **any other controller calls its own
     `Destroy()`.** `WillowMind`'s default has `bIsPlayer` false, so the mind destroys itself here.
4. **`Pawn.UnPossessed`** (the pawn's side, inside step 3): `bForceNetUpdate` set; with a `DrivenVehicle`,
   `NetUpdateFrequency` 5; the pawn's `PlayerReplicationInfo` None; `SetOwner(None)`; the pawn's `Controller` None.

`WillowMind.UnPossess` is **not** on this path, so the mind's `MyWillowPawn` and the pawn's `MyWillowMind` are left as
they were (a script reading, UNVERIFIED in play).

## 2. The controller's destroy (the mind)

The destroy routine of NATIVE_ENGINE_CORE.md, with what a controller adds, in order:
1. **The entry checks**, then the controller and player case (NATIVE_ACTOR_DESTROY.md). A `WillowMind` is not a player
   controller, so it takes the generic path.
2. `bPendingDelete` is set. The Kismet destroyed events, the engine callback, the physics teardown and the `EndState`
   probe follow, as NATIVE_ENGINE_CORE.md has them.
3. **Script `Destroyed()`:** `WillowMind.Destroyed` first calls `Controller.Destroyed`, which, on the authority:
   - a player with a game info: the game info's `Logout(self)` (not for the mind);
   - with a `PlayerReplicationInfo`: removes it from its team unless a spectator, then `CleanupPRI` (the replication
     info destroyed, the field None). The mind has none (its default is None and `PostBeginPlay` makes one only for a
     player);
   - with a `ResourcePoolManager`: it is destroyed and the field set to None;
   - then `Actor.Destroyed` (`CheckForOwnedInstanceData`).

   Then `WillowMind.Destroyed` clears its perches: `PatrolPerch.User` None and `PatrolPerch` None; `CombatPerch.User`
   None (written twice, the same value) and `CombatPerch` None.
4. **The class's world-drop step** (new in this note; NATIVE_ENGINE_CORE.md step 6 called it "one world-level
   unregister call, not identified"). It runs right after `Destroyed()` and before the unbase step. Each class layers
   its own on its parent's:
   - **Actor:** one engine notification (not read; no game state seen to change, which does not prove it
     irrelevant: section 5.2 gives it a `NotYet`).
   - **Controller** (also `AIController`, which keeps it): the actor step; then, on the authority with a
     `ResourcePoolManager` still set, that manager is destroyed through the destroy routine and the field set to None;
     then the controller is unlinked from the world's `WorldInfo.ControllerList` (the list head or the predecessor's
     `NextController` takes its `NextController`), and its own `NextController` set to None. A controller not in the
     list is left alone, its `NextController` still set to None.
   - **GearboxMind:** two engine lookups whose effect was not read (no game state seen to change; section 5.2 gives
     them a `NotYet`); then the `Controller` step; then, when `MyRuleEngine` is set, one call on the rule engine (not
     read) and `MyRuleEngine` set to None.
   - **WillowMind:** the `GearboxMind` step; then, when `InteractionProxy` is set, that proxy is destroyed through the
     destroy routine (the field is not cleared).
   - **PlayerController** (for comparison; `WillowPlayerController` keeps it): `CheatManager`, `PlayerInput` and the
     controller's `NavigationHandle` set to None; then the actor step, the pool manager on the authority, and the
     `ControllerList` unlink, as for `Controller`.
5. The rest of NATIVE_ENGINE_CORE.md's routine: unbase, the attached and touching lists, `SetOwner(None)` with
   `LostChild`, the net and world notifications, removal from the level list, `bDeleteMe`, the components finalised.

## 3. The pawn's own destroy (later)

The dead pawn is destroyed later, by its life span or its corpse timers (`UpdateAndDestroyNonVisibleActorInTimer`,
`DissolveVisibleBody`), through the same routine:
1. **Script `Destroyed()`:** `WillowAIPawn.Destroyed` (a driven vehicle left; `PlayerMaster` and `PlayerMasterPRI`
   None; with mission directives, unregistered from the mission tracker), then `WillowPawn.Destroyed` (waypoints
   removed, melee-over-time ended, charm undone, `Pawn.Destroyed`, shield armor destroyed, removed from each local
   player's HUD post-render list, light projection managers destroyed, owned instance data destroyed, the obstacle
   utility told), with `Pawn.Destroyed` in it: the health pool destroyed, `DetachFromController`, the inventory manager
   destroyed, and the anchor, weapon, path step and allegiance parent and children cleared. After the kill the pawn's
   `Controller` is already None, so `DetachFromController` does nothing.
2. **The pawn's world-drop step:** the actor step, then the pawn is unlinked from `WorldInfo.PawnList` (the head or the
   predecessor's `NextPawn` takes its `NextPawn`). Its own `NextPawn` is left as it is.
3. The rest of the routine, as for the controller.

## 4. What the world and the game info drop

- **The world:** the controller from `ControllerList` at its destroy; the pawn from `PawnList` at its own destroy; each
  from the level list at the end of its routine.
- **The game info:** for a player controller, `Logout` (the counts, the access control, arbitration, the mutator
  notification; `WillowGameInfo.Logout` adds its player-left event and per-player clean-up). For an AI mind, nothing.
- **Adding, for reference** (read with medium confidence): a controller is added to the head of `ControllerList` by a
  native begin-play step, unless it is already being deleted; a pawn is first removed and then added to the head of
  `PawnList`. So `ControllerList` runs newest first. The step that adds them was not identified by name (UNVERIFIED).

## 5. Off answers, step by step, with the writes in order

Review r1 of #472 folded in. The hub rules for the Off answers apply throughout:
- a refusal checked before the first write leaves nothing changed;
- a typed `VmError::NotYet` after any write is terminal: prefixed "stopped after partial writes", its kind and record
  kept, nothing rolled back;
- other errors propagate unchanged.

### 5.1 Two boundaries

The mind's destroy is reached from inside the kill, so a refusal there has two callers to answer to.
- **The nested `Destroy()` boundary.** A refusal checked before the destroy's own first write (`bPendingDelete`) leaves
  the mind as the destroy found it. Seen from the destroy, that is clean.
- **The kill boundary.** By the time the mind's `Destroy()` runs, the kill has already written:
  - `Pawn.Died` and `PlayDying`, before the state change;
  - the pawn's `Dying.BeginState`: the dead flags, collision, life span, the corpse timers (section 1);
  - `WillowMind.PawnDied` and `GearboxMind.PawnDied`: the radar, the AI component's target and state;
  - `Controller.PawnDied`: the latent actions emptied, the mind moved to the pawn's location, and both possession links
    cleared (the pawn's `Controller` None through `UnPossessed`, the mind's `Pawn` None).

  So any `NotYet` that leaves the kill from the mind's destroy, even one that was clean at the nested boundary, is
  terminal at the kill boundary and carries the prefix. Nothing is rolled back.
- **The earliest certain point.** The destroy's target and path are certain at the entry of the pawn's
  `Dying.BeginState`, before its first write, when all three hold:
  - the pawn has a `Controller`;
  - that controller's `Pawn` is this pawn;
  - the controller is not a player (`bIsPlayer` false).

  Then `Controller.PawnDied` will reach `Destroy()` on that controller. No script on the path changes the inputs of
  the checks in 5.2: the world's lists, the mind's class, its `MyRuleEngine`, its script state, its Kismet events.
  So 5.2's refusals are checked there. Even that point comes after `Pawn.Died`'s and `PlayDying`'s writes, so a refusal
  there is prefixed at the kill boundary too. Checking there, rather than at the nested boundary, keeps the pawn's
  dying state and both possession links unwritten when the mind's destroy cannot run.

### 5.2 The mind's destroy: what is supported, and the refusals

The calls the controller's destroy makes on the modelled path, one by one:

| Step | Off answer | Why |
|---|---|---|
| The entry checks and the controller case (NATIVE_ACTOR_DESTROY.md) | supported: the generic path | read; a `WillowMind` is not a player controller |
| `bPendingDelete`, the script `Destroyed()` chain, the generic tail | supported, as the port's generic path runs them | read (section 2) |
| The actor step's engine notification | **`NotYet { NotRead }`**, "the actor step's engine notification at destroy" | not read; not proven irrelevant |
| `GearboxMind`'s two engine lookups | **`NotYet { NotRead }`**, "the mind's two engine lookups at destroy" | not read; not proven irrelevant |
| The pool manager on the authority, if still set | supported: the destroy routine on it, the field None | read; `Controller.Destroyed` has normally cleared it already |
| The `ControllerList` unlink | supported where the world holds the list; else **`NotYet { LayerAbsent }`**, "the world's controller list" | read |
| `MyRuleEngine` set: the call on the rule engine | **`NotYet { NotRead }`**, "the rule engine's call at its mind's destroy" | not read |
| `InteractionProxy` set: its destroy | supported: a nested destroy under these same rules | read |

- **No refusal is skipped because another one is absent.** An empty `MyRuleEngine` removes only the rule engine's
  row. The notification and the two lookups run on every `GearboxMind` destroy whatever its fields hold, so the Off
  destroy of a mind refuses with the notification's `NotYet` until one of them is read. It never reports success with
  those effects unknown.
- **Checked before any write.** Each of those refusals is certain once the target is known (its class, and the world's
  list), so all of them are checked at 5.1's earliest certain point, in table order. The first one found is the answer.
- **The generic path today.** The actor step's notification belongs to every actor's destroy, not only a
  controller's. The port's generic path omits it today, and its Off destroys of pawns and other actors succeed without
  it. Applying the same refusal there would stop every Off destroy that now succeeds. That is a hub decision. The
  alternative is to keep the omission as a documented, UNVERIFIED gap for non-controllers only.

### 5.3 States (W5-STATES)

- **The port's refusal is broader than the routine's condition.** The port's generic destroy refuses any actor that
  is in a script state, before any write (`EndState` and the state frame, W5-STATES). The routine described in
  NATIVE_ENGINE_CORE.md only runs `EndState` behind its own probe, after `bPendingDelete` and the destroyed events.
- **The mind.** `GearboxMind.PawnDied`'s `ClearState` is on the mind's AI component, not the mind. It does not show
  that the mind has left its own script state. A mind that is in a state stays blocked by the port's refusal, before
  its destroy's first write, and is prefixed at the kill boundary (5.1).
- **The corpse.** It may still be in `Dying` when its life span or corpse timers remove it. It stays blocked the same
  way, before its destroy's first write. When a corpse timer's script wrote before calling `Destroy()`, the refusal is
  prefixed at that caller's boundary.
- **If stateful destroy is supported later:** `Destroyed()` comes after the probe-gated `EndState`. The order is
  `bPendingDelete`, the destroyed events, the probe and `EndState`, then `Destroyed()`. It must not move ahead of the
  probe.

### 5.4 The writes, in order (when nothing refuses)

1. `bPendingDelete`.
2. The script `Destroyed()` chain, run as script. Its own writes: the `ResourcePoolManager` destroyed and set to None,
   and the perches cleared.
3. The world-drop step:
   - the pool manager, if still set;
   - the `ControllerList` unlink, and `NextController` set to None;
   - `InteractionProxy` destroyed when set: a nested destroy that follows these same rules (a refusal inside it comes
     after writes, so it is prefixed).
4. The generic tail: `SetOwner(None)`, the level list, `bDeleteMe`, the heap's destroyed mark, the timers.

### 5.5 The pawn's later destroy

The same rules, with the pawn's world-drop step and no rule engine:
- the actor step's notification, as in 5.2;
- the `PawnList` unlink:
  - **an absent list:** a world that does not hold `PawnList` answers `NotYet { LayerAbsent }`, "the world's pawn
    list", before any write;
  - **a modelled list without this pawn:** not a refusal. The walk finds nothing and the list is left unchanged, as
    for a controller not in `ControllerList`. It was read as the same walk (medium confidence; UNVERIFIED in play).

### 5.6 A player controller (not the kill's)

NATIVE_ACTOR_DESTROY.md's local-player row answers false and destroys nothing. The `Logout` path above is reached only
when a player controller does take the generic path: a network player, not the slice.

## Open questions

1. **UNVERIFIED:** the rule engine's call at its mind's destroy (what it tells the engine).
2. **UNVERIFIED:** the actor step's engine notification and `GearboxMind`'s two lookups (no game state was seen to
   change; their targets were not read).
3. **UNVERIFIED:** which native begin-play step adds a controller to `ControllerList`, and whether the port's
   `AllControllers` iteration must follow its newest-first order.
4. **UNVERIFIED:** whether `InteractionProxy` is left pointing at the destroyed proxy (the field is not cleared).
5. **UNVERIFIED:** whether the mind and the corpse are in a script state at their destroy's entry (5.3), and whether
   `EndState` runs for either.
6. **Hub decision:** whether the generic path's omission of the actor step's notification becomes a refusal too
   (5.2).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

Final states alone do not show the order, so each check takes a snapshot at several points. **A snapshot** records:
- the pawn's `Controller` and the mind's `Pawn` (both possession links);
- whether each is in `WorldInfo.ControllerList` / `WorldInfo.PawnList`, walked through `NextController` / `NextPawn`,
  and each one's own next link;
- the mind's `InteractionProxy`, and that proxy's `bDeleteMe` and `bPendingDelete`;
- the mind's `ResourcePoolManager`, `MyRuleEngine` and both perches;
- both actors' `bPendingDelete`, `bDeleteMe` and current script state.

1. **The kill.** Kill a bullymong. Take a snapshot at the entry and the exit of each script hook: `WillowPawn`'s
   `Dying.BeginState`, `WillowMind.PawnDied`, `GearboxMind.PawnDied`, `Controller.PawnDied`, `Pawn.UnPossessed`,
   `WillowMind.Destroyed`, `Controller.Destroyed`. Also take one at the entry of `Actor.Destroy` on the mind.
2. **The destroys inside it.** Hook `Actor.Destroy` on the mind's pool manager and on its interaction proxy, and take
   a snapshot at the entry and exit of each. That places the pool manager's destroy (in `Controller.Destroyed`, or in
   the world-drop step) and the proxy's destroy relative to the `ControllerList` unlink.
3. **The states at destroy entry.** At the entry of `Actor.Destroy` on the mind and, later, on the corpse, record each
   one's script state. Hook `EndState` and the `Destroyed` event on both, and record whether either fires, and in
   which order.
4. **The corpse removal.** Take a snapshot just before and just after the corpse's `Actor.Destroy`, whether it came
   from its life span, `UpdateAndDestroyNonVisibleActorInTimer` or `DissolveVisibleBody`, and note which one.
5. **The list order.** Spawn two AI pawns one after the other and walk `ControllerList`, to confirm the newest-first
   order.

**Left open on purpose:** the unnamed native calls are not observable with script hooks: the actor step's engine
notification, `GearboxMind`'s two lookups, and the call on the rule engine. These checks place the observable writes
around them; they do not show what those calls do. Open questions 1 and 2 stay open until those calls are read.
