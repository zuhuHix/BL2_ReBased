# Native rules of GotoState('Auto'): the auto state flag, its lookup, and when an actor enters it (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-AUTO-STATE (decision 2026-11-19-NNN, number pending). Analyst
only: a different lane implements it (clean room).

**Sources, in the order used:**
- the installed script, read with `research/script_disasm.py`: `Actor.SetInitialState`, every `GotoState('Auto')` call
  site, and the state code of every state the packages flag as auto;
- the packages: a census of the state flags of every `State` export and of every class's own state flags, in the nine
  code packages, with the layout `ow-vm`'s `states::read_state_body` and `read_class_state_part` already read
  (decision 2026-11-15-27);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock), for
  the state change routine's `Auto` branch and its field iterator only.

No bl-sdk session was run, and no recorded live evidence touches these states (searched: the verification records and
decisions). Raw output stays under ignored `local/analysis/auto-state/`. This note is in our own words: no decompiler
output, pseudo-code, addresses, offsets or listing structure. **Every rule here is UNVERIFIED** in the running game.

**Why it matters.**
- On the map-load path, the bring-up's 597 `SetInitialState` stops all end in `GotoState('Auto')` (decision
  2026-11-19-20). The same stop ends the game info's `InitGame` and `PreBeginPlay`. By inference, not traced
  here, that is through the actors those events spawn after play began, whose `SetInitialState` reaches it. That blocks the script-made GRI and mission tracker (SLICE_MAPLOAD.md, lane 2 and the auto-state
  row).
- The port's `ow-vm` refuses `Auto` unless the class's own state flags and every state in its chain have flags 0. Most
  classes have their own auto flag set, so it refuses almost every actor.

## 1. The auto flag is bit 1 (value 2) of the state flags

**From the packages** (structural oracle, high confidence for the bit, all nine code packages):

| What | Count |
|---|---|
| `State` exports | 166: flags 0 on 129, 2 on 11, 4 on 25, 6 on 1 |
| Classes (their own state flags) | 3,339: flags 2 on 3,317, 0 on 22 |

Read on the two bits:
- **Bit 1 (value 2) on a state is set on 12 states, in 12 classes, never twice in one class.** That is the shape the
  `auto` keyword gives: at most one auto state per class.
- **Bit 1 on a class is set exactly when no state in the class's chain has bit 1.** All 22 classes with own flags 0
  declare an auto state (12) or inherit one (10). All 3,317 with the bit set have none in their chain. That is 3,339 of
  3,339 classes with no exception. By public UE3 convention the compiler marks a class itself as auto when no auto
  state is declared, which fits.
- **Bit 2 (value 4)** is on 26 states in 8 classes (weapons, pawns, vehicles, the action skill), up to 7 per class. That
  fits the public UE3 "simulated state" bit. It is not needed for `Auto`, and it stays the port's existing `NotYet` for
  state code on an actor that is not the authority.

The bit's value matches the public UE3 `STATE_Auto`. That is a fit, not a recovered spec (AGENTS.md rule 4).

**From the executable** (high), on the state change routine's `Auto` branch:
1. **The class's own flag is tested first.** When the object's class has the auto bit set, the target is the class
   itself, with the state name reset to None, so the object ends with no state.
2. **Otherwise the states are walked.** The walk covers the class's own fields, in field-list order, and then each
   super class's, most derived first. Only states are considered. The first state with the auto bit set is the
   target.
3. **When none is found**, the target is the class itself, with the name None, as in step 1.
4. In every case the rest of the state change routine runs as NATIVE_ENGINE_CORE.md describes: the push stack,
   `EndState`, the switch, `BeginState`. Then `GotoState` applies the label, by default `Begin`.

So an inherited auto state is found. For example, `WillowCoopGameInfo` declares no state, but its parent
`WillowGameInfo` does, so `Auto` resolves to `WillowGameInfo.PendingMatch`.

## 2. When an actor enters its auto state

- **`Actor.SetInitialState`** (script) sets `bScriptInitialized`, then calls `GotoState('Auto')`. It does nothing else.
  Seven classes override it:
  - `Volume`, `PickupFactory` and its `Disabled` state;
  - `WillowBoundaryTurret`, `WillowInteractiveSwitch`, `WillowProjectile` and `WillowVehicleFactory`.

  `WillowProjectile`'s override still calls `GotoState('Auto')`. The others were not read here.
- **Who raises `SetInitialState`:**
  - **The bring-up's routing**, pass 2: right after `PostBeginPlay`, unless the actor is being deleted
    (NATIVE_MAP_LOAD.md section 2.1).
  - **A spawn after play began:** after `PreBeginPlay` and `PostBeginPlay` (NATIVE_ENGINE_CORE.md, "Actor.Spawn",
    step 10).

  So the auto state is entered after `PostBeginPlay`, and not at the first tick.
- **The order inside it.** The auto state's `BeginState` runs inside the `GotoState` call, after the switch. That makes
  it after `PostBeginPlay` and before the spawn or the bring-up moves on.
- **The state code** does not run at `GotoState`. `GotoState` only sets the code position to the `Begin` label, when
  the state has one; a missing default `Begin` is silent. The code runs at the actor's state-code step, which comes after
  its `Tick` event in each world tick (NATIVE_ENGINE_CORE.md, "State-code step"). By that rule it runs only on the
  authority.
- **Other script calls of `GotoState('Auto')`.** 10 of the 224 `GotoState` call statements in the packages name
  `Auto`:
  - `Actor.SetInitialState` and `WillowProjectile.SetInitialState`;
  - four functions of `WillowAIPawn`'s `Staggered` state;
  - `WillowCoverComponent.ClearCover`;
  - `WillowProjectile.WaitingForVelocity.Tick`;
  - two functions of `WillowVehicle_ChopperVehicle`'s `AutoLanding` state.

## 3. What the auto states hold (script; names and counts only)

| Class chain (and its subclasses) | Auto state | State code | Events the state (or its parent) defines |
|---|---|---|---|
| `GameInfo` | `PendingMatch` | empty (`Stop`) | `EndState`, `StartMatch`, the arbitration functions |
| `WillowGameInfo` (so `WillowCoopGameInfo`) | `PendingMatch` (child of `GameInfo`'s) | `Begin:` label; when the game info's `StartGameState` is in a given range, it calls `StartMatch`; then `Stop` | none of its own |
| `PlayerController` | `PlayerWaiting` | empty | `BeginState`, `EndState`, and input and restart functions |
| `WillowPlayerController` | `PlayerWaiting` | `Begin:` label; it calls `WaitForGRI`, then `Stop` | `CanRestartPlayer`, `RestartAfterLogin`, `WaitForGRI` |
| `DebugCameraController` | `PlayerWaiting` | empty | `PlayerMove` |
| `Weapon` | `Inactive` | empty | `BeginState` and fire functions |
| `WillowWeapon` | `Inactive` (bits 1 and 2) | empty | `BeginState`, `EndState`, three more |
| `PickupFactory`, `DroppedPickup` | `Pickup` | `Begin:` label; it calls `CheckTouching`, then `Stop` | `BeginState`, touch functions |
| `WillowPickup` | `Pickup` | empty | `BeginState`, `EndState`, `PostRenderFor` |
| `WillowHUD` | `Startup` | empty | `DoStartupDrawHUD` |
| `KActorFromStatic` | `Initializing` | empty | `Tick` |

**What every other class gets: no state.** That covers `WillowGameReplicationInfo`, `MissionTracker`,
`WillowPlayerPawn`, `WillowAIPawn`, and every static mesh, light, trigger and population actor of Sanctuary.

**The state-code features the slice's auto states use:** the `Begin` label, `JumpIfNot`, a virtual call, a comparison,
and `Stop`. There is no latent call (no `Sleep`, no `FinishAnim`), no `PushState`, and no label other than `Begin`. The
port's state machine already runs all of these (`ow-vm` `states`, decision 2026-11-15-27).

## 4. What the port does today (crates/ow-vm/src/states.rs and the world tick)

- **`GotoState`, the state change routine, the push stack, labels, probes and the state-code step** are implemented
  in Off, from NATIVE_ENGINE_CORE.md.
- **`auto_state`** answers "no state" only when the class's own state flags and every state of its chain have flags
  0. Otherwise it refuses with `NotYet { NotRead }`, "GotoState('Auto'): which state flag bit marks an auto state".
  - Classes have their own bit set (3,317 of 3,339), so the refusal hits almost every actor. These are the 597 bring-up
    stops.
  - The 22 classes with their flag clear refuse too, because they have an auto state in their chain.
- **The world tick** (`ow-world` `World::tick_actor`) steps state code through a hook, `World::set_state_step`. No
  production world sets it. So once an actor is in a state, its first tick is
  `NotYet { LayerAbsent }`, "the state-code step of an actor in a state". After `Auto` resolves, this is the next stop
  on the map route: the game info (`PendingMatch`) and the player controller (`PlayerWaiting`) are in a state from
  their first tick.
- **State code on an actor that is not the authority** is `NotYet { NotRead }` (the simulated bit, section 1). Single
  player is the authority, so the slice does not reach it.

## 5. Proposed ParityMode Off answer (for a separate implementer)

**1. `auto_state(class)`:**
- When the class's own state flags have bit 1 (value 2) set: no state. `GotoState` then behaves as for `None` and stays
  silent, as `is_auto` already makes it.
- Otherwise: walk the chain from the class itself to the root. In each class, look at its own declared states, and
  pick the first with bit 1 set. None found: no state.
- Reference is unchanged (the C++ approximation).

**2. Typed refusals:**
- **A class with bit 1 clear whose chain has more than one auto state in one class:** `NotYet { NotRead }`. The
  engine takes the first in field-list order, and the port's state list does not keep that order. The packages hold no
  such class (section 1), so this guard is never reached on installed data.
- **The oracle broken:** a class with bit 1 set and an auto state in its chain, or bit 1 clear and none found. This
  is `NotYet { NotRead }`, "GotoState('Auto'): class flags and state flags disagree". The engine would still answer
  (no state in the first case, and the class in the second); the refusal only marks data the census never saw.
  Proposed as a refusal rather than an answer. UNVERIFIED either way.
- **A class or state body that does not read:** the existing decode error, unchanged.

**3. The state-code step on the map route.** The world that runs the bring-up sets `World::set_state_step` to
`ow_vm::states::process_state` in Off. Today's `LayerAbsent` stays for a world that does not. This belongs to the
map-load lanes (SLICE_MAPLOAD.md); the implementer of item 1 may add it, or leave it to lane 4. `process_state` already
runs state code only on the authority.

**4. Tests the implementer can build** (synthetic packages, `package()` / `tag()` helpers):
- a class flagged auto (no state);
- a parent with an auto state and a child class without one (the child resolves to the parent's state);
- a child overriding the parent's auto state with its own auto state (the child's wins);
- a child that declares a non-auto state of the same name (the walk continues to the parent's auto state);
- the two oracle refusals;
- `BeginState` of the auto state raised inside `SetInitialState`, after `PostBeginPlay`;
- the `Begin` label set and the code run only at the next state-code step, not at `GotoState`.

## 6. The slice's links (question d of the brief)

- **No link driver reaches `GotoState('Auto')` today.** The drivers' worlds never begin play (only the bring-up sets
  `bBegunPlay`), so a spawn there raises no `SetInitialState`. No stop in the current Off gate names it.
- **Link 12's next stop, `WillowPlayerController.UnlockAchievementIfConditionsMet`, is an unbound native** on the
  turn-in path. It is not this question.
- **Where it may come in:**
  - On the map route (SLICE_MAPLOAD.md), every actor of the bring-up, and every spawn after it.
  - In combat, `WillowAIPawn`'s `Staggered` state calls `GotoState('Auto')` when a stagger ends. `WillowAIPawn` has no
    auto state, so on this rule it returns to no state. Links 3 and 4 may reach it once their hits stagger an enemy.
  - `WillowProjectile` calls it from its own `SetInitialState` and its velocity wait. This matters for any shot that
    spawns a projectile after play began.

## Open questions

1. **UNVERIFIED in the running game:** the classes' resolved states. A bl-sdk read of `GetStateName()` right after
   load, on the game info, the local player controller, a held weapon and a dropped pickup, would confirm section 3's
   table. This port expects `PendingMatch`, `PlayerWaiting`, `Inactive` and `Pickup`.
2. The six other `SetInitialState` overrides (section 2) were not read for whether they reach `GotoState('Auto')`.
3. The game info's `StartGameState` range for `StartMatch`. The state code compares it against two small values. What
   sets `StartGameState` before the first tick (the game info's `PreBeginPlay` copies it from an older GRI when there is
   one) is the map-load lanes' to trace.
4. The order of the class's own fields where two auto states could meet. The packages have none, so it is moot for
   installed data.

## Live check that would confirm the rule (bl-sdk, the RTSE way)

After loading Sanctuary, read `GetStateName()` on `WorldInfo.Game`, the local player controller, its pawn, the held
weapon, and a `WillowAIPawn`. Hook `BeginState` on `WillowGameInfo.PendingMatch` and on
`WillowPlayerController.PlayerWaiting`, and log the order against `PostBeginPlay`. Expect `PostBeginPlay`, then the
state's `BeginState`, then the first state-code step at a later tick.
