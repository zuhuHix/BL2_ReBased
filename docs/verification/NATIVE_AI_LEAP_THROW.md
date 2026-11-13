# Bullymong leaps and thrown rocks: who does which, what chooses it, and how each one plays out (draft, 2026-11-13)

AI-assisted (Claude Code), analyst lane L. Written in our own words for a later implementer, who works from this note only. No listing,
pseudo-code, script dump or extracted table is reproduced here; tables below are summaries with the few values an implementer needs.

**Status, once for the whole note.** No real-game session was available. Unless a line says otherwise, every rule here is **read from data or
code and UNVERIFIED in the running game**. Lines that lean on an observation say which record holds it. Each statement carries one of these
source tags:

- **[record]**: a tracked record on main (`AI_MELEE_TARGETING_LIVE.md` = L-4, `AI_AWARENESS_LIVE.md` = L-9, `COMBAT_RANGE.md`, `NATIVE_AI_MELEE.md`,
  `NATIVE_AI_AWARENESS.md`, `COMBAT_MELEE.md`, `COMBAT_GOLDEN_TRACE.md`).
- **[den]**: one private capture, the den-down stage of the 2026-11-12 S0 smoke run (`cash_capture.ps1`, Three Horns Divide): behaviour
  observations only (relative placement, timing, displacement), from one run. Nothing from the player's save is used. The 2026-11-07 den
  read's numbers are already on main in `COMBAT_RANGE.md` and are cited as [record].
- **[data]**: the installed packages read with `ow-package --object-dump` (`Ice_Dynamic.upk` for the five bullymong AI definitions and
  `GD_PrimalBeastShared`, `WillowGame.upk`, `GearboxFramework.upk` and `Engine.upk` for enums). Clip lengths come from the same packages
  (the private P13 read of `Anim_PrimalBeast.Unarmed`).
- **[script]**: `research/script_disasm.py` on `WillowGame.upk` (`Action_LeapAtTarget`, `Action_AnimAttack`, `Action_GenericAttack`,
  `WillowActionSequencePawn.TargetTooFar`, `Behavior_AIThrowProjectileAtTarget`).
- **[native]**: native analysis of `Borderlands2.exe` with Ghidra (`tools/ghidra/run.ps1`, read-only queries against the saved project, six runs,
  2026-11-13, granted by the hub): the trajectory solvers, `WillowAIComponent.IsLocationValid` and four `AttackLocation_*.ValidLocation` bodies.

It builds on `NATIVE_AI_MELEE.md` (tree evaluation, melee volume) and `NATIVE_AI_AWARENESS.md` (node eligibility, completion, `TimedReset`,
the leap natives' contract) and does not repeat them.

## 1. Which kinds can do what

All five kinds share one tree shape: a root priority node over "go to scripted destination", the combat `Priority` node and `Patrol`. What
differs is the combat node's children. [data]

| Kind (AI definition) | Plain melee | Leap | Rock throw | Other |
|---|---|---|---|---|
| Regular (`AIDef_PrimalBeast`) | yes | yes, no roar | only the fallback throw | perch, ice spike |
| Adult (`AIDef_PrimalBeastAdult`) | yes | yes, after a roar | yes, and the fallback | perch, ice spike |
| Baby (`AIDef_PrimalBeastBaby`) | the grab | yes, as its backup attack | only the fallback | none |
| Slinger (`AIDef_PrimalBeast_Ranged`) | close melee only | **no** | yes, two throw nodes and the fallback | perch, ice spike |
| Badass (`AIDef_PrimalBeastBadass`) | close melee, radius attack, charge | yes, after a roar | yes, and the fallback | perch, ice spike |

The badass is outside the slice's range and is listed for completeness. The psycho is not a bullymong and is not covered.

## 2. What chooses between melee, leap and throw

### 2.1 The combat node of each kind, in priority order [data]

A priority node tries its children top to bottom every state update (every 0.2 s, `NATIVE_AI_MELEE.md`) and takes the first that can run;
all five combat nodes are interruptible (`bInterrupt` true), so at a state update a higher child that can run replaces the running action,
unless that action is in its atomic attack phase (section 5). Bands are the `AttackLocation_Range` limit of the action, in uu (section 2.3 says what they measure).

**Adult.**
1. Pick up object (combat perch): not before 10 s after the first target, then on a cooldown; only while not injured and while the ice-spike
   flag is false. Not described further here.
2. Throw ice spike: only while `Flag_ThrowIceSpike` is true (set by something outside these definitions; not read). Not described further.
3. Close melee: band 0 to 384, height limit 128, line of sight, may not path (`bCanPath` false). Melee is the plain swing.
4. A random node, weights **1.0 : 0.25 : 0.25**:
   - Melee: band 256 to 394, height 128, line of sight. The plain swing after an approach.
   - A sequence of **Roar then Leap**. Both gated on "not injured", both hold `AIResource_Leap` (at most **1** user), both carry the
     stop-when-ineligible flag (section 5). Leap band **1024 to 2048** with line of sight.
   - Throw rock: only while the target is in front (the target's `DotTo` attribute above 0); holds `AIResource_Throw` (at most **3** users);
     band **512 to 3072**, line of sight, and a move limit of 1024 (section 2.3).
5. Back up (throw rock): band 512 to 3000, line of sight, faces the target first.

**Regular.**
1. Perch and 2. ice spike as for the adult.
3. "Melee override": only while the target's horizontal distance is below **320**; runs the plain melee (band 0 to 320, line of sight, and
   a height limit of 128 that is optional on this action).
4. A random node, weights **1.0 : 0.5**: the same melee, or a **leap with no roar**, no injured gate and no resource (band 1024 to 2048).
5. Back up (throw rock): band 512 to 3000.

**Baby.** No random node and no perch.
1. Melee grab: band 0 to 480, height 128, full line of sight.
2. "Leap attack [backup attack]": band **480 to 2048**, line of sight.
3. Back up (throw rock): band 512 to 3000.

**Slinger.** No leap.
1. Perch and 2. ice spike, as for the adult.
3. Close melee: band 0 to 384.
4. Throw rock from high ground: band **1536 to 3072**, the throw spot at least **320 above** the target (`HeightRestriction_Above`), a
   condition tree that includes "target in front", the throw resource, and a move limit of 2048.
5. Throw rock: band 1536 to 3072, target in front, throw resource, move limit 2048.
6. Back up (throw rock): band 1536 to 3072, line of sight; the band is marked not required (`bRequired` false). What that changes is not
   established: by default an attack asks for all its limits, and an optional limit only drops out on a search path whose location choice was
   not decoded (section 2.3).

### 2.2 What that means in a fight (reading of 2.1 with the tree rules of `NATIVE_AI_MELEE.md` / `NATIVE_AI_AWARENESS.md`)

- **No distance trigger picks the leap or the throw directly; a weighted draw does.** For the adult and the regular, the random node draws a child
  by weight; a child that cannot run is removed and the draw repeats. While a drawn child runs, the random node re-evaluates only that child.
  So per draw: adult melee 4 in 6, roar-and-leap 1 in 6, throw 1 in 6 when all three can run; regular melee 2 in 3, leap 1 in 3. A drawn leap or
  throw that cannot run (wrong band, no line of sight, resource taken, target behind for the throw) falls back to the next draw. [data, record
  for the draw rule]
- **Close range usually wins.** Within 384 (adult, slinger) or 320 (regular), the close-melee child sits above the random node, so at the next
  state update it takes over, provided its own checks pass and the running action is not in its atomic attack phase (an atomic action is not
  re-evaluated, `NATIVE_AI_AWARENESS.md`, the state update). The baby's grab sits first. So leaps and throws are mainly what the pack does from
  the middle distance and while closing in.
- **The baby leaps only when it cannot grab.** Its leap is the second child, so it runs only when the grab cannot (no reachable spot within 480
  with full sight). Whether the grab action may walk in from any distance (an `Attack_Close` type) or only swings in place cannot be told from
  the data read: the action `Type` is not stored on these objects (L-4 read the adult's live types: close melee `Attack_InPlace`, melee
  `Attack_Close`, ice spike `Attack_Far`). Open.
- **The slinger throws from 1536 to 3072** and prefers a raised spot 320 above the player. Whether its fallback throw can also run inside
  1536 depends on the optional-limit path above (not established). It only swings when the player is within 384.
- **Cooldowns.** None of the leap, throw or melee nodes has a `TimedReset`, so there is no per-attack cooldown in the data. What spaces them is
  the clip length, the random draw, the leap resource (one leaper at a time, scope below) and the throw resource (three throwers). [data]
- **Gates on the target.** Every attack action refuses when there is no target, when pathing failed, or when the target is not reachable. A
  downed player is refused only when both the action's `bCanBeLimitedByDownedPlayers` and the game's
  `bEnemiesCannotDoCloseAttacksOnDownedPlayers` are set; neither value was read, so whether bullymongs leap or throw at a downed Maya is not
  established. The leap also refuses with `TooFar` when the target pawn is
  farther from the navigation mesh than the globals' `AIMinAttackDistance`, or vertically farther than `AIMinVertAttackDistance`, or outside the
  AI's combat volume (unless that volume only limits ranged attacks). Neither global is set in the globals object, so their values are the class
  defaults, not read. [script, data]
- **Injured.** An injured adult or badass neither roars, leaps nor perches (the flag gates). The regular's leap has no such gate. [data]

### 2.3 What the limits measure [native]

`WillowAIComponent.IsLocationValid` tests one candidate spot (where the attacker would stand) against the action's list of limits. Each limit
answers pass or fail; a failed limit marked required makes the spot invalid at once; a failed optional one only does not count. The attack's
script asks for all limits to pass unless the action says otherwise. A far attack (`Attack_Far`) also checks that the spot is on the
navigation grid (the exact grid test was not decoded).

- **Range:** the horizontal distance from the candidate spot to the target's location must lie between the minimum and the maximum. The maximum
  can be widened by an allowance tied to the target's size; when it applies and how large it is was not resolved (it may be none). For an in-place attack the candidate spot is the attacker's own location. L-4 and the den
  read support the band on two samples (an adult's first swing at 368 against 394, a baby's grab at 425 against 480, `COMBAT_RANGE.md`). [native,
  record]
- **Height:** the spot's height minus the target's. `Within`: the absolute difference at most the limit (128 for the melee actions). `Above`:
  at least the limit above the target (the slinger's 320). `Below`: at most the limit.
- **Line of sight:** `Keep` needs the target exposed to the spot (more than half exposed when the full-sight flag is set, read as
  `bFullLineOfSight`, which the baby's grab sets); `Avoid` is read as the reverse (not followed through). An `Arc` kind makes
  trajectory-based visibility checks using the limit's own speed and angle fraction; a continuous clearance test along the arc is not
  established. The leap limits carry such numbers (2500 or 2000 and 0.4, the baby 0.45), but whether their kind is `Arc` is not stored on
  them, so it is the class default, not read.
- **Move:** measured against the attacker's current combat area. It passes when the attacker holds no combat area, or when the candidate
  spot lies more than the limit's distance (horizontal) from the centre of the area it holds; it fails when the spot is within that distance,
  and also when the attacker itself cannot be resolved. The limits are 1024 on the adult's throw and 2048 on the slinger's. How combat areas
  are assigned, and what this does to where a thrower stands in a fight, were not read.

### 2.4 The leap and throw resources [data; scope open]

`AIResource_Leap` allows 1 user and `AIResource_Throw` 3. The AI component keeps a `ResourceData` list (per resource: who may run it and who
runs it now). Whether that list is per pawn, per den (a parent AI) or per map was **not settled**: the native registry was not found in this
run's budget. An implementer has to choose (section 5).

## 3. The leap, step by step

### 3.1 Timeline [script, data]

| Step | Adult and badass | Regular | Baby |
|---|---|---|---|
| Roar | `Roar_var1`, 3.33 s at play rate 1 (the rate is not set on the move; a private earlier read said 0.5, not found here) | none | none |
| Launch (wind-up) | `Attack_Leap_Start` 0.833 s; turns toward the target over half of it | the baby's clips | `Leap_Start` 0.833 s |
| Flight | in-air idle (`Attack_Leap_Idle`, looped) for the solver's flight time | baby idle | `Leap_Idle` |
| Landing | `Attack_Leap_End` 1.233 s; the landing hit at its start | baby landing | `Leap_End` 0.8 s |
| End | the action finishes after the landing clip | | |

- **Lifecycle as it would be seen:** the leaper winds up for the launch clip, turning toward the target during its first half, and leaves the
  ground when the launch clip ends. In the air it turns toward its launch direction during the first half of the flight and plays the in-air
  idle for the solver's flight time. On touching down it plays the landing clip, with the landing hit at the clip's start (L-4: the hit came
  in the `Landed` frame in 34 of 157 pairs, the rest within a few frames). The action ends with the landing clip. [script, record]
- **An interrupted leap** ends at once: interrupted during the wind-up, or with the target gone when take-off is due, there is no flight;
  interrupted in the air, there is no landing clip and so no landing hit. Whether touching down counts as such an interruption of the in-air
  idle (which would skip the landing clip) was not read; the L-4 landing hits say a normal landing does play it. [script, record]
- Before the solver runs, the script raises the aim point's height by the leaper's collision half-height minus the target's, so the bodies'
  bottoms meet (adult 90 against Maya's 80: +10). [script, data]
- **Check against L-4, not settled:** L-4 measured `Attack` to `Landed` 1.08 to 3.48 s, mean 1.68, n 157, kinds mixed [record]. Where the
  leap action raises `Attack` was not read (L-4 places it at take-off), so both cases:
  - `Attack` at take-off: the interval is the flight alone, 0.51 to 1.01 s for the adult and 0.63 to 1.27 s for the regular over the 1024 to
    2048 band (flat, no lead). All of the adult's range and most of the regular's lie below the observed minimum of 1.08 s.
  - `Attack` at the start of the launch clip: add 0.833 s, giving 1.34 to 1.85 s (adult) and 1.47 to 2.10 s (regular).

  Neither case is established, and with mixed kinds and unknown slopes no leap distance can be inferred from the observed spread. The event
  boundary is listed under "What a session should read".

### 3.2 The flight [native for the solver; data for the numbers]

The leap options per kind: adult speed **2500**, angle fraction **0.4**, lead cap 92; regular **2000**, **0.4**, lead cap **0** (no lead);
baby **2500**, **0.45**, lead cap **128**. `Error` is 0 on all of them; the offset is zero apart from the height lift above.

The trajectory at an actor, as the projectile solver does it:

1. **Aim point** = target location + the offset (turned by the thrower-side object's rotation in the projectile solver; `NATIVE_AI_AWARENESS.md`
   read the leap variant as turning it by the target's rotation; with a vertical-only offset both give the same point) + a random error (a
   random direction times a uniform amount up to `Error`; zero here).
2. **Launch angle** = the elevation of the line to the aim point, plus `AnglePct` times the angle between that line and straight up. On flat
   ground that is `AnglePct x 90` degrees: **36 degrees** for the 0.4 leaps, **40.5** for the baby. `Angle` is not used by this solver.
3. **Speed is fixed** at `Speed`, split into horizontal `Speed x cos(angle)` and vertical `Speed x sin(angle)`.
4. **Lead:** with the target's horizontal velocity, solve for the time at which a point moving with the target can be reached at the horizontal
   speed (the positive root of the usual intercept quadratic); aim at that predicted point. If the lead is longer than `PredictDistMax`, it is
   cut to that length along the target's velocity. No real root means no lead. A cap of 0 means no lead.
5. **Flight time** = horizontal distance to the final aim point / horizontal speed.
6. **Gravity is solved, not given:** the vertical acceleration that brings the arc to the aim point's height at that time,
   `g = 2 x (height difference - time x vertical speed) / time squared` (negative on a normal arc). The script hands it to the projectile, and
   the leap native to the pawn, as a custom gravity scale of `-g / 1000`.

On flat ground this gives a peak of `distance x tan(angle) / 4` whatever the speed. Worked values (flat, no lead):

| Arc | 1024 uu | 1600 uu | 2048 uu |
|---|---|---|---|
| Adult leap (2500, 36 deg) | 0.51 s, peak 186 | 0.79 s, peak 291 | 1.01 s, peak 372 |
| Regular leap (2000, 36 deg) | 0.63 s, peak 186 | 0.99 s, peak 291 | 1.27 s, peak 372 |
| Baby leap (2500, 40.5 deg) | 0.54 s, peak 219 | 0.84 s, peak 342 | 1.08 s, peak 437 |

The leap native itself (`StartLeapAtActor`) reaches its solver through a virtual call that this run did not resolve; that it uses the same
solver is `NATIVE_AI_AWARENESS.md`'s reading, and the table assumes it. The leap's in-place `StartLeap` clamps the angle to 85 degrees and
non-positive values to 1 (`NATIVE_AI_AWARENESS.md`).

### 3.3 The landing hit [data, record]

At the start of the landing clip, two behaviours run: a `Behavior_MeleeAttack` with `MeleeDef_PrimalBeast_RadiusLeap` and a camera-shake-only
explosion (radius 2048 for the adult's clip, 512 for the baby's; L-4 saw this as an extra hit with damage 0 and the grenade damage type, 80
times). The melee volume is a full circle of radius **320** around a point **200 ahead** of the leaper, `DamageHeight` 192 (the vertical test
of that definition was not measured; the plain melee's measured band is in `NATIVE_AI_MELEE.md`). Damage: the same attribute formula as the plain
melee (`Init_EnemyNonWeaponDamage`, scale 1; `COMBAT_MELEE.md` reproduces an adult 5's 73.6974). Knockback 1.0, so a horizontal momentum of
**100,000** away from the leaper (seen live on the regular's leap, L-4). One behaviour on hitting an enemy plays a camera animation on the player.

L-4 saw 157 landing calls in a long fight and 52 of them damaged the player [record]: a landing often misses. In the den-down run of 2026-11-12
the first hit on Maya was this landing from an adult placed 1,601 uu away, about 2 s after she was placed, and each hit pushed her about 500 uu
[den, one run].

## 4. The rock throw, step by step

### 4.1 Timeline [data; the time scale is a reading]

The throw nodes play a random pick from a set (weights equal):

| Set (who) | Clips | Rock appears in hand | Release | Clip ends |
|---|---|---|---|---|
| `AnimRandom_PrimalBeast_Throw` (adult, badass, and every kind's back-up throw) | `Attack_ThrowRock_var1`, plain and mirrored, two variants each, at play rate 1.25 | 0.384 s | **1.064 s** | 1.733 s |
| `AnimRandom_Slinger_HiGround` (both slinger throw nodes) | two one-rock throws as above; one two-rock throw (`Attack_ThrowRock_var4`, 3.067 s clip at 1.25) | 0.32 s and 1.30 s | **0.68 s and 1.66 s** | 2.453 s |

The timed events are stored in clip seconds (0.48 and 1.33 on the one-rock throw); the table divides them by the play rate. That reading
comes from the plain swing: its hit event at 0.50 clip seconds at play rate 1.25 lands 0.40 s in, and L-4 measured 0.400 to 0.425 s from the
`Attack` trigger to the hit. UNVERIFIED for the throw. The ice spike is a separate move and is not described here.

What happens at the events: at the first one the rock is shown in the hand (a body switch, plus particles); at the release the hand rock is
hidden and the thrown rock is spawned at the hand socket (`R_Hand_Rear` or `L_Hand_Rear`, the front sockets on the two-rock throw). If the
action stops after the grab, the held rock is dropped and hidden. [data]

### 4.2 The flight [script, native, data]

- The rock is `Projectile_IceRock` (`WillowServerSideProjectile`, at most 20 s alive); the bedrock throw variant uses
  `Projectile_AshRock`. [data]
- Each release has **two** throw behaviours behind a float comparison: one leads the target (cap **480** on the adult's clips, **512** on the
  slinger's), the other does not lead (cap 0). What the comparison compares was not decoded, so how often a rock leads is open. Speed **2000**,
  angle fraction **0.3** (27 degrees on flat ground), no error, no offset. [data]
- The arc is section 3.2's solver: fixed speed 2000, gravity solved for the aim point, custom gravity scale `-g / 1000`. Flat, no lead:
  1,024 uu in 0.57 s (peak 130), 1,600 uu in 0.90 s (204), 2,048 uu in 1.15 s (261), 3,000 uu in 1.68 s (382). [native, script]
- Spawn level: the thrower's experience level. Rocks per behaviour: `NumProjectiles`, not set on these behaviours, so the class default
  (not read); live rocks show it is at least one. [script, data]

### 4.3 The rock's hit [data, record]

On hit the rock's `Behavior_CauseDamage` damages what it touched: damage from the rock's `ProjectileDamage` attribute, which the throw sets from
`Init_EnemyNonWeaponDamage` at scale 1; momentum constant **50,000**; damage type `DmgType_Normal`; damage source `WillowDmgSource_Grenade`
(`COMBAT_MELEE.md`). A radius part was not found in the data read. Live, a slinger's rock at level 5 arrived as 44.218 (`COMBAT_GOLDEN_TRACE.md`),
the same closed form as the melee with the slinger's multiplier. [record for that number]

## 5. What interrupts a leap or a throw

- **The tree.** At each state update (every 0.2 s) a running action that is not in an atomic phase is asked again whether it can run, and it
  stops when its own check fails (no target, pathing failed, not reachable). Nodes that carry the stop-when-ineligible flag
  (`bStopIfCannotRun`) also stop when their node gates fail (the injured flag, the target-in-front condition, the resource). In the data that
  flag is on: the adult's and the badass's roar, leap and main throw; the slinger's two throw nodes; the badass's charge; every kind's ice
  spike except the baby's, which has none. It is **off** on the regular's leap, the baby's leap, every back-up throw and every melee. A
  higher-priority child that becomes eligible (close melee when the player walks in) also replaces a non-atomic running action. [data, record
  for the update rule]
- **Once the attack has started** the attack state marks itself atomic, which the tree treats as not interruptible
  (`NATIVE_AI_AWARENESS.md`, the state update's "running and non-interruptible" case). For the throw that covers the throw clip. For the leap the
  attack state's own code was not read, so whether the launch and the flight are atomic is open; once airborne, physics carries the pawn
  regardless. [script]
- **The leap's own exits:** section 3.1, "An interrupted leap".
- **Holds** (Phaselock's lift is one) stop the running action (`NATIVE_AI_MELEE.md`, cadence). What a hold does to a pawn in mid-air was not read.
- **A downed player:** only under the two flags of 2.2, not established. The attacker's death ends it.

## Where the den captures agree and where they do not

- [record, `COMBAT_RANGE.md`, the 2026-11-07 den read, three trials] Most first blows were not the plain swing: a slinger's rock from 800 uu
  and more about 2.8 s after it took its target, a baby's leap at 3.9 s, a baby's grab at 4.1 s, an adult's first hit with no melee call at
  6.7 s. The slinger's rock at about 3 s fits a throw chosen at once (release 1.06 s into the clip, about a second of flight, plus turning or
  moving to a spot). Its distance (800 uu and more) is **below** the slinger's 1536 band. Two readings, neither established: the band was
  met from a spot chosen before the player moved, or the back-up throw's optional band dropped out (section 2.1, slinger item 6). Open.
- [record, L-4; den, one run] Twice an adult went straight into the leap: L-4 saw the patrol stop and the **leap** start in the acquiring
  frame, and in the den-down run an adult 1,601 uu away landed on Maya about 2 s after she was placed, which leaves no room for the 3.33 s
  roar. One reading the tree rules offer for L-4: a sequence advances when a child finishes and goes back to its first child only after a
  failure or after its last child (`NATIVE_AI_AWARENESS.md`, completion), so a roar that finished before the target was lost can leave the
  sequence on the leap for the next acquisition. That does not explain the den-down adult, which took a freshly placed Maya. An implementer
  should not make the roar mandatory before every leap until a read shows when it plays. Open.

## What an implementer needs to decide

The host range has a hand-written brain (`OpenWillowEnemyAI.cpp`), not the stock tree, so the leap and throw need these stand-ins:

1. **The chooser.** Either an explicit tree per kind (section 2.1, with the random weights and the fall-through redraw), or a smaller rule that
   reproduces it: close melee inside the kind's close band; otherwise one weighted draw per finished attack (adult 4:1:1, regular 2:1), skipping
   a drawn attack that cannot run. Mark either as a host stand-in for the stock tree.
2. **Where an attack may start.** Use each band as the horizontal distance from where the enemy stands to Maya, with line of sight. The
   target-size allowance on the maximum is unresolved: none, or Maya's radius 42, are the plain stand-ins (UNVERIFIED either way). Decide
   whether a leaper or thrower walks into its band first (the `Attack_Far` / `Attack_Close` types are not in the data) and whether the
   throw's move limit is modelled at all (the range has no combat areas; ignoring it is the simple choice).
3. **The resource scope.** One leaper at a time: per den is the reading that fits a pack attacking one player; per map is the safe default if a
   choice is needed now. Three throwers likewise.
4. **The roar.** Mandatory before the adult's leap per the data, contradicted by two observations; the simpler stand-in is to skip it, logged.
5. **The arcs.** Implement section 3.2's solver once and use it for both leaps and rocks; the lead needs Maya's horizontal velocity at the moment
   of take-off or release.
6. **The hits.** Leap: section 3.3's volume at the landing, damage as the melee rows, 100,000 knockback. Rock: a touch hit on Maya, the melee rows'
   damage formula, 50,000 knockback. The host has no knockback on Maya today (`COMBAT_RANGE.md`, "Not modelled"); without it the den-down's
   "pushed out of reach" effect will not appear.
7. **Timing.** The clip times in sections 3.1 and 4.1 at their play rates; the event-time scaling of 4.1 is a reading.

## What a session should read

1. **Per attack, what started and when:** for each bullymong after it takes its target, the tree leaf that starts (`Start` on the action, as
   L-4 already hooks), its distance to the player at that moment, and for a leap the `StartLeapAtActor` return value, `LeapStartVelocity` and
   `LeapGravity`; for a throw the rock's spawn (`Behavior_AIThrowProjectileAtTarget` apply) and its velocity. That settles the bands, the draw
   frequencies over many samples, the roar question and the solver numbers in one capture. For the leap, also the frame of the action's
   `Attack` trigger against the end of the launch clip and the take-off, which settles the event boundary of section 3.1.
2. **The rock's hit:** the `TakeDamage` on the player from a `Projectile_IceRock` (damage, momentum, damage type), and the time from release to
   impact against the distance.
3. **The resource scope:** two adults of one den and one of another den, all with leaps available: whether two leap at once.

## Open

- The resource registry's scope (2.4); the comparison that picks a leading or a non-leading rock (4.2); the action `Type` of the baby's grab,
  the regular's melee and the throw actions (2.2); the line-of-sight kind on the leap limits (2.3); the globals `AIMinAttackDistance` and
  `AIMinVertAttackDistance` (class defaults); whether the leap's attack state is atomic (5); the roar's play rate.
- The leap native's own solver call was not resolved (3.2); the table assumes the projectile solver.
- The navigation-grid test for far attacks and the target-size allowance in the range limit (2.3).
- Where the leap raises `Attack` (launch start or take-off), and so the meaning of L-4's interval (3.1).
- What the optional-limit path changes for the slinger's back-up throw (2.1, 2.3).
- The perch and the ice spike were only placed in the order; their timings and throws are not described here. The badass is listed but not
  detailed.
- Everything above is UNVERIFIED in the running game except the lines tagged [record] or [den] that say what was seen.
