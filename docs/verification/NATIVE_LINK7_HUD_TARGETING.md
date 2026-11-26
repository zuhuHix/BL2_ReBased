# Link 7's HUD pickup targeting: what it needs headlessly, and the shrink bookkeeping read (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK7-HUD-TARGETING (decision 2026-11-26-63), the analyst half of
the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/hud/`. This note
is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here is
UNVERIFIED in the running game.**

**Why.** On main 21a668a, link 7 (`pick_up_aimed_not_nearest`) stops at its `look` step (two of its four steps pass) with "the HUD
pickup targeting": the pass's first `LayerAbsent`, because the headless world has no HUD. The hub held that stop as
is. The question here is whether the HUD's answer can be decided headlessly in Off, as a pure query on actor state, or
truly needs the Flash/HUD layer.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`, and a local scan of every script function for the fields named):
  - `PlayerController.PostBeginPlay`, `SpawnDefaultHUD`, `ClientSetHUD`, and `GameInfo.GenericPlayerInitialization`;
  - `WillowPlayerController.ClientSetHUD` and `SwitchPlayerClass`;
  - `WillowHUD.PostBeginPlay`, `OpenHUDMovie`, `_OpenHUDMovie` and `CreateHUDMovie`;
  - `WillowPickup.PostBeginPlay`.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read:
  - `GetPlayerViewPoint`'s exec and its bodies on the `Controller`, `PlayerController` and `WillowPlayerController`
    tables;
  - the targeting pass's shrink step, found through the `BeginShrinking` name;
  - `PlayerController.IsSplitscreenPlayer`'s body.
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. Fields land exactly, except `WillowPickup`'s
  (16 bytes short, as NATIVE_PICKUP_TARGETING.md records) and one `WillowHUD` field the tool could not lay out (named
  by fit).
- **`ow-package --def-dump`:**
  - `Default__WillowHUD`, `Default__WillowPlayerController`, `Default__WillowPickup`;
  - `GD_HUD.HUDDefinition`;
  - the game infos' `HUDType`.
- **A local probe** (temporary code, reverted, never committed): installed Off strict `--slice-gate` runs on main
  21a668a, under the machine lock. In link 7's `look` it:
  - spawned the player's HUD through the installed `ClientSetHUD`;
  - set the HUD's data fields;
  - registered the two pickups;
  - ran the port's pass (`ow_inventory::pickup_targeting`) with a probe `Host`.

  Section 6 gives the sequence.

## 2. The answer: the pass is a pure query; its inputs are not

**The targeting pass itself needs no Flash.** NATIVE_PICKUP_TARGETING.md reads it as a native per-tick pass of the
`WillowHUD` actor. It reads:
- the HUD's own fields (`PlayerOwner`, `WPlayerOwner`, `HUDDef`, the cached pickups);
- the HUD definition's data (`PickupRange`);
- the globals' `PickupList` and each pickup's fields;
- three inputs from the world: the view point, a pickup's component bounds, and a line check.

The port already implements it that way (`pickup_targeting.rs`, a `Host` for the three inputs). The Flash movie is
never read by the targeting.

**What the headless world lacks is the inputs, each for its own reason:**
1. **the HUD actor and its data** (section 3): not Flash, but the HUD's set-up script reaches presentation natives;
2. **the pickups' registration** (section 4): a begin-play gap of the port's pickup world;
3. **the view point** (section 5): a camera layer, not Flash;
4. **the bounds and the line check:** the port's bounds rule answers the bounds, and the line check is collision;
5. **the shrink bookkeeping** (section 7): part of the pass, now read.

## 3. The HUD actor and its data

**How the game makes it:**
- `GameInfo.GenericPlayerInitialization`, and `WillowPlayerController.SwitchPlayerClass`, call the controller's
  `ClientSetHUD(GameInfo.HUDType)`. `WillowCoopGameInfo`'s `HUDType` is `WillowHUD`; plain `GameInfo`'s is `Engine.HUD`;
- `ClientSetHUD` spawns it, owned by the controller, into `myHUD`;
- its `PostBeginPlay` then runs:
  - `HUD.PostBeginPlay` sets `PlayerOwner` from the owner;
  - `WillowHUD.PostBeginPlay` runs `InitHud` and `LoadBuildInfo`, sets `WPlayerOwner` := `PlayerOwner` as a
    `WillowPlayerController`, and starts a 0.01 s timer to `OpenHUDMovie`;
- `OpenHUDMovie`, then `_OpenHUDMovie`, then `CreateHUDMovie`. **`HUDDef` is set only there:** `HUDDef` := the globals
  definition's `HUDDefinition`, unless a `HUDMovie` already exists. Then the Flash movie is made and started.

So `HUDDef` is a data read that happens to sit beside the movie's creation.

**What the probe found:**
- `ClientSetHUD` spawns the `WillowHUD` (owner the controller). The pickup world has not begun play (section 4), so
  `PostBeginPlay` does not run and `PlayerOwner` stays None. The pass then ends at once ("Idle": no owner);
- run by hand, `PostBeginPlay` sets `PlayerOwner`, then stops in `InitHud` at an unbound
  `WillowPlayerController.IsBinkMoviePlaying(string)`, a movie-player query, before it reaches `WPlayerOwner`;
- with `WPlayerOwner` and `HUDDef` (`GD_HUD.HUDDefinition`, `PickupRange` 400) set directly, the pass goes on to the
  view point's `LayerAbsent`.

**Hub ruling (review of #806): yes, a harness HUD, data steps only** (Off only, logged as a harness input,
UNVERIFIED, Reference unchanged). No movie native is called:
- **when:** the harness HUD is made **before the world begins play** (section 4), so the port's spawn runs no
  begin-play event on it (review r1 of #806). Why this and not a spawn without events: the port's `begin_play` runs
  no pass over actors that already exist (`ow_world::actors`: the game's later begin-play pass is not read), so a HUD
  made first never meets `WillowHUD.PostBeginPlay` (`InitHud`, the unbound `IsBinkMoviePlaying`, the `OpenHUDMovie`
  timer), and no new spawn variant is needed. Made after play began, it would run them. If a later lane ports the
  begin-play pass over existing actors, this choice must be revisited (test 1 guards it);
- **the class:** `HUDType` of the world's game (`WorldInfo.Game`). `Game` None: `NotRead`, "the harness HUD: the
  world has no game". A `HUDType` that is None or not `WillowGame.WillowHUD` or a subclass: `NotRead`, "the harness
  HUD: HUDType is not a WillowHUD";
- spawn that class owned by the controller into `myHUD`, as `ClientSetHUD` does;
- **a second set-up** (`myHUD` already set): `NotRead`, "the harness HUD: a second set-up", before any write. The
  game's `PlayerController.ClientSetHUD` destroys the old HUD first and then spawns the new one; the old HUD's
  destroy events were not read, so the harness does not run them;
- set `PlayerOwner` := the controller (HUD's `PostBeginPlay`), `WPlayerOwner` := the controller (`WillowHUD`'s), and
  `HUDDef` := the globals definition's `HUDDefinition` (`CreateHUDMovie`'s first step);
- skip the presentation steps: `InitHud`, `LoadBuildInfo`, the timer and the movies, and the material instances;
- refusals before any write:
  - the class missing: `NotRead`;
  - the definition not resident: `NotRead`;
  - each field declared, else `VmError::Script`.

## 4. The pickups' registration: `WillowPickup.PostBeginPlay`

`WillowPickup.PostBeginPlay` calls `WillowGlobals.AddPickup(Self)`; `AddPickup` is bound in Off
(NATIVE_WILLOW_GLOBALS.md). But the port's spawn sends `PreBeginPlay` and `PostBeginPlay` only once the world has
begun play (`ow_world::actors`, "after play began"). Only the map bring-up begins play, and the pickup world (links 6
and 7) never does. So neither pickup is in `PickupList`, the pass has no candidate, and it answers "no winner" (the
probe).

**Hub ruling (review of #806): yes, begun play or the equivalent registration** (Off only, logged as a harness
input, UNVERIFIED, Reference unchanged). This note's choice is **the pickup world begins play at set-up**, so each
spawn runs its begin-play events as in the game, where the level has begun play before anything drops. **The order
at set-up** (review r1 of #806): the harness HUD first (section 3), then the world begins play, then the pickups and
the gun are spawned.
- this changes link 6's path too: its pickup and gun would run their `PostBeginPlay`. The implementer re-runs link 6
  under the setting;
- the narrower alternative, registering the two pickups with `AddPickup` at the drop, leaves the rest of
  `PostBeginPlay` out and is a stand-in of its own. It is the fallback if begun play breaks link 6;
- **how to confirm link 6 keeps passing** (the hub's condition):
  - an installed Off strict `--slice-gate`, under the machine lock, with the setting on: link 6 ok, steps 3/3, the
    same as without it (main eef4f83: ok, 3/3); the overall count must not drop;
  - in that run's call trace, each of link 6's spawns (the pickup and its gun) runs `PreBeginPlay` and
    `PostBeginPlay` once, and the pickup is in `PickupList` once;
  - **the spawn's base-search refusal** (review r1 of #806): once play has begun, the port's spawn refuses a template
    that needs a base search (`Base` None, `bCollideWorld`, `bShouldBaseAtStartup`, `Physics` `PHYS_None` or
    `PHYS_Rotating`) with the collision `NotYet` "the base search after PostBeginPlay". Before the run, check each
    template links 6 and 7 spawn after begun play (the pickups, the gun, the HUD is made before) against that test;
    a template that needs it gives that refusal under the setting, and that is a stop to report;
  - the synthetic tests in section 9, row 2;
  - if link 6 regresses, report the new stop and fall back to the `AddPickup` registration.

## 5. The view point: `GetPlayerViewPoint`

The exec calls the controller's class-table virtual:
- **`Controller`:** the controller's own `Location` and `Rotation`;
- **`PlayerController`:**
  1. with no `PlayerCamera` and a `CameraClass`: it **spawns the camera** of that class, owned by itself, and
     initialises it for itself, lazily, on the first call;
  2. with a camera: the camera's cached point of view;
  3. else the view target's `Location` and `Rotation`, else its own;
- **`WillowPlayerController`** (`CameraClass` `WillowGame.WillowPlayerCamera`):
  - a per-frame cache: the same view when neither the pawn's place nor the time changed;
  - otherwise the `PlayerController` path, or the view target's camera calculation when there is no pawn;
  - then a further adjustment virtual, and the controller's own rotation offsets added.

**So the view needs a camera layer:** the `WillowPlayerCamera` actor and its per-tick point of view (its camera modes),
plus the controller's offsets and time. It is not Flash, and it is not a pure read of actor state.

**Hub ruling (review of #806): yes, a view stand-in at the pawn's eye point** (Off only, logged as a harness input,
UNVERIFIED, Reference unchanged):
- the point: the pawn's `Location` plus (0, 0, `BaseEyeHeight`); here (0, 0, 75). The reading gives nothing more
  exact headlessly: the camera's own point of view (its modes) is not read;
- the rotation: the controller's `Rotation` (zero here). The stand-in **drops `WillowPlayerController`'s own rotation
  offsets** and its further adjustment virtual (section 5 above; review r1 of #806);
- with no pawn: the camera `LayerAbsent` stays;
- the first-person view by fit. The probe used it.
- Declared, else `VmError::Script`: `Pawn`, `Location`, `BaseEyeHeight` (a float), the controller's `Rotation`.

## 6. The probe's sequence, with every input stood in

In link 7's `look`, on main 21a668a:

| Step | Outcome |
|---|---|
| no HUD | `LayerAbsent` (main's stop) |
| HUD spawned by `ClientSetHUD`, `PostBeginPlay` not run | the pass ends: "Idle" |
| `WPlayerOwner` and `HUDDef` set | the view's `LayerAbsent` |
| the eye-point view | "no winner": `PickupList` is empty (section 4) |
| both pickups added with `AddPickup` | the aimed gun is scored, then the shrink `NotRead` (section 7) |

Details of the last two rows:
- **the aimed gun is scored.** Its bounds come from the port's bounding cylinder: half height 32.7, so the box height
  is 65.4. It lies 150 uu out, beyond a quarter of `PickupRange` (100), so the line check runs; in the probe it was
  answered clear;
- **then the pass stops** at its shrink step's `NotRead` (open question 3 of NATIVE_PICKUP_TARGETING.md), read in
  section 7;
- **with section 7's rule** (nothing to shrink), the pass answers **`Hover` with the aimed gun, reported**:
  `SawPickupable` sets Maya's `CurrentSeenPickupable` to the aimed gun. **The `look` step passes.**

On 21a668a, link 7 then reached `key` and stopped at `Actor.SetBase`'s "the class's own implementation is not
resolved" `NotRead`. **Rerun on main eef4f83**, which binds NATIVE_ACTOR_SETBASE.md's rule for its seven classes
(#800), with the same stand-ins: **link 7 passes, all four steps** (`make`, `touch`, `look`, `key`; result ok), and
link 6 stays ok. In that run the pickups were registered with `AddPickup` at the look, not by begun play, so begun
play's effect on link 6 (section 4) was not measured.

## 7. The shrink bookkeeping (NATIVE_PICKUP_TARGETING.md section 8, open question 3)

- **When it is enabled:**
  - the HUD has a `WorldInfo`;
  - the world's `NetMode` is not the client's (3);
  - and **the owner is not a split-screen player:** the controller's `IsSplitscreenPlayer` (its exec reaches the same
    body) answers no.

  That test reads the controller's `Player`:
  - with none: no;
  - a local player: yes when the engine holds more than one local player;
  - a child network connection: yes.
- **The duration:** the `WillowPickup` class default's `PickupShrinkDuration`, 15 in the installed data.
- **The count:** each counted pickup (eligible, as the pass's candidates) whose `LifeSpan` is above the duration counts.
  The one with the smallest such `LifeSpan` is tracked.
- **The event:** when the count is **above the HUD's `MaxNumPickups`** (by fit: the one field the layout tool could not
  place; class default 200), `BeginShrinking` is raised on the tracked pickup. Otherwise nothing happens.

**Proposed Off answer** (replacing the pass's shrink `NotRead`):
- that rule;
- the split-screen test answers no for a `Player` of None, the headless case (the pickup worlds). **The `--map`
  route sets `Player`** (`ow_world` `mapload/player.rs`, the login step), so there the test reads it (review r1 of
  #806): a local player answers yes when more than one local player exists, and `ActorWalk::LocalPlayers` (the local
  players' controllers) can give that count; a child network connection answers yes; any other `Player` object is
  `NotRead`. Without that count wired, a set `Player` stays `NotRead`;
- `BeginShrinking` is a script event on the pickup, planned like the pass's other events: resolved and checked in the
  plan, raised after the writes;
- declared, else `VmError::Script`: `MaxNumPickups` (an int), `PickupShrinkDuration` and `LifeSpan` (floats),
  `NetMode` (a byte), `Player` (an object).

**On the slice:** two pickups at most, far below 200, so nothing happens.

## 8. Predicted stop

- **Today** (main): the no-HUD `LayerAbsent`.
- **The line check under `NoLevel`. Hub ruling (review r1 of #806)** (Off only, logged as a harness input, UNVERIFIED,
  Reference unchanged):
  - the setting: `pickup_targeting::NoLevelLineCheck`, turned on by `enable_no_level_line_check` (a registration
    error outside Off or twice), which the slice's two `NoLevel` worlds turn on, as they do `NoLevelBaseSearch`. Its
    set-up log line: "harness input: the world turns on the pickup targeting's no-hit line check for a world with no
    level geometry (the hub's ruling on review of #806; UNVERIFIED)";
  - with it on and a `NoLevel` service, the line check answers **no hit**. Only three actors are passed over: the
    owner's pawn, the target pickup `W`, and `W`'s `Base`;
  - **any other actor holding collision** (a `CollisionComponent` set) is `NotRead`, "the pickup targeting's line
    check: another actor's collision", decided before any write, unless geometry clears it: the line's own box (the
    segment's two ends) is disjoint from the actor's bounds box (the bounding-cylinder rule). That is the bounds step
    every walker runs first. It is the form of the combat world's geometric clearance (#807) that fits a slanted line;
    #807's exact test assumes a vertical line and does not apply as is;
  - the per-call log line: "harness input: the pickup targeting's line check hit nothing (a world with no level
    geometry; the owner's pawn, the target pickup and its Base passed over; UNVERIFIED stand-in, the hub's ruling on
    review of #806)";
  - so the aimed gun's line is clear, as in the probe;
  - **read since** (NATIVE_LINK7_LINE_CHECK.md): the game's check never tests pawns or pickups (their owner filters
    reject them with its flags), so the three-actor list can become that rule (its open question 2).
- **With the four rulings in** (a harness HUD, begun play, the eye-point view, the line check), the bounds from the
  port's rule, and section 7's rule: **`look` passes, and on main eef4f83 (SetBase bound) link 7 passes** (the
  probe). No further stop on link 7's path.
- **Begun play's effect on link 7 is not measured either** (review r1 of #806): that probe registered the pickups
  with `AddPickup` at the look, so neither link 6 nor link 7 ran with the world begun. Section 4's confirmation plan
  covers both links.

## 9. Tests (synthetic fixtures)

1. **The harness HUD:**
   - after set-up: `myHUD` is a toy `WillowHUD`, its `PlayerOwner` and `WPlayerOwner` the controller, its `HUDDef` the
     globals definition's, and no movie made;
   - an unloaded definition: `NotRead` before any write.
   - (hub ruling) with the setting off: the no-HUD `LayerAbsent`; Reference with it on: unchanged; one harness-input
     log line;
   - made before begun play: no `PostBeginPlay` runs on it (no `InitHud`, no timer); then the world begins play and
     still none runs;
   - `Game` None; a `HUDType` that is not a `WillowHUD`: each `NotRead`, nothing written;
   - a second set-up with `myHUD` set: `NotRead`, the first HUD kept, nothing written.
2. **Begun play:** a toy `WillowPickup` spawned in a world that has begun play is in `PickupList`; one spawned before is
   not.
   - (hub ruling) with the setting on, a toy pickup world: begins play at set-up; a toy pickup and a toy gun spawned
     after it each run `PreBeginPlay` and `PostBeginPlay` once, and the pickup is in `PickupList` once;
   - with it off: neither event, `PickupList` empty; Reference: unchanged;
   - a toy template that needs a base search, spawned after begun play: the collision `NotYet` "the base search after
     PostBeginPlay", nothing made.
3. **The view stand-in:** a toy pawn with `BaseEyeHeight` 64 and a controller rotation give that eye point and rotation;
   with the setting off, the camera `LayerAbsent`.
   - (hub ruling) a controller with no pawn: the camera `LayerAbsent`; a mistyped `BaseEyeHeight`: `VmError::Script`;
     one harness-input log line; Reference: unchanged.
4. **(hub ruling) The line check under `NoLevel`:** with the setting on, a toy pickup beyond a quarter of
   `PickupRange` is clear and can win, with the per-call log line; with it off, the line check's own refusal;
   Reference: unchanged;
   - the owner's pawn, `W` and `W`'s `Base` each holding collision on the line: still no hit;
   - a fourth actor holding collision whose bounds meet the line's box: `NotRead`, nothing written; the same actor
     with bounds disjoint from it: no hit;
   - the setting enabled twice, or outside Off: a registration error.
5. **The shrink step:**
   - (review r1 of #806) a controller whose `Player` is a toy local player, one local player: not split-screen;
     two: split-screen, nothing counted; a `Player` of another class: `NotRead`;
   - three toy pickups with `LifeSpan` 20, 18 and 5, duration 15, `MaxNumPickups` 1: `BeginShrinking` on the 18;
   - with `MaxNumPickups` 2: nothing;
   - a client world, or a split-screen owner: nothing counted.
6. **Link 7's look on a toy fixture,** with all of the above: the aimed pickup wins and is reported.
7. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **By fit:**
   - `MaxNumPickups`' place;
   - the eye-point view as the first-person camera's answer.
3. **Not read:** the camera's per-tick point of view (`WillowPlayerCamera`).
4. **Ruled by the hub on #806** (all four yes, Off only, logged, UNVERIFIED, Reference unchanged):
   - the harness HUD (section 3);
   - begun play or registration (section 4), with link 6 confirmed as section 4 says;
   - the eye-point view (section 5);
   - the line check under `NoLevel` hits nothing (section 8).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After spawn, read `myHUD`'s class, `PlayerOwner`, `WPlayerOwner`, `HUDDef` and `MaxNumPickups`.
2. Read the controller's `GetPlayerViewPoint` against the pawn's `Location` + `BaseEyeHeight` in first person.
3. Drop more than `MaxNumPickups` pickups and watch which one starts shrinking.
