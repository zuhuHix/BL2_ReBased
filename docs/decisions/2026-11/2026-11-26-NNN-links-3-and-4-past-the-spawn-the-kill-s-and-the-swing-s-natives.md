## 2026-11-26: Links 3 and 4 past the spawn: the kill's and the swing's natives

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-LINKS-3-4-TAIL, on fix/rust-spawn-setbasevalue (#815 at
15a2953), so the gate can measure it. Clean room: implemented from docs/verification/NATIVE_LINKS_3_4_TAIL.md (#812 as
landed) and, for the plain reads, NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md section 4.2, with the stage rules of
NATIVE_DAMAGE_DEATH.md section 1 and NATIVE_DOWN_TIMERS.md, alone; this lane did not read the executable. No session
was run. Every rule is UNVERIFIED in the running game. Off only; Reference unchanged.

- **What was decided**, in `ow-combat`:
  - **the plain reads** (`links34_tail`): `DamagePipeline.GetBarrelSourceTime`, `GetPlantSourceTime`,
    `GetCanInflictFriendlyFire`, `Pawn.IsUsingWalkingPhysics` (the `EPhysics` names `PHYS_Walking` and
    `PHYS_NavMeshWalking`) and `SpecialMoveComponent.IsPlayingAny` (`CurrentSMData.SMD` set), each a declared field, else
    `VmError::Script`. `IsPlayingAny` was unbound in Off before; `ow-skills`' special-move test now expects the read;
  - **`GameInfo.IsFriendlyFire`'s interface argument** (#812 section 3, a hub question): option (B), in its own commit so
    it can be swapped: an `Object` whose class implements the declared interface is accepted (`Vm::implements_read_only`;
    a table that would need resolving first is `NotRead`). Option (A), the VM-wide conversion, is not done. **Pending the
    hub's ruling**;
  - **`AdjustDamage`'s hit-region cap** (stage 13, `hit_region_cap`): `AddDamageToHitRegion` as section 4 reads it, a
    read-only plan that makes every refusal (the consumer of a non-Willow pawn, the Kismet event, a reaction that would
    fire, the region's death with behaviours or provider sequences, the damage-number notification, an object not
    resident, a field not declared) before one write of the helper's record list. The capped damage goes on through the
    pipeline, and `HitRegionCapReduction` is what the cap took. A reaction whose other conditions hold is refused
    whatever its `DamageTriggers` say (their fields are not named);
  - **`GetDisplayNameAtGrade`, `NotifyKilledEnemy`'s gates, `IsActionSkillRunning` and the dialog globals' `Get`** in the
    combat world (`links34_tail`), read-only, as sections 5 to 8 propose. `Get` is bound only where no pack bound it
    first; the mission bridge keeps its own;
  - **the shield stage and the protection timer** (stages 10 and 15, `shield_stage`): stage 10's plan computes the
    absorbed damage and the remainder; the pool's write waits until after stage 15's decision at the end of the pass, so
    a refusal in either writes nothing of both. Refused in the plan: a player instigator (its controller's damage-number
    notification and the penetration roll), a shield that would reach 0 (`OnShieldDepleted`), and **any absorbed damage**,
    because the victim's recent-damage record of the shield side names a tracker field the note does not name (the
    packages hold several candidates; none is picked). Stage 15: a player victim of an AI, the globals definition
    resident, its three fields declared; the timer's start is `NotRead`; an active timer keeps
    `MinimumHealthMaintainedByProtectionTimer`, with `ProtectionTimerReduction` written;
  - **the combat set-up preloads the globals definition's `Dialog`** when it is a handle the store has not loaded, logged
    as a harness input (UNVERIFIED; the shape of #808's manager-input preload).
- **Not done:** row 6 of link 4 (`DoForceFeedbackWaveform`'s order fix, section 9) was not in the brief; row 4 (Maya's
  class-level resistance) needs #796's deferred `PlayerPawnDuringSetUp`.
- **Tests** (invented classes and the combat run's fixture): the hit-region cap (`hit_region_cap_tests.rs`: nothing for
  a clear flag or no helper, an unlimited region summed, a limit of 0.5 capping 60 to 50 and then 0, an independent region
  0 with its refusal, an allowed-type list, the death's behaviours and an unloaded provider refused before any write, a
  reaction that would fire, the Kismet event and a non-Willow victim, a mistyped list); the natives
  (`links34_tail_tests.rs`: the pipeline reads, `GetDisplayNameAtGrade`'s playthrough source and its refusals,
  `NotifyKilledEnemy`'s gates, `IsActionSkillRunning`, the dialog globals); stage 15 (`shield_stage_tests.rs`); stage
  10's depletion and record refusals (`run/tests.rs`); `IsFriendlyFire`'s object case (`link34_tests.rs`). **Not
  tested:** the cap's feedback through a whole `AdjustDamage` pass (the toy run's regions do not track damage); the
  installed gate runs it.
- **Measured:** see the GATES row of this lane.
- **Verified:** the tests on invented data. **Not verified:** everything in the running game.
- **Sensitive files:** none.
