## 2026-11-16: Rust AI awareness and targeting, part 1

AI-assisted (Claude Code). Lane W7-AI part 1 (architecture section 8, "Wave 7 and the host track"). Number 59 was reserved
for this lane by the hub. It fills `crates/ow-ai/src/awareness.rs` and its modules `record`, `threat`, `iterators`,
`targets`, `tick` and `vm`, with their tests, and adds the ignored installed-game run
`crates/ow-ai/tests/installed_awareness.rs`. No other crate changes. There was no real-game session. Every rule is
**UNVERIFIED** unless its doc names the live record that confirmed it, and every live confirmation was of a
den-spawned bullymong on one map (Three Horns Divide).

- **Sources.** `NATIVE_AI_MELEE.md` (G25) and `NATIVE_AI_AWARENESS.md` (G26), read from the executable by earlier
  analyst lanes. The live records are `AI_MELEE_TARGETING_LIVE.md` (L8) and `AI_AWARENESS_LIVE.md` (L9). Where a note
  and the game disagree, the game wins. No AI runs in the C++ reference, so there is no stand-in: both parity modes
  run the same rules. This lane read no executable code, so it wrote no new analyst note; the analyst questions are
  listed below.

- **Scope: a lone component.** That is a component with no parent, which is what the Rust world spawns, since it has
  no population dens yet. A component with a parent (a den member) answers `NotYet { NotPorted }`. Every live pawn
  had a den, so the lone rules are the notes' readings, and that case is open question 17 ("lone bullymong").

- **The rules.**
  - **Threat levels.**
    - `SetThreatLevel`: the level only rises. The first level of 2 or more stamps `AggroTime`. A level of 2 pushes
      `ForgetTime` to at least now + 2.5 s, a level of 3 to at least now + 10 s. Those are the non-den defaults the
      note reads; a den definition's `MinEngagementTime` and `MinProvokedTime` are used when the definition has them.
      Live on den records: 10 and 25.
    - `GetThreatLevel`: 2 for a Friendly target, else the record's level, else 0.
    - A fresh record shows level 1. The code writes 0, but the game shows 1; the writer of the 1 was not located.
    - **The fall after `ForgetTime`.** At the next target update after `ForgetTime`, a level of 2 or 3 drops to 1 in
      place and `AggroTime` returns to 0. This was observed on den records; the code that does it is not located.
    - `IsAwareOfTarget`: false without a record, else `0.1 + AggroTime < now`. Live: true for a fresh record, false
      for 0.1 s after a stamp.
  - **Noticing.** This is `WillowTI_CalcThreat`; it reads the previous update's exposure and facing, as live. It
    applies to an Enemy target, and only when the pawn has its collision component.
    - **Sight:** the exposure is not none, the facing dot product is above `SightViewCone` (strict), and the horizontal
      distance from the threat origin is below the search radius.
    - **Proximity:** the target is not a vehicle, and the 3D distance minus the pawn's collision radius is below
      `AICloseThreatRadius`.
    - A lone origin is the pawn's location, or `ProvokedLocation` once the component is provoked. There is no patrol
      limit.
  - **The list.**
    - `FindTargets` walks the host's registry of targetables. It skips an entry the AI can't target, one the AI
      regards as Neutral, and one beyond the horizontal search radius. Height does not count, as live.
    - `ValidTarget` follows the base and Willow order of G26. Step 2 is the adapter's: a controller is tested as its
      pawn, and so is an actor whose controller has a pawn; a controller without a pawn is tested as itself (G26 step 2
      replaces only when the pawn is set; decision 2026-11-16-66, review r2 L1). Step 7 requires the AI's own
      pawn in the host's registry. Live: the player, None, the AI's own pawn, a same-allegiance pawn, a cloaked player
      and an AI without an allegiance all give the recorded answers.
    - The radius test compares the squared horizontal distance with the squared radius in f64, as the note reads it;
      an entry exactly on the radius is in the list (the edge is not read).
    - The removal pass runs first. A pawn on foot has no hand-off; a vehicle answers `NotYet`.
    - A lone list never drops a record by distance. (Live, a den member's list was not cut at 4,096. Its list comes
      from the den.)
  - **Choosing.**
    - The priority is the weighted mean of the Prioritization iterators: nearby, exposed (unclamped), previous, bonus;
      the formula is live-confirmed to 5e-8. Idle and Attacking are `NotYet` when their weight is not 0: their inputs
      are not read. Idle's count is 1 for an actor with no readable counter (G25). A priority that is not finite is
      refused.
    - **A weight-0 iterator is skipped (UNVERIFIED).** It adds nothing to the priority, but in the game it still runs.
      That matters for `WillowTI_PrioritizeAttacking`, whose run keeps the record's decaying damage meter
      (`TotalDamagePct`). The meter is not modelled here, because its hit history is not read, so it stays 0. L8 read 0
      too, but in a fight where the player did no damage, so the skip is untested.
    - **The exposure** is the host's fraction. A value that is not finite is refused as the host's error, because NaN
      would read as full sight; -1.0, which the game shows, is kept.
    - `ChooseBestTarget` takes the highest valid record, with ties in list order. `ValidCurrentTarget` needs level 2
      or more and a target that isn't `bHidden`; a scripted target is the only valid one; the evaluators are `NotYet`.
    - `SetTarget` gives `Target_Lost`, then `Target_Acquired`. `TargetChanged` re-arms the timed starts, sets
      `bProvoked` and `ProvokedLocation` on a first acquisition, moves the head look, and updates
      `NumAICurrentlyTargeting` on both targets.
  - **The tick, in order.**
    1. The first-tick initialisation.
    2. On hold: stop. Nothing else runs, and stopping the running action is part 2.
    3. The target update, when `now >= LastStateUpdateTime + UpdateRate`. It runs the fall, the removal,
       `FindTargets`, `CalcTargetInfo` on the records it already had, and `ChooseBestTarget`.
    4. The action update and the timed failures (part 2).
    5. The state update's time (the tree selection is part 2).

    The cadence is live: both times are equal and spaced at least 0.1999 s apart at a rate of 0.2. The steps that
    belong to part 2 are listed in the tick report; they are neither errors nor run.
- **The engine and the host, behind traits.** `Senses` is the rules' input. The VM adapter's `Host` answers three
  things: the exposure of a target to a viewer (line of sight through the level's collision, which `ow-host` owns:
  `ow-ai` does not depend on it), the registry of targetables, and an actor's own "can be targeted". The rules never
  guess these.
- **The VM adapter (`awareness::vm`).**
  - **Reads.** The settings come from the component's `AIDef` and `TargetingDef` (iterators by class path, with their
    weights and fields), the mind, the pawn's cylinder and `GD_Globals.General.Globals.AICloseThreatRadius`.
  - **Writes.** It writes back `CurrentTarget`, both update times, `bProvoked`, `ProvokedLocation` and the target's
    counter. Before each tick it reads back what script can change: `bProvoked`, `Holds`, `ScriptedTarget`.
  - **No state is lost on a failed read** (review r1). Everything that can fail is read before the held state is taken
    out, and the state is put back before the writes.
  - `GetThreatLevel` on a component the adapter has not ticked builds the component from its settings, so the
    Friendly rule applies. It finds such a component's mind through its `MyWillowMind`, which the AI start binds
    (G26).
  - **Unbound definitions.** A component whose `AIDef` or `TargetingDef` is None answers `NotYet`. The native that
    binds them at AI start is not read: installed script never sets `AIDef`.
  - **The natives** (`awareness::vm::Pack`): `WillowMind.NotifyAttackedBy` (which `WillowAIPawn.TakeDamage` calls),
    `WillowMind.IsAwareOfTarget` and `WillowAIComponent.GetThreatLevel`, the latter answered by enum name. They need
    `vm::install` (the host and the globals).
  - **The behavior kernel.** No AI behavior class needs a handler here. The awareness behaviors (`Behavior_AIProvoke`,
    `Behavior_AITargeting`, `Behavior_AIPriority`) are script that calls these natives. Whoever runs their provider
    collects links with `ow_behavior::kernel::natives::collect` and routes consumers there (decision 2026-11-16-50).

- **Verified (automated, local, pc-hummer).**
  - 30 `ow-ai` unit tests on invented data: 24 on the rules (mode-free, since there is no stand-in) and 6 on the VM
    adapter over invented classes, each in both modes. Review r1 added 8 of them: the kept state, the
    controller-to-pawn step in the rules and in the adapter, the non-finite exposure and priority, the registry
    clause, the radius edge, and `GetThreatLevel` without state.
  - **The installed run** (`installed_awareness.rs`, ignored; output in `local/p2/w7-ai-awareness/installed_run.json`).
    A stock regular bullymong (level 5) and a level-8 player pawn, spawned as `--combat-run` spawns them, eight
    placements, 1.25 s each.
    - **With the live iterator weights** (Idle and Attacking 0, as L8 read them), all eight agree with the rule:
      - aware: 3,000 uu in front and clear; 800 behind and blocked (proximity: 700 < 768); shot from 6,000 behind
        (level 3, `ForgetTime` + 10);
      - not aware: 4,200 in front (no record: beyond 4,096); 3,000 in front but blocked; 3,000 behind and clear (the
        cone); 900 behind (800 > 768); exactly side-on (dot 0: the installed mind's `SightViewCone` is 0.0, see open
        items).
    - The priority at 3,000 uu, clear, as the current target, is 0.4904641 = (1 - 3000/4096 + 1 + 0.4) / 3.4, the live
      formula.
    - **With the cooked weights** (Idle 0.5, Attacking 0.25), every case that reaches prioritising stops on
      `WillowTI_PrioritizeAttacking`, as designed.
  - **Set-ups of the run, not rules.**
    - It runs in Reference mode: in Off, the stock spawn stops on a definition-store limit outside this lane (a
      reference into a class default object does not load; `--combat-run` shows the same).
    - The component's `AIDef` and `TargetingDef` are set as the live game shows them.
    - The exposure is given per case: there is no level geometry headless.

- **Open items.**
  - **Analyst questions** (each rule answers `NotYet` or is noted; no executable was read here):
    - Which native binds `AIComponent.AIDef` and `TargetingDef` when the AI starts, and from where?
    - What does an actor's `ITargetable` "can be targeted" answer test?
    - What writes the level 1 that a fresh record shows, when the code writes 0?
    - Which code lowers a threat level after `ForgetTime`, and when does it remove the record instead?
    - How does `WillowTI_PrioritizeAttacking` fill its meter from the hit history, and does it run at weight 0?
    - What does `WillowTI_PrioritizeIdle` do with a `MaxAICurrentlyTargeting` of 0 or less? It read 0 live (L8); here,
      with a weight, it is `NotYet`.
    - How is a den member's parent found (`GetParentComponent`)?
    - What does `WillowMind.CanTickAI` test?
    - What are the three pawn notifications of `TargetChanged`?
  - **The cooked against the live weights** of `GD_AI_Targeting.Targeting_ShortRange` (Idle 0.5 and Attacking 0.25
    cooked; 0 and 0 live), and the fifth Knowledge entry (`WillowTI_CalcCombat` cooked, None live). This looks like
    an overlay applied at run time (open question 19, micropatch), or a per-pawn copy. It decides whether the
    installed slice can pick a target without the two unread iterators.
  - **The cone edge** (question 17). The installed `WillowMind` class default `SightViewCone` is 0.0; the live mind
    read -4.371e-8. With a strict test, exactly side-on is outside with 0.0 and inside with the live value.
  - **Seams in other crates** (asked here, not made):
    - `ow-world`: the mind's actor tick calls `tick_mind`.
    - `ow-host`: implements `Host` (exposure from H4's collision, the registry).
    - `ow-combat`: registers `awareness::vm::Pack` and calls `install` where a combat run wants AI.
    - The population lane: dens and `has_parent`.
  - **Part 2:** the action tree, the `Target_*` events to the actions, `SetLookAtTarget`, and the records as script
    objects (`WillowActionSequencePawn` reads `TargetRec`).
- **bl-sdk reads that would confirm the lone rules** (`tools/real_game/`, a session the maintainer asks for):
  1. A bullymong with no den (or `GetParentComponent()` None); read its `AIComponent.Targets` records while
     placing Maya 4,000 and 4,200 uu in front (the 4,096 cut and the origin at the pawn).
  2. The same pawn with Maya behind at 3D distance minus `CollisionRadius` of 760 and 776 (768).
  3. Maya at 85 and 95 degrees, held facing (the cone edge), and the mind's `SightViewCone` on that pawn.
  4. `ForgetTime` after a notice (+2.5) and after a shot (+10) on the pawn's own record, and the fall.
  5. `Targeting_ShortRange.Prioritization[*].Weight` and `Knowledge[4]` in the running game against the cooked
     values.
  6. `AIComponent.AIDef` / `TargetingDef` on a freshly spawned pawn (who binds them).
- **UNVERIFIED.** Every rule above, as read; the lone case was never seen live.
