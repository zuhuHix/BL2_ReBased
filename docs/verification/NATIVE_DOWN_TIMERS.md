# Native down timers: the second-wind reason, the injured queries, timers and special moves in the down state, the protection timer, respawn natives (draft, 2026-10-09)

AI-assisted (Claude), analyst lane G27. Read from the game executable with Ghidra (tools/ghidra/, driven by `tools/ghidra/run.ps1`), written in
our own words; no decompiler output, pseudo-code or listing structure is reproduced here. Script facts (names, parameter lists, enum values, who
calls what, the data fields a native touches) come from the cooked packages (`research/script_disasm.py`, `tools/ghidra/class_layout.py`).
**Every rule is UNVERIFIED in the running game** unless a line says how it was confirmed; each section ends with the live observation that would
confirm it. Purpose: the "analyst-read requests" of plan step S-f ([COMBAT_DOWN_RESPAWN.md](COMBAT_DOWN_RESPAWN.md)), i.e. what the VM stubbed
when Maya goes down, bleeds out, takes a second wind or respawns. Already covered elsewhere and not repeated: the actor timer table and its update
pass ([NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md), "Actor timers"), the damage pipeline's protection stages ([NATIVE_DAMAGE_DEATH.md](NATIVE_DAMAGE_DEATH.md)
section 4) and the down state's script ([NATIVE_DAMAGE_DEATH.md](NATIVE_DAMAGE_DEATH.md) section 10).

Tooling note (no addresses): the exec thunks of these natives are folded with look-alike thunks of other classes, so the label the analysis project
shows for some of them is wrong (the code is right). Every thunk of this note only calls one virtual function of the pawn, so the class tables were
needed: the `WillowPlayerPawn` table was found from the class registration (its constructor chain stores exactly that table), the `WillowPawn`
and `WillowAIPawn` tables came from the earlier lane (G26). Field names come from `class_layout.py`; for the player pawn its computed offsets
sit four bytes below the offsets the code uses for the fields after the interface tables, which was checked against the field that each read
must be (a flag word, a float, an interface table), and the pawn base class, the globals definition and the player class definition match exactly.

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| WillowPawn.SetSecondWindReason / ClearSecondWindReason | `native final function SetSecondWindReason(ESecondWindReason Reason)`, `ClearSecondWindReason()` | S-f: stores a reason byte only for an injured pawn, first writer wins; it does NOT raise health | high | UNVERIFIED |
| WillowPawn.PassSecondWindReasonToLeviathan | `native final function PassSecondWindReasonToLeviathan(WillowPlayerController WPC, ESecondWindReason Reason)` | S-f: statistics hand-off only; stub it | medium | UNVERIFIED |
| Pawn.IsInjured (and the AI pawn override) | `native simulated function bool IsInjured()` | S-f: the injured flag; for AI pawns also the AI pawn's own injured byte | high | UNVERIFIED |
| Pawn.IsDead | `native simulated function bool IsDead()` | S-f: the deletion flag or the dead flag; false for a downed player | high | UNVERIFIED |
| WillowPawn.IsInjuredDead | `native simulated function bool IsInjuredDead()` | S-f: injured-dead state byte is not None | high | UNVERIFIED |
| Actor.SetTimer / ClearTimer / IsTimerActive / GetTimerCount (as the down state uses them) | see NATIVE_ENGINE_CORE.md | S-f: the timers the state sets, which are one-shots | high | UNVERIFIED |
| SpecialMoveComponent.Play | `native final function float Play(SpecialMoveDefinition NewSMD, optional float PlayRateScale, optional float Duration, optional name CallbackName, optional Object CallbackObject, optional Object Data)` | S-f: the returned length and when the named callback runs (SwitchToSidearm, InjuredDeathAnimDone) | medium | UNVERIFIED |
| SpecialMoveComponent.GetAnimLength | `native final function float GetAnimLength(GearboxAnimDefinition CustomAnimSMD, optional bool bIncludeNextSMD, optional SkeletalMeshComponent SMC)` | S-f: a length query on the injured special move | medium | UNVERIFIED |
| WillowPlayerPawn.EnableProtectionTimer (WillowPawn version is empty) | `native function EnableProtectionTimer()` | S-f: armed at the end of a second wind; the 2 s damage floor | high | UNVERIFIED |
| WillowPlayerPawn.IsProtectionTimerActive / GetMinimumHealthMaintainedByProtectionTimer / ProtectionTimerElapsed | `native function bool IsProtectionTimerActive()`, `native function float GetMinimumHealthMaintainedByProtectionTimer()`, `native function ProtectionTimerElapsed()` | S-f: the interface the damage pipeline reads, and the timer callback | high | UNVERIFIED |
| WillowPlayerPawn.CheckLowHealthState | `native function CheckLowHealthState()` | S-f/HUD: low-health enter/recover behaviours of the player class | high | UNVERIFIED |
| WillowGameInfo.ResetAI | `native function ResetAI()` | S-f: called before a player resurrection (one player, or all) | low | UNVERIFIED |
| Pawn.SetAnchor | `native final function SetAnchor(NavigationPoint NewAnchor)` | S-f: respawn placement bookkeeping, harmless to stub | high | UNVERIFIED |
| BehaviorBase.RunBehaviors | `native static final function RunBehaviors(out array<BehaviorBase> Behaviors, Object SelfObject, optional Object MyInstigatorObject, optional Object OtherEventParticipantObject, optional BehaviorParameters EventData)` | S-f: the injured definition's behavior lists | high | UNVERIFIED |
| TeleporterDestination.GetNextExitPoint, TravelStation.CanResurrectHere, WillowPlayerPawn.NotifyTeleported | script, not native (read for completeness) | S-f: respawn placement and who schedules the respawn dialog | high (script) | UNVERIFIED |

## WillowPawn.SetSecondWindReason / ClearSecondWindReason

- **Signature:** `SetSecondWindReason(Reason)`; `ClearSecondWindReason()`. The reason is the byte enum `ESecondWindReason`: None 0, KilledEnemy 1,
  LeveledUp 2, PartnerRevived 3, Skill_Res 4 (the resurrect skill), AutoRevive 5, then unused values and a block of "extension" values up to 15.
- **Reads:** the pawn's own answer to `IsInjured` (a virtual call, see the next section) and the stored `SecondWindReason` byte.
- **Does (Set):** stores the argument in `SecondWindReason` only when both hold: the pawn answers that it is injured, and the stored byte is still
  0. Otherwise nothing happens at all. So the first reason set during an injury wins and later ones are ignored until the byte is cleared, and a
  call on a healthy pawn is lost. It touches nothing else: no health, no shield, no state change, no timer, no script call, no replication.
- **Does (Clear):** writes 0 to the byte, unconditionally.
- **Answer to "is the 25 percent raise in the same frame?":** not from this native. It only stores a byte. The raise is the script's
  `GoFromInjuredToHealthy` (health to max x `RejuvenatedHealthPctOfMax` x the revival multiplier if lower), which runs from the state's `Tick` the
  next time it sees a non-zero reason, or directly from `CheckShouldBeAutoRevived`, or from the state's helper function `Behavior_GiveInjuredPlayerSecondWind`.
  The live capture saw the health already at 25 percent in the kill frame with no script `SetHealth`, and the shield only in the next frame. What
  in the kill frame could do that is script and data, not this native: `injured.KilledEnemy` sets reason 1 as its first act, then runs
  `BehaviorBase.RunBehaviors` over the injured definition's `KilledEnemyBehaviors` in the same call. A data-driven pool change in that list
  would raise the health without a `SetHealth` call. The content of that list was not read (it is package data; see Open).
- **Calls into script / other natives:** none.
- **Who calls it, with what (script facts):** reason 1 (KilledEnemy) as the first statement of `injured.KilledEnemy` (skipped when `bCausePlayerDeath`
  is set); reason 2 (LeveledUp) from `WillowPlayerController.ExpLevelUp` on the controller's pawn, whether or not it is injured (a healthy pawn
  ignores it, thanks to the guard above); reason 3 (PartnerRevived) from the injured `Tick` when the revive timer passes `ReviveDuration`;
  reason 3 also from `Behavior_ReviveInjuredPlayer.ApplyBehaviorToContext` on the revived pawn;
  reason 4 (Skill_Res) from `LiftActionSkill.ResurrectTarget` on the target. Reason 5 (AutoRevive) is not stored by `CheckShouldBeAutoRevived`: it
  only hands the value 5 to the statistics service and calls `GoFromInjuredToHealthy` itself. `ClearSecondWindReason` is called by the injured
  `BeginState` and `EndState`, so the byte is clean on entering and leaving the state.
- **Constants / formulas:** none.
- **Edge cases:** a reason set while the pawn is not injured is dropped (the level-up case); a second reason during one injury is dropped; Clear
  never fails. For an AI pawn the "injured" answer also includes the AI pawn's own injured byte.
- **Implementer checklist:** (1) store only if `IsInjured` is true and the byte is 0; (2) clear writes 0 always; (3) no other effect; (4) the
  VM stand-in that stores unconditionally differs in one case: a level-up while healthy would leave a stale reason 2 that the next injury would
  see at once, so give it the guard; (5) health is raised by script, not here.
- **Open:** the content of `KilledEnemyBehaviors` of `GD_PlayerShared.injured.PlayerInjuredDefinition`; the byte may also be read by natives
  other than the script `Tick` (not searched).
- **Live observation that confirms it:** with `SecondWindReason` and `Pawn.bIsInjured` read before and after the call from a pyunrealsdk hook:
  call `SetSecondWindReason(2)` on a healthy Maya (byte stays 0), on an injured Maya (byte becomes 2), then `SetSecondWindReason(1)` (byte stays
  2), then `ClearSecondWindReason()` (byte 0); read `Pawn.GetHealth` around each call (unchanged).

## WillowPawn.PassSecondWindReasonToLeviathan

- **Signature:** `(WPC: WillowPlayerController, Reason: ESecondWindReason)`. The injured `Tick` calls it with the controller and the stored reason
  just before it calls `GoFromInjuredToHealthy`.
- **Does:** if the controller argument is not None, it translates the reason into a code of a statistics service's own numbering and passes
  that on to a helper (the helper was not followed, so "records an event for that controller" is a guess). It changes no pawn state. The translation
  maps reasons 1, 2, 3 and 4 to the codes 0, 1, 2 and 4, and leaves 5 up to 15 unchanged; a reason of None, or above 15, passes nothing.
- **Slice:** stub it (no effect on health, state or timers). **Confidence** medium (the helper itself was not followed).
- **Live observation:** none needed for the slice.

## Pawn.IsInjured, Pawn.IsDead, WillowPawn.IsInjuredDead

All three exec thunks only call a virtual function, so the answers depend on the class.
- **IsInjured:** the base pawn (and so `WillowPawn` and `WillowPlayerPawn`) returns the `bIsInjured` flag of `Pawn`. The `WillowAIPawn` override
  returns that flag **or** the AI pawn's own `bInjured` byte. The script sets the flag true in `injured.BeginState` and false in
  `injured.EndState` (and in `InjuredClient`), and sets it false again in `Dying.BeginState`.
- **IsDead:** the base pawn returns true when the actor's `bDeleteMe` flag is set **or** `Pawn.bIsDead` is set. The script sets `bIsDead` true only in
  `Dying.BeginState` (and `DyingVehicle.BeginState`) and false in the injured states. So a downed Maya, including the whole injured-dead
  sequence (death camera, ragdoll), is **not dead**: `IsDead` is false and `IsInjured` stays true until `EndState`. The VM's stub (always false) is right for the
  down state; it is wrong only after a real death (`Dying`) or a deletion.
- **IsInjuredDead:** true when the `InjuredDeadState` byte is not None (`INJUREDDEAD_None` 0; Start 1, StartCausePlayerDeath 2, StartKillVolume 3,
  InitRagdoll 4). Every pawn class that was read shares this one answer.
- **Controller.IsDead** (a separate virtual of the controller) was not read.
- **Edge cases:** no arguments; no side effects; no script calls.
- **Implementer checklist:** `IsInjured` = `bIsInjured` (AI pawn: or `bInjured`); `IsDead` = `bDeleteMe` or `bIsDead`; `IsInjuredDead` =
  `InjuredDeadState` != 0. `ResurrectAtLocation` asks all three (IsInjured, IsDead, reason value) when deciding what to reset.
- **Live observation that confirms it:** poll the three answers on Maya through one injury and respawn (expect IsInjured true from the injury until
  the second wind or respawn, IsInjuredDead 0 then 1 to 4 after the bleed-out, IsDead false throughout) and on a killed bullymong (IsDead true).

## Actor timers, as the down state uses them

The data model, the replace rule, the clear rule and the update pass are in [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) and were not re-read in
full. The internal set routine was re-read once (it is also called from native code) and agrees with that note on the points the down state
depends on: a static actor ignores the call; an entry with the same function name and object is replaced in place; a rate of 0 clears the
entry's rate; setting a non-zero rate resets the elapsed count and clears the pause flag. What the script around the down and respawn path adds
(all one-shot, all on the pawn itself, none with an explicit object):

| Set from | Rate | Function called | Notes |
|---|---|---|---|
| `WillowPawn.injured.BeginState` | the injured animation length (only if it is above 0, only on the override path through the action skill's override) | `PlaySpecialMoveOverride` | plays the injured special move again with callback `SwitchToSidearm` |
| `WillowPlayerPawn.injured.BeginState` | `InjuredDefinition.AutoReviveCheckDelay` (2 s) | `CheckShouldBeAutoRevived` | **one-shot and not re-armed by the function**: it runs once per injury, 2 s after `BeginState`, and acts only when the `ShouldGetResurrected` attribute is above 0 and a uniform roll is below it |
| `WillowPlayerPawn.CalcInjuredDeadCamera` | 1.0 s | `DeRezGear` | fires 1 s after the dead camera starts; `DeRezGear` starts the weapon's de-rez effect and, if that returns a length above 0, sets a second one-shot `DeRezzedGearFinished` for that length (else calls it at once) |
| `WillowPlayerPawn.CalcAwaitingRespawnCam` | 0.6 x the re-rez effect duration, or 1.0 s without an effect | `RezGear` | `RezGear` clears `DeRezzedGearFinished` first |
| `WillowPlayerPawn.AwaitingRespawnDisplayRespawnCost` and `NotifyTeleported` | 2.0 s | `ShowRespawnDialog` | both skip it in the Coliseum |
| (native) `ProtectionTimerElapsed` | `GlobalsDefinition.ProtectionTimerDurationInSeconds` (2 s) | native function, see below | the only timer in this lane set from native code |

What a faithful minimal facility needs, beyond the engine note: (1) a timer whose function is a **native** function (`ProtectionTimerElapsed`)
as well as script ones; (2) the update pass runs once per actor tick after the state step, so a one-shot set in `BeginState` first fires on the
first tick whose elapsed count is strictly greater than its rate, never at the set instant and never early; (3) all of these are non-looping,
so a one-shot clears itself after firing unless the callback sets the same timer again; (4) if the pawn is destroyed first, the pass stops and
the callback never runs (the respawn keeps the same pawn, so the timers survive the down state); (5) among the functions read here, `ClearTimer` is
used once (`RezGear` clears `DeRezzedGearFinished`); `IsTimerActive` and `GetTimerCount` are not used by the down or respawn script.
- **Open:** whether `EndState` of the injured state clears `CheckShouldBeAutoRevived` (not searched); the 2 s value is data.
- **Live observation that confirms it:** hook `WillowPlayerPawn.CheckShouldBeAutoRevived` and read `Actor.GetTimerRate` for it in the injured
  state: expect a call about 2.0 s after the injury (strictly later by up to one frame) and none after, and a rate of 0 or -1 afterwards.

## SpecialMoveComponent.Play

- **Signature:** as in the summary. Defaults: rate scale 1.0 (taken as 1 when omitted), duration 0, no callback name, no callback object, no data.
  Returns a float: the length of the move in seconds, or 0 when nothing started.
- **Reads:** the component's owner (a pawn through the special-move interface) and the owner's skeletal mesh, the component's current move and queue,
  the move definition (`SpecialMoveDefinition`), the owner's role.
- **Does (in order):**
  1. Returns 0 (starts nothing) if the definition is None or the component has no owner.
  2. Resolves the definition to the one that will really play: a plain definition resolves to itself, an expression list or a random list
     picks one of its members (that virtual is native on the definition class; its list logic was not read). None resolves to 0 length.
  3. Asks the resolved definition whether this machine is the authority for it (`IsLocalAuthority`, native) and then its script function
     `AuthorityCanPlay` (default true). If either is false nothing plays and the result is 0.
  4. Clears the queued moves and starts the move: if another move is current it is stopped first, as an interruption that also clears the
     queue (that stop runs the old move's finish handling and its callback with `bInterrupted` true, see below). The component then
     records the new current move: the definition, the data object, the rate scale, the duration, the callback name and callback object (the callback object
     defaults to the owner), and the start time in world time. The chain of definitions that follow it (each definition's "next" link) is put in the queue
     list, each follower with the same data object, rate scale 1, no duration and no callback.
  5. Asks the owner for its default skeletal mesh (result unused) and, with authority, hands the new move data to the owner to replicate. With authority it
     then runs the definition's script `ServerStarted` (for an animation definition: it sets up the behavior provider and runs the start behaviors),
     and in every case the script `ClientStarted`.
  6. The result of `ClientStarted` is the returned length. For an animation definition (`GearboxAnimDefinition`) that script starts the
     animation: it adds the animation set, then calls `PlayAnim`, which returns what the animation node's play function returns. **If that length is 0 or
     less, the move is stopped at once as a finished (not interrupted) move, so its callback runs inside `Play` before `Play` returns.**
  7. If the world is a network client's view and the owner is not the authority, the call is also forwarded to the server (not relevant to a single player).
- **Returned length for an animation move:** `PlayAnim` computes a rate: when the call gave a positive `Duration` and the definition's ending condition is
  not `EC_Loop`, the rate is the animation's natural length divided by the duration (the move is stretched to the duration); otherwise it is the
  call's rate scale. It then asks the `AnimNodeSpecialMoveBlend` of the owner to play. That node (a slot-style animation node) refuses (returns 0) when the
  definition is not an animation definition, when the animation name cannot be found in the mesh's animation sets, or when the effective rate is
  below 0.0001 in magnitude. Otherwise it starts the animation named by the definition's `AnimName` (no per-call name override from `Play`), with
  effective rate = the definition's `PlayRate` x the rate argument, the definition's `BlendInTime` and `BlendOutTime` (negative blend overrides mean "use
  the definition's"; the blend-out is 0 for `EC_StopOnLastFrame` and `EC_OnAnimEnd`; both blends are forced to 0 when the animation is shorter than
  blend-in plus blend-out and the ending condition is not `EC_Loop`; a negative blend-in is raised to 0), looping only for `EC_Loop`; if a move was already active on the node it is finished as interrupted first. The value
  it returns is the sequence length (or the node's explicit end position when that is positive) divided by the **effective playback rate** (the
  rate argument x the mesh's global rate scale x the sequence's own rate scale x the animation group's scale), as a positive number. Blends are not
  added. So the injured special move's 2.28 s live value should equal natural length / effective rate for that animation (UNVERIFIED which rates apply).
- **When the callback runs:** `CallbackName` is a function called on `CallbackObject` (the owner when none was given) with one optional bool
  parameter named `bInterrupted` (any other parameter is zero). The function is looked up by name with the state-aware lookup (the same kind the timer
  update pass uses), so a state's override such as the injured state's `SwitchToSidearm` is the one called, which the live behaviour needs; a name that does not
  resolve, or a function that is neither defined in script nor native, is silently skipped. It runs once when the move ends, in three ways: (a) the animation ends: the node's finish handler calls the definition's script `AnimFinished`,
  which stops the move through `SpecialMoveComponent.Stop` (or `LocalAnimFinished` when the component no longer lists it as current) and the callback
  runs with `bInterrupted` false; the finish handling also runs the script `ServerFinished`/`ClientFinished`, the stop behaviors and the root-motion
  restore first, and starts the next queued move afterwards; (b) another `Play` or a `Stop` with the interruption flag replaces it: callback with `bInterrupted` true;
  (c) `ClientStarted` returned 0 or less: callback at once, `bInterrupted` false. The callback runs on the authority side only (the `IsLocalAuthority` test).
  Both `SwitchToSidearm` and `InjuredDeathAnimDone` take no argument in the script, so the flag is simply dropped.
  The exact moment of (a) inside the animation (at the last frame, or when the blend-out begins) was not determined: the ending condition values are
  `EC_StopOnLastFrame`, `EC_OnAnimEnd`, `EC_OnBlendOut`, `EC_Loop`, and the live 2.28 s for `SwitchToSidearm` and 0.84 s for `InjuredDeathAnimDone` are the measured ends.
- **Calls into script:** `AuthorityCanPlay`, `ServerStarted`, `ClientStarted` (and through it `OnStart`, `AddAnimSet`, `PlayAnim`), later `AnimFinished`,
  `ServerFinished`, `ClientFinished`, then the named callback.
- **Calls other natives:** `AnimNodeSpecialMoveBlend.PlayData` / `Finish` / `Stop`, `SpecialMoveDefinition.IsLocalAuthority`, `GetSMDToPlay`.
- **Constants / formulas:** effective rate = definition `PlayRate` x call rate scale (x mesh and sequence scales for the length); stretch rate =
  natural length / `Duration`; minimum effective rate magnitude 0.0001; default blend arguments -1 (meaning "use the definition's").
- **Edge cases:** None definition or no owner: 0; no authority: 0; animation missing: 0 and (see step 6) an immediate finished callback; replacing a
  playing move fires the old move's callback with `bInterrupted` true before the new move's `Play` returns.
- **Implementer checklist (for a VM without animation):** return a configured length per definition (the measured 2.28 s and 0.84 s are
  placeholders until the animation lengths come from data); schedule the callback at that length as a one-shot on the pawn; on a replacing `Play`
  or `Stop`, call the old callback at once with `bInterrupted` true; if the length is 0 call it at once; look the callback up in the pawn's current state first.
- **Open:** the list/random definitions' selection; the per-definition native length function and whether it already folds in `PlayRate`; the moment
  of the end notification relative to the blend-out; `PlayLocal`, `Queue`, `Stop`, `StopAny` and `GetSMNode` only as far as they share the code above.
- **Live observation that confirms it:** hook `SpecialMoveComponent.Play` and the two callbacks on Maya's injury and death: record the returned float, the
  time the callback runs (expect `Play` returned about 2.28 s and the callback 2.28 s later within a frame, and 0.84 s for the death animation) and
  `AnimSequence` length, play rate and `BlendOutTime` of the definition (to decide the "last frame or blend-out" question); call `Play` twice in a row
  and see the first callback run with `bInterrupted` true.

## SpecialMoveComponent.GetAnimLength

- **Signature:** `(CustomAnimSMD: GearboxAnimDefinition, bIncludeNextSMD: bool, SMC: SkeletalMeshComponent)`; returns a float. The injured `BeginState`
  adds `GetAnimLength(InjuredSpecialMoveOverride)` to `InjuredAnimLength` on the action-skill override path.
- **Does:** 0 for a None definition or a component without owner. The mesh defaults to the owner's. It returns the definition's own length (a native
  virtual of the definition, given the mesh; not read) and, when `bIncludeNextSMD` is true, adds the length of its "next" definition, resolved like `Play` does and
  only if it is an animation definition, continuing along the chain until there is no next one or a definition repeats itself.
- **Open:** the definition's own length function.
- **Live observation:** call it on `InjuredSpecialMoveOverride` with and without the flag and compare with `Play`'s return.

## The protection timer: EnableProtectionTimer, IsProtectionTimerActive, GetMinimumHealthMaintainedByProtectionTimer, ProtectionTimerElapsed

The player pawn carries a small interface (`IProtectionTimer`, three natives) implemented by itself; the damage pipeline stage 15 of
[NATIVE_DAMAGE_DEATH.md](NATIVE_DAMAGE_DEATH.md) uses it. Fields: `bIsProtectionTimerActive` (a bool of the player pawn) and
`MinimumHealthMaintainedByProtectionTimer` (float).
- **IsProtectionTimerActive:** returns `bIsProtectionTimerActive`. **GetMinimumHealthMaintainedByProtectionTimer:** returns the float field.
  Neither has side effects.
- **EnableProtectionTimer:** the `WillowPawn` version (and so every AI pawn) does nothing. The `WillowPlayerPawn` version:
  1. Needs the game's globals object and its `GlobalsDefinition`; if either is missing, or the timer is already active, it returns without any change.
  2. Sets `bIsProtectionTimerActive`.
  3. Sets a one-shot timer on the pawn for `GlobalsDefinition.ProtectionTimerDurationInSeconds` (2 s in the data) calling the native `ProtectionTimerElapsed`.
  4. Computes the floor: ceiling = the pawn's maximum health x `GlobalsDefinition.ProtectionTimerMaintainedMaxHealthPct`, raised to 1.0 if smaller;
     `MinimumHealthMaintainedByProtectionTimer` = a uniform random value in [1, ceiling) (the C runtime `rand` divided by 32767 times (ceiling - 1), plus 1).
  5. Then caps it: if the pawn's **current** health is below that value, the floor becomes the current health.
  The earlier note describes the floor as exactly "max x ProtectionTimerMaintainedMaxHealthPct (0.1)"; the random part is new. With the data values
  (0.1) and a maximum of 547.874 the floor is a random value between 1 and 54.8 HP, never above the health at the time of arming.
- **ProtectionTimerElapsed:** clears `bIsProtectionTimerActive`. It does not reset the floor value.
- **Who calls EnableProtectionTimer (script facts):** the injured `Tick`, right after the second wind has run `GoFromInjuredToHealthy`, the recovered behaviors, the
  statistics and the skill event (so the 2 s protection is the tail of every second wind); and the damage pipeline's stage 15 (the existing note: when
  a hit from an AI would take the player below `ProtectionTimerThreshold` x max health; the call site in the pipeline was not located here, and no script reads
  `bIsProtectionTimerActive` or the floor, so the pipeline reads them natively).
- **Calls other natives:** `Actor.SetTimer` (internal form).
- **Constants / formulas:** `ProtectionTimerDurationInSeconds` 2 s, `ProtectionTimerMaintainedMaxHealthPct` 0.1, `ProtectionTimerThreshold` 0.5 (data values from the earlier note); random divisor 32767.
- **Edge cases:** active already: no re-arm and no new floor; max health below 10: the ceiling clamps to 1.0 and the floor is exactly 1.0 (or the lower current health).
- **Implementer checklist:** one flag, one float, one 2 s one-shot that clears the flag; the floor is `min(current health, 1 + rand01 x (max(max x 0.1, 1) - 1))`
  with `rand01 = rand / 32767`; a VM that wants determinism can take the middle or the maximum of the range and must say so.
- **Open:** whether the random draw uses the engine's generator state (it calls the C runtime `rand`, so the seed is not the unreal one).
- **Live observation that confirms it:** after a second wind read `bIsProtectionTimerActive`, `MinimumHealthMaintainedByProtectionTimer` and
  `GetTimerRate` of `ProtectionTimerElapsed` (expect true, a value in [1, 0.1 x max) no larger than the health at that moment, 2.0 s), repeat
  several injuries for the spread, and hit Maya during the window to see damage stop at the floor.

## WillowPlayerPawn.CheckLowHealthState

- **Signature:** no parameters, no result. Not called from script. The player pawn's native per-frame function (a virtual that takes the frame's delta
  time, almost certainly the actor tick override) calls it every frame after the base pawn tick and one other per-frame virtual, so it is a poll.
  No script reads or writes `bLowHealth`.
- **Reads:** the pawn's controller (must be a `WillowPlayerController`; otherwise nothing happens), the controller's `PlayerClass` (a
  `PlayerClassDefinition`) and from it `LowHealthPercent`, `OnLowHealth` and `OnLowHealthRecover` (two arrays of behaviors); the pawn's current and maximum health;
  the pawn's `bLowHealth` flag.
- **Does:** fraction = current health / maximum health. If the fraction is below `LowHealthPercent` and the flag is not set: sets the flag and runs
  the `OnLowHealth` behavior list. If the fraction is at or above `LowHealthPercent` and the flag is set: clears the flag and runs the
  `OnLowHealthRecover` list. In the other two cases nothing happens (no repeat while it stays low). A missing player class or controller: nothing.
  The lists run through the same behavior runner as `BehaviorBase.RunBehaviors` (see below); the arguments after the list (self object, instigator) could
  not be read from the decompiler output, the pawn is the likely self object.
- **Constants:** none in the code; `LowHealthPercent` is class data (a fraction, compared with a fraction).
- **Edge cases:** maximum health 0 gives a non-finite fraction (no guard); a downed Maya (health 0) is "low" and keeps the flag until the second wind or respawn lifts the health
  to or above the threshold; the flag is only changed by this poll (no script touches it).
- **Implementer checklist:** per frame, two edge-triggered branches on `health/max` against `PlayerClassDefinition.LowHealthPercent`; run the two lists on the edges.
- **Open:** which behaviors Maya's lists hold (data); exact order against the other per-frame virtual.
- **Live observation that confirms it:** read `bLowHealth` and `LowHealthPercent` while stepping Maya's health across the threshold; hook `RunBehaviors`
  to see the list run once per crossing, in each direction.

## WillowGameInfo.ResetAI and Pawn.SetAnchor

- **WillowGameInfo.ResetAI():** a virtual of the game info (no parameters). The game info's version walks the world's registry of interface-based
  actors, picks the AI pawns that are active, and for each one resets part of its AI state and fires the behavior kernel event named `OnReset`
  for that pawn as the consumer. The script calls it in `HandlePlayerDeathResurrection` before the resurrection when one player dies. This is the
  least certain section: the per-entry conditions (a registry entry flag, a definition flag, the target-record check) and the exact AI resets were not identified.
  For the slice (Maya respawns, no AI reset needed to observe the respawn itself) a no-op is acceptable; the visible effect is AI dropping targets and replaying "reset" behaviors.
- **Pawn.SetAnchor(NewAnchor):** if the pawn already has an anchor whose "anchored pawn" back-reference points at this pawn, the back-reference is
  cleared and a "last anchor release time" is written on that old anchor. Then the pawn's anchor is set to the argument. If the argument is not None,
  the pawn records the world time of the change, the "last valid anchor" and, unless its controller (or the controller of the pawn it is driving) has the
  flag that disables anchor claiming, writes itself into the new anchor's back-reference. None just detaches. Pure bookkeeping; harmless to stub.
- **Live observation:** `ResetAI`: hook `HandlePlayerDeathResurrection` and watch an adult bullymong's `AIComponent.Targets` and `CurrentTarget` before and
  after (expect them dropped) and count `OnReset` event activations; `SetAnchor`: read `Pawn.Anchor` before and after the respawn.

## BehaviorBase.RunBehaviors

- **Signature:** `(out Behaviors: array of BehaviorBase, SelfObject, optional MyInstigatorObject, optional OtherEventParticipantObject, optional EventData)`. Static.
- **Does:** for each entry of the array in order, skips None entries and calls the entry's script function `ApplyBehavior` with: the self object, an
  empty kernel-info record (the same record is passed to every entry, so a later behavior sees what an earlier one wrote into it), the instigator,
  the other participant and a copy of the event data. `ApplyBehavior` (script, Engine) then looks up the behavior's contexts and calls
  `ApplyBehaviorToContext` on each. Nothing is returned. No exceptions for an empty or None array.
- **Down state use:** `injured.KilledEnemy` passes the definition's `KilledEnemyBehaviors` with the pawn as self; the injured `Tick` passes
  `RecoveredBehaviors` (self = the pawn, instigator = the controller) after the second wind; `BeginState`/`EndState`/revive use the targeted, injured, revived and
  resurrected lists the same way (script call sites of the lists other than the first two were not enumerated here). The five pool refills are data inside those lists.
- **Implementer checklist:** a loop over the array calling the script `ApplyBehavior` with the arguments above; the VM's existing stand-in that applies attribute
  effects directly is a shortcut that skips `ApplyBehaviorToContext`.
- **Live observation:** hook `BehaviorBase.RunBehaviors` during an injury and a second wind: record the array contents (class and name of each behavior),
  the argument objects, and compare the health and shield pools before/after each call.

## Respawn placement and the respawn dialog (script, no native)

Read from the script so the VM lane does not look for natives that are not there.
- **TeleporterDestination.GetNextExitPoint(TeleportActor):** returns None for a None actor or an empty exit list. Otherwise it walks the exit list
  round robin starting at a stored counter, takes the first exit point of the right kind for the actor with no pawn inside the actor's collision radius (32 uu when it
  has no cylinder; a collision query around the exit point; vehicle exit points for vehicles and ordinary ones otherwise, with a second pass over the
  other kind; the exact acceptance rules were not worked out), advances the counter past the one it used (modulo the list length), and returns it. If every
  exit is blocked it returns the one at the stored counter anyway and still advances. `ResurrectAtLocation` later calls `Pawn.SetAnchor` on a result.
- **TravelStation.CanResurrectHere(bIsLevelTravel):** the base station says false; `FastTravelStation` says true when it should be active, is
  currently active, or its definition is initially active; `ResurrectTravelStation` says true except for a level travel. Called by
  `WillowPlayerPawn.GetBestPlayerPlacementPoint`.
- **Who schedules `ShowRespawnDialog` (the 11.8 s after `ResurrectPlayer`):** the script, twice, each a 2.0 s one-shot on the pawn:
  `WillowPlayerPawn.NotifyTeleported` (when the respawn goes through a teleport, after the re-rez effect is pushed and popped, not in the Coliseum) and
  `AwaitingRespawnDisplayRespawnCost` (called by `CalcAwaitingRespawnCam` once the head-zoom fraction passes 0.95 and the camera hands control back).
  No native schedules it. The other seconds of the 11.8 come from the awaiting-respawn camera: the re-rez coordinated effect's duration, the
  `RezGear` timer, a 0.3 s-scaled zoom and the camera lerp time, all script and data. The VM can reproduce the 11.8 s if it has the one-shot timer facility above
  and the effect duration from the package, with the camera function asked once per frame.
- **Open:** exact sum for Maya (needs the effect duration and `AwaitingRespawnLerpTime` from data).
- **Live observation:** log the time of `ResurrectPlayer`, `NotifyTeleported`, `CalcAwaitingRespawnCam`'s exit and `ShowRespawnDialog` and the `Duration`
  of `BodyClassDefinition.PlayerReRezCoordinatedEffect`; the 2.0 s gap before the dialog should appear at each.

## Not read yet

- `Controller.IsDead`; `PlayLocal`, `Queue`, `Stop`, `StopAny`, `ClearQueue`, `GetSMNode` beyond their shared code with `Play`.
- `GearboxAnimDefinition.GetAnimLength` (the definition's own length), `SpecialMoveDefinition.GetSMDToPlay` (lists and random lists).
- The per-entry conditions of `WillowGameInfo.ResetAI` and the AI resets it makes.
- `WillowPlayerPawn.CheckLowHealthState`'s sibling virtual that runs just before it.
- The content of `GD_PlayerShared.injured.PlayerInjuredDefinition`'s behavior lists (package data).

## Corrections to earlier notes

- [NATIVE_LEVEL_UP_ATTRIBUTES.md](NATIVE_LEVEL_UP_ATTRIBUTES.md) section 9 says the native stores the byte "only when a pawn virtual (not read) reports a
  condition": the condition is `IsInjured`, and ExpLevelUp's call therefore does nothing for a healthy pawn.
- [NATIVE_DAMAGE_DEATH.md](NATIVE_DAMAGE_DEATH.md) section 10.2 says `CheckShouldBeAutoRevived` runs "every `AutoReviveCheckDelay`": script has one arming
  (`WillowPlayerPawn.injured.BeginState`) and the function does not re-arm itself, so it is checked once per injury.
- NATIVE_DAMAGE_DEATH.md stage 15 gives the protection floor as exactly max x `ProtectionTimerMaintainedMaxHealthPct`: it is a random value between 1 and that
  product, capped by the current health, drawn when the timer is armed.
- [COMBAT_DOWN_RESPAWN.md](COMBAT_DOWN_RESPAWN.md) "Natives bound for this step": `SetSecondWindReason` should store only when the pawn is injured and the byte is 0;
  `Pawn.IsDead` false is right while the pawn is injured or injured-dead.
