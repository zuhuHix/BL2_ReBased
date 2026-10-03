# Native Phaselock targeting, skill constraints and lift bob (2026-10-02)

AI-assisted. Behaviour notes written from a local reading of `Borderlands2.exe` in Ghidra, under the policy in
[LEGAL.md](../LEGAL.md) ("Analysing the executable") and [NATIVE_ANALYSIS.md](../NATIVE_ANALYSIS.md), plus script
read with `ow-package --disasm`. Nothing below is listing, pseudo-code or an address. **Every rule here was
`UNVERIFIED`** when written: read from native code or script. Each section ends with the observation that would
confirm it. On 2026-10-02 the lift bob (section 5) and the reload/put-away constraints (section 4) were confirmed in
game; the target score (sections 2-3), the injured/downed rules and the rest stay `UNVERIFIED`
([REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md)). Data values quoted are those the host's Phaselock manifest already decodes
(`GD_Autoaim.Default`, `Skill_Phaselock.SkillConstraints`, `LiftActionSkill` settings).

Host code cited is the working tree on top of commit `17a7664`.

## 1. How Phaselock asks for a target

`WillowPlayerController.StartActionSkill` (script, local player only): if the controller has an `AutoAimStrategy`, it
calls `GetPreferredTarget(InPlayer = self, bGetCachedResult = false, bGetInstantaneousTarget = true,
bMustBeWithinWorldSpaceRadius = false)`; the result becomes `CurrentActionSkillTarget` only if it is a `WillowPawn`,
otherwise none. Then `ServerStartActionSkill(target)`. Everything after that (`LiftActionSkill.SelectTarget`,
`CanPhaseLockTarget`, blocked/fizzle) is script the host already follows. Note that no separate Phaselock trace
exists in this path: the target is whatever the auto-aim strategy prefers at that moment.

## 2. `WillowAutoAimStrategy.GetPreferredTarget`

Strategy state (script class fields): `InstantaneousTarget`, `LastInstantaneousTarget`, `LockedTarget`,
`AcquireStartTime`, `SustainStartTime`, `CurrentProfile` / `PrevProfile`, flags. Definition (`GD_Autoaim.Default`):
`MaxTargetDistance` 50,000, `MinTargetDistance` 0, `RadiusMultiplier` 0.8, `MaxSnapAngle` 0.5, `DistanceOffset`
275, `AcquireTime` 0, `SustainTime` 0, `ChangeTime` 1.

**Order of effects:**

1. No player or no pawn: the cached result is cleared (if one was computed) and nothing is returned.
2. Unless the result was already computed this frame or `bGetCachedResult` is set, a new search runs (section 3)
   and its best candidate becomes `InstantaneousTarget` (the previous one is kept as `LastInstantaneousTarget`).
3. When the instantaneous target changed, the owner is told it lost the old one and acquired the new one (these reach
   `ActionSkill.OnOwnerLostAutoAimTarget` / `OnOwnerAcquiredAutoAimTarget`, read only as far as the calls), and
   `AcquireStartTime` = now.
4. Lock: with a locked target the switching delay is `ChangeTime`, else `AcquireTime`. If the instantaneous target
   differs from the locked one and has been held longer than that delay, and a check on the pawn's weapon accepts it,
   it becomes `LockedTarget`. While both agree, `SustainStartTime` = now. The lock is dropped when locking is disabled,
   or when there is no instantaneous target and more than `SustainTime` has passed.
5. Return: the locked target if it is still a valid target and either `bGetInstantaneousTarget` is false or there is
   no instantaneous target (and, if required, it is within the world-space radius); otherwise the instantaneous target
   (same radius condition); otherwise none.

For Phaselock (`bGetInstantaneousTarget` = true, `SustainTime` 0) this reduces to: **the best-scoring target of this
frame**, else, only in the instant after losing it, the locked one.

## 3. The search and its score

Candidates come from the engine's targetable list (filtered by the strategy's `TargetSet`). A candidate is skipped
unless it reports itself an auto-aim target and passes a targetability check (which honours `bIgnoreCloakAbility`);
the viewer's own pawn and the vehicle the viewer is driving are excluded. The highest score above 0 wins; ties keep the
first found. For each candidate:

1. **In front:** the target's location must be in front of the view plane (positive dot product with the view
   direction). The closest point on the view ray to the target is recorded.
2. **Screen position:** the target location is projected with the local player's view-projection; `x` is corrected by
   the aspect ratio so `x` and `y` are in the same units (fractions of the half-screen height); `depth` = the view-space
   depth `w` (not straight-line distance).
3. **Target's own screen radius:** `rT = radius × RadiusMultiplier / (depth × tan(FOV / 2))`, with the target's
   auto-aim radius (an `ITargetable` query, not read further) and the camera FOV.
4. **Magnetism radius** (`GetLogMagnetismRange` is the matching script-visible name):

   ```
   rS = MaxSnapAngle × max(0, 1 + log2(MaxTargetDistance) − log2(max(1, depth − DistanceOffset))) / log2(MaxTargetDistance)
   rS = max(rS, rT)
   ```

   With the data: depth 300 → 0.38, 500 → 0.28, 1,000 → 0.23, 2,000 → 0.19, 5,000 → 0.14, 10,000 → 0.11 (fractions
   of half the screen height). Near targets get a wide cone, far ones a narrow one, never narrower than the target.
5. `s = sqrt(x² + y²)` (distance of the projected target from screen centre). `bWithinWorldSpaceRadius = s ≤ rT`.
   If `s > rS` the score is 0.
6. **Score** = `0.5 × ((2 − depth / MaxTargetDistance) − s / rS)`: in (0, 1], favouring targets near the crosshair
   first and near the player second.
7. **Distance gate:** `MinTargetDistance ≤ depth ≤ MaxTargetDistance`, else 0.
8. **Line of sight:** a trace from the view location to the target's aim point (another `ITargetable` query). Passes
   if it hits nothing, hits the target, or hits an actor that is hard-attached (`bHardAttach`, following `Base`) to
   the target; otherwise the score is 0.

**Host today.** `OpenWillowWalker.cpp:781-786`: a view-ray line trace to `MaxTargetDistance`, else a 30 cm sphere
sweep, the first `AOpenWillowCombatTarget` hit, and a straight-line `MinTargetDistance` check. Differences: no
screen-space magnetism (the native cone is about 0.23 of the half-screen height at 10 m, far wider than 30 cm), no
best-of-many scoring, line of sight to the target's aim point instead of "first thing the ray touches", depth instead
of distance, and no per-frame strategy state. **Implement** (host, Walker or a small targeting helper): iterate the
host's targetable actors, project with the player camera, apply steps 1-8 with the manifest's auto-aim settings, keep
the best score; the auto-aim radius and aim point of the dummy need a host stand-in (collision radius and a chest
point, labelled as such).

**Confirmation:** in the game, cast Phaselock with the crosshair off a target: at ~10 m the cast should still lift a
target whose centre is within about a quarter of the half-screen height from the crosshair, and the threshold should
shrink with distance. A synthetic oracle with invented numbers: a target at depth 1,000, `rT` 0.05, screen offset
0.20 → score `0.5 × (2 − 0.02 − 0.20/0.228) ≈ 0.55`; at offset 0.25 → 0.

## 4. Skill constraints (`Skill_Phaselock.SkillConstraints`)

**How constraints run** (`Skill.CalculateStateBasedOnConstraints(bActivation)` and the skill's tick): each
`SkillConstraintData` applies when `bApplyConstraintOnActivatation` and the call is an activation check, or
`bApplyConstraintWhileActive` and the skill is `SKILL_Active`, or `bApplyConstraintWhilePaused` and it is
`SKILL_Paused`. It holds when its `Evaluator` (if any) and every `EvaluatorDefinitions` evaluator return true, each
evaluated with the skill's instigating controller. The first failing constraint sets the result to its `OnFailure`;
if that is `SKILL_Deactivated` the check stops there. All pass → `SKILL_Active`. The skill's tick re-runs the
non-activation check every frame while the skill is not deactivated and switches state when the result differs. All
three Phaselock constraints fail to `SKILL_Deactivated`.

| Constraint | When | Native `Evaluate` reading | Host mapping today |
|---|---|---|---|
| `WeaponActionAvailableExpressionEvaluator` | activation only | calls script `WillowPlayerController.CanPerformWeaponAction(0)`: true unless `bWeaponsRestricted`; with a pawn holding a `WillowWeapon`, that weapon's `CanPerformAction()` (true, except in states `WeaponPuttingDown` and `Inactive`); and false while `bPerformingSharedWeaponAction` | "not reloading" (`OpenWillowPhaselock.cpp:176-178`). **Differs:** reloading does not block; putting a weapon away, a holstered weapon, restricted weapons and a shared weapon action (melee/grenade-style) do |
| `HealthStateExpressionEvaluator` (`bHealthy`) | activation and while active | true if the controller's pawn is alive and well (`Pawn.IsAliveAndWell`); `bInjured` / `bDead` would test `IsInjured` / `IsDead`; no pawn → false | `Health > 0` at activation only. **Differs:** an injured (Fight For Your Life) Maya cannot cast, and becoming injured during the lock deactivates the skill |
| `VehiclePassengerExpressionEvaluator` (`bNotInVehicle`, `bAttachedRider`) | activation, while active and while paused | not in a vehicle → `bNotInVehicle`; in one → true if `bDriver` and it is a `WillowVehicle`, or `bAttachedRider` / `bPassenger` matches the seat kind | always true (no vehicles). Consistent for the slice |

`Pawn.IsAliveAndWell` itself was not read (only that the evaluator calls it); "alive and not injured" is the reading
of its name. **Implement:** `GateOpen` (`OpenWillowPhaselock.cpp:171`) with weapon-action = not putting away / not
holstered / no melee in progress (reload allowed), healthy = alive and not injured, and a per-tick while-active check
that ends a running Phaselock when Maya goes down.

**Confirmation:** in the game, press the action-skill key during a reload (the reading says it casts), during weapon
swap put-away (should not), and get downed while a target is held (the reading says the lock ends immediately).

**Confirmed in game on 2026-10-02** (SDK marks of `StartActionSkill` and the weapon's reload/put-down calls, per-frame
skill state): during a manual reload the cast starts and lifts the target, and it aborts the reload
(`OnAbortReload`); during a swap put-down (`IsPuttingDown()` true) the skill stays idle and no cooldown is used, 2 of 2,
with a control cast right after each succeeding. Not tested: going down while a target is held, casting while injured
([REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md)).

## 5. Lift bob and the held target

Script (`LiftActionSkill.UpdateLiftedPawn`, `GetBobLocation`): while lifting (less than `LiftDuration` since
`LiftStartTime`) the pawn follows `GetLiftLocation`; after that it follows the bob; in lift state 4 it stays where it
is. The bob target is `LiftEndLocation` with `Z += LiftBobAmplitude × sin((now − SkillStartTime) × LiftBobFrequency ×
3.14159)` (amplitude 30, frequency 0.5: a 4 s period), then smoothed with `VInterpTo(PrevBobLocation, target,
DeltaTime, 1.0)`.

`VInterpTo` (stock native, read here): speed ≤ 0 or squared distance < 0.0001 returns the target; otherwise moves by
`clamp(DeltaTime × speed, 0, 1)` of the remaining difference. At speed 1 this is a lag of about one second: the visible
bob is roughly 0.54 × 30 ≈ 16 units and about 0.6 s behind the sine (first-order filter at π/2 rad/s; exact values
depend on frame rate).

**Host today.** `OpenWillowCombatTarget.cpp:25-30` applies the unsmoothed sine, timed from the end of the lift,
already labelled as host choices. **Implement:** time from `SkillStartTime` (the cast) and apply the `VInterpTo` lag
from the lift end position. The cooldown pool's native tick was not read in this batch; the host's script/data model
(refill at cast, cooldown manager's consumption change while a target is held) stands as before.

**Confirmation:** a capture of a lifted enemy's height over time (sdk trace of `LiftedPawn.Location.Z`): amplitude
about 16, period 4 s, phase relative to the cast.

**Confirmed in game on 2026-10-02** by a per-frame SDK trace of the lifted bullymong: this rule, run on the game's own
frame times and seeded at the end of the lift, reproduces its height over 435 frames with RMS error 0.001 units
(peaks +15.6/+16.2, trough −16.3, 4.0 s period, first peak 1.63 s after the cast). Lift 0.7 s, hold 3.9 s, release
1.1 s at level 8 ([REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md)).

## What was not read

The `ITargetable` radius and aim-point implementations on `WillowAIPawn`; the weapon check before a lock is taken; the
candidate list's `TargetSet` filter; `Pawn.IsAliveAndWell`'s body; who calls the activation constraint check (script
`ActivateSkill` is the expected caller); the cooldown pool's native update.
