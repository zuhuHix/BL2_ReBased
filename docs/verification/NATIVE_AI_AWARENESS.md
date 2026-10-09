# Native AI awareness: Willow AI component overrides, mind awareness, the melee attacker interface and node completion (draft, 2026-10-09)

AI-assisted (Claude), analyst lane G26. Read from the game executable with Ghidra (tools/ghidra/, driven by `tools/ghidra/run.ps1`), written in
our own words; no decompiler output, pseudo-code or listing structure is reproduced here. Script facts (names, parameter lists, enum values, who
calls what) come from the cooked packages (`research/script_disasm.py`, `tools/ghidra/class_layout.py`). **Every rule is UNVERIFIED in the running
game** unless a line says how it was confirmed; each section ends with the live observation that would confirm it. Purpose: plan step S-h
groundwork. This note continues [NATIVE_AI_MELEE.md](NATIVE_AI_MELEE.md) (lane G25), whose "Not read yet" list it works through; it does not repeat what
that note already says.

**Live status (lane L9, 2026-10-09).** The running game was observed for the notice rules, the threat level over time, `ValidTarget`, `ValidCurrentTarget`, `TargetChanged`, the attack route, the melee sweep and the tree timers. Counts and conclusions are in [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md) (sections 0 to 8 hold the observations, section 11 the corrections). Where this note and the game disagree, the game wins, and the paragraphs below say so and carry the matching number. Main changes: a bullymong spawned by a population den is judged on the den (the den is its parent, and the den gives the threat origin and radii); the den's record is the one stamped; a fresh record shows level 1; and the level does fall back after `ForgetTime`, so the earlier "no reader" statement is refuted. Rules L9 did not exercise (the cone boundary near zero, a bullymong without a den, the `LoopCountMax` cap, the 1.0 s failure block, `Finish`, the leaps, the chase natives, the awareness zones, the hold natives) remain UNVERIFIED.

How the classes were found (tooling note, no addresses): the Willow classes have no registration pattern the query script understands, so
their function tables were located by comparing the table of the already resolved base `AIComponent` with every table in the read-only data
(the Willow table shares nearly all slots), and the result was confirmed from the class registration code (its constructor stores exactly that
table). The same was done for `GearboxMind` / `WillowMind`, `WillowPawn` / `WillowAIPawn` and their `IMeleeAttacker` sub-tables. Which slot is which
script function came from the exec thunks (each thunk calls one slot). `tools/ghidra/class_layout.py` fails on `GearboxMind` because of its one
map property; treating a map as 60 bytes reproduces the registered class sizes of both mind classes exactly, so it was used for field names.

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| WillowAIComponent.GetTargets / GetTargetsFromParents / AddDenTargets | `native function GetTargets()`, `GetTargetsFromParents()`, `AddDenTargets(PopulationOpportunityDen Den)` | S-h: where a bullymong's candidate list comes from (den members) | medium | UNVERIFIED; lone case refuted for den bullymongs (L9) |
| AIComponent.FindTargets (hidden-owner condition resolved) | `native function FindTargets()` | S-h: who is a candidate | medium | UNVERIFIED |
| WillowAIComponent.ValidTarget (base and Willow part) | `native function bool ValidTarget(Actor Target)` | S-h: candidate filter | medium | UNVERIFIED; six answers live (L9) |
| WillowAIComponent.ValidCurrentTarget | `native function bool ValidCurrentTarget(MindTargetInfo TargetRec)` | S-h: keep and acquire gate (threat level) | high | UNVERIFIED; threat gate live (L9) |
| WillowAIComponent.InitializeTargetRecord / TargetRemoved | `native function InitializeTargetRecord(MindTargetInfo)`, `TargetRemoved(MindTargetInfo)` | S-h: record start state, hand-off to a vehicle or occupant | medium | UNVERIFIED; start level corrected (L9) |
| WillowAIComponent.TargetChanged | `native function TargetChanged(Actor OldTarget, Actor NewTarget)` | S-h: head look, provoked flag, targeting counters | medium (counters, flag), low (presentation calls) | UNVERIFIED; flag, look-at and counters live (L9) |
| WillowAIComponent.NotifyAttackedBy / SetThreatLevel / GetThreatLevel / GetHighestThreatLevel | `native function NotifyAttackedBy(Actor Attacker)`, `SetThreatLevel(Actor Target, EThreatLevel NewThreat)`, `GetThreatLevel(Actor Target)`, `GetHighestThreatLevel()` | S-h: how a bullymong notices and stays on Maya | high (formulas), medium (den helper) | UNVERIFIED; stamps and fall-back live (L9) |
| WillowTI_CalcThreat (refinement of the G25 reading) | C++ virtual, no script name | S-h: the "notice" rule when nobody attacked | medium | UNVERIFIED; both rules live (L9) |
| Leaders and forced target: HasForcedTarget / GetForcedTarget / Get/SetTargetLeader / Get/SetFormationLeader | `native function` | S-h: only for groups; kept short | medium | UNVERIFIED |
| WillowAIComponent combat zones (Claim / Unclaim / Available / ByLocation / MarkCurrent / Invalidate) | `native function` on a `CombatZone` struct | not for the bullymong (ranged AI) | medium | UNVERIFIED |
| WillowAIComponent.Hold / Release / OnHold / ClearState / head look and phase lock holds | `native function Hold(Name HoldName, Object Tag)` etc. | S-h and Phaselock: how holds are stored | medium | UNVERIFIED |
| WillowMind.NotifyAttackedBy | `native function NotifyAttackedBy(Actor Attacker, bool bDamaged)` | S-h: what a hit on a bullymong does | high | UNVERIFIED; call live, field writes not read (L9) |
| WillowMind.IsAwareOfTarget | `native function bool IsAwareOfTarget(Actor PotentialTarget)` | S-h/S-e: "unaware" damage bonus | medium | UNVERIFIED; fresh-record True live (L9) |
| ISeeTargetable.AddTargetable (WillowMind) | `native function bool AddTargetable(Actor NewTarget, bool bSeeTarget, bool bHearThreat, bool bSmellTarget)` | low | medium | UNVERIFIED |
| WillowMind.NotifyTakeHit | `native function NotifyTakeHit(Controller InstigatedBy, Pawn HitPawn, Vector HitLocation, float Damage, class DamageType, Vector Momentum, optional DamagePipeline Pipeline)` | low (reaction plumbing) | low | UNVERIFIED |
| TargetIteratorCheckAwareness and the other iterators found (PrioritizeHealth, PrioritizeOpinion, CalcCombat) | C++ virtuals | not used by the bullymong's targeting definition | medium (awareness), high (the two prioritizers) | UNVERIFIED; zone collections None live (L9) |
| IMeleeAttacker on WillowPawn / WillowAIPawn | ten interface methods | S-h/S-e: melee origin, hit list | high | UNVERIFIED; trace source live (L9) |
| Melee over time (MeleeAttack entry, BeginMeleeOverTime / EndMeleeOverTime, the per-frame tick) | `native function BeginMeleeOverTime(MeleeDefinition)` / `EndMeleeOverTime()` | S-h: charge and grab; repeated hits | high | UNVERIFIED; charge run live (L9) |
| AI tree natives: eligibility, activation, completion (loops, TimedReset), interruption, ActionSequence.Finish | no script names except `ActionSequence.Finish(optional bool bFailed)` | S-h: which action runs and when it can repeat | high | UNVERIFIED; timers and counters live (L9) |
| WillowAIPawn.StartLeapAtActor / StartLeap / GetLeapRotation | `native function float StartLeapAtActor(TrajectoryData Options, Actor Target)`, `StartLeap(Vector StartLoc, Vector EndLoc, float LeapSpeed, float LeapAngle)` | S-h (stretch): the bullymong leap | medium (StartLeap), low (StartLeapAtActor numerics) | UNVERIFIED |

## What the script side calls (grep of the package listings)

- `WillowAIPawn.TakeDamage` (after the normal damage chain): when the pawn has a mind, the attacker is the damage causer's instigator, else the controller's pawn
  of `InstigatedBy`; if there is one, `MyWillowMind.NotifyAttackedBy(attackerPawn, true)`. This is the normal route by which Maya shooting a bullymong provokes it.
- `Behavior_AIProvoke`, the Kismet provoke action (`WillowMind.OnAIProvoke`, `PopulationOpportunityDen.OnAIProvoke`), `DeathtrapActionSkill.OnActionSkillStarted`,
  `Behavior_AIPriority` and `GearboxMind.OnTargetPriority`: `AIComponent.NotifyAttackedBy(someActor)` (provoke, taunt, priority bump).
- `Behavior_AITargeting`: `AIComponent.ForceUpdateTargets()`. `WillowProjectile.ShowSelfAsTarget`: `ISeeTargetable.AddTargetable(Self, true, false, false)`.
- `Action_AnimAttack.HoldTarget`: when `GetThreatLevel(Target)` is Provoked it takes the named target hold `Attack`; `ReleaseTarget` gives it back.
- `Action_Patrol.SetMoveNode` clears `WillowMind.bProvoked`. `WillowInteractiveObject.TakeDamage` calls `NotifyTakeHit`. Nothing in script calls
  `SetThreatLevel`, `IsAwareOfTarget` or `GetHighestThreatLevel`; those are native-to-native (the damage pipeline asks `IsAwareOfTarget`).

## Threat level, the field that gates everything

`WillowAIComponent.EThreatLevel` (package enum): `ETL_NoThreat` 0, `ETL_InThreatRadius` 1, `ETL_SeenInThreatRadius` 2, `ETL_Provoked` 3.
`WillowMindTargetInfo.ThreatLevel` is a byte holding one of these. `TargetExposure`: `Exposure_None` 0, `Exposure_Half` 1, `Exposure_Full` 2. The opinion enum is the
one in NATIVE_COMBAT_GROUNDWORK (Enemy 0, Neutral 1, Friendly 2). Everything below uses these names.

## WillowAIComponent: GetTargets, GetTargetsFromParents, AddDenTargets

- **Signature:** see the summary. `AddDenTargets` takes a `PopulationOpportunityDen`.
- **Reads:** the parent AI component (via the owner chain of the AI interface), the parent's owner when that is a den (`PopulationOpportunityDen`), the den's
  `bPatrolDen`, `SharedThreatDens` and `SpawnedDenMembers`, and the component's `AIDef` (`WillowAIDefinition.bGetTargetsFromParents`).
- **Does (GetTargets override):** runs the base rule first (a component with a parent inherits the parent's targets, otherwise it runs `FindTargets`). Then, if
  the parent component is a Willow component owned by a den that has `bPatrolDen` set, it also runs `FindTargets` on itself, so members of a patrol den search
  for their own targets in addition to what they inherit. Finally, if the AI definition has `bGetTargetsFromParents`, it runs `GetTargetsFromParents`.
- **GetTargetsFromParents:** finds the parent component's owning den (nothing happens when there is none), calls `AddDenTargets` for that den, then for each
  den in its `SharedThreatDens`.
- **AddDenTargets(Den):** calls `AddTarget` for every pawn in the den's `SpawnedDenMembers`. `AddTarget` applies `ValidTarget`, so same-allegiance members are
  normally rejected; the purpose of adding den members is not established (see Open).
- **Implementer checklist:** for a lone bullymong (no den, no parent) only the base behaviour matters: `FindTargets` is the only source of candidates.
- **Open:** why a den adds its own members. Whether a bullymong has a den parent is answered for Three Horns Divide (it does, lane L9); Sanctuary was not checked.
- **Live confirmation (lane L9, 2026-10-09):** refuted for bullymongs spawned by a den. `GetParentComponent()` is the den's component (a population den, not a patrol den) and `bGetTargetsFromParents` is false, in about 25 pawns of three dens. The lone case (no den) was not seen, so the lone-bullymong checklist above stays UNVERIFIED. See sections 0 and 8 of [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md).

## AIComponent.FindTargets: the hidden-owner condition (resolves a G25 open item)

G25 recorded a skip it could not decode. It is this: the world keeps one registry of every targetable (pairs of object and targetable interface). `FindTargets` walks it
and, for each entry, requires in order: the entry is valid and its interface says it can be targeted at all; the interface yields an actor; **if this AI has an
allegiance object, the AI's opinion of the entry's allegiance must not be Neutral** (a Neutral entry is skipped; an AI with no allegiance skips nothing here);
the horizontal distance (X and Y only) to the actor is within the search radius; `ValidTarget(actor)`; then `AddTarget(actor)`. The radius is `TargetingDefinition.SearchRadius`
when `bOverrideSearchRadius` is set, else `AIDefinition.TargetSearchRadius` (confirmed again here; the same value is the "threat radius" used by `WillowTI_CalcThreat` when there is no non-patrol den).
- **Implementer checklist:** iterate every registered targetable; the Neutral skip depends on the AI having an allegiance; the radius is 2D.
- **Live confirmation:** with Maya's allegiance Neutral to a test mob and the mob given an allegiance, `AIComponent.Targets` must never contain her; clear the mob's allegiance and she must appear.

## AIComponent.ValidTarget (base) and WillowAIComponent.ValidTarget (override)

- **Signature:** `native function bool ValidTarget(Actor Target)`.
- **Reads:** the AI interface and its allegiance, the actor, `TargetingDefinition.bCanTargetFriendlies` / `CanTargetFriendliesIf`, the world's targetable registry, the
  global "no target all enemies" switch (the `GearboxMind` static set by `SetNoTargetAllEnemies`), the target's driven vehicle and (Willow) vehicle seats, the
  target's controller, its cloak flag, the den's leader data.
- **Does, base rule, in order (first failure returns false):**
  1. the AI interface is bound (it binds it on demand from the owner) and the actor is not None;
  2. if the actor reports a controller whose pawn is set, the controller's pawn replaces the actor for the rest of the test;
  3. the global "no target all enemies" switch must be off (the console cheat makes every actor invalid);
  4. the actor's own "can be targeted" test is true and the component has an allegiance (an AI with no allegiance never has a valid target);
  5. the actor is not the AI's own pawn;
  6. **friendliness:** if the AI's pawn says the actor is **Friendly** (opinion Friendly, not "not an enemy": Neutral actors pass this step), the actor is valid only if
     `bCanTargetFriendlies` is set and, when `CanTargetFriendliesIf` exists, it evaluates true for the actor;
  7. the AI's own pawn must be a registered, targetable pawn with an allegiance (it must itself be found in the world registry), and the candidate must not be a pawn
     that is driving a vehicle (a pawn with a driven vehicle is not a direct target; its vehicle is);
  8. a last exclusion based on the `Base` (attachment) chains of the AI's controlled object and of the candidate. It starts from a query that is a plain "return None" stub for pawns, so for a normal AI pawn this
     step accepts the candidate at once (UNVERIFIED reading).
- **Does, Willow override:** the base rule must pass, then:
  - a **vehicle** is valid only if at least one of its seats has an occupant;
  - a **pawn without a controller** is invalid (unpossessed or controller-less pawns are never targets); a non-pawn actor skips this;
  - a vehicle-weapon seat pawn (`WillowWeaponPawn`) is never a target (its vehicle is);
  - **group leader rule:** if the root component has a forced target (see leaders) and that forced target is not the candidate, then a component that is not the group's
    target leader and has a parent is not allowed to take any other target;
  - the pawn must not be **cloaked** (`WillowPawn.bCloaked`); a non-pawn actor passes.
- **Edge cases:** a bullymong therefore ignores a cloaked Maya (not relevant to the base game: Zer0 cloak) and an actor that has lost its controller (a dead or downed player whose pawn is no
  longer possessed) as soon as the next removal pass runs `ShouldRemoveTarget` (which is "ValidTarget fails now, or a parent exists and does not know the target").
- **Implementer checklist:** the order above; the friendliness test uses opinion Friendly (not "not Enemy"); no allegiance on the AI means no valid target; the search radius is not part of
  `ValidTarget` (a target that walks out of the radius stays valid and keeps its record; only `StopTargetingIf` or invalidity drops it).
- **Live confirmation (lane L9, 2026-10-09):** confirmed for six answers on a regular bullymong: Maya true; a null false; the pawn itself false; another bullymong of its own allegiance false; Maya with `bCloaked` set false (true again once cleared); the pawn's allegiance cleared (memory only, one call, restored) gives false for Maya, which confirms the "no allegiance, no valid target" rule once. Not tried: a downed Maya, a friendly NPC (none on that map), a vehicle, and `bCanTargetFriendlies`. Section 4 of [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md).

## WillowAIComponent.ValidCurrentTarget (the real acquire/keep gate)

- **Signature:** `native function bool ValidCurrentTarget(MindTargetInfo TargetRec)`.
- **Does:** the base rule (CanTargetIf to acquire, StopTargetingIf to drop, scripted target, as in G25) must pass, **and** the record's actor must not be `bHidden`, **and** `GetThreatLevel(actor)` must be at least
  `ETL_SeenInThreatRadius` (2).
- **Why this matters:** G25 listed the base rule only. Without threat level 2 or 3 a record is never chosen as `CurrentTarget`, so a bullymong that has Maya in `Targets` does nothing
  until it has noticed her (see `WillowTI_CalcThreat`) or been attacked or provoked.
- **Implementer checklist:** keep and acquire both require threat level >= 2; there is no decay (see "Threat level storage").
- **Live confirmation (lane L9, 2026-10-09):** confirmed. For minutes at level 1 (records present, exposure Full but out of the cone or the radius) the pawn had no `CurrentTarget`; the target was set in the same update as the level-2 stamp; it was kept for 10.8 s with Maya 6 km away and dropped 0.13 s after the level fell. The `bHidden` clause was not tested. Sections 2 and 10 of [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md).

## InitializeTargetRecord, TargetRemoved

- **InitializeTargetRecord(record):** base is a no-op. Willow: for a `WillowMindTargetInfo`, store the target's targetable interface in `Targetable` and set `ThreatLevel` to `ETL_NoThreat`. **Live correction (lane L9):** the first level visible on a fresh record is `ETL_InThreatRadius` (1); `GetThreatLevel` answers 0 only when there is no record, and `TargetExposureToMe` reads -1.0 on the first update of a fresh record. The code writes 0 and the game shows 1; the reason was not separated. Section 2 of [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md).
- **TargetRemoved(record):** base is a no-op. Willow, for a Willow record, picks a replacement actor and hands the threat over:
  - if the removed actor was a pawn that was driving a vehicle (or sitting in a vehicle weapon seat): the replacement is that vehicle;
  - if the removed actor was a vehicle without a driver: the replacement is its first seat's occupant pawn, else that seat's last occupant;
  - otherwise, if the record has a `Targetable` interface, the replacement is the actor that interface yields.
  If the replacement exists and is not `bDeleteMe`: `AddTarget(replacement)`, copy `ForgetTime` and `ThreatLevel` from the old record to the replacement's record
  (looked up through the root component), and if the removed actor was the `CurrentTarget`, make the replacement the `CurrentTarget` with `SetTarget`.
- **Implementer checklist:** the hand-off matters when a player leaves a vehicle; for the slice (Maya on foot) it never runs. Keep the hook but it can be a no-op.
- **Live confirmation:** not needed for the slice; if wanted, enter and leave a vehicle near a mob and watch `Targets`.

## WillowAIComponent.TargetChanged

- **Signature:** `native function TargetChanged(Actor OldTarget, Actor NewTarget)`.
- **Does (in order):** the base rule (G25: from no target to a target re-arms every node's timed start); then, when the owner is a `WillowMind` with a pawn:
  - acquiring from no target: the mind's `bProvoked` is set, `ProvokedLocation` is set to the pawn's current location (this is the "threat origin" `WillowTI_CalcThreat` uses while provoked),
    and two notifications are sent to the pawn (an "engaged" call and a "target acquired" call carrying the new target);
  - losing the last target: one notification to the pawn;
  - unless the component is on hold, the pawn gets the script event `SetLookAtTarget(NewTarget, zero offset)` (head look);
  - counters: the old target's targeting counter is decremented (`WillowPawn.NumAICurrentlyTargeting` for a pawn, a similar counter on an interactive object), the new one's is incremented;
    these counters are what `WillowTI_PrioritizeIdle` reads;
  - the old and new targets are told that this AI stopped / started targeting them (separate calls for a player pawn, a vehicle and an AI pawn);
  - presentation: on acquisition and on loss the code raises dialog / audio / stat events depending on whether the target is a player (not decoded; the player-only parts are the "enemy noticed you" cues).
- **Calls into script:** the pawn event `SetLookAtTarget`.
- **Constants:** none.
- **Implementer checklist:** set `bProvoked` and `ProvokedLocation` on the first acquisition; keep the counters in step (decrement old, increment new, including to/from None); send `SetLookAtTarget` unless held.
- **Open:** the three pawn notifications and the presentation calls.
- **Live confirmation (lane L9, 2026-10-09):** confirmed for the flag, the location, the look-at call and the counters. On acquisition `bProvoked` went False to True, `ProvokedLocation` became the pawn's location in that frame, `SetLookAtTarget(player, zero offset)` was called in that frame, and Maya's `NumAICurrentlyTargeting` rose by one per AI and fell to 0 when the targets were lost. The flag is not a record of "has been noticed": den-spawned pawns could already have it true before anything noticed Maya. The three pawn notifications and the presentation calls were not visible. Section 5 of [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md).

## NotifyAttackedBy, SetThreatLevel, GetThreatLevel, GetHighestThreatLevel

- **NotifyAttackedBy(Attacker) (component):** base: if the attacker is not None and the AI is bound: when there is a parent component, forward the call to it, otherwise `AddTarget(Attacker)`.
  Willow: run the base rule, then `SetThreatLevel(Attacker, ETL_Provoked)`.
- **SetThreatLevel(Target, NewThreat):**
  1. if a parent component exists, forward the call to it (so the root holds the truth);
  2. at the root: find the record for `Target` (no creation); no record means nothing happens;
  3. den alert helper, **before** the record changes: if `NewThreat` >= 2 and the highest threat over this component's records is < 2, and the component's owner is a den, call the den's
     `TriggerProvokedEvents` on that den and on every den in its `SharedThreatDens`;
  4. `record.ThreatLevel` becomes the larger of its old value and `NewThreat` (it can only rise);
  5. if `NewThreat` >= 2 and the world exists and `record.AggroTime` is still (almost) zero, `AggroTime` is set to now;
  6. if `NewThreat` is `ETL_SeenInThreatRadius`: `ForgetTime = max(ForgetTime, now + d)` with d = `WillowAIDenDefinition.MinEngagementTime` when the AI definition is a den definition, else **2.5 s** (not seen for a den-spawned bullymong; the den value 10.0 was, lane L9);
     if it is `ETL_Provoked`: the same with `MinProvokedTime` or **10.0 s** (not seen; the den value 25.0 was, lane L9). (Levels 0 and 1 do nothing more.)
- **GetThreatLevel(Target):** walks up the parent chain starting at this component. A component that has an allegiance and whose pawn regards `Target` as Friendly answers **2** (friendlies always count as
  "seen"). When the walk reaches the root without that, it returns the root record's `ThreatLevel` for `Target`, or 0 when there is no record.
- **GetHighestThreatLevel():** the largest `ThreatLevel` over this component's own Willow records (0 when none).
- **Threat level storage:** `ThreatLevel` is raised through `SetThreatLevel`; `WillowTI_CalcThreat` re-reads it every target update. `ForgetTime` is written here and copied in `TargetRemoved`. **Live correction (lane L9):** the level does fall. After `now` passes `ForgetTime`, the den's next update lowers it: to 1 in place while Maya is inside the list radius, or the record is removed when she is beyond it (0.03 to 0.92 s after `ForgetTime` in three runs). While the notice condition holds, `ForgetTime` is refreshed at every member update, so a level lasts as long as the refresh does. The "no reader" statement of the earlier draft is refuted; the code that reads `ForgetTime` was not located by the search in this note.
  or in the WillowGame native code that was searched, so nothing was found that lowers a threat level. A bullymong provoked once stays provoked for as long as it keeps the record (UNVERIFIED; a reader may exist elsewhere).
- **Calls into script:** none (the den helper calls a den native).
- **Constants:** 2.5 s and 10.0 s defaults (not seen for a den-spawned bullymong, where the den values 10.0 and 25.0 apply); levels as above.
- **Implementer checklist:** Maya hits bullymong: `AddTarget(Maya)` (if valid) then `SetThreatLevel(Maya, 3)`: `ThreatLevel` 3, `AggroTime` now, `ForgetTime` now + 25 on the den's record for a den-spawned bullymong (lane L9); next target update the record can become `CurrentTarget`.
- **Live confirmation (lane L9, 2026-10-09):** confirmed on the den's (root) record, not on the pawn's. A real shot 1,200 uu from Maya gave level 3, `AggroTime` the frame's world time and `ForgetTime` AggroTime + 25.0 (the den definition's provoked time); a level-2 notice gave + 10.0. The members' copies keep `AggroTime` and `ForgetTime` at 0.0 and copy only the level. The fall after the window is confirmed (storage paragraph above). Not tried: a second hit inside the window. Sections 2 and 3 of [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md).

## WillowTI_CalcThreat (refinement of the G25 note)

G25 summarised this iterator; with the threat enum known its behaviour is: for a Willow record whose component is Willow, first `record.ThreatLevel = GetThreatLevel(target)`. Then, only if the mind has a pawn and an AI
definition and **the AI's pawn regards the target as an Enemy**, it may raise the level to 2 through `SetThreatLevel(target, 2)` when either of these holds (the mind's den and flags decide the origin and radii):
- **Gate:** the mind's pawn must have its collision component (the check sits before both rules). **Sight rule:** `Exposure` is not None, **and** the target is in the pawn's forward cone (the dot product of the pawn's facing with the direction to the target is greater than `WillowMind.SightViewCone`, which is a
  cosine threshold; a patrol den skips the cone test), **and** the horizontal distance from the threat origin to the target is less than the threat radius.
- **Proximity rule** (skipped when the target is a vehicle, so vehicles need the sight rule): the horizontal distance from the threat origin is less than a patrol limit (unlimited unless the den is not a patrol den, then
  the den's `PatrolRadius`) **and** the 3D distance from the pawn to the target minus the pawn's collision radius is less than `GlobalsDefinition.AICloseThreatRadius`.
- **Threat origin and radius:** with no den, or a patrol den: the pawn's location (the mind's `ProvokedLocation` when `bProvoked` is set) and the AI's search radius (4096 for the adult bullymong); with a non-patrol den: the den's location, its `ThreatRadius`.
- **Implementer checklist:** run after the exposure iterator (Knowledge order); the result is level 2 plus the `SetThreatLevel(…, 2)` side effects (AggroTime, ForgetTime 10 s for a den definition; the 2.5 s default was not seen). Live (lane L9): the exposure these rules read is one update old, and level 2 and `CurrentTarget` come in the same update.
- **Live confirmation (lane L9, 2026-10-09):** the rules are confirmed in the running game with the den as origin. The sight rule fired at 4,660 uu and not at 4,680 (den `ThreatRadius` 4,665.625), with a lag of 0.209 to 0.220 s from the exposure turning Full; it did not fire at 4,640 with the member behind (dot -0.67). The proximity rule fired at 728.0 (3D distance minus collision radius, below 768) and not at 770.0, and did not fire beyond the den's patrol radius (6,200 against 3,667.19). The cone was tested only by sign (+0.91 fires, -0.67 does not); the boundary near zero is open. The collision-component gate, the patrol-den skip and the `ProvokedLocation` origin were not seen. Sections 0 and 8 of [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md).
- **Open:** `ITargetable` is read as "slot 28 = IsEnemy" because slot 2c is certainly IsFriendly; the enemy gate is that reading.

## Leaders, forced target, formations

- `GetTargetLeader` / `GetFormationLeader` return the leader interface stored on the **root** component (the call climbs to the topmost Willow parent first); `SetTargetLeader(NewLeader, bOverride)` forwards to the parent when there is one,
  otherwise first drops a stored leader whose actor no longer exists, then stores `NewLeader` when `bOverride` is true, or no leader is stored, or `NewLeader` is None (which clears it). `SetFormationLeader` is the same for the formation link.
  `HasForcedTarget` is "the root has a target leader"; `GetForcedTarget` is the **current target of the leader's AI component**.
- `ClearState` (a component virtual): after the base clean-up, and unless the object is flagged in a shutdown style way, it clears the component's own target-leader and formation-leader links when they point at the component itself.
- **Implementer checklist:** not needed for a lone bullymong; keep the accessors returning None.
- **Live confirmation:** none for the slice.

## Combat zones

- A combat zone is a cube of the world grid: 512 uu on a side, 1024 cells per axis, origin at -262144 on each axis. The index is x + 1024 * (y + 1024 * z) where each coordinate is the integer part of
  (position + 262144) / 512; a valid index is 0 to 1024 cubed minus 1. `GetCombatZoneByLocation(Vector)` returns the struct (`Index` plus the box bounds, the box being the cell).
- `MarkCurrentCombatZone` copies `CurrentZone` into `LastZone`. `InvalidateCombatZone(ZoneIndex)` appends the index (once) to this component's `InvalidCombatZones`; the omitted / -1 form uses the zone containing the AI's own location.
- `ClaimCombatZone(Zone)`: needs a blackboard component, no zone currently claimed by this component, a valid index not already in the blackboard's claimed list; it appends the index to that shared list and stores the zone as `ClaimedZone`.
  `UnclaimCombatZone` removes the index from the shared list and clears `ClaimedZone.Index` to -1 when it is the claimed one. `CombatZoneAvailable(Zone)`: true for the zone this component claimed; false for an invalidated zone;
  otherwise true when the index is valid and not in the shared claimed list.
- **Implementer checklist:** ranged AI only; the bullymong does not claim zones (its attacks are melee). Port when a ranged enemy is needed.
- **Live confirmation:** the numbers (512, 1024, -262144) are static data and were read directly; confirm by calling `GetCombatZoneByLocation` at two known points.

## Holds, OnHold, ClearState, InitAI

- Holds are lists of (name, tag object) pairs; `Hold(HoldName, Tag)` adds a pair unless the same pair exists or `Tag` is None; `Release` removes the pair. `OnHold` (base) is "the Holds list is not empty".
  `HoldTarget` / `ReleaseTarget` / `TargetOnHold` do the same on the target-hold list.
- Willow `OnHold` is also true in one extra case reached through the pawn (a pawn flag plus a nested object reporting itself on hold; a cinematic or dialog style hold, not decoded); otherwise it is the base rule.
- Willow `Hold`: after the base rule, if the component is now on hold, the pawn gets `SetLookAtTarget` with no target (head look stops). Willow `Release`: after the base rule, if it is no longer on hold, the pawn gets
  `SetLookAtTarget(CurrentTarget)`.
- `HoldHeadLook(HoldName, Tag)` / `ReleaseHeadLook` / `HeadLookOnHold`: same list mechanism on `HeadLookHolds`; a pawn flag is set to "the list is not empty" (read as the head-look-disabled flag). `HoldPhaseLock` / `ReleasePhaseLock` /
  `PhaseLockOnHold`: the same on `PhaseLockHolds` with no pawn side effect (the effect of a phaselock hold is read elsewhere; see NATIVE_PHASELOCK_TARGETING.md).
- The AI initialisation hook (virtual used when the AI starts) binds `MyBlackboard` and `MyWillowMind` and then runs the base set-up (builds the node array, creates one action object per action node, raises `Init` on each).
- **Live confirmation:** hook `Hold`/`Release` while Maya phaselocks a bullymong and read `PhaseLockHolds` and `Holds` (the hold names show what the phaselock script registers).

## WillowMind.NotifyAttackedBy

- **Signature:** `native function NotifyAttackedBy(Actor Attacker, bool bDamaged)`.
- **Reads/does:** requires the mind's AI component to be a Willow component, the mind to have a pawn and the world to exist. It calls the component's `NotifyAttackedBy(Attacker)` (so the attacker becomes a target and Provoked);
  then, **if the pawn does not regard the attacker as Friendly**, it records on the component: `LastAttackedTime = now`, `LastAttacker = Attacker`, and (only when `bDamaged`) `LastHurtTime = now`.
- **Implementer checklist:** the three field writes are gated by "not Friendly"; `bDamaged` only controls `LastHurtTime`.
- **Live confirmation (lane L9, 2026-10-09):** partly confirmed. For a real shot the hook saw the call from `WillowAIPawn.TakeDamage` with `bDamaged` true, with the player's pawn as attacker, and the level-3 stamp followed (section 3 of [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md)). The three field writes (`LastAttackedTime`, `LastHurtTime`, `LastAttacker`) were **not read**, and the "not friendly" gate was not tested.

## WillowMind.IsAwareOfTarget (and the damage pipeline use)

- **Signature:** `native function bool IsAwareOfTarget(Actor PotentialTarget)`.
- **Does:** false when the world does not exist. Looks up the record for the actor **at the root component** (the lookup climbs; it never creates). No record: **false**. Otherwise true when
  `WillowMind.MinAggroTimeToConsiderAwareOfTarget + record.AggroTime < now`. `AggroTime` is zero until `SetThreatLevel` stamps it, so a record that exists but has never been raised to threat 2 counts as aware once the delay has passed
  (Live, lane L9: the fresh record's `AggroTime` read 0 every time, and the answer was True; see the live line below.)
- **Used by:** one stage of the damage pipeline. Reading: when the victim's mind is not aware of the attacker, the damage is multiplied by a per-attacker "unaware" multiplier and the hit is tagged with the unaware flag value (25) in
  its flag list. This is the source of the `OnDamagedUnawareEnemy` skill events. Which object holds the multiplier is not fully identified.
- **Implementer checklist:** record-presence first, then the aggro delay; only the damage stage needs it for the slice, and only for skills that depend on unaware hits.
- **Live confirmation (lane L9, 2026-10-09):** confirmed. No record (Maya beyond about 7 km, or after the removal) gives False; a fresh record (level 1, `AggroTime` 0) gives True; after a level-2 or level-3 stamp it is False for 0.1 s and True from then on (last False 0.098, 0.090 and 0.084 s, first True 0.114, 0.102 and 0.108 s after the stamp, three runs). So an "unaware" bonus would only apply with no record or in the 0.1 s after a stamp. Section 2 of [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md).

## ISeeTargetable.AddTargetable (WillowMind) and WillowMind.NotifyTakeHit

- **AddTargetable(NewTarget, bSeeTarget, bHearThreat, bSmellTarget):** the mind's implementation uses only `NewTarget`: if the mind has an AI component it calls `AddTarget(NewTarget)` and returns true, else false.
  The three flags are not used by this implementation (UNVERIFIED reading of the implementation's argument use).
- **NotifyTakeHit(...):** gated on the game-info being present and the mind having a controlled pawn. When the damage is non-zero it notifies a game-level object with the instigator and damage; when the pawn has its behavior component it raises the "took hit"
  behavior events with the hit pawn, location and damage (two parallel kernels, one for a known instigator and one without). It then calls the base function, which does nothing. Pure plumbing for animation/behavior reactions; not part of target choice.
- **Live confirmation (lane L9, 2026-10-09):** not seen. The hook on `NotifyTakeHit` never fired for a shot bullymong (the route is native-to-native), and `AddTargetable` was not seen either. Both stay UNVERIFIED.

## Awareness zones and the other iterators found

- **TargetIteratorCheckAwareness** (a Knowledge-style iterator over the mind's awareness zone collection): not in the bullymong's `Targeting_ShortRange` list (that list is in NATIVE_AI_MELEE.md), so the zone logic below is
  not part of the slice unless another targeting definition uses it. Rule, at contract level: the mind keeps a collection of zones sorted from near to far (`AwarenessZoneDefinition`: `OuterRimDistance`, `OuterRimInfinitelyFarAway`,
  `ViewConeFieldOfViewInDegrees`, `MinExposureRatingConsideredVisible`, `HearingBehavior`, `SpecialExposureBehavior`, `ForgetHiddenThreatsAfterXSeconds`, …). A record is evaluated only if its distance is within the farthest usable rim
  (50000 when the farthest zone is infinite or unusable); the nearest zone whose rim covers the distance is selected (and cached on the mind as `CurrentAwarenessZone`); then the record is "seen" when the zone always sees,
  or its `TargetExposureToMe` reaches `MinExposureRatingConsideredVisible` and the target is inside the view cone (a cone of 360 degrees or more always passes), or the zone's hearing behaviour passes; "seen" sets the record's
  `bCanSeeOrHearTarget`, stamps `IAmConsciousTime` from the zone and fires the hidden-threat bookkeeping; not seen clears the flag. When a particular conscious-flag of the record is set the mind's `AtLeastOneKnownThreat` byte is also set. The view-cone and hearing tests are two mind virtuals (not read).
- **WillowTI_PrioritizeHealth** (Prioritization, field `Priority`: `HealthPriority_PrioritizeWeak` 0 / `HealthPriority_PrioritizeHealthy` 1): for a pawn target, ratio = health / max health clamped 0..1; `OnePriority = 1 - ratio` for Weak,
  `ratio` for Healthy. Not used by the bullymong.
- **WillowTI_PrioritizeOpinion** (field `Opinion`): `OnePriority = 1` when the AI's opinion of the target equals the iterator's `Opinion`, else 0 (mind-based evaluation). Not used by the bullymong.
- **WillowTI_CalcCombat:** the script class adds no fields and no C++ class or function table of its own was found for it, so it behaves as the base `TI_Calc` no-op in native (UNVERIFIED; it may still carry script-side state).
- **Live confirmation (lane L9, 2026-10-09):** the bullymong's mind has no awareness zones: every zone collection read as None, so the awareness iterator is not in play for this pawn. The targeting definitions were not dumped in full, and the view-cone and hearing virtuals were not exercised.

## IMeleeAttacker on WillowPawn and WillowAIPawn

The pawn implements `IMeleeAttacker` as a second function table. `WillowAIPawn` replaces only one entry. Methods, in the interface's order:
| Method | WillowPawn (players, generic) | WillowAIPawn |
|---|---|---|
| `GetMeleeTraceSourceActor()` | the pawn itself | same |
| `GetMeleeInstigator()` | the pawn's `Instigator` reference | same |
| `GetMeleeTraceSourceLocationAndRotation(out Vector, out Rotator)` | location: the pawn's view / eye location (the engine query evaluated with its default argument); rotation: the controller's rotation when there is a controller, else the pawn's own rotation with the replicated view pitch and yaw bytes (each times 256) added | location: the pawn's world `Location` (**no eye offset**); rotation: the current aim of the pawn's `AimOffset` node when that exists, else the actor's `Rotation` |
| `GetMeleeAttackerMass()` | a float field of the pawn (read as its mass) | same |
| `BeginMeleeOverTime(MeleeDefinition)` | see below | same |
| `EndMeleeOverTime()` | see below | same |
| `AddActorToListOfHitActors(Actor)` | appends to `MeleeState.HitActors` (no duplicate check; None ignored) | same |
| `HasActorAlreadyBeenHitByMelee(Actor)` | linear search of `HitActors` | same |
| `GetMeleeState(out StartTime, out LastTime, out StartLoc, out StartRot)` | `StartTime` = `MeleeState.MeleeStartTime`; `LastTime` = now minus `TimeElapsedSinceLastMelee` (the accumulator below, so it is the moment of the last attack tick); then the saved start location and rotation | same |
| `ShouldIgnoreInstigatorVelocity()` | true unless the pawn's controller has a particular flag set (player-side flag; unnamed) | same |

- **State:** `WillowPawn.MeleeState` (`MeleeOverTimeState`: `bIsMeleeActive`, `MeleeDefinition`, `MeleeStartTime`, `TimeElapsedSinceLastMelee`, `HitActors`, `MeleeStartLocation`, `MeleeStartRotation`).
- **BeginMeleeOverTime(def):** if a melee is already active, end it first; then `MeleeDefinition = def`, `MeleeStartTime = now`, active = true; if `def` is not None the definition's own virtual is called with the attacker, its interface and the two start
  fields, which fills `MeleeStartLocation` / `MeleeStartRotation` (read as the definition's origin helper: trace source shifted by `HitStartOffset` / socket). `TimeElapsedSinceLastMelee` is not touched (it is zero from the last End).
- **EndMeleeOverTime():** clears `MeleeDefinition`, `MeleeStartTime`, `TimeElapsedSinceLastMelee`, empties and releases `HitActors`, clears active.
- **Consequence for the G25 melee volume:** for an AI pawn the volume origin before `HitStartOffset` is the actor location (not an eye point); the vertical band therefore centres at foot / capsule-centre height of the actor
  plus `DamageHeightOffset`, and the facing is the aim-offset rotation when the pawn has an aim-offset node.
- **Implementer checklist:** port the table above; the AI variant's origin is the actor location.
- **Live confirmation (lane L9, 2026-10-09):** the location is confirmed: the trace source equalled the actor's `Location` to 0.01 uu on one pawn (two reads). The rotation equalled the actor's `Rotation` on that pawn, which had no aim difference, so the aim-offset case is not separated. `MeleeState` read during a charge matches the next section. Section 6 of [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md).

## Melee over time: who repeats the hit, and when it stops

- **MeleeDefinition.MeleeAttack(Context) entry (native):** the context is converted to `IMeleeAttacker` (no interface: nothing). **If `bMeleeOverPeriodOfTime` is set, `BeginMeleeOverTime(def)` is called first**; then the first hit sweep (radius
  volume, else cone, as in NATIVE_AI_MELEE.md) runs **immediately** in the same call.
- **Per-frame tick (a stage of the WillowPawn tick, authority only):** runs only when the pawn's role is Authority, `MeleeState.bIsMeleeActive` is set and a definition is stored.
  1. If the pawn is dead / pending deletion (a pawn "gone" test), `EndMeleeOverTime()` and stop.
  2. interval = `def.AttackInterval`, with anything not larger than **1/15 s (0.0666667)** raised to 1/15 s (so `AttackInterval` 0 means 15 hits a second at most; the live charge value 0.0667 is exactly this clamp).
  3. `TimeElapsedSinceLastMelee += dt`. While the accumulator is at least the interval: run one more hit sweep (`MeleeAttack` core with the pawn as attacker), stop at once if that ended the melee, then subtract the interval from the accumulator.
     So a long frame produces several sweeps, and the first repeat comes one interval after the start (the immediate sweep was the first).
  4. After the sweeps, if the world exists and `def.MaxMeleeDuration` is less than now minus `MeleeStartTime`, `EndMeleeOverTime()`. The check follows the sweeps, so the sweep of the frame in which the duration is exceeded still happens.
- **Other things that end it (script):** `WillowPawn.Landed` when `bStopMeleeOnLanded`, `WillowPawn.Destroyed`, `Behavior_StopMeleeAttack`.
- **Constants:** minimum interval 1/15 s; for the five live bullymong definitions: `_MeleeGrabBaby` 0.75 s / 0.1 s, `Melee_Charge` 5 s / 0.0667 s (clamped value), others are single-shot.
- **Implementer checklist:** immediate sweep; then accumulate dt and sweep per whole interval (catch-up allowed); end on duration, death, landed (flag), destroy or an explicit stop; hit-once lists live in `MeleeState.HitActors` and are cleared on end. A target is hit once per melee when the definition's `bHitEachTargetOnlyOnce` is set (the live charge: one hit on the player in 5 s, lane L9).
- **Live confirmation (lane L9, 2026-10-09):** the sweep rate is confirmed; the expectation of repeated damage is refuted. A charge ran 74 sweeps in 5.007 s (median spacing 0.0674 s, frame granularity), so the 1/15 s clamp holds, and the first sweep came at the call. The player was hit **once** in the 5 s, because `bHitEachTargetOnlyOnce` is true on this definition. The melee ended 5.0072 and 5.0077 s after its start and cleared the hit list. A player outside the volume at the call was hit 0.0035 s after moving in at 1.0 s; moving out and in again at 3.0 s gave no second hit. The final-frame sweep was not separated. Section 6 of [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md).

## AI tree natives: eligibility, activation, completion, interruption

Fields come from `AIComponent.AITreeData` (type, children, resource, `CanRunIf`, `TimedStart` / `TimedReset` / `TimedFail` ranges, `bStopIfCannotRun`, `ActiveChild`, `NextActiveChild`, `LastFailTime`, `NextTimedRun`, `NextTimedFail`,
`Action`, `LoopCount`, `LastRandomChild`, `Weights`, `LoopCountMax`, `bInterrupt`, `Conditions`, `NextSequentialChild`, `ActionFailName`). Random values below are uniform in [min, max] using the engine's integer random divided by 32768.

- **Node eligibility (resolves the G25 "unresolved" cool-off):** a node is selectable only if all hold, in this order:
  1. `NextTimedRun` is zero or already passed;
  2. unless it is a currently running action node without `bStopIfCannotRun`, `CanRunIf` (when set) evaluates true for the AI's pawn (a running action node without that flag skips this re-check);
  3. **if the world exists (always in play) and `LastFailTime` is non-zero, now must be at least `LastFailTime + 1.0 s`** (the G25 "global debug flag" is simply the world pointer);
  4. for an action node that needs checking, `Action.CanRun` is called (script event) and its returned name stored in `ActionFailName`; the node is eligible only when that name is None and an action object exists;
  5. if the node has a `Resource`, the AI resource registry (two component virtuals, not read) must report it available.
- **Selecting within a node:**
  - **Priority node** (requires `Children.Num == Conditions.Num`, else selects nothing): with `bInterrupt` false and an active child, re-evaluate the active child only; otherwise try children in order, a child with a condition only when the condition evaluates true
    for the pawn, and take the first child whose subtree selects an action.
  - **Random node** (requires `Children.Num == Weights.Num`): with an active child, re-evaluate only it. Otherwise build the candidate list from all children, **excluding a child whose index equals `LastRandomChild`, has a non-zero `LoopCountMax` entry, and whose
    `LoopCount` has reached that maximum**; draw a child with probability proportional to its weight; if its subtree selects nothing remove it and draw again, until the list is empty.
  - **Sequence node:** only the child at `NextSequentialChild` is tried (index out of range selects nothing); if that child's subtree does not select, the sequence selects nothing.
  - **Action node:** selects itself.
  - The overall selection returns the chosen **leaf action node** (or nothing).
- **Activation (when a leaf starts):** for every node along the chosen path `ActiveChild = NextActiveChild`; for each such node with a valid `TimedFail` range (both > 0, min <= max) `NextTimedFail = now + random(min, max)`; each node's `Resource` is acquired through the component.
- **Completion (a leaf action finished or was stopped):** walking the active path from the root down and unwinding back up, each node on the path finishes. When a node finishes:
  - if its `TimedReset` range is valid (both > 0, min <= max), `NextTimedRun = now + random(min, max)` (**the cooldown before the node may run again**; for sequences this happens only on failure or after the last child);
  - its `Resource` is released;
  - **sequence:** unless it failed or this was the last child, `NextSequentialChild` advances by one and the sequence is not done; otherwise the index resets to 0, `ActiveChild = -1`, done;
  - **random:** if the finished child is the same as `LastRandomChild`, `LoopCount` increases by one, else `LoopCount` becomes 1 and `LastRandomChild` is updated; `ActiveChild = -1`;
  - **others:** `ActiveChild = -1`, done.
- **`ActionSequence.Finish(optional bool bFailed)` (script-callable):** marks the action finished; if the component's current state is this action's node: when `bFailed`, `node.LastFailTime = now`; then the component runs "action finished": stop the state (flag `bFailed`), **select a new
  leaf immediately** (no wait for the next `UpdateRate`), and start it.
- **Stopping a state:** the component clears `CurrentState`, raises `Stop` on the action, and (if the pawn is alive) clears the action's running flag, puts the action back to its default script state, runs the completion walk for the whole path and releases the root resource.
  **Starting a state** acquires the root resource, sets `CurrentState`, activates the path, clears the action's "finished" flag and raises `Start` on the action.
- **The periodic state update (every `UpdateRate`, as in NATIVE_AI_MELEE.md), exact interruption rules:** nothing happens while on hold or before the interval. If there is a running leaf: no action object means stop here; if the action has **both** its running and its non-interruptible flag set, nothing else happens;
  otherwise `Action.CanRun` is asked again: when it returns a failure name, or the node has `bStopIfCannotRun` and is no longer eligible, the state is stopped with the failed flag; otherwise the timed-fail timers are checked. Then a new leaf is selected; if it is a valid leaf (an action node with an action object) that differs from the current one,
  the old state is stopped (failed flag set) and the new state started. The only "interruption test" is that validity check; priority nodes with `bInterrupt` and the `CanRun` result decide what can interrupt what.
- **Timed fail tick (each frame):** for every node whose `NextTimedFail` is non-zero and has passed, if that node is on the running path the "action finished (failed)" path runs.
- **Script events on actions:** `Init`, `CanRun`, `Start`, `Update(dt)` (each frame for the running action), `Stop`, `ActivateEvent(name)`.
- **Constants:** 1.0 s failure cool-off; random scale 32768.
- **Implementer checklist:** leaf-returning selector; eligibility order; completion writes `TimedReset` cooldown and loop counters as above; `Finish` re-selects immediately; update-time interruption as above.
- **Live confirmation (lane L9, 2026-10-09):** partly confirmed on a 62 s fight and two 60 s recordings. The `TimedStart` re-arm on acquisition set `NextTimedRun` to now + 10.0003 s in the acquiring frame. The `TimedReset` after the perch finished set it to now + 27.05 (inside 20 to 30). Random node: `LoopCount` went from 4 to 1 when a different child ran, then 2 and 3 on the same child's next finishes, then 1 after the other child, and the last-child index followed. The `LoopCountMax` cap was **not exercised** (both maxima 0). `LastFailTime` stayed 0 on every node, so the 1.0 s failure block and the `Finish` rule were **not exercised**. `ActionFailName` values seen: NoPathToPerch, NotReachable, PathingFailed, NoTarget, NotScripted, InvalidLocation. Section 7 of [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md).

## WillowAIPawn.StartLeapAtActor, StartLeap, GetLeapRotation (stretch)

- **StartLeap(StartLoc, EndLoc, LeapSpeed, LeapAngle) returns float (flight time):** sets the pawn's `bLeaping` flag, stores `LeapStartLoc` and `LeapStartTime = now`; a non-positive `LeapSpeed` becomes 1.0, a non-positive `LeapAngle` becomes 1.0 and an angle above 85 degrees is capped at 85;
  a ballistic solver then fills `LeapTotalTime`, `LeapGravity` and `LeapStartVelocity`; a non-finite total time becomes 0. Returns `LeapTotalTime`.
- **StartLeapAtActor(Options, Target) returns float:** builds the leap target from the actor (its location plus `Options.Offset` turned by the target's rotation, a random error of up to `Options.Error`, and a velocity prediction bounded by `Options.PredictDistMax`), runs the same solver with
  `Options.Speed` / `Options.Angle` / `AnglePct`, then sets the pawn's physics to falling, sets the pawn's velocity to `LeapStartVelocity`, sets a gravity scale of `LeapGravity * -0.001` (field read as the pawn's custom gravity scaling) and returns the flight time. The numeric details of the prediction are not read.
- **GetLeapRotation(DestLoc):** a rotation (pitch, yaw, roll) computed from the pawn's current orientation axes and the normalised direction to the destination, i.e. the turn that would face the destination (read as expressed relative to the pawn's own frame; UNVERIFIED).
- **Script side (`Action_LeapAtTarget`):** `CanRun` adds `TooFar`; `PlayLaunch` plays the launch animation and turns the pawn toward the target for half the flight time; `PlayIdle` calls `StartLeapAtActor`, turns toward `LeapStartVelocity` for half the flight time and plays the in-air idle; the `Landed` event stops the in-air animation; `PlayLand` plays landing and `LandFinished` calls `Finish`.
- **Open:** the solver itself and the exact predicted aim point.
- **Live confirmation:** call `StartLeapAtActor` through pyunrealsdk for a bullymong with Maya at 600 / 1000 / 1500 uu and read the return value, `LeapStartVelocity`, `LeapGravity`, `LeapTotalTime`, then compare the landing position with Maya's position at `LeapStartTime + LeapTotalTime`.

## Chase movement natives (contract only, bodies not read)

`Action_GenericAttack` drives the approach through script: `PathFind` clears the failure flag, calls `WillowNavigationHandle.CreateConstrainedPath(PathData, Target.Location, Limits)` (or `CreateVectorPath` for fixed locations), then
`WillowActionSequencePawn.PathIsValid()`; `GetFinalDestination` is the attack location; the move states then use `FollowPath()` (latent: walks the stored path), `WaitForPath()`, `ReachedDestination()` (bool), `FinishPath()`, `InvalidatePath()`, `CanMoveTo(GoalPoint)` (bool),
`GetPointOnGround(StartPoint, optional Tolerance)` (Vector), `SetPawnMovementSpeed(EDefaultStance)`, `WaitForPawnToTurn()`, `WaitForPawnToStop()`. The slice host needs equivalents: path creation toward a point, a per-tick follow with arrival reporting, ground snapping. The native bodies (navigation mesh queries)
were **not** read in this lane.

## Not read yet
- The three AI-pawn notifications and the presentation calls in `TargetChanged`; the targetable interface slots named only by position here (can-be-targeted, IsEnemy, IsFriendly, GetAllegiance, GetActor).
- The AI resource registry virtuals (`Resource` availability, acquire, release) and the exact "action default state" call at state stop.
- The view-cone and hearing virtuals of the awareness iterator; the hidden-threat bookkeeping helpers.
- The leap solver, the navigation handle natives and `FollowPath`; cover natives (`TryEnterCover`, `GetCoverExposure`), `GetFormation`.
- The code that lowers a threat level after `ForgetTime` (the game shows the fall; the reader was not located here). A fresh record's `AggroTime` and `ForgetTime` read 0, but a den record can carry a stale `ForgetTime` from an earlier engagement.
- Which object holds the "unaware" damage multiplier in the damage pipeline stage.

## Corrections to earlier notes
- NATIVE_AI_MELEE.md, "ValidTarget": the friendliness test is "opinion Friendly", not "not an enemy"; Neutral actors pass it. The note also omitted the Willow additions (controller-less pawns, cloak, vehicles, leader rule) and the "AI without an allegiance has no valid target" rule.
- NATIVE_AI_MELEE.md, "FindTargets": the undecoded skip is "the AI has an allegiance and its opinion of the candidate's allegiance is Neutral".
- NATIVE_AI_MELEE.md, "ValidCurrentTarget": the Willow override additionally requires threat level >= 2 (and the actor-flag exclusion). Without it a record is never chosen.
- NATIVE_AI_MELEE.md, node eligibility: the 1.0 s cool-off after a failure is not debug-only; its flag is the world pointer and it applies in normal play. `TimedReset` is the post-completion cooldown (`NextTimedRun`), now confirmed.
- NATIVE_AI_MELEE.md, "WillowTI_CalcThreat" and "WillowTI_CalcCombat": the threat iterator's rules are above; `CalcCombat` has no native body.
- NATIVE_AI_MELEE.md, "Melee over time": the repeater is the pawn's per-frame tick (section above); `MeleeAttack` calls `BeginMeleeOverTime` itself and sweeps once immediately.
- NATIVE_AI_MELEE.md, melee origin: for an AI pawn it is the actor's location, not an eye point.

## Live corrections from lane L9 (2026-10-09)

Each item is observed in the running game; the numbers are in [AI_AWARENESS_LIVE.md](AI_AWARENESS_LIVE.md) section 11.

- **Lone case:** a bullymong spawned by a population den has a den parent. The threat origin is the den's location with the den's `ThreatRadius`, and the patrol limit is the den's `PatrolRadius`; the pawn's own location and the 4,096 radius were not the operative values for these bullymongs.
- **`ForgetTime`:** it has a reader in the running game. The level falls at the den's next update after `ForgetTime` (to 1 in place, or the record is removed), and `AggroTime` returns to 0 in the first case.
- **`SetThreatLevel`:** it stamps the den's (root) record; members only copy the level.
- **Fresh record:** the first visible level is `ETL_InThreatRadius` (1); `GetThreatLevel` is 0 only when there is no record. `TargetExposureToMe` reads -1.0 on the first update of a fresh record.
- **Defaults:** the 2.5 s and 10 s defaults for non-den definitions were not seen; the den definition's 10.0 and 25.0 are the operative values for a den-spawned bullymong.
- **Timing:** the exposure read by `WillowTI_CalcThreat` is one update old; level 2 and `CurrentTarget` come in the same update; `IsAwareOfTarget` is False for 0.1 s after a stamp and True for a fresh record.
- **Melee over time:** `bHitEachTargetOnlyOnce` is why a charge hits a target once; the sweeps themselves still run at 1/15 s.
- **`bProvoked`:** it is set on acquisition, but den-spawned pawns can already have it true before anything noticed the player, so it does not record "noticed".
