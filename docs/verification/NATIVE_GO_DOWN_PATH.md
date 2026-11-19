# Look-ahead: what slice links 13 and 14 reach after maya:8 (go down, bleed out, respawn) (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane MAIN-READING + LOOKAHEAD-13-14 (decision 2026-11-19-NNN, number to
be given by the hub).
- **Method.** The natives were listed from the installed path itself: the combat driver's steps of links 1, 4, 13
  and 14 (`maya:8 players:1 spawn:adult:7 swing:0:melee`, then `cash:3000` and three more swings, then `bleed:50`)
  were run in `ParityMode::Reference` on main 0235937, whose stub policy logs and counts every unbound native. The
  counts were diffed step by step, so each native below is attributed to the link that first reaches it.
- **The natives** with no note yet were then read from the game executable with Ghidra (`tools/ghidra/run.ps1`, two
  headless runs under the machine lock). Field names come from the cooked packages (`tools/ghidra/class_layout.py`).
- **Own words only:** no decompiler output, pseudo-code, addresses, offsets or listing structure is reproduced here.
  Raw output stays under ignored `local/analysis/godown/`.
- **No live session** was run. **Every rule is UNVERIFIED in the running game.**

**Clean room.** This lane implements nothing. Every native here stays unbound in Off until a fresh implementer codes it
from this note (or the note it points to) alone.

**Why it matters.** Links 13 (`go_down`) and 14 (`bleed_out_respawn`) are blocked by link 4, which is blocked by
link 1, which stops at `maya:8`. Lane H6 is clearing that stop. Once it is cleared, these are the natives the two
links meet.

## 1. What the list is, and what it is not

- **A lower bound.** Reference answers every unbound native with a zero result. A zero can send the script past a
  branch that a real answer would take, so in Off a link can reach natives that are not listed here. Examples: the
  controller's skill manager is None in Reference, and the respawn has no travel station, so it falls back to
  `StartSpot` (COMBAT_DOWN_RESPAWN.md, verdict 4).
- **An unbound native in Off is an error, not a `NotYet`.** The combat run's Off stub policy is `Refuse`: a native that
  nothing binds stops the step with `UnboundNative`, which the gate grades `error`. Links 6 and 7 on main show it
  (section 6). So each native below needs an Off answer: a rule, or a named `NotYet`.
- **Who adds the prefix.** Every native below that answers `NotRead` answers it at entry, before any write of its
  own, as a plain `NotYet`. The native does not add "stopped after partial writes" itself: the layer that has
  written adds it to an escaping `NotYet`, as the combat run, the slice gate and the attribute evaluator already do
  (review r1 of #489).
- **"Bound" means a binding string exists in `crates/`.** Some bindings are Off only (`Pawn.IsDead`), and a few may be
  Reference only. That was not separated here.
- **The script order** of the down path is COMBAT_DOWN_RESPAWN.md's "What a run does": `TakeDamage` -> the
  pawn's health-depleted handling -> `PlayInjured` -> `GotoState(injured)` and its `BeginState`; then `Tick` times the
  bleed-out; then `SetInjuredDeadState`, `StartInjuredDeathSequence`, `InjuredDeathAnimDone`, the death camera,
  `EndInjuredDeadCamera` -> `InjuredRespawn` -> `HandlePlayerDeathResurrection` -> `ResurrectPlayer` ->
  `ResurrectAtLocation` -> `GoFromInjuredToHealthy`; then the fee through `ShowRespawnDialog`. Which script function
  calls each native below was not traced one by one; the link and phase are what the stub diff shows.

## 2. Link 13 (`go_down`): the three swings that take Maya down

The three swings run the same damage path as link 4's one swing. **23 natives** that link 13 reaches are already on
link 4's path, so link 4 meets them first. They are listed in section 5 and are not read here.

Reached only once Maya goes down (the injured entry):

| Native | Bound? | Record | Off answer |
|---|---|---|---|
| `AttributeDefinitionBase.ResolveContext` | yes | NATIVE_ATTRIBUTES.md | as bound |
| `BehaviorBase.RunBehaviors` (outside the level-up stand-in) | yes | NATIVE_DOWN_TIMERS.md | as bound; the injured definition's behaviour lists |
| `Pawn.IsDead` | Off only | NATIVE_DOWN_TIMERS.md | as bound |
| `PlayerController.IsLocalPlayerController` | yes | | as bound |
| `WillowAIPawn.GetExpLevel` | yes | | as bound |
| `WillowDialogGlobalsDefinition.Get` | yes | NATIVE_DIALOG.md | as bound |
| `WillowPlayerController.GetInventoryPawn` | yes | | as bound |
| `Pawn.IsLocallyControlled` | no | this note, 3.1 | the rule of 3.1 |
| `PlayerController.IsLocalController` | no | this note, 3.2 | `NotRead` |
| `StatusEffectsComponent.ClearAllStatusEffects` | no | this note, 3.3 | `NotRead` |
| `WillowAIPawn.GetParent` | no | this note, 3.2 | `NotRead` |
| `WillowPawn.GetTechDeathIndexForDamageType` | no | this note, 3.4 | the rule of 3.4, `NotRead` for the unread test |
| `WillowPlayerController.GetSkillManager` | no | this note, 3.5 | the rule of 3.5 |
| `WillowPlayerPawn.SetDefaultStance` | no | this note, 3.2 | `NotRead` |

`GetTechDeathIndexForDamageType` and `WillowAIPawn.GetParent` run on the enemy side of the swing: the pawn the script
asks about is the adult, not Maya. The injured entry is the rest.

## 3. What was read

### 3.1 `Pawn.IsLocallyControlled()`

- The pawn's controller is its own `Controller`, or, when that is None, its `DrivenVehicle`'s `Controller`.
- With no controller, the answer is false.
- When the world is not a networked one, the answer is true. The test reads a byte of the actor's world that fits the
  stock net-mode field; the layout tool cannot lay out `WorldInfo`, so the field's identity is **not proven**.
- Otherwise the answer is true for a controller that is not a player controller, and the controller's own
  local-player answer for one that is. Both are calls on the controller's class that were not resolved (3.2).

**Off answer.** The port's worlds are standalone. So: false with no controller (own or vehicle driver's), else true.
That the headless world counts as standalone, and that the byte is the net mode, are **Off assumptions** (UNVERIFIED).
A world that models a network game answers `NotRead`, "Pawn.IsLocallyControlled on a networked world". The
networked-world test reuses the port's existing `WorldInfo.NetMode` read (the behavior kernel's `CanPlayBehaviorData`
in `ow-behavior`'s `kernel/run.rs`, `NetMode` is not `NM_Client`), not a new reading of the byte.

### 3.2 The natives that hand straight to an overridable implementation

Each of these unpacks its arguments and then calls the receiver's own overridable implementation. The project's
class-table lookup finds no constructor pattern for these classes, so **none of the implementations was resolved**.
This is the same limit NATIVE_CACHE_WEAPON_CARD.md (#484) met for `WillowWeapon`.

- `Controller.IsLocalController` (also reached as `PlayerController.IsLocalController`). `PlayerController`
  declares its own `IsLocalController`, so `Engine.PlayerController.IsLocalController` is a separate native key with
  the same body. The behavior kernel's `READ_ONLY_QUERIES` allow-lists only the `Controller` key. Binding one key does
  not bind the other: a lane that binds them binds both, and adds the `PlayerController` key to that allow-list if a
  preflight is to call it. Until then both answer `NotRead`, and the kernel's `PlayLocal` stays `NotYet`;
- `WillowPawn.GetParent` (also reached as `WillowAIPawn.GetParent`);
- `WillowPawn.SetDefaultStance` (also reached as `WillowPlayerPawn.SetDefaultStance` and `WillowAIPawn`'s);
- `Actor.SetHidden(bool)`, `Actor.SetHardAttach(bool)` (its omitted argument defaults to the current `bHardAttach`)
  and `Actor.SetBase(Base, FloorNormal, Bone, AttachName)` (an omitted normal defaults to straight up);
- `Pawn.TermRagdoll()`;
- `WillowPlayerPawn.InitRagdoll()`. `WillowPlayerPawn` declares its own `InitRagdoll`, so this is a separate native key
  from `Engine.Pawn.InitRagdoll`, with the same body. That body only calls the pawn class's own implementation, which
  was not resolved, so whether the player pawn's ragdoll differs is not known. `ow-combat`'s `death.rs` already
  binds `Engine.Pawn.InitRagdoll` to true, a presentation answer of the death chain (COMBAT_DEATH.md: the host builds
  the ragdoll). **Off answer for the `WillowPlayerPawn` key:** the same presentation answer, true, with no write, under
  that record's terms (a stand-in, UNVERIFIED). Its result is the script's only use of the call on this path. It is
  not a reading of the implementation;
- `WillowPlayerController.PlayCameraAnim(...)`, `StopCameraAnim(bool)` and `UpdateInputContext()`;
- `WillowPlayerPawn.MarkDLEsForUpdate()`;
- `WillowPawn.RefreshPawnGearLikenessComponents()`, which calls three such implementations in turn.

**Off answer:** `NotYet`, kind `NotRead`, naming "<Class>.<Function>: the class's own implementation is not
resolved", record this note: a plain `NotRead` at entry; the caller layer that wrote adds the "stopped after partial
writes" prefix (section 1). `WillowPlayerPawn.InitRagdoll` is the exception above. Reading them
needs a way to resolve a class's implementation of an overridable native, or a live trace.

### 3.3 `StatusEffectsComponent.ClearAllStatusEffects(bool)` (outline, medium-low)

In outline it:
1. walks the component's `ActiveEmitters`, entry by entry, and lets each hit-region emitter go. The flag picks
   between two ways of doing it;
2. builds a list from `ActiveEffectSounds` and calls a per-kind removal (an overridable implementation, not
   resolved) for each, passing the flag;
3. finds the owner pawn's controller and, for a Willow player controller, its skill manager (3.5);
4. walks a further list of the component, resets a seven-entry per-kind table to zero, empties `ActiveEffectSounds`,
   sets a flag on the component and calls another object the component holds.

Steps 2 to 4 beyond the outline (the removal, what the skill manager is used for, the further list, the flag and the
last call) were **not read**. So the native writes
script-visible state in several places through code that was not read.

**Off answer:** `NotRead`, "StatusEffectsComponent.ClearAllStatusEffects: the per-kind removal and the skill manager
step", record this note: a plain `NotRead` at entry; the caller layer that wrote adds the "stopped after partial writes"
prefix (section 1).

### 3.4 `WillowPawn.GetTechDeathIndexForDamageType(DamageType)` (partial, medium)

It answers an index into the pawn's `MyDeathDef.TechDeaths`, or -1.
1. It answers -1 when the pawn is `bGibbed`, when it has no `MyDeathDef`, or when it has no `StatusEffectComp`.
2. It answers -1 for a None damage type, and for a damage type that is not of one Willow class. That the class is
   `WillowDamageTypeDefinition` is by fit: the byte it reads next lands on that class's `DamageType`.
3. It reads the damage type's `DamageType` byte. For four of its values (1, 2, 4 and 7) it runs a test (below). For any
   other value, or when the test is false, it answers -1.
4. Otherwise it walks `MyDeathDef.TechDeaths` in order and answers the index of the first entry whose
   `DamageTypeTriggers` holds this damage type. With none, -1.

**The test of step 3 was not read.** Each of the four values maps to one of four status-effect kinds, and the test
asks something about that kind (most likely whether the pawn is under that effect).

**Off answer.** Steps 1, 2 and 4 are pure reads and can be implemented. The answer is -1 whenever steps 1 to 3 say so
without the test. When the test would decide the answer, it is `NotRead`, "GetTechDeathIndexForDamageType: the
status-effect test of the damage type's kind": a plain `NotRead` at entry; the caller layer that wrote adds the "stopped
after partial writes" prefix (section 1). Writes: none.

### 3.5 `WillowPlayerController.GetSkillManager()`

It answers the game info's skill manager: the controller's world's game info, when that is a `WillowGameInfo`, asked
through its own `GetSkillManager`. With no such game info it answers None. It writes nothing.

**Off answer:** the same, delegating to the bound `WillowGameInfo.GetSkillManager` (NATIVE_SKILL_MANAGER.md). The
field that holds the world's game info was read through `WorldInfo`, which the layout tool cannot lay out, so that it
is `WorldInfo.Game` is an **Off assumption** (UNVERIFIED).

## 4. Link 14 (`bleed_out_respawn`): bleed-out, death and respawn

| Native | Bound? | Record | Off answer |
|---|---|---|---|
| `Actor.Trace` (349 calls) | no | none | `NotYet`, kind `LayerAbsent`, "Actor.Trace: the world's collision", as the spot search's refusal on main (4.5) |
| `BehaviorBase.RunBehaviors` | yes | NATIVE_DOWN_TIMERS.md | as bound |
| `PlayerController.IsLocalPlayerController`, `WillowDialogGlobalsDefinition.Get`, `WillowGameInfo.GetSkillManager` | yes | | as bound |
| `Actor.SetCollision(bool, bool, bool)` | no | this note, 4.1 | `NotRead` (4.1) |
| `Actor.SetCollisionSize(float, float)` | no | this note, 4.1 | `NotRead` (4.1) |
| `Actor.SetBase`, `SetHardAttach`, `SetHidden` | no | 3.2 | `NotRead` |
| `Actor.SetLocation`, `Actor.SetRotation` | no | NATIVE_RESPAWN_STATIONS.md (not read) | `NotRead` |
| `Controller.IsUsingVehicleEx` | no | NATIVE_RESPAWN_STATIONS.md 3.1 | that note's |
| `Controller.IsUsingVehicle(bool)` | no | none (not read here; that note covers only the `Ex` form) | `NotRead`, "Controller.IsUsingVehicle" |
| `Engine.GetCurrentWorldInfo()` | no | this note, 4.2 | the world's `WorldInfo` |
| `Engine.IsSplitScreen()` | no | this note, 4.2 | false |
| `GearboxDialogComponent.StopTalking` | no | NATIVE_DIALOG.md | that note's |
| `InteractionProxy.IsInteractionDebugEnabled()` | no | this note, 4.2 | false |
| `Pawn.IsLocallyControlled` | no | 3.1 | 3.1 |
| `Pawn.SetAnchor`, `WillowGameInfo.ResetAI` | no | NATIVE_DOWN_TIMERS.md | that note's (no rule read for `ResetAI`'s entries: `NotRead`) |
| `Pawn.TermRagdoll` | no | 3.2 | `NotRead` |
| `WillowPlayerPawn.InitRagdoll` | no (a separate key from the bound `Engine.Pawn.InitRagdoll`) | 3.2, COMBAT_DEATH.md | true, the death chain's presentation answer (3.2) |
| `WillowDialogGlobalsDefinition.StaticTriggerTemplateEvent` | no | NATIVE_DIALOG.md | that note's |
| `WillowGlobals.GetCustomizationManager()` | no | this note, 4.2 | the field |
| `WillowGlobals.GetEffectCoordinator()` | no | NATIVE_RESPAWN_STATIONS.md 3.2 | that note's |
| `WillowGlobals.GetVehicleLifetimeManager()` | no | this note, 4.2 | the field, or `NotRead` |
| `WillowLeviathanService.RecordCharacterKilledEventForPlayer(Player, Byte)` | no | this note, 4.3 | `NotRead` |
| `WillowLeviathanService.RecordCharacterRevivedEventForPlayer` | no | NATIVE_RESPAWN_STATIONS.md 3.5 | that note's |
| `WillowPawn.RefreshPawnGearLikenessComponents` | no | 3.2 | `NotRead` |
| `WillowPlayerController.PlayCameraAnim`, `StopCameraAnim`, `UpdateInputContext` | no | 3.2 | `NotRead` |
| `WillowPlayerPawn.MarkDLEsForUpdate` | no | 3.2 | `NotRead` |
| `WillowPlayerPawn.SetDefaultStance` | no | 3.2 | `NotRead` |
| `WillowPlayerPawn.SetShadowCasting(bool)` | no | this note, 4.4 | the rule of 4.4 |
| a native iterator (`foreach` over an engine iterator) | no | none | see 4.5 |

### 4.1 `Actor.SetCollision` and `Actor.SetCollisionSize` (outline)

- **`SetCollision(bCollideActors, bBlockActors, bIgnoreEncroachers)`.** Each omitted argument defaults to the actor's
  current flag. When nothing changes, it does nothing. Otherwise, when collision with actors is being turned off, it
  first ends every touch in `Touching`. When `bCollideActors` changes, it walks the actor's `Components` and updates
  each primitive. The rest, including where and when the three flags are stored, was **not read**. The touch and
  component steps belong to the world's collision hash, which the VM world does not model.
- **`SetCollisionSize(Radius, Height)`.** When the actor's `CollisionComponent` is a cylinder, its radius and height
  are set. Two further steps follow that were not read, and the actor's `bNetDirty` is then set.

**Off answer:** `NotRead`, naming "Actor.SetCollision: the collision hash and touch updates" and
"Actor.SetCollisionSize: the component's world update", each a plain `NotRead` at entry; the caller layer that wrote
adds the "stopped after partial writes" prefix (section 1). The flag and cylinder writes
alone would leave a collision state that no collision layer saw. Once the world models collision, these are a read of
their own.

### 4.2 Small getters

- **`Engine.GetCurrentWorldInfo()`** answers the current world's `WorldInfo` (the lookup through the world's levels
  was not followed in full). Off: the world's
  `WorldInfo`. A world without one answers *layer absent*, "the world's WorldInfo".
- **`Engine.IsSplitScreen()`** answers true when the engine object's `GamePlayers` holds more than one local player
  (the engine object is named by fit: the field lands on `Engine.GamePlayers`). Off:
  false, since the headless world has at most one player. A world that models more answers by the same rule.
- **`InteractionProxy.IsInteractionDebugEnabled()`** answers a process-wide debug flag. It starts false, and the only
  code that writes it flips it (a toggle, reached from one caller that was not read). Off: false. The toggle is not
  ported.
- **`WillowGlobals.GetCustomizationManager()`** answers the globals' `TheCustomizationManager` as it stands. It writes
  nothing. Off: the field. None in the supported headless fixture.
- **`WillowGlobals.GetVehicleLifetimeManager()`** answers the globals' `TheVehicleLifetimeManager`. When the field is
  None, a global is set and a world mode byte is not 3 (by fit, the world is not a network client), it makes a manager
  first (a write), in code that was not read further. Off: the field when it is set; `NotRead`, "the vehicle
  lifetime manager's lazy creation", when it is None.

### 4.3 `WillowLeviathanService.RecordCharacterKilledEventForPlayer(Player, Byte)`

It unpacks a player and a byte and hands them to a statistics routine that builds an event record. Only its start was
followed, and it was not read to the end. Its sibling `RecordCharacterRevivedEventForPlayer` is
NATIVE_RESPAWN_STATIONS.md 3.5. Off: `NotRead`, "the killed-event statistics", a plain `NotRead` at entry; the caller
layer that wrote adds the "stopped after partial writes" prefix (section 1). If a
later reading shows it touches only telemetry, that note's revived-event answer is the model.

### 4.4 `WillowPlayerPawn.SetShadowCasting(bool)`

For each entry of the pawn's `InstanceDataState.Data` that names a body part (`BodyCompositionIndex` set) and whose
`ComponentData` resolves to a component, it sets two of that component's rendering flags to the argument and then
refreshes the component. The two flags fit the shadow-casting flags, but the field mapping does not land on the
declared `CastShadow` and `bCastDynamicShadow` exactly, so their identity is **not proven**. Writes: those flags only.

**Off answer:** with no entry to visit (the headless pawn has no instance data), it does nothing. An entry that would
be visited answers `NotRead`, "SetShadowCasting: which component flags it sets".

### 4.5 `Actor.Trace` and the native iterators (not read)

- **`Actor.Trace`.** No crate binds it: the only mention is a test that expects the unimplemented stub in Reference.
  Its 349 Reference calls on link 14 were stub calls, most likely the death camera's per-frame view traces (not
  traced to the caller). It was not read here. **Off answer:** `NotYet`, kind `LayerAbsent`, "Actor.Trace: the world's
  collision", record this note. That is the kind and reason of the spot search's refusal on main ("needs collision").
  Since the camera traces every frame, this stops link 14 at the death camera until a collision layer answers
  traces. With #477's BSP-only service, a trace is a read of its own (open question 6).
- **The native iterators** (one on link 13's swing path, one on link 14). Reference counts a `foreach` over an unbound
  iterator under one generic text, so the report does not say which iterator it was. **Off answer:** in Off the
  VM's refusal for an unbound iterator stops the step and names the iterator native's key, so the first Off run past
  link 4 names it. Until an iterator is bound and read, a lane that binds it answers `NotYet`, kind `NotRead`, "the
  engine iterator <key>", record this note.

## 5. Link 4's swing path (met first by link 4, listed for completeness, not read here)

`Controller.InGodMode`, `DamagePipeline.GetBarrelSourceTime`, `GetCanInflictFriendlyFire`, `GetPlantSourceTime`,
`DamageTypeDefinition.IsHealingDamageType`, `GameInfo.IsFriendlyFire`, `Pawn.GetController`, `Pawn.InGodMode`,
`Pawn.IsUsingWalkingPhysics`, `Pawn.PawnCheckPlayHitPreconditions`, `PlayerController.DoForceFeedbackWaveform`,
`WillowDamageSource.GetAmmoTheftAmounts`, `WillowGameInfo.GetCombatMusicManager`,
`WillowGameInfo.TravelPostCountdownInProcess`, `WillowGameInfo.UpdateTookDamageFromEnemyStats`,
`WillowPawn.AdjustMomentumForSurfaceType`, `WillowPawn.GetMaxShieldStrength`, `WillowPawn.TryFullBodyGib`,
`WillowPlayerController.ShakeView`, `WillowPlayerPawn.IsVulnerableToFriendlyFire`, and a native iterator. None of them
is bound, except `WillowGameInfo.TravelPostCountdownInProcess`, which is bound **Off only** (`ow-world`'s `world.rs`,
NATIVE_TRAVEL.md): Reference counted it because the C++ binds none. The bound ones on that path
(`WillowGameInfo.GetSkillManager` and others) are not listed.

## 6. The main reading (slice gate on main 0235937, Off, strict)

`target/release/ow-package.exe --slice-gate --cooked <dir>` on main 0235937 (#462, #464, #467 and #475 in), under the
machine lock: **ok 0, notyet 12, error 2 of 14.** Steps passed / driver steps and the first stop:

| # | Link | Result | Steps | First stop |
|---|---|---|---|---|
| 1 | spawn_maya | notyet | 0 / 2 | `maya:8`: stopped after partial writes, the ammo resource pool's idle-reset hook (NATIVE_RESOURCE_POOL_IDLE.md) |
| 2 | reach_fight_area | notyet | 3 / 4 | `fight_area`: no fight area placed in the map world |
| 3 | kill_enemy | notyet | 0 / 2 | blocked by link 1 |
| 4 | enemy_attacks_maya | notyet | 0 / 2 | blocked by link 1 |
| 5 | loot_drop_on_kill | notyet | 0 / 0 | blocked by link 3 |
| 6 | pick_up_gun | **error** | 0 / 3 | `make`: unbound native `WillowGlobals.GetWillowGlobals()` |
| 7 | pick_up_aimed_not_nearest | **error** | 0 / 4 | `make`: unbound native `WillowGlobals.GetWillowGlobals()` |
| 8 | equip_gun | notyet | 1 / 5 | `init`: InventoryAttributeContextResolver, the property of its equipped-only flag |
| 9 | phaselock_enemy | notyet | 0 / 2 | `spawn`: the spot search needs collision (main has not taken #477) |
| 10 | mission_accept | notyet | 3 / 8 | `accept`: stopped after partial writes, GetActiveMissionNumber behind the online game settings refresh |
| 11 | mission_objective | notyet | 0 / 8 | blocked by link 10 |
| 12 | mission_turn_in | notyet | 0 / 6 | blocked by link 11 |
| 13 | go_down | notyet | 0 / 4 | blocked by link 4 |
| 14 | bleed_out_respawn | notyet | 0 / 1 | blocked by link 13 |

Mean step share per link: 9.5 percent, with link 5's empty step list counted as 0. Links 6 and 7 are `error` because
the globals natives (#469) are not on main yet. They are an unbound native, not a `NotYet`, which is the point of
section 1.

## Open questions

1. **The overridable implementations** of section 3.2: a way to resolve a class's implementation of an overridable
   native (the class-table lookup finds no constructor pattern for these classes), or a live trace of each.
2. **The status-effect test** of 3.4, and the per-kind removal, skill-manager step and listener of 3.3.
3. **The net-mode byte** of 3.1 and the game-info field of 3.5: `WorldInfo` cannot be laid out by the layout tool (a
   map property stops it), so the two fields are named by fit, not by layout.
4. **The Off path beyond Reference's zeros** (section 1): a fresh stub diff once links 1 and 4 pass in Off.
5. **The native iterator** each link reaches: the stub count does not say which one (4.5).
6. **`Actor.Trace`** (4.5): its native body, and whether a BSP-only collision service can answer the death camera's
   traces.
7. **`Controller.IsUsingVehicle`** (the non-`Ex` form): not read.

## Live checks that would settle each rule (bl-sdk, the RTSE way)

- **IsLocallyControlled:** on the player's pawn in a solo game, expect true; on a pawn with no controller, false.
- **GetSkillManager:** on the player controller, expect the same object as the game info's `GetSkillManager`.
- **GetTechDeathIndexForDamageType:** on an enemy, call it with a fire, a shock, a corrosive and a slag damage type,
  with and without that status effect on the enemy, and with a gibbed enemy (expect -1). Compare with
  `MyDeathDef.TechDeaths`.
- **ClearAllStatusEffects:** on a burning pawn, hook the call during an injury and read `ActiveEffectSounds` and the
  per-kind state before and after.
- **IsSplitScreen and IsInteractionDebugEnabled:** expect false in a solo game.
- **SetShadowCasting:** read `CastShadow` and `bCastDynamicShadow` on the player's body-part components before and
  after the injured-death sequence.
- **GetVehicleLifetimeManager:** read `TheVehicleLifetimeManager` before the first respawn of a session, to see
  whether the call ever makes it.
