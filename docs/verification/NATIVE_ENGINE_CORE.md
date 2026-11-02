# Native engine core: actor timers, spawn and destroy, iterators, lookups, state machine (2026-10-05)

AI-assisted (Claude), analyst lane G3. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Nothing in this note was confirmed in the running game.

Scope: the stock Core/Engine natives the script VM needs first to run gameplay script. Lane C1 owns MissionTracker, lane
G1 loot and G2 behavior/population; where a native belongs to them it is only named. The existing VM (`src/natives_core.cpp`)
implements `GotoState`/`GetStateName`/`IsInState` as a name-only approximation and `Enable`/`Disable` as no-ops; the mover
adapter (`src/mover.cpp`) binds scoped `SetTimer`/`ClearTimer`. This note says what the real ones do so those can be replaced.

Method and caveats. The registered native functions were read directly. The engine-side machinery they call (the timer
update, the actor tick, the spawn and destroy routines, the state-frame code) has no registered name; it was found by
following calls and by matching against the shape of public UE3 behaviour, and each place where a call target was
identified by inference rather than by a name is marked "inferred". Field names come from `tools/ghidra/class_layout.py`
over the cooked packages (`Engine.Actor`, `Engine.WorldInfo`, `Engine.Player`, `Engine.Engine`); the layout reproduced every
offset the code used for the fields named here. The `TMap` size had to be assumed to compute `WorldInfo` offsets after the
map members; the fields used here (`bBegunPlay`, `TimeDilation`, `TimeSeconds`, `RealTimeSeconds`, `AudioTimeSeconds`,
`NetMode`, `GRI`, `Game`) all sit before the first map and match the code, so the assumption does not touch them.

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| Actor.SetTimer | `native final function SetTimer(float InRate, optional bool inbLoop, optional name inTimerFunc, optional Object inObj)` | Fire mission: 41 call sites in WillowGame (UI, save, mission HUD); mover | high (data model), medium (firing) | UNVERIFIED |
| Actor.ClearTimer | `... ClearTimer(optional name inTimerFunc, optional Object inObj)` | 30 call sites | high | UNVERIFIED |
| Actor.ClearAllTimers | `... ClearAllTimers(optional Object inObj)` | 1 call site | high | UNVERIFIED |
| Actor.PauseTimer | `... PauseTimer(bool bPause, optional name inTimerFunc, optional Object inObj)` | none seen | high | UNVERIFIED |
| Actor.IsTimerActive | `... bool IsTimerActive(optional name inTimerFunc, optional Object inObj)` | 13 call sites | high | UNVERIFIED |
| Actor.GetTimerCount | `... float GetTimerCount(optional name inTimerFunc, optional Object inObj)` | none seen | high | UNVERIFIED |
| Actor.GetTimerRate | `... float GetTimerRate(optional name TimerFuncName, optional Object inObj)` | none seen | high | UNVERIFIED |
| Actor.ModifyTimerTimeDilation | `... ModifyTimerTimeDilation(name TimerName, float InTimerTimeDilation, optional Object inObj)` | none seen | high | UNVERIFIED |
| Actor.ResetTimerTimeDilation | `... ResetTimerTimeDilation(name TimerName, optional Object inObj)` | none seen | high | UNVERIFIED |
| Actor tick and timer update (engine side, no script name) | n/a | when every timer, Tick, state code and LifeSpan run | medium | UNVERIFIED |
| Actor.Spawn | `native noexport final function coerce Actor Spawn(class<Actor> SpawnClass, optional Actor SpawnOwner, optional name SpawnTag, optional vector SpawnLocation, optional rotator SpawnRotation, optional Actor ActorTemplate, optional bool bNoCollisionFail)` | 70 call sites; every dynamic actor | medium | UNVERIFIED |
| Actor.SpawnForMap | same parameters as Spawn | interface `ISpawnActor`; not seen in script | medium | UNVERIFIED |
| Actor.Destroy | `native final function bool Destroy()` | 105 call sites (iNative 279) | medium | UNVERIFIED |
| Actor spawn and destroy events (engine side) | n/a | order of PreBeginPlay, PostBeginPlay, SetInitialState, EndState, Destroyed, GainedChild, LostChild | medium | UNVERIFIED |
| Actor.AllActors | `native final iterator function AllActors(class<Actor> BaseClass, out Actor Actor, optional class<Interface> InterfaceClass)` (iNative 304) | 17 `foreach` sites | high | UNVERIFIED |
| Actor.DynamicActors | same shape (iNative 313) | 11 sites | high | UNVERIFIED |
| Actor.ChildActors / BasedActors / TouchingActors | `iterator function X(class<Actor> BaseClass, out Actor Actor)` (305/306/307) | 5 sites (Touching) | high | UNVERIFIED |
| Actor.VisibleActors | `(class<Actor> BaseClass, out Actor Actor, optional float Radius, optional vector Loc)` (311) | none seen | medium | UNVERIFIED |
| Actor.VisibleCollidingActors | `(BaseClass, out Actor, float Radius, optional vector Loc, optional bool bIgnoreHidden, optional vector Extent, optional bool bTraceActors, optional class<Interface> InterfaceClass, optional out TraceHitInfo HitInfo, optional bool bSkipTraceTest)` (312) | none seen | low | UNVERIFIED |
| Actor.CollidingActors | `(BaseClass, out Actor, float Radius, optional vector Loc, optional bool bUseOverlapCheck, optional class<Interface> InterfaceClass, optional out TraceHitInfo HitInfo)` (321) | 6 sites | medium | UNVERIFIED |
| Actor.OverlappingActors | `(BaseClass, out Actor out_Actor, float Radius, optional vector Loc, optional bool bIgnoreHidden)` | none seen | medium | UNVERIFIED |
| Actor.LocalPlayerControllers | `iterator function LocalPlayerControllers(class<PlayerController> BaseClass, out PlayerController PC)` | 69 call-site mentions | high | UNVERIFIED |
| Actor.AllOwnedComponents | `iterator function AllOwnedComponents(class<ActorComponent> BaseClass, out ActorComponent OutComponent)` | none seen | high | UNVERIFIED |
| Actor.GetALocalPlayerController | `native final function PlayerController GetALocalPlayerController()` | 7 call sites | high | UNVERIFIED |
| WorldInfo.GetWorldInfo / SequenceObject.GetWorldInfo | `native static final function WorldInfo GetWorldInfo()` | Kismet and behavior script | high | UNVERIFIED |
| Object.GotoState | `native final function GotoState(optional name NewState, optional name Label, optional bool bForceEvents, optional bool bKeepStack)` (113) | 125 call sites | medium | UNVERIFIED |
| Object.PushState / PopState | `PushState(name NewState, optional name NewLabel)`, `PopState(optional bool bPopAll)` | 4 `PushState` sites | medium | UNVERIFIED |
| Object.IsInState / GetStateName | `bool IsInState(name TestState, optional bool bTestStateStack)` (281), `name GetStateName()` (284) | 21 and 20 sites | high | UNVERIFIED |
| Object.Enable / Disable | `native final function Enable(name ProbeFunc)` / `Disable(name ProbeFunc)` | event gating | high | UNVERIFIED |
| Actor.Sleep and the latent poll | `native final latent function Sleep(float Seconds)` (256) | none in WillowGame script (Engine and Gearbox state code not counted) | high | UNVERIFIED |

Call-site counts are occurrences in `local/disasm/willowgame_all.txt` (WillowGame script only; Engine, GearboxFramework
and the other packages add more).

## Conventions used below

- **Hard-coded name ids.** These names have fixed numbers in this executable. The numbers matter because the native code
  compares against them: `None` 0, `Begin` 100 (the default state label), `Destroyed` 300, `GainedChild` 301,
  `LostChild` 302, `Touch` 306, `UnTouch` 307, `BeginState` 309, `EndState` 310, `Tick` 317, `PreBeginPlay` 328,
  `PostBeginPlay` 329, `Auto` 690, `PoppedState` 398, `PushedState` 399, `PausedState` 843, `ContinuedState` 844,
  `Timer` 1210 (the default timer function name). Implementers should key on the strings, not the numbers.
- **Probe events.** Names 300 to 331 are "probe" events. Each state frame carries a 64-bit probe mask (only the upper 32
  bits, names 300 to 331, are touched by `Enable`/`Disable`). The mask starts as the union of the probe bits the object's
  class implements and the probe bits the current state implements. The engine sends a probe event to script only when
  the object has no state frame at all, or the frame's mask has that event's bit. In practice: the event is sent only if
  the class or the active state defines a function of that name, and `Disable('Tick')` suppresses it until `Enable`.
  Events that are not probes (`SetInitialState`, `PushedState`, `PoppedState`, `PausedState`, `ContinuedState`, the timer
  function) are sent unconditionally through name lookup.
- **Pending kill.** "Destroyed" below means the actor's `bDeleteMe` flag is set (the `Actor` boolean) or its object
  flags carry the pending-kill bit. The engine checks this repeatedly between callbacks and abandons the rest of the
  routine as soon as a callback destroyed the actor.
- **Function lookup.** Calling a script function by name looks in the object's active state chain first (state, then its
  parent states), then in the class chain. A missing function in the timer path is silent. Missing functions in several
  event paths log "Failed to find function %s in %s".
- **Actor fields used** (names from the package layout): `Location`, `Rotation`, `Owner`, `Base`, `Instigator`,
  `WorldInfo`, `LifeSpan`, `CreationTime`, `Touching`, `Attached`, `Components`, `AllComponents`, `GeneratedEvents`,
  `Timers`, `Role`, `RemoteRole`, `Physics`, `LatentFloat`, `CustomTimeDilation`; flag bits `bStatic`, `bNoDelete`,
  `bDeleteMe`, `bHidden`, `bNetTemporary`, `bCollideActors`, `bCollideWorld`, `bCollideWhenPlacing`, `bPendingDelete`.
- **Role values** used: `ROLE_Authority` is 3; the single-player game keeps the local actors at authority with remote
  role none, which selects the authoritative branch of the tick.

## Actor timers

### Data model
Each actor owns a `Timers` array. An entry has: `bLoop`, `bPaused`, `FuncName` (name), `Rate` (seconds), `Count` (elapsed
seconds), `TimerTimeDilation` (float, default 1.0) and `TimerObj` (the object whose function is called). Entries are
identified by the pair (`FuncName`, `TimerObj`); `TimerObj` omitted or None means the actor itself everywhere, in set,
clear, pause, query and fire. The same function name on two different objects is two separate timers. An entry is never
removed by `ClearTimer`; removal happens only in the engine's update pass (below), which is why script may clear and
re-set timers from inside a timer callback without disturbing array indices.

### Actor.SetTimer
- **Signature:** (`InRate`, optional `inbLoop` default false, optional `inTimerFunc` default `Timer`, optional `inObj`
  default self).
- **Does:**
  - An actor with `bStatic` set ignores the call entirely.
  - It looks for the first existing entry with the same function name and object. If found it is replaced in place:
    when the new rate is exactly 0 the entry's `Rate` is set to 0 and nothing else changes (the entry will be culled at the
    next update, `bLoop` untouched); when the new rate is non-zero `bLoop` and `Rate` are overwritten and `Count` is reset
    to 0. Either way `bPaused` is cleared. `TimerTimeDilation` is left alone on replacement.
  - If no entry matches, a new entry is appended with the given object, name, loop flag and rate, `Count` 0, not paused,
    `TimerTimeDilation` 1.0. This happens even when the rate is 0 (the entry is then culled on the next update).
- **Calls into script:** none now; the function is called later by the update pass.
- **Constants:** default function name `Timer`; initial dilation 1.0.
- **Edge cases:** negative rates are stored as given (a non-zero rate replaces, a new entry is appended); they are not
  rejected. A name that does not resolve to a function is stored anyway and silently dropped when it comes due.
- **Implementer checklist:** replace-not-add on the same (name, object); a rate of 0 stores nothing live; `SetTimer`
  clears pause; re-setting resets `Count`; separate timers per object; static actors ignore it.
- **Open:** negative-rate behaviour at fire time (see update pass).

### Actor.ClearTimer / ClearAllTimers
- `ClearTimer(name = Timer, obj = self)`: every entry whose name and object match gets `Rate` set to 0. It does not
  delete the entry, does not touch `Count`, and does nothing for a missing entry.
- `ClearAllTimers(obj = self)`: every entry whose `TimerObj` equals the object gets `Rate` set to 0 (the name is not
  considered). With no argument that is every timer targeting the actor itself.
- Implementer checklist: after either call, `IsTimerActive` is false and `GetTimerRate` returns 0 until the next update
  pass removes the entry (then -1).

### Actor.PauseTimer
- Arguments in order: `bPause`, name (default `Timer`), object (default self). Sets the entry's `bPaused` flag to
  `bPause` for every entry that matches (the loop does not stop at the first). Pausing stops `Count` accumulating in the
  update pass; it does not stop a timer that is already due from firing (see update pass) and does not change `Rate`.

### Actor.IsTimerActive / GetTimerCount / GetTimerRate
- All three use the first matching (name, object) entry.
- `IsTimerActive`: false if absent; otherwise true exactly when `Rate` is greater than 0. Paused timers still report
  true; a cleared timer reports false.
- `GetTimerCount`: that entry's `Count`, or -1.0 if absent.
- `GetTimerRate`: that entry's `Rate` (0 after a clear), or -1.0 if absent.

### Actor.ModifyTimerTimeDilation / ResetTimerTimeDilation
- First matching (name, object) entry: set `TimerTimeDilation` to the argument, or to 1.0 for reset. Nothing if absent.
  The name argument has no default here (both functions require it). The dilation multiplies how fast `Count` grows
  (below); it is per timer and independent of `CustomTimeDilation` and of the world's `TimeDilation`.

### Timer update pass (engine side, inferred as the actor's timer update)
Runs once per actor tick with that tick's delta seconds (see the tick order section for where).
1. For every entry in order, if it is not paused: `Count` grows by `TimerTimeDilation * delta`. Entries with rate 0 also
   accumulate; it is harmless.
2. For each entry in index order (re-reading the array length as it goes, so entries appended by callbacks in the same
   pass are visited):
   - If the actor is already destroyed, stop the whole pass.
   - If `Rate` is exactly 0, or `TimerObj` is None, or `TimerObj` is destroyed: remove the entry.
   - Otherwise the timer is due when `Count` is strictly greater than `Rate` (equal is not due; with float values that
     means a rate of 1.0 ticked in 0.5 steps fires on the third tick).
     - Number of firings this pass: 1 for a non-loop; for a loop, the integer part of `Count / Rate` (a long frame fires
       the callback several times in a row).
     - The function is looked up by name through the state-aware lookup on the target object. If it is not found the entry
       is removed silently (no log).
     - For a loop timer `Count` is reduced by firings times `Rate` before the callbacks run; a non-loop timer keeps
       `Count`.
     - Each callback is a normal script call on the target object with every parameter zero-initialised (so timer
       functions with parameters get zeros).
     - After each callback: if the actor was destroyed, stop; if `Rate` became 0 (script cleared it) the entry is
       removed; if `Count` became exactly 0 (script re-set the same timer, which resets `Count`) the entry is kept even
       though it was a non-loop timer.
     - After the callbacks, a due non-loop entry (or one whose function was missing) is removed, unless the re-set case
       above kept it.
   - Entries that are not due are left alone.
- **Implementer checklist:** strict greater-than; loop catch-up fires several times per pass; non-loop entries disappear
  after their single firing unless the callback re-armed the same timer; clearing inside a callback removes at the next
  check without index shifts; callbacks run with zeroed arguments; the pass is stopped by actor destruction.
- **Open:** negative rates (a negative rate makes the "due" test true on every pass, and the loop count is then the
  integer part of a negative or infinite quotient; the code does not guard it, so a faithful port should not either, but
  this was not traced); whether the delta handed to the actor tick has already been multiplied by `CustomTimeDilation`
  and the world's `TimeDilation` was not located (see Open at the end).

### Actor tick order (authoritative actors) and where timers fit
Read from the actor's tick routine and its authoritative branch. For an actor whose `RemoteRole` is not simulated proxy and
whose `Role` is at least 2 (all single-player gameplay actors), one tick does, in this order:
1. The script `Tick(DeltaTime)` event, subject to the probe rule.
2. The state-code step (`ProcessState`, see the state section): runs the state's latent code and labels.
3. The timer update pass above.
4. `LifeSpan`: if non-zero it is reduced by delta; when it falls to 0.0001 or below the actor is destroyed through the
   destroy routine directly (no script event other than the normal destroy sequence below). The routine then returns
   without doing physics.
5. Physics (the actor's movement step) when the actor is not destroyed, `Physics` is not none, `Role` is not
   simulated-proxy and (the actor has no `Base` or its base is not itself being moved by a rigid-body step).
Actors with `Role` 1 take the same authoritative path. Actors with `Role` 0 (none) are not ticked here at all except for
a physics-only step when `Physics` is one of falling, rotating, projectile or interpolating; their timers and `Tick` do not
run. Actors whose `RemoteRole` is 2 (replicated to clients as simulated proxies, a multiplayer case) have a variant that
runs `Tick`, state code and timers inline under an extra condition and otherwise takes the authoritative path; that
variant was not read in detail and does not matter for single-player.

## Spawning and lifetime

### Actor.Spawn / SpawnForMap
- **Signature:** as in the summary. Operands are read in that order.
- **Reads:** the spawner's `Location`, `Rotation`, `Instigator`.
- **Does (outcomes, in order):**
  - Defaults: an omitted location or rotation becomes the spawner's own `Location`/`Rotation`. `SpawnTag` is read from
    the stream and then **not used**: `Engine.Actor` in this build has no `Tag` property and the world spawn is called
    with no object name, so a tag has no effect (UNVERIFIED; confirm by spawning with a tag in the running game).
  - A None class returns None without a log.
  - The world spawn is called with: the class, no name, the location and rotation, the template, `bNoCollisionFail`,
    remote-owned false, the owner, **the spawner's `Instigator`** (not the spawner), no "no-fail", and no level override
    for `Spawn`. `SpawnForMap` is the same call with an explicit level: the level that contains the owner (found from the
    owner's outer) or, with no owner, the level the world info reports as its own; it exists for map placement code.
  - The result is the new actor, or None when the spawn was refused.
- **Spawn routine, outcomes in order (inferred as the world's spawn):**
  1. Refusals that return None: class is None; class flags include the abstract bit or a second "not instantiable" bit
     (0x2000000, meaning not identified); the class is not an actor class; the world has begun play and the class
     default object is `bStatic` or `bNoDelete`; a template was passed whose class differs from the requested class
     (this last rule applies before and after play begins). A "no-fail" override (not used by `Spawn`) skips the static and
     template refusals.
  2. Template: if none was given the class default object is the template for copying properties.
  3. Placement: if the template has `bCollideWorld`, or `bCollideWhenPlacing` and the world is not a network client,
     and `bNoCollisionFail` is false, the world looks for a collision-free spot for the template's collision extent
     (a spot-finding routine that may nudge the location in place); if none is found the spawn returns None.
  4. The object is constructed in the chosen level (the owner's level if an owner exists, otherwise the world's current
     level) from the template, registered in the level actor list.
  5. Fields set before any script runs: `bTicked` from the level's tick state, `CreationTime` = world `TimeSeconds`,
     `WorldInfo`, optionally the role pair swapped when remote-owned, `Location`, `Rotation`, `PhysicsVolume` = the default
     physics volume, then `Owner` through `SetOwner` (below), then `Instigator`.
  6. When the world has begun play: one call that initialises each attached primitive component's physics body (inferred
     as the actor's physics init). In a non-editor game two more engine calls follow (state-frame creation and the
     collision-type update, both inferred); the collision type becomes none, no collision, normal, or one of the
     block/touch variants depending on the collision component's flags.
  7. Script `PreBeginPlay` (probe rule). Then, if the actor is not destroyed, the engine sets its zone (physics volume) and,
     for a rigid-body actor not already in the post-async tick group, moves it to that group. If the actor was destroyed
     during `PreBeginPlay` the spawn returns None.
  8. Each component flagged as attached gets one engine call (inferred as a begin-play or transform refresh; not
     read).
  9. When `bNoCollisionFail` is false the world runs an encroachment check at the final location; a failed check
     destroys the new actor (ordinary destroy sequence) and returns None. When it is true and the actor collides with
     actors, a separate relocation step runs (not read).
  10. Script `PostBeginPlay` (probe rule), then script `SetInitialState` (unconditional), then, if the actor has no `Base`,
     collides with the world, wants to be based at startup and has `Physics` none or rotating, the engine tries to find a
     base (the base-finding step). A subclass may override the step that wraps these (one `StaticMeshActor` override only
     sets a flag instead). If the actor is destroyed after `PostBeginPlay` the spawn returns None.
  11. The new actor is appended to the world's list of newly spawned actors and the generic spawn callback fires; the
     remaining code is debug bookkeeping (a spawn log and a loader-map trace for "WillowMissionItem"), not behaviour.
- **Calls into script, in order:** `GainedChild(newActor)` on the owner (from `SetOwner`), `PreBeginPlay()`,
  `PostBeginPlay()`, `SetInitialState()`; any of them can destroy the actor. `SetInitialState` is, by public UE3 convention (not
  read from the packages here), the script routine that enters the actor's initial state, which is where the first
  `BeginState` comes from; check it in the package before relying on that.
- **Calls other natives:** `Actor.Destroy` machinery on a failed check; `Object.GotoState` from script.
- **Constants:** none numeric apart from the flag meanings above.
- **Edge cases:** the world not yet "begun play" (map loading): steps 6 to 10 are skipped; the actor is flagged for the
  begin-play pass the world runs later (not read). A spawn during `Destroyed` handling of the same actor is not
  specially guarded.
- **Implementer checklist:** the three script events in that exact order after fields are set; the instigator comes from
  the spawner; the tag does nothing; a failed collision spawn returns None and creates no persistent actor; a callback
  that destroys the actor makes the spawn return None; `Owner.GainedChild` happens before `PreBeginPlay`.
- **Open:** the identity of the two unnamed engine calls in step 6, the encroachment check internals and the begin-play
  pass for a world that has not begun play.

### SetOwner (engine routine used by Spawn and Destroy, inferred)
Does nothing if the new owner equals the old one, the actor is destroyed, or the actor's object flags carry the
pending-kill bit. A new owner that would create an ownership loop is refused. If there was an old owner, `LostChild(this)`
is sent to it (probe rule); if script changed the owner during that event the routine stops. The field is set, then
`GainedChild(this)` is sent to the new owner (probe rule), again stopping if the owner changed during it. Finally the actor
is marked dirty for replication.

### Actor.Destroy
- **Signature:** no parameters, returns bool.
- **Does:** asks the world to destroy this actor with no forced network destroy and level modification allowed, and returns
  the routine's result: true when the actor was destroyed or already being destroyed, false when it was refused.
- **Refusals (return false):** actor is `bStatic` or `bNoDelete`; actor is already destroyed (this one returns true);
  the actor is not authority and is not a "net temporary" actor.
- **Destroy routine, in order (inferred as the world's destroy):**
  1. Controller special case: if the actor reports a controller or player association, the world first detaches it from
     that association (player-owned actors are handled before the generic path); not read in detail.
  2. `bPendingDelete` is set.
  3. For each Kismet event in the actor's `GeneratedEvents` that is a destroyed-event class (the class compared is found
     by name in the Engine package, probably `SeqEvent_Destroyed`; not confirmed), the event is activated with the actor
     as originator and instigator.
  4. A generic destroyed callback to editor/engine listeners; then one call that tears down each component's physics
     body (inferred as the physics term step).
  5. If the actor has a state and a state frame with non-zero latent bookkeeping, script `EndState(None)` is sent
     (probe rule on the `EndState` bit); if that destroyed the actor the routine returns true. Then, if the actor has no
     state frame or the frame's probe mask includes `Destroyed`, script `Destroyed()` is sent. Both are normal script
     event calls. (This `EndState` condition is the least certain part of the routine.)
  6. One world-level unregister call (not identified); if `Base` is set the actor is unbased; every actor in `Attached` whose `Base` is this actor is
     unbased (the list is copied first, then cleared); each actor in `Touching` whose own touching list includes this actor
     gets an untouch (script `UnTouch` is sent through the normal touch-removal routine); then `SetOwner(None)`
     (which sends `LostChild` to the old owner); then the world and, if present, the net driver are told so they drop the
     actor. After each step the routine stops if the actor was already finalised.
  7. The actor is removed from the level list, `bDeleteMe` is set, the component list is cleared and finalised.
- **Calls into script, in order:** Kismet destroyed events, `EndState`, `Destroyed`, `UnTouch` on touched actors,
  `LostChild` on the old owner.
- **Edge cases:** destroying inside `BeginState`, `EndState` or `Destroyed` is tolerated: every later step re-checks the
  destroyed flag. In a world that has not begun play the destroy is the simplified editor path (not read).
- **Implementer checklist:** `Destroy` on a destroyed actor returns true; static and no-delete actors refuse; the event
  order above; clear `Attached` bases and untouch neighbours; `LifeSpan` expiry calls the same routine with no extra
  script event (there is no `LifeSpanExpired` event in this tick path).
- **Open:** the controller special case; the exact Kismet event class; the world callback targets.

### Object.new for non-actors
`new` is not a native function in the table; it is a bytecode construct and is outside this lane.

## Iterators

Shared mechanism. Each iterator native is entered once by a `foreach` statement. It first evaluates its arguments, then
repeats: find the next acceptable actor, store it in the out argument, run the loop body, and continue; when the body
finishes or `break`s (`continue` is handled by the bytecode), control leaves. When no further actor is found the out
variable is set to None and the loop ends. The search for the next actor is done lazily (no snapshot): array growth during
the body is seen, a destroyed actor is skipped when reached, and removals shift the scan position (the code does not
compensate). A `BaseClass` of None means `Actor` for the actor-returning iterators except as noted. A class test is "is the
candidate an instance of the class (subclass allowed)". `InterfaceClass`, if given, additionally requires that the candidate
implements it.

### Actor.AllActors
Walks the world's levels in level-list order (persistent first). For a level the walk is skipped when the level is flagged
as not currently usable (a level-state field) unless a global override is set. In the persistent level the walk starts at
actor index 0 (which includes the `WorldInfo` actor); in every other level it starts at index 1. Null entries and actors with
`bDeleteMe` are skipped; the class (and interface) filter is applied; hidden or static actors are NOT skipped.
Order = level order then array order = effectively spawn order.

### Actor.DynamicActors
Same walk as `AllActors`, but each level is scanned starting at the level's first-dynamic-actor index, and actors with
`bStatic` are skipped.

### Actor.ChildActors
Walks the same level actor lists as `AllActors` and yields actors whose `Owner` is this actor (a direct-ownership scan; it does
not use a child list), with the class filter. Order as `AllActors`.

### Actor.BasedActors
Walks this actor's `Attached` array in order; skips null and destroyed entries; class filter. (Nothing checks that the
entry's `Base` still equals this actor.)

### Actor.TouchingActors
Walks this actor's `Touching` array in order; skips null and destroyed entries; class filter.

### Actor.CollidingActors
- Arguments: `BaseClass`, out actor, `Radius`, `Loc` (default: this actor's location), `bUseOverlapCheck`, `InterfaceClass`,
  optional out `HitInfo`.
- Does: asks the world's collision hash for the actors within `Radius` of `Loc` (when `bUseOverlapCheck` the test is an
  overlap test, otherwise a point-in-sphere style query), then yields those that have `bCollideActors`, are not destroyed,
  and pass the class and interface filters, in the order the hash returned them. If `HitInfo` was passed it is filled
  from the collision record of each yielded actor (the fields filled are the hit-info item, material, component, bone
  name and physical material; not mapped to names beyond that).
- The out actor is set to None between steps and when the list is exhausted.

### Actor.OverlappingActors
Queries the world's collision hash for actors overlapping a sphere (radius, location default this actor's location) and
yields them in hash order, skipping nulls and destroyed actors, applying the class filter, and, when `bIgnoreHidden`,
skipping `bHidden` actors. It does not require `bCollideActors`. The query takes this actor's own `Owner` field as an
argument (probably an actor to ignore; unverified).

### Actor.VisibleActors
Walks the same level lists as `AllActors` (including index 0 of the persistent level), skipping null, destroyed and
`bHidden` actors, applying the class filter. If `Radius` is not 0, the actor must be strictly closer to `Loc` than
`Radius` (squared distance compared). It then needs a clear line to the actor: a trace from `Loc` toward the actor's
location with a fixed set of trace flags (0x2286); the actor is accepted when the trace hits nothing or hits the candidate
itself.

### Actor.VisibleCollidingActors
Like `CollidingActors` for candidate selection (always overlap-style, `bCollideActors` required) and then like
`VisibleActors` for the line test, with trace flags 0x2086, or 0x20bf when `bTraceActors` is true. `bIgnoreHidden`,
`Extent`, `bSkipTraceTest` and `HitInfo` are honoured as their names say; the exact handling of `Extent` and
`bSkipTraceTest` was not traced. Low confidence.

### Actor.LocalPlayerControllers
Requires `BaseClass`: with None it yields nothing. Otherwise iterates the engine's local player list in index order, takes
each local player's `Actor` (its `PlayerController`), skips None, applies the class filter, and yields it. In this game
that is player index order, normally one entry.

### Actor.AllOwnedComponents
Requires `BaseClass`: with None it yields nothing. Walks this actor's `AllComponents` array in order, skips None, applies
the class filter. The index advances past each yielded component, so a component removed during the body shifts the scan.

### WorldInfo.AllControllers / AllPawns
Registered natives of `WorldInfo`; not read (see Not read yet).

## Lookups

### Actor.GetALocalPlayerController
- Walks the engine's `GamePlayers` array in index order and returns the first local player whose `Actor` is not None,
  otherwise None. No arguments. It does not look at the calling actor.
- Checklist: first non-null local controller wins; None before login.

### WorldInfo.GetWorldInfo, SequenceObject.GetWorldInfo, BehaviorBase.GetWorldInfo
`WorldInfo.GetWorldInfo` returns the persistent level's first actor (the `WorldInfo`), or None when there is no world.
`SequenceObject.GetWorldInfo` and `BehaviorBase.GetWorldInfo` share an implementation that returns the world's own world
info (None when there is no world); they ignore `self`. `Actor.WorldInfo` is a field set at spawn and is the same object.

### World time fields (what the natives above read)
`WorldInfo.TimeSeconds` (game time, advances with the dilated delta), `RealTimeSeconds` (wall time) and
`AudioTimeSeconds` exist next to `TimeDilation`. The spawn routine sets `CreationTime` from `TimeSeconds`. Where and by what
formula the world advances them each frame was not located in this lane; use public UE3 behaviour only as a placeholder
(see Open). `WorldInfo.bBegunPlay` gates spawn and destroy behaviour as described; `NetMode` value 3 means network client and
is used by the spawn placement rule.

### Controller.IsLocalPlayerController
`AController.IsLocalPlayerController` and the `PlayerController` override share one implementation that calls a virtual of
the controller; not read (belongs with controller natives).

## Object state machine

State storage, as used below: an object that supports states has a state frame holding the current state node, the
code position for state code, a latent action code (a 16-bit number, 0 meaning none), a probe mask, and a stack of pushed
states. The object's class itself counts as "no state" (the state name reads as None).

### Object.GetStateName
Returns the active state's name, or None when the object has no state frame, no state node, or the state node is the
class itself. A state whose name is uninitialised reads as `<uninitialized>`.

### Object.IsInState
`IsInState(TestState, bTestStateStack)`: true if `TestState` matches the active state or any of its parent states (the
state inheritance chain: a child state is "in" its parent). When `bTestStateStack` is true, any state on the push stack
also counts. False without a state frame.

### Object.GotoState
- **Signature:** (`NewState` optional, `Label` optional, `bForceEvents` optional, `bKeepStack` optional).
- **Does (outcomes):**
  - Omitted `NewState` means "stay in the current state" (the current state's name is used); the label then restarts that
    state's code.
  - If the target equals the current state and `bForceEvents` is false, no events are sent and only the label is applied.
  - Otherwise the state change routine runs (below); only if it reports success is the label applied.
  - Label: if omitted the default is `Begin`. The label is looked up in the new state's label table and its parents'.
    Execution of state code resumes there on the next state-code step. If a label was explicitly given and is not found,
    the log shows "GotoState (<state> <label>): Label not found". A missing default `Begin` is silent.
  - If the state change routine does not succeed: a result of 2 (script changed state again during `EndState` or
    `BeginState`) is silent; a `NewState` of None or the literal state `Auto` is silent; any other name logs a
    not-found style warning.
- **State change routine:**
  1. No state frame: fail (returns 0).
  2. Resolve the target. `Auto` selects the class's own auto state: if the class itself is flagged auto it is used, otherwise
     the first child state flagged auto, otherwise the class (no state). Any other name is looked up as a state of the
     object's class (including inherited ones); an unknown name falls back to the class (no state) and the requested
     name is reset to None.
  3. Unless `bKeepStack`, the push stack is cleared by popping every pushed state (the pop routine below, which sends
     `PoppedState` once per state when the object had a state node and a non-empty stack); if there was no stack the
     storage is just emptied.
  4. `EndState`: sent when `bForceEvents` is true, or when the old state is not None and the new name differs from it;
     gated by the `EndState` probe bit and by a re-entrancy guard (it is not sent from inside another `EndState`).
     Argument = the new state's name. If script changed state again during `EndState` the routine returns 2 and stops (the
     later state change wins).
  5. The frame is switched to the new state: latent action cleared, node and state node set, code position cleared, the
     "continuing" marker cleared, probe mask = class probe mask OR new state probe mask.
  6. `BeginState`: unless `bForceEvents` is false and the new name is None (then nothing and return 0): sent when the new
     name differs from the old name or when forced; skipped silently if no `BeginState` exists. Argument = the previous
     state's name. Returns 2 if script changed state again during it.
  7. Returns 1 on success (and marks the object as having completed a state set); returns 0 if the new name is None.
- **Calls into script:** `EndState(name NextState)`, `BeginState(name PreviousState)`; both probe-gated.
- **Edge cases:** `GotoState('None')` is allowed and leaves the object stateless (`BeginState` is not sent; `EndState` is).
- **Implementer checklist:** model the per-object state frame, not just a name; ordering EndState (old state still
  active) then switch then BeginState (new state active); `GotoState` with the same state is a label jump only; the
  default label is `Begin`; a stale `BeginState` after a nested `GotoState` is abandoned.
- **Open:** the exact meaning of two frame flags (a re-entrancy guard and a "state set" marker) beyond what the above
  uses; the debugger hooks (ignored).

### Object.PushState / Object.PopState
- `PushState(NewState, NewLabel)`: needs a state frame and a resolvable state; if the state is already on the push stack
  nothing happens (a state cannot be pushed twice). Otherwise, if the new state is not the current one: send
  `PausedState()` to the object (non-probe), push the current state, current node and current code position, make the
  new state active with its probe mask, clear the latent action, send `PushedState()`, then jump to the label (default
  `Begin`).
- `PopState(bPopAll)`: while a state is pushed: send `PoppedState()` (non-probe), restore the top entry (state, node,
  code position), pop it, set the "continuing" marker, rebuild the probe mask, clear the latent action and send
  `ContinuedState()`. Without `bPopAll` only one entry is popped. State code resumes from the saved position (not the
  label).
- Checklist: Paused (old) then switch then Pushed (new); Popped (leaving) then restore then Continued (resumed);
  `GotoState` without `bKeepStack` empties the stack with Popped events.

### Object.Enable / Disable
`Enable(name)` sets, `Disable(name)` clears, the probe mask bit for that name when the name is a probe (ids 300 to 331)
and the object has a state frame; `Enable` only sets it if the class or active state actually defines the probe function
(it ANDs with the implemented-probe set). For any other name it logs "Enable: '<name>' is not a probe function"
(`Disable` is the mirror). The current VM treats both as no-ops; with probes implemented they have an effect on `Tick`,
`Touch`, `Destroyed` etc.

### Actor.Sleep (latent) and the latent poll
- `Sleep(Seconds)` stores `Seconds` into the actor's `LatentFloat` and sets the frame's latent action code to a fixed
  value (0x180). The script statement then yields: the state code does not continue until the code is cleared.
- Each state-code step first calls the poll for the latent action: it subtracts the step's delta from `LatentFloat` and,
  when the result is below half of that delta, clears the latent action (the sleep is over). A sleep therefore ends on
  the first step where the remaining time is under half a frame, slightly early rather than late. Then code continues in
  the same step.
- `Actor.FinishAnim` does not exist in this build's Engine natives (only matinee `MAT_FinishAnimControl`).

### State-code step (engine side, inferred as the object's process-state step)
Runs from the actor tick (step 2 above). Does nothing if the object has no state frame or code, if the actor is not
authority and the state is not flagged to run for non-authority, or if the actor is destroyed. If a latent action is
pending, its poll runs; if still pending the step ends. Otherwise state code executes statement by statement until the
code runs out or a statement sets a latent action. If script changes state while running (GotoState), the loop restarts
at the new state and at most five such restarts are performed per step.
- Checklist: Sleep is polled with the actor's tick delta, not wall time; at most one state-code step per actor tick;
  `GotoState` inside state code restarts execution immediately but is capped at five per tick.

## Not read yet
- World-level time advance: where `TimeSeconds`, `RealTimeSeconds`, `AudioTimeSeconds` and the per-actor
  `CustomTimeDilation` scaling of the tick delta are applied.
- `WorldInfo.AllControllers`, `AllPawns`, `AllNavigationPoints` (iterators over the controller, pawn and nav lists
  hanging off `WorldInfo`).
- The collision-hash query internals (`CollidingActors`, `OverlappingActors`), `Extent` and `bSkipTraceTest` handling in
  `VisibleCollidingActors`, the spawn placement routine (spot finding) and the post-spawn encroachment check.
- The begin-play pass for actors spawned before the world begins play.
- The Kismet destroyed-event class check, and the controller special case in the destroy routine.
- Touch/UnTouch dispatch routines (belong with physics natives).
- `Object.new` (bytecode, not a native).
- `Actor.SetTickIsDisabled`, `SetHidden` and other flag natives that affect the tick.

## Corrections to earlier notes
- `docs/verification/NATIVE_SLICE_CENSUS.md` lists timers as "host adapter only"; the real data model is per-actor
  `Timers` entries with the semantics above (replace on re-set, cleared by rate 0, culled in the update pass). The mover
  adapter (`src/mover.cpp`) matches for rate 0 (entry gone) and for replacement (a re-set gets a fresh schedule), but it
  schedules by absolute due time and rejects negative or oversized rates and unknown functions at set time, where the
  engine stores them and drops them later; the due test and the catch-up count for loops were not compared.
- `src/natives_core.cpp` tracks a state name only: it does not model the state frame, `BeginState`/`EndState` ordering,
  the default `Begin` label, the push stack or the probe gating. This note is the reference for replacing that.
