## 2026-11-16: Rust resource pools in Off: the game's route to Maya's health pool, and the idle timer read

AI-assisted (Claude Code; recovery and scope separation: Codex / GPT-6.1 Sol). Lane FIX-POOLS-OFF,
step 3 of `docs/rust-port/SLICE_RUN_PLAN.md`. Two parts:
- an analyst note on the unread pool natives this step meets (clean room: their Off rules stay `NotYet` for a fresh
  implementer);
- the game's own route to Maya's health pool in `ParityMode::Off`, in place of a refusal.

Reference keeps the C++ stand-ins (R3): Reference JSON below agrees before and after excluding timing fields; stderr is byte-identical. There was no
real-game session: every rule is **UNVERIFIED** in the running game.

- **Analyst note: `docs/verification/NATIVE_RESOURCE_POOL_IDLE.md`** (new; read in Ghidra 12.1.4, raw output under
  ignored `local/analysis/` only; the clean-room scan of the note is clean). `NATIVE_INDEX.md` now points at it.
  - **`ResourcePool.ResetIdleTimer`:**
    1. `PoolIdleDelayStartTime` := the world's `TimeSeconds` (0 with no world);
    2. then a per-class hook runs. It is empty for the base, health, ammo and experience pools. For the shield pool it
       ends in an unread chain that looks like a presentation update.
    The C++ stand-in (`src/combat.cpp`) had the stamp only.
  - **`HasIdleDelayPassed(CurrentTime)`:** false while the stamp is above 0 and `CurrentTime` is below the stamp plus
    `OnIdleRegenerationDelay`; otherwise true, and the stamp is cleared to 0.
  - **`GetTimeSeconds`:** the world time.
  - **`ResourcePoolManager.ShutDown`:** with authority, the per-slot delete for every slot, in order: the body
    `DeletePoolAtIndex` runs, so it is `DeleteAllPools` on the manager itself.
  - **Structural checks:**
    - the field names come from the package layout of `Engine.ResourcePool`, whose computed size equals the size the
      native class registration gives;
    - `WorldInfo.TimeSeconds` lands at the read position for every size the layout tool could give the class's map
      property, which comes after it;
    - the hook was read from each of the five pool classes' own tables.
  - **Not implemented here** (the analyst does not implement): `ResetIdleTimer`, `HasIdleDelayPassed`,
    `GetTimeSeconds` and `ShutDown` stay as they were: `ResetIdleTimer` `NotYet` in Off, `ShutDown` unbound. A fresh
    implementer builds them from the note.
- **Maya's health pool in Off** (`crates/ow-combat/src/run/player.rs`, `HealthPool::ensure_off`).
  - Before, Off refused at once (`NotYet { NotPorted }`: the C++ builds the pool by hand). Now the run takes the
    route NATIVE_COMBAT_GROUNDWORK.md sections 1.8 and 1.10 record:
    1. the controller's installed script `ApplyCharacterClassDefaults(class, pawn)` runs on the VM, its arguments
       bound by parameter name;
    2. the pawn's `SetHealthResourcePoolDefinition` makes the manager and the pool through `CreatePool`;
    3. the pool's own script sets its starting values.
  - The pool kept is the one the pawn's `HealthPool` names afterwards.
  - `mayahealth` sets the share through `Pawn.SetHealth`. The Reference seed (an evaluator stand-in) is not used in
    Off, and the pawn's reference is not relinked by hand.
  - Reference keeps the C++ stand-in unchanged.
  - This file belongs to lane W6-COMBAT-RUN2 and is edited here, with its fixture and tests (named for the hub).
- **Verified (automated).** On invented packages:
  - `ow-combat`: the `maya` and `mayahealth` tests now check Off end to end. The toy `WillowPlayerController` has an
    `ApplyCharacterClassDefaults` making the record's call, and the toy pawn has authority, as the server's does.
    - after `maya`: a manager on the controller with one pool in slot 0, its provider the controller, the pawn's
      `HealthMaxVar` and health at the pool's maximum, no errors;
    - after `mayahealth:0.5`: half the maximum;
    - Reference as before.
- **Verified (local, installed game, pc-hummer; `origin/main` feda4cd against the recovered combined work; outputs under ignored
  `local/p2/fix-pools-off/` only).**
  - `--combat-run maya:8 players:1 spawn:adult:7 tick:100 ...` and `--slice-run` over the Fire mission (the step
    list of `tools/verify_fire_event_order.py`), both modes.
  - Reference: standard output (timings aside) and error byte-identical.
  - Combat stops before and after are in the table below. The saved after-slice result also included a separate
    definition-store fallback. It is not POOLS-only proof; that fallback is preserved outside this lane.

| Run, step | Before | After |
|---|---|---|
| `--combat-run` `maya:8` | `NotYet { NotPorted }`: the C++ hand-built pool | the game's route runs and stops at the unbound `ResourcePoolManager.ShutDown()` (read in the note, left for an implementer) |
| `--combat-run` `spawn:adult:7` | unbound `ResourcePoolManager.ShutDown()` | the same |
| `--slice-run` | the template archetype `WillowGame.Default__WillowAIPawn.GearboxDialogComponent0`: "no package holds it" | unchanged for POOLS alone; the saved combined run reached `Behavior_AIHold` only with the separate store fallback |

- **Open.**
  - The implementer lane for the note: `ResetIdleTimer`, `HasIdleDelayPassed`, `GetTimeSeconds` and `ShutDown`.
    After `ShutDown`, `ResetIdleTimer` is the next stop on the route (every installed `InitializeAuthoritative` calls
    it, decision 2026-11-15-29).
  - The shield pool's hook chain.
  - The regeneration rules (`GetTotalRegenRate`, `GetCurrentPulseSpeed`, `AddCurrentValueImpulse`, any pool tick).
  - The mission bridge's `set_player_health` (`--slice-run`) is still `NotYet` in Off, not routed here.
  - Two real-game questions are in the note.

Recovery (2026-11-17, Codex / GPT-6.1 Sol): the store fallback and its synthetic test are excluded from this
lane and preserved as an ignored patch for separate diagnosis. No parser or definition-store change ships here.
The installed slice remains blocked by #370. No new game or host session was run. The analyst-only natives above
remain `NotYet` or unbound; this is a route and evidence handoff, not completed pool gameplay parity.

The isolated recovery CLI run confirms the same Off combat stops and the unchanged #370 slice setup stop.
The legacy Python combat suite has three identical failures on the recovered main binary and this lane in
Reference (event-log assertions and random loot quantity); it is not reported as passing. The saved installed
Reference combat trace checker reports 8 PASS, 0 FAIL, 51 MISSING of 59 rows, with 43 loot rows not listed.
