# Maya goes down, bleeds out, takes a second wind or respawns, on the VM (plan step S-f, 2026-10-09)

AI-assisted (Claude), implementer lane Sf. Worked from the own-words notes [NATIVE_DAMAGE_DEATH.md](NATIVE_DAMAGE_DEATH.md) (section 10), [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md)
(the currency natives), [NATIVE_ATTRIBUTES.md](NATIVE_ATTRIBUTES.md) (section 5), the live rows of [COMBAT_GOLDEN_TRACE.md](COMBAT_GOLDEN_TRACE.md) (section D steps 11 to 17 and the lane L5b
question-6 table) and the installed script, read with `research/script_disasm.py` (the allowed bytecode reader). No decompiler output and nothing under `local/analysis/` was read. Every
rule below is `UNVERIFIED` as a statement about the native code unless a row names the live number it reproduces; "reproduces" means "the VM's number equals the recorded live number", not "the
native does exactly this". It builds on the hit chain ([COMBAT_HIT_CHAIN.md](COMBAT_HIT_CHAIN.md)), the death chain ([COMBAT_DEATH.md](COMBAT_DEATH.md)), the kill experience ([COMBAT_KILL_XP.md](COMBAT_KILL_XP.md))
and the melee fixture ([COMBAT_MELEE.md](COMBAT_MELEE.md)).

## The verdicts

1. **The injured state does not use latent code.** The state-level script of `WillowPawn.injured` and `WillowPlayerPawn.injured` (and of every other player-pawn state: `InjuredClient`, `MostlyDead`,
   `MostlyDeadClient`, `AwaitingRespawn`, `ClientAwaitingRespawn`, `Dying`) is two bytes long: the public UE3 token for a state's `Stop` and the end-of-script token (read from the stored sizes of
   the state exports: script size 2; a `Sleep` or a label would need a label table and a longer script; the reading of the first byte as `Stop` is UNVERIFIED, the size is not). Everything the state does is
   `BeginState`, `EndState`, `Tick` and functions called by other code. The VM's rule "latent state code stays out" therefore costs nothing here, and no new VM facility was needed.
2. **The bleed-out is 12 / 12 / 8 s for injuries 1 / 2 / 3, not 12 / 8 / 4**, when the previous injury is more than 30 s old for the first of them; it is 12 / 8 / 4 / 1 when the first injury falls in the first 30 s of
   game time. No native differs: the old note read the counter after the injury had started. Details in "Bleed-out" below.
3. **What the script times itself, and what it hands to the animation and the station.** `Tick` times the bleed-out (12.0 s) and the revive. The death camera is timed by the script too: the
   pawn is asked for its camera view every frame (`CalcInjuredDeadCamera`) and ends the camera by itself after delays that are data (the VM gives 5.75 to 5.78 s against the live 5.76 s). Three delays belong to things the VM does not have and are played
   at the live values by the driver, as labelled stand-ins: the injured special move's end (`SwitchToSidearm`, live 2.28 s), the injured-death animation's end (`InjuredDeathAnimDone`, 0.84 s) and the station's
   call that shows the respawn dialog and charges the fee (`ShowRespawnDialog`, 11.8 s after the respawn).
4. **The respawn point is a stand-in.** The game picks a `TravelStation` of the level and an exit point of it (script over the level's actors); the VM world has no level, so the installed script
   falls back, by its own rule, to the controller's `StartSpot`, which the run makes a bare `PlayerStart`. The station choice, the exit-point round robin, `CanResurrectHere` and the holding cell are not run.

## What a run does

`ow-package <any.upk> --combat-run --cooked <dir> maya:8 players:1 spawn:adult:7 ... tick:100 cash:2715 swing:0:melee swing:0:melee swing:0:melee swing:0:melee bleed:50 ...` (the full step list is under
"Reproduce"). Nothing in it is coded as a rule of the game that the script already has: the steps give state and time, the installed script does the rest.

- **Down.** The swings (S-e) take Maya's health to 0 through the installed `TakeDamage`; `WillowPawn.HandleHealthDepleted` sees a pawn that can be injured and calls `PlayInjured`, which does `GotoState(injured)`.
  The VM's `GotoState` already runs `EndState`/`BeginState`, so the state's `BeginState` runs: the flags, the bleed-out total, the injury counter, the injured screen setup (presentation natives are stubs),
  the injured state byte 1. No script change, no native for the transition.
- **Targeted.** The special move that puts the weapon away ends 2.28 s after the injury in the live game and calls `SwitchToSidearm` (the name is the callback `BeginState` hands to the special move component).
  The `bleed` step calls it after 2.28 s of game time. It sets the injured state byte to 2 and starts the timer (`flInjuredTargetedTime` = 0).
- **Tick.** While the state is `injured` the step calls the pawn's installed `Tick` every frame (default 1/60 s). With no duel (`HasActiveDuel` is bound to false) it is `WillowPawn.injured.Tick`: it
  counts `flInjuredTargetedTime` up, sets the injured bonus in the last 10 percent, reads the second wind reason, and, once the timer passes the total, starts the death.
- **Death.** `SetInjuredDeadState(1)`, `StartInjuredDeathSequence` (camera set-up, input off), then 0.84 s later the driver calls `InjuredDeathAnimDone` (dead state 4, the ragdoll call). The camera is asked
  for its view each frame; when the script's end time has passed it calls `EndInjuredDeadCamera`, whose state version calls `InjuredRespawn`.
- **Respawn.** `InjuredRespawn` -> `WillowGameInfo.HandlePlayerDeathResurrection` (one player: `players:1`, `ResetAI`, then `ResurrectPlayer(1)`) -> `ResurrectAtLocation` -> `GoFromInjuredToHealthy` -> `GotoState(None)`
  (`EndState` clears the flags). `ResurrectPlayer` resets the injury counter to 0. Health is raised by the script to the maximum times `ResurrectedHealthPctOfMax` (1.0 in the data).
- **Fee.** 11.8 s after the respawn the driver calls `WillowPlayerPawn.ShowRespawnDialog`, which evaluates `GlobalsDefinition.TeleportCost` with the controller as the source (the data: 0.07 x credits, rounded half up),
  takes at most the cash and charges it with `AddCurrencyOnHand(0, -amount)`.
- **Second wind.** A kill while down (the `hit` step; `injured.KilledEnemy` runs on her pawn by the state's own function) sets the second wind reason through the native `SetSecondWindReason`; the next `Tick`
  sees it and calls `GoFromInjuredToHealthy`, which raises health to the maximum x `RejuvenatedHealthPctOfMax` (0.25) x `RevivalHealthMultiplier` (the attribute, 1.0 at its default), if it is lower, and leaves the state.

## Bleed-out: the answer to the 12 / 12 / 8 against 12 / 8 / 4 question

`BeginState` first works out `TotalBleedoutTime = BaseRejuvenateDelay (12 s for one player) x TimeToBeRevivedMultiplier`, then the penalty `total / MaxDesiredSuccessiveInjuries (3) x NumSuccessiveInjuries`,
then `total = max(total - penalty, 1)`. **Only at the end of `BeginState`** does it update the counter: if `TimeSeconds - LastInjuryTime` is below `MaxTimeBetweenInjuries` (30 s) the counter is raised by one, else it is
set to 0, and `LastInjuryTime` is stamped. So the cut uses the counter as it was **before** this injury; the counter that lane L5b read ("0, 1, 2") was read after `BeginState` had raised it. The cut is therefore
`total / 3 x (counter after - 1)` (never below 0): 0, 0, 4, 8 and 12 s for the counters 0, 1, 2, 3, 4 read after the injury, and the 1 s floor from the fourth. That is L5b's fit `total x (1 - (N - 1) / 3)`; the old note's `total / 3 x N` is the same formula with N the counter before the injury, read as if it were the counter after.

| Run (the VM, the installed script) | Counter read after the injury | `TotalBleedoutTime` read after `BeginState` | Timer when the dead state began |
|---|---|---|---|
| Six-injury run, injuries 1 to 3 (each the first injury, game time 100 s, a respawn between) | 0, 0, 0 | 12, 12, 12 | 12.0167 s (a 1/60 s frame past the total) |
| Injuries 4 to 6 (a second wind from a kill between, 9 s apart) | 0, 1, 2 | **12, 12, 8** | injury 6: 8.0000 s |
| Early run (first injury at game time 0, a second wind between each) | 1, 2, 3, 4 | **12, 8, 4, 1** | injury 4: 1.0167 s |

The live table (L5b: the counter 0, 1, 2 gave 12.0, 12.0, 8.0 s, the previous injury more than 90 s, 11.5 s and 7.45 s earlier) is the second line. The third line is what the same script does when the first injury
falls less than 30 s after game time 0, because `LastInjuryTime` starts at 0 (the class default read by the VM); whether the game's default is also 0 is not known, and the live runs never fell in that window
(UNVERIFIED). The 1 s floor is reached by the fourth successive injury in the VM; it was not seen live (UNVERIFIED there). The measured bleed in the rows is the script's own timer at the frame the dead state began, `bleed_over`
= timer minus total, which is at most one frame; the live bleed-outs were 12.00 to 12.04 s.

## Results against the live rows

`local/p2/Sf/run_all.sh` (one `--combat-run`: the earlier S-a/S-d/S-e rows, then the six injuries), `python tools/verify_combat_pawn.py --vm local/p2/Sf/all_rows.json`: **55 PASS, 0 FAIL, 4 MISSING of 59** (the 4 missing
are the Defense Boost rows, as before; the 25 earlier passes are unchanged; 30 passes are new). The comparator checks the VM rows against the recorded live numbers and rules (constants in `tools/verify_combat_pawn.py`):

| Row | Live | VM |
|---|---|---|
| `down.total.k`, `down.successive.k` | 12, 12, 8 for the counter 0, 1, 2 (L5b) | 12, 12, 12, 12, 12, 8 and 0, 0, 0, 0, 1, 2 |
| `down.bleed_over.k` | bleed-outs 12.00 to 12.04 s | 0.0167 (injuries 1 to 3), 0.0000 (injury 6) |
| `down.camera.k` (dead state to the end of the death camera) | 5.761, 5.765, 5.762 s | 5.751, 5.782, 5.782, 5.782 (a 1/60 s frame; derived by the script from the data) |
| `down.second_wind_health.k` | 136.97 = 0.25 x 547.874 | 0.2500 of the maximum (53.1689 of 212.6755 at level 8), twice |
| `down.respawn_health.k` | full health after the respawn | 1.0000, four times |
| `down.fee.k` | 190, 177, 165 on 2,715, 2,534, 2,357 (L3) | 190, 177, 165, and 153 on 2,192 (the rule only) |

What this does **not** show: the shield half of the second wind and of the respawn (0.9 and 1.0 of the maximum; the VM's Maya has no shield pool), the Defense Boost that follows a second wind, the
kill-frame health (below), the sidearm switch, any animation, and the station.

## Delta against the live game (reported, not fitted)

- **Second wind health comes one frame late.** Live, the health is already 25 percent in the frame of the kill, with no script `SetHealth` visible. In the VM `SetSecondWindReason` only stores the
  reason (the stand-in) and the next `Tick` raises the health, so the number is the same one frame later. If the native raises the health itself, that is the analyst-read request below.
- **Death camera 5.75 to 5.78 s against 5.76 s.** A frame of quantisation at the step's 1/60 s; the camera time is the script's (`GetInjuredDeadCameraValues` and the effect duration of
  `GlobalsDefinition.DigistructCoordinatedEffect`), but the VM only has that duration because the run builds the effect object (`loadGlobalsReference`): the VM leaves a reference to another export as an
  empty stand-in, and with it the camera ended after 0.77 s.
- **The 11.8 s to the fee.** The script's own part is small: `AwaitingRespawnDisplayRespawnCost` sets a 2.0 s timer for `ShowRespawnDialog` (the VM has no timers, so the driver calls the dialog itself).
  The remaining 9.8 s are the station path's and its caller was not traced; the 11.8 s is a live input, not a result.
- **The respawn point** and **`ResetAI`, `SetAnchor`, `Actor.SetLocation`, the teleport effect** (stubs; `NoFailSetPawnLocation` is bound to write the location, because the script falls back to it when `SetLocation` does nothing).

## Natives bound for this step (all `UNVERIFIED`, in `src/combat_down.cpp`, enabled with `enableDeaths`)

| Native | Stand-in | Why this much |
|---|---|---|
| `WillowPlayerController.GetInjuredDefinition` | the player class's `CharacterInjuredDefinition` (the bridge supplies it) | the note names it for Maya (NATIVE_DAMAGE_DEATH 10.1); without it every read of the definition lands on `None` |
| `WillowPawn.SetSecondWindReason` / `ClearSecondWindReason` | store and clear the `SecondWindReason` byte | the byte the `Tick` reads; nothing else |
| `WillowPlayerPawn`/`WillowPawn.GetRevivePct`, `SetRevivePct` | the replication info's `RevivePct` when the pawn has one | `Tick` calls it every frame; the bridge's pawn has no info of its own in the slice, so the call is dropped there |
| `Actor.SetCanTarget`, `CanTarget`, `GetCanTarget` | the `bCanTarget` flag | `StartInjuredDeathSequence` and the second wind call it |
| `Pawn.IsInjured` | the `bIsInjured` flag the state sets and clears | `ResurrectAtLocation` asks (reason 1 makes the test true anyway) |
| `WillowPlayerPawn.HasActiveDuel` | false | the VM has no versus duel; `Tick` asks every frame |
| `WillowPlayerController.NoFailSetPawnLocation` | writes the location, succeeds | the respawn's last resort |
| `WillowPlayerReplicationInfo.GetCurrencyOnHand`, `AddCurrencyOnHand` | the rules of NATIVE_CONTROLLER_HELPERS.md (type above 12 and the golden-key type ignored, the minimum 0, the caps), then `NotifyCurrencyDelegates` | the fee |
| the value resolver `CurrencyAttributeValueResolver` and the context resolver `ControllerAttributeContextResolver` (evaluator, `src/progression.*`) | the amount of the resolver's `FormOfCurrency` on the controller's replication info; a controller source resolves to itself | the cost formula (0.07 x credits) reads them; their native bodies are unread (NATIVE_ATTRIBUTES.md "Not read yet", medium for the controller one). The live fee rows match the arithmetic. |

For the run only (`Run`, `MissionScript`, `CombatSystem::preparePlayerPawn`): Maya's pawn gets the world (`WorldInfo`, authority), the controller's replication info and a `StartSpot`, which the slice's pawn does not
have and the slice never asks for; the `DigistructCoordinatedEffect` objects of the globals definition are built.

## Analyst-read requests (natives or facilities nobody has read; what the script expects)

1. `WillowPawn.SetSecondWindReason(Reason)`: does it raise health to 25 percent in the same frame (live kill frame)? What else does it do with the reason (a kill, a level-up, a partner revive, a resurrect skill, auto revive)?
2. `Pawn.IsDead`, `Pawn.IsInjured`, `WillowPlayerPawn.IsInjuredDead`: what they answer (the VM reads the flags for `IsInjured`; `IsDead` is still a stub that answers false).
3. `Actor.SetTimer` / `ClearTimer` (and the timer table): the state sets `CheckShouldBeAutoRevived` (every 2.0 s, only acts when `ShouldGetResurrected` is above 0), `PlaySpecialMoveOverride`, `DeRezGear` and `ShowRespawnDialog` through them. No timer facility was built; a minimal one is a per-object list of (name, time left, loop) fired by the driver's time step.
4. `SpecialMoveComponent.Play` (the callback name, the animation length it returns, and when the callback runs): `SwitchToSidearm` after 2.28 s and `InjuredDeathAnimDone` after 0.84 s are live inputs now.
5. `WillowPlayerPawn.EnableProtectionTimer`, `IsProtectionTimerActive`, `GetMinimumHealthMaintainedByProtectionTimer`, `CheckLowHealthState`: armed at the end of a second wind (a stub in the VM, so the protection stage of `AdjustDamage` stays not implemented).
6. The respawn's station choice: `TeleporterDestination.GetNextExitPoint`, `TravelStation.CanResurrectHere`, the holding cell, `WillowGameInfo.ResetAI`, `Pawn.SetAnchor`, `Actor.SetLocation` (the effects of a teleport). The caller that schedules `ShowRespawnDialog` 11.8 s after the respawn.
7. `CurrencyAttributeValueResolver` and `ControllerAttributeContextResolver` (above), `WillowGlobals.GetEffectCoordinator` (the digistruct effect of the respawn; a stub, so the effect is absent).
8. `BehaviorBase.RunBehaviors` for the injured definition's lists (`InjuredBehaviors`, `TargetedBehaviors`, `RecoveredBehaviors`, `ResurrectedBehaviors`, the five pool refills): the VM logs them as not implemented; the health after a respawn is the script's own `SetHealth`.

## Not done

Versus duels, hardcore permadeath, a revive by a partner, the auto revive, the Defense Boost after a second wind, the shield, the pre-death animation path, the injured bonus damage in the damage pipeline (the flag is set by `Tick`,
no pipeline stage reads it here), a fall into a kill volume (`bCausePlayerDeath`), vehicles, the level-travel respawn reasons, `OtherPlayerDiedCostMultiplier`.

## Reproduce

Steps added to `--combat-run` (`src/combat_run.cpp`; documented in the comment of `src/combat.hpp`): `cash:<amount>` and `bleed:<seconds>[:<dt>]`; `players:1` (existing) sets the game's player count that
`HandlePlayerDeathResurrection` tests. The six-injury run (ignored `local/p2/Sf/run_all.sh`, 111 s of wall time of which about 100 s are the code packages loading), after the S-a/S-d/S-e steps of `local/p2/Se/final_rows.sh`
(pawns 0 to 14), with `S` = `swing:15:melee` and `K` = 9999:

`maya:8 players:1 spawn:adult:7 spawn:baby:6 spawn:baby:6 tick:100 cash:2715 S S S S bleed:50 cash:2534 S S S S bleed:50 cash:2357 S S S S bleed:50 S S S S bleed:8 hit:16:K:GD_Impact.DamageType.DmgType_Normal:Shotgun bleed:1 S S bleed:8 hit:17:K:...:Shotgun bleed:1 S S bleed:30`

and the early run (`run_early.sh`: no `tick:100`, three babies, `bleed:4` before the third kill) for 12 / 8 / 4 / 1. Without a game install: `ctest` runs `combat-synthetic`, which checks the down natives on a toy pawn
(invented values) and that `bleed` and `cash` refuse a run without the mission bridge; `python tools/verify_combat_pawn.py --self-test` checks the new rows' PASS / FAIL / MISSING logic on invented inputs.

## The host's state: shield, health and the down state in the UE host (2026-11-03, maya lane)

AI-assisted (Claude). `host/ue5/OpenWillow/Source/OpenWillow/OpenWillowMayaState.{h,cpp}` holds Maya's survival state as plain C++ (no world):
the walker passes game time in and does the presentation (weapon put-away, Phaselock end, teleport, logs). The down rules that were in
`OpenWillowWalker.cpp` moved there unchanged; what is new is the shield and one damage entry point.

- **Stats in one struct.** `FOpenWillowMayaStats` holds every number the rules read (maximum health, health regeneration, shield capacity,
  recharge delay and rate, the bleed-out multiplier, the second-wind and respawn fractions). `FOpenWillowMayaState::Effective()` is the one
  place the rules read them from; the skills lane adds its modifiers there.
- **Shield first, then health.** A hit takes the shield first; the hit that empties it sends the rest to health in the same hit (live:
  COMBAT_GOLDEN_TRACE.md step 9, 31.616 = 23.387 + 8.229). The amount is the damage as it arrives at the pools: the stock pipeline's factors
  (the 0.715 to 0.741 seen live) are not applied here and are the caller's.
- **Recharge.** Every hit restarts the delay; after it the shield rises at the rate, up to the capacity (live: the shield rose 1.613-1.618 s
  after the last hit, the delay read 1.611 s plus one frame, at about 25.9 per second; step 10). Health does not regenerate (live rates 0).
  The numbers come from the equipped shield's card (`Capacity`, `Recharge Rate`, `Recharge Delay`; the last two labels are BL2's card labels
  and have not been seen in the local card data: UNVERIFIED); a card without them keeps the live read for the one shield measured
  (1.611 s, 25.88 per second) as a HOST STAND-IN.
- **Host rules, UNVERIFIED:** no recharge while down; the session's first shield starts full; a swapped shield starts empty and waits its
  delay; the second wind raises the shield to 90 % of its capacity (live: section D) and the respawn fills it.
- **`OpenWillowApplyDamageToPlayer(WorldContext, Amount, Causer, SourceLocation)`** (in `OpenWillowMayaState.h`) is the entry point for
  enemies and hazards; UE's `ApplyDamage` reaches the same path through the walker's `TakeDamage`.
- **Checks.** `OpenWillow.MayaState` (UE automation, `OpenWillowMayaStateTest.cpp`, invented numbers only) pins the arithmetic: shield then
  health, the delay and rate, the bleed-out cut for successive injuries, the second wind, the dead state, the respawn after the death camera,
  a revive on a refill, a swapped shield. The in-engine `-owhuddownshots` run and `tools/check_hud_down.py` keep checking the down timeline.

## Stand-in shield capacity (2026-11-06, maya lane)

AI-assisted (Claude). A first playtest logged capacity 0 on a bought shield: vendor and loot shields are stand-in gear with no stats, and the walker read capacity only from a card, so no shield obtained in normal play protected Maya.

- **Rule, UNVERIFIED:** a shield whose card has no "Capacity" stat takes `capacity x growth ^ (item level - anchor level)` when equipped; a card's own capacity wins. The form is the host's choice, copied from how item value grows, and nothing shows real capacity follows it. It is not derived from the shield's parts, so every shield of one level is the same.
- **Numbers:** ignored `local/slice/stand_in_shield.json` (`at_level`, `capacity`, `growth_per_level`), hand-kept like the twirl tint. The anchor is the shield value the real game's HUD showed for a level 8 Maya, whose shield's own level was not read. The growth is the price scaler. A planned bl-sdk read of capacity against level replaces them. Without the file a warning is logged and such a shield still protects nothing.
- **Checked:** the synthetic `OpenWillow.MayaState` covers parsing, growth, a card's capacity winning, and a stand-in shield through equip, damage and recharge. The file read and the walker's equip path have not been run by that test; a playthrough has to show them.
- **Not changed:** the card and backpack row still show no stats, and Maya starts with no shield until she gets one.

