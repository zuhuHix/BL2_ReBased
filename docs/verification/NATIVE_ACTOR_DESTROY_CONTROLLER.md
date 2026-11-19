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
   - **Actor:** one engine notification (not read; no game state seen to change).
   - **Controller** (also `AIController`, which keeps it): the actor step; then, on the authority with a
     `ResourcePoolManager` still set, that manager is destroyed through the destroy routine and the field set to None;
     then the controller is unlinked from the world's `WorldInfo.ControllerList` (the list head or the predecessor's
     `NextController` takes its `NextController`), and its own `NextController` set to None. A controller not in the
     list is left alone, its `NextController` still set to None.
   - **GearboxMind:** two engine lookups whose effect was not read (no game state seen to change); then the
     `Controller` step; then, when `MyRuleEngine` is set, one call on the rule engine (not read) and `MyRuleEngine` set
     to None.
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

For the mind's `Destroy()` from `Controller.PawnDied`:
1. **The entry and the controller case:** NATIVE_ACTOR_DESTROY.md's rows. The mind takes the generic path, replacing
   today's Off `NotYet` for every controller.
2. **Preflight, before any write (cheap and certain):**
   - the existing refusals of the port's generic path (state, Kismet events, base, attached, touching), unchanged. An
     AI mind in a script state meets the `EndState` refusal there (W5-STATES);
   - `MyRuleEngine` set: `NotYet { NotRead }`, "the rule engine's call at its mind's destroy". No script on this path
     clears it before the world-drop step (read above), so the refusal is certain;
   - the world's controller list must be modelled: a world that does not hold `ControllerList` answers `NotYet {
     LayerAbsent }`, "the world's controller list", never "not in the list".
3. **The writes, in order:** `bPendingDelete`; the script `Destroyed()` chain, run as script (its own writes:
   `ResourcePoolManager` destroyed and None, the perches cleared); then the world-drop step: the pool manager if still
   set, the `ControllerList` unlink and `NextController` None; then `InteractionProxy` destroyed when set (a nested
   destroy, which follows these same rules); then the generic tail (`SetOwner(None)`, the level list, `bDeleteMe`, the
   heap's destroyed mark, the timers).
4. **A `NotYet` after the first write** (a nested destroy's refusal, a script native's): terminal, prefixed "stopped
   after partial writes", its kind kept; other errors unchanged; no rollback.

For the pawn's later destroy: the same, with the pawn's world-drop step (the `PawnList` unlink) and no rule engine. A
world that does not hold `PawnList` answers `NotYet { LayerAbsent }`, "the world's pawn list".

For a player controller (not the kill's): NATIVE_ACTOR_DESTROY.md's local-player row answers false and destroys
nothing. The `Logout` path above is reached only when a player controller does take the generic path (a network
player, not the slice).

## Open questions

1. **UNVERIFIED:** the rule engine's call at its mind's destroy (what it tells the engine).
2. **UNVERIFIED:** the actor step's engine notification and `GearboxMind`'s two lookups (no game state was seen to
   change; their targets were not read).
3. **UNVERIFIED:** which native begin-play step adds a controller to `ControllerList`, and whether the port's
   `AllControllers` iteration must follow its newest-first order.
4. **UNVERIFIED:** whether `InteractionProxy` is left pointing at the destroyed proxy (the field is not cleared).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- Kill a bullymong. Hook `Controller.PawnDied`, `Controller.Destroyed`, `WillowMind.Destroyed` and `Actor.Destroy` on
  its mind, and record their order and the mind's `bDeleteMe` after.
- Before and after the kill, walk `WorldInfo.ControllerList` through `NextController`, and record whether the mind
  leaves the list at its destroy and keeps nothing in its own `NextController`.
- After the corpse is removed, walk `WorldInfo.PawnList` through `NextPawn` the same way.
- Read the mind's `ResourcePoolManager`, `MyRuleEngine`, `InteractionProxy` and both perches before and after its
  destroy, and the pawn's `MyWillowMind` after the kill.
- Spawn two AI pawns one after the other and walk `ControllerList`, to confirm the newest-first order.
