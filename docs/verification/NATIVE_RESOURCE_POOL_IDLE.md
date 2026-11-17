# Native resource pools: the idle timer and the manager's shut-down

AI-assisted (Claude Code; recovery review: Codex / GPT-6.1 Sol). Lane FIX-POOLS-OFF, analyst only (`docs/LANE_BRIEF.md`, "Clean room (native code)"): this
note is written from a local reading of `Borderlands2.exe` in Ghidra 12.1.4 (`tools/ghidra/run.ps1`; the raw output
stays under ignored `local/analysis/`). Nothing here was seen in the running game: every rule is **UNVERIFIED** until
a live read confirms it. An implementer works from this note only.

Field names below are the script properties of `Engine.ResourcePool` at the byte positions the native reads, laid out
from the installed packages by `tools/ghidra/class_layout.py`. Oracle for that layout: the native class registration
gives `ResourcePool` the same instance size the package layout computes. The world-time field was named the same way
on `Engine.WorldInfo`.

## The time these natives use

"The world time" below is the `TimeSeconds` of the world info of the world the engine is running: the world's first
level's first actor. When the engine has no world at all, the time is 0. `ResourcePool.GetTimeSeconds()` returns
exactly this value (a float). `SetCurrentValue`'s stamp (NATIVE_LEVEL_UP_ATTRIBUTES.md section 4) is taken from the
same source.

## ResourcePool.ResetIdleTimer()

- **Inputs:** the pool. No arguments, no return value.
- **Effects, in order:**
  1. `PoolIdleDelayStartTime` := the world time.
  2. The pool's "idle timer was reset" hook runs (a virtual of the native class):
     - for `ResourcePool`, `HealthResourcePool`, `AmmoResourcePool` and `ExperienceResourcePool`: it does nothing;
     - for `ShieldResourcePool`: it does what the base does (nothing), then, only when the pool's `Definition` is set,
       it follows the pool's `AssociatedProvider` through three further engine objects. When all of them exist, it hands
       four of the pool's values to a function at the end of that chain: `bHasPoolBeenFullSinceLastBeingDepleted`,
       `CurrentValue`, `MinValue` and `MaxValue`. Neither the chain nor that function was read. It changes nothing in
       the pool itself and looks like a presentation update (a shield meter), but that is a guess.
- **Does not:** change `CurrentValue`, `MaxValue`, any regeneration value, or any flag of the pool.
- **Implementer checklist:**
  - after the call, `PoolIdleDelayStartTime` equals the world time, and 0 with no world;
  - a base, health, ammo or experience pool changes in nothing else;
  - a shield pool: the stamp is the same; the hook's chain is not read, so it stays `NotYet` in `ParityMode::Off`
    (it may only matter for presentation, which is UNVERIFIED).

## ResourcePool.HasIdleDelayPassed(float CurrentTime) -> bool

- **Inputs:** the pool, and `CurrentTime` from the caller (the script passes it; nothing here reads the world).
- **Rule:**
  - **false**, with nothing changed, when `PoolIdleDelayStartTime` is above 0 **and** `CurrentTime` is below
    `PoolIdleDelayStartTime + OnIdleRegenerationDelay` (strictly);
  - **true** otherwise, and then `PoolIdleDelayStartTime` := 0.
- **Notes:**
  - a stamp of 0 or below always means "passed";
  - `OnIdleRegenerationDelay` is the attribute's current value (the field, not its base value);
  - the comparison is between the two floats as stored, with no rounding.
- **Implementer checklist:**
  - stamp 0 gives true, and the stamp stays 0;
  - stamp 10, delay 3: time 12.9 gives false and the stamp stays 10; time 13 gives true and the stamp becomes 0;
  - a second call after true gives true.

## ResourcePool.GetTimeSeconds() -> float

- Returns the world time (above), with no other effect.

NATIVE_WEAPON_TRIGGER_AMMO.md (the accuracy pool) describes the same delay from its caller's side: regeneration waits
`OnIdleRegenerationDelay` seconds after the last change. This note agrees with it.

## ResourcePoolManager.ShutDown()

Read because Off spawns reach it since decision 2026-11-16-70 (`--combat-run` `spawn:` stopped at it as an unbound
native).

- **Inputs:** the manager. No arguments, no return value.
- **Rule:** only when the manager's `Role` is authority (3): for each slot from 0 to 15 in order, the manager's
  delete-one-slot routine runs. That routine is the same body `DeletePoolAtIndex(i)` runs
  (NATIVE_COMBAT_GROUNDWORK.md, "DeletePoolAtIndex"): for a used slot it removes the pool's resource from the map,
  runs the pool's script `ShutDown`, empties the slot and marks the manager for a network update. A manager without
  authority is left as it is.
- **So:** `ShutDown` on a manager does what `DeleteAllPools(Mgr)` does with that manager.
- **Implementer checklist:**
  - a manager with authority and pools in slots 1 and 4 ends with all 16 slots empty, each pool's script `ShutDown`
    run once, in slot order;
  - a manager with another role changes in nothing.

## What this note does not cover

- The regeneration rules themselves: `GetTotalRegenRate`, `GetCurrentPulseSpeed`, `AddCurrentValueImpulse`, and the
  pool's tick, if the native has one. They were not read.
- The shield hook's chain and the function it ends in.
- Where the engine sets the world time (only where these natives read it).

## Real-game questions

1. After a hit on Maya's shield, `PoolIdleDelayStartTime` of her shield pool equals the world's `TimeSeconds` at the
   hit, and returns to 0 once the recharge delay has passed. A bl-sdk read of the pool and of `WorldInfo.TimeSeconds`
   settles it.
2. With the main menu open (no game world), a pool's `GetTimeSeconds` gives 0.
