## 2026-11-16: Rust skills, skill trees, the action skill and Phaselock

AI-assisted (Claude Code). Lane W6-SKILLS of the Rust port fills `crates/ow-skills/src/{skills,tree,action_skill,phaselock}.rs`
and their tests (`skills/`, `tree/`, `action_skill/`, `phaselock/`), architecture section 8, Wave 6. No parsing
behaviour changes. The rules come from NATIVE_SKILLS.md, NATIVE_ACTION_SKILL.md, NATIVE_PHASELOCK_TARGETING.md,
NATIVE_ATTRIBUTES.md and PHASELOCK_STOCK_DATA.md; the Python references are `tools/skill_stats.py` and
`tools/prepare_skill_effects.py`. The installed script was read locally with `ow-package --disasm` (listings under
ignored `local/analysis/`, not quoted). There was no real-game session: every rule keeps its record's label, and
**everything not listed as seen live is UNVERIFIED in the running game**.

- **Edits outside the lane's files** (each granted): the new, comment-only `tests/rust_parity.d/skills.txt`; and, in
  review r1 of PR #342, two seam edits:
  - `crates/ow-world/src/pools.rs`: `stored_value` (and its `Stored`), the value rule of `set_current` (the integer
    truncation, then its clamp order) without the write, with the bounds the clamp used (review r2); `set_current`
    now calls it, so the rule exists once. Its behaviour is unchanged (`ow-world`'s tests pass as before).
  - `crates/ow-vm/src/natives_core.rs`: `Object.VInterpTo(vector,vector,float,float)`, next to `FInterpTo` and in its
    form (each axis in double, rounded once), with its unit test. It moved there from `ow-skills` (hub decision), so a
    later Core binding cannot fail on a duplicate key. The C++ has no `VInterpTo`: a C++ sweep that reaches it logs a
    stub where the Rust VM now binds it. No tracked test reaches it; `--native-selftest` counts a private VM and is
    unchanged.
- **Shipped shapes.**
  - `skills`: `SkillError` (`NotYet`, `Attr`, `Vm`, `BadData`; causes through `source()`), the enums (`SkillType`,
    `SkillState`, `DurationType`, `EffectTarget`), `SkillDef` / `SkillEffect` / `Constraint` / `Bonus` and
    `read_skill`; the rules `passes_min_grade_test`, `update_grade`, `bonus_at`, `modifier_value`, `shown_value`; the
    tool views `Reader` (`constant_of`, `classify` into `InitValue`), `expression` / `conditions` (`Test`,
    `Condition`), `skill_entry`, `effect_rows`, `format_number`, `localization_key`, `read_localization`,
    `read_presentation`, `skill_lines`, and `local_output`.
  - `tree`: `TreeDef` / `BranchDef` / `TierDef` and `read_tree`; `SkillTree` (private fields with `def`, `skills`,
    `action_skill` and `unlocked(b, t)` readers; `initialize` with the proficiencies or `None`, `reset`, `set_grade`,
    `upgrade`, `run_unlock`, `skill_state`, `branch_state`, `tier_state`, `points_spent`, `has_trained_of_type`,
    `grade`); the oracles `tree::oracle::{can_upgrade, UpgradeFailure, upgrade_cost, server_upgrade}`; the `Trees` VM
    service and `Pack` (six `PlayerSkillTree` natives), and `register_player_skill_query`.
  - `action_skill`: the oracles `action_skill::oracle::{StartInputs, StartDecision, decide_start, cooldown_flags}`;
    `state_from_constraints`, `calculate_state`, `evaluate`, `is_skill_active`, `calculate_modifier_value`; `Pack`
    (nine natives); `drain` (the cooldown pool's per-frame update, `Drained`); `run_skill_behavior`,
    `apply_to_contexts` and `skill_behavior_handlers`.
  - `phaselock`: `lock_duration` (the stack rule on given modifiers), `Schedule` (an oracle), `repeat_target_scale`,
    and the target score `AutoAim` / `Candidate` / `Scored` / `magnetism` / `score` / `pick_target`. The bob's
    restatement (`bob_target_z`, `vinterp_to`, `SCRIPT_PI`) is a test-only oracle (`phaselock::oracle`,
    `#[cfg(test)]`), and the record's `5 + 0.5 g` lives in the tests only: the game reads both numbers from the data,
    and other modifiers (Thoughtlock and the rest) join the same stack.
- **Natives are bound by function path** (`ow_combat::hit::bind`): the key is the one the loader derives from the
  installed declaration, a function the VM does not declare is skipped, and the first binding of a key wins. Bound:
  `PlayerSkillTree` `Initialize`, `UpgradeSkill`, `SetSkillGrade`, `GetSkillPointsSpentInTree`, `GetActionSkill`,
  `HasTrainedASkillOfType`; `SkillEffectManager.IsSkillActive`; `SkillDefinition.DoesSkillPassMinGradeTest`;
  `Skill.CalculateModifierValue`, `CalculateModifierValueFromDefinitionEffectArray`,
  `CalculateStateBasedOnConstraints`; the `Evaluate` of `AttributeExpressionEvaluator`,
  `VehiclePassengerExpressionEvaluator`, `HealthStateExpressionEvaluator`, `WeaponActionAvailableExpressionEvaluator`.
  `BehaviorKernel.ActivateBehaviorOutputLink` is **not** bound here (hub decision, review r2): `ow-mission` registers it
  with `register_native` (a duplicate fails) and `ow-combat` skips a bound key (its links would come back empty), and
  the skill behaviors never call it (below).
- **Cross-owner items** (recorded, hub decision in review r1): `AttributeExpressionEvaluator.Evaluate` is an
  attribute-layer native bound here for now, because no pack binds it and the skill constraints need it; the hub
  records one owner in ARCHITECTURE.
- **What runs as installed script** (architecture R2) and is not restated: the cooldown's start
  (`StartActiveSkillCooldown` through the pool's own `RefillPercentage`), `IsActionSkillOnCooldown`,
  `ActionSkillCallback`, `SkillEffectManager.ActivateSkill`, `Skill.Activate` / `Deactivate`, the skill behaviors'
  `ApplyBehaviorToContext`, and the lift's `GetLiftLocation` / `GetBobLocation` / `UpdateLiftedPawn`. **Oracles, not
  for gameplay callers** (review r1, item 3): `action_skill::oracle` restates `ServerStartActionSkill`'s decision list
  and the cooldown tests, `tree::oracle` restates `CanUpgradeSkill`, `GetSkillUpgradeCost` and the point half of
  `ServerUpgradeSkill`, and `Schedule` restates the cast's timers. Each is for a differential test against the
  installed script, which is Open until the behavior kernel's script natives are bound.
- **Not here, and why.** `Skill.AddSkillEffect` and `Skill.AdjustModifiers` (the applied-effect record struct is not
  named in the record); the manager's per-frame skill refresh and the expiry handshake; `NotifySkillEvent` and the
  `ActionSkill` event natives (they need the kernel's consumer natives); `GetActionSkillDuration` and
  `SetActionSkillTime` (how a controller reaches its skill manager is not read); `PlayerSkillTree.GetSkillState`,
  `GetBranchState` and `GetTierState` (this crate does not write their out structs yet; the installed
  `ServerUpgradeSkill` calls `GetSkillState`, so a spend through the script stops there). Each stays unbound, so a
  gameplay run refuses at it (`StubPolicy::Refuse`).
- **Choices where the record is silent, each UNVERIFIED.**
  - The tree: the branch walk opens nothing while the branch's own first tier is locked (that tier opens only by the
    reset or by the parent, the record says; a first tier's cumulative requirement is 0, so a walk from it would open
    it on the first grade the load path sets). The child branches are checked in the same call as the walk. Branch
    points count every skill of its tiers; tier points count the first `cells` skills (the layout row's occupied
    cells, 3 without a layout). A skill outside every tier (a proficiency) is `NotYet { NotRead }` to upgrade, to
    report and to test for a trained type. A `DefaultStartingGrade` outside `0..=MaxGrade` is bad data (no clamp).
  - Proficiencies (review r1, item 2): the `Initialize` native builds the tree without the globals' proficiency list
    (its field is not named in the record) and marks it so. On such a tree `HasTrainedASkillOfType(Proficiency)`, and
    `SetSkillGrade`, `UpgradeSkill`, `GetSkillState`, the tree's grade and the two oracles that read them, for a skill
    the tree lacks whose type is Proficiency, are `NotYet { NotRead }`, so a saved grade is never dropped as "not in
    this tree" (the readers and oracles since review r2).
  - The constraint check: each failing constraint sets the result and only a Deactivated one stops (a Paused failure
    followed by a Deactivated one gives Deactivated). Phaselock's three constraints all fail to Deactivated, so both
    readings of the record's sentence agree for it.
  - The evaluators: "in a vehicle" is the context's pawn being an `Engine.Vehicle` (that branch is `NotYet`); the
    health evaluator asks the pawn's own `IsAliveAndWell` / `IsInjured` / `IsDead` binding, and none or several flags
    are `NotYet`; the weapon evaluator needs a player controller and calls its installed `CanPerformWeaponAction(0)`,
    and with no source it is `NotYet`. A value of the wrong kind (a non-object where an object is read, a non-array
    skill list, a non-bool grade flag) is bad data, never read as "nothing" or false.
  - The grade rule's bonus: the largest `GradeToApplyAt` at or below the grade, the first of equal grades.
  - The drain (review r1, item 1): a delta that is not finite or is negative is refused, as `World::tick` refuses
    it. The step's target goes through `SetCurrentValue` only when the value it would store
    (`ow_world::pools::stored_value`: the truncation, then the clamp) differs from the current one, so a pool resting
    at an extreme keeps its idle stamp; the edge test uses that stored value and the bounds the clamp used, so an
    integer truncation onto the minimum counts, and an integer-only pool with `MaxValue` 10.5 that fills to 10 has
    reached its maximum. Which bound the native's edge test uses is not in the record (review r2: the bounds used,
    rather than a `NotYet` for a fractional maximum). `NotYet`, before any write: a pool class other than `Engine.ResourcePool`, an attached
    regeneration pool, an integer-only pool whose step moves only its remainder, a step that reaches or leaves an
    extreme when the definition lists behaviors for those events, and idle regeneration without a level's world time.
  - The skill behavior handlers (review r1, item 7; r2, items 1 and 2): the installed `ApplyBehaviorToContext` runs
    once per context with `ContextObject` and `SelfObject` set, and the answer is the empty list of output links.
    Checked in the installed bytecode with `research/script_disasm.py` (local): neither
    `Behavior_ActivateSkill.ApplyBehaviorToContext` nor `Behavior_DeactivateSkill.ApplyBehaviorToContext` calls
    `ActivateBehaviorOutputLink`, and none of the six functions they reach (`Behavior_ActivateSkill` and
    `Behavior_DeactivateSkill` of `ISkillBehavior`, `WillowPlayerController` and `WillowMind`) calls it or receives the
    kernel information it takes. `Behavior_ActivateSkill` reads the instigator, the other participant and the kernel
    information only under `bUseAdditionalSkillTarget`, and `Behavior_DeactivateSkill` never, so that flag set is
    `NotYet`, that flag of another kind than a bool is bad data, and the other parameters may stay zero.
  - The target score is computed in doubles through the `ow_core::num` choke points; the native's precision is not
    read. `VInterpTo`'s precision is not read either.
- **Where the Rust differs from the Python tools, on purpose** (each pinned by a test): single-precision numbers (the
  tools widen to doubles and round to 6 places); the grade rule's bonus term (`skill_stats.py` leaves it out, no
  Python case has one); an untagged start grade is the decoded struct default (`skill_stats.py` assumed 1, no Python
  case relies on it); a constant attribute is one whose value chain is exactly one `ConstantAttributeValueResolver`
  (the tool took the first child export of that name); an untagged effect target is `TARGET_None` (the tool's `None`);
  `local_output` resolves `..` lexically (the tool follows symbolic links).
- **No stand-ins.** No rule here has a `ParityMode::Reference` form: library code takes no mode, and nothing in
  architecture 6.1 is this lane's.
- **Findings for the maintainer** (for a later docs pass, labelled UNVERIFIED; the records are not edited here).
  - The bob's constant: the installed `GetBobLocation` multiplies by the single nearest to pi (`0x40490FDB`, found in
    the export's bytecode). NATIVE_PHASELOCK_TARGETING.md section 5 quotes `3.14159`, which is how the listing prints
    that float (six significant digits); with `3.14159f32` the script's bob differs by one or more ulps. The live
    confirmation (RMS 0.001 uu) is unaffected.
  - The repeat-lock row: PHASELOCK_STOCK_DATA.md's "same target again within 25 s: 3.7 s" takes the target's time scale
    as `1 - 0.4 = 0.6`, the fitted form it labels UNVERIFIED. The stack rule read later from the executable
    (NATIVE_SKILLS.md 5.2, NATIVE_WEAPON_RULES.md section 1, `ow-attr`) divides a negative Scale: `1 / 1.4`, so about
    4.27 s. Neither was seen live; `repeat_target_scale` follows the stack rule.
- **Contract problems** (stopped, not patched; items 1 to 3 become follow-up lanes, hub decision).
  1. `EngineQuery::PlayerSkill`: `ow-attr`'s resolver reads `Grade` on the object the query returns, but the record's
     value is the grade in the tree, the native tree's own memory. The handler this lane registers answers
     `NotYet { NotPorted }` naming the gap; a grade-valued query is the fix.
  2. Script context resolvers: `Att_Phaselock_Duration` (and `Cooldown_Phaselock`, the cooldown pool's maximum)
     resolve through `DesignerAttributeContextResolver`, which is script; `ow-attr`'s `ContextResolver` gets an
     `AttrWorld`, not the VM, so it answers `NotYet { LayerAbsent("ow-vm") }`. The game's duration chain stops there.
  3. The behavior kernel's script-facing natives (`RegisterBehaviorConsumer`, `IntializeBehaviorProviderForConsumer`,
     `BeginNondeterministicProviderRegistration`, the `ActionSkill.On*` event senders) are bound by no general pack
     (only `ow-mission`'s bridge binds some, for its run), so `Skill.Activate` and the cast stop at them. A shared
     output-link collector would belong there too (`ow-mission` and `ow-combat` each bind `ActivateBehaviorOutputLink`
     for their own runs); the skill behaviors need none, as checked above.
  4. `registry::HandlerFn` gives no consumer (decision 2026-11-15-30), so the skill behavior handlers are set per
     provider (`skill_behavior_handlers(consumer)`), as `ow-combat` does.
  5. Unread Core natives the lift and the designer attribute reach: `Object.VLerp` (`GetLiftLocation`) and
     `Object.PathName` (`GetInstancedDesignerAttribute`, `InstancedDesignerAttribute.Initialize`). Neither is read in
     any record, so neither is bound.
- **Verified (automated, local, pc-hummer).**
  - 64 `ow-skills` unit tests on synthetic objects (and the ignored installed-game run): every case of `tests/skill_stats_test.py` (5 tests: the seven
    number formats, the two localization keys, the effect values, the three traced line shapes, the effect rows) and of
    `tests/skill_effects_test.py` (5 tests: the skill fields, the effect values, the conditions, the output folder, the
    formula value) re-expressed with the Python's answers, plus a test of every other rule above and each `NotYet`.
  - The installed game (`installed_game_maya_skills_duration_and_lift`, ignored, `OW_BL2_ROOT`, Off mode; output
    `local/p2/w6-skills/maya_installed.txt`):
    - Maya's tree, built as the `Initialize` native builds it: 4 branches (1 + 3 x 6 tiers, cumulative 0 to 25, every
      skill within its cells, 19 of 19); the action skill opens the 3 trees; Motion's second tier opens at 5 points;
      Suspension takes grades 1 to 5 and refuses a sixth; 11 points spent.
    - The skill manifest: 31 skills, 50 effect rows, 32 on an attribute and a target, 34 constraint rows, the counts
      NATIVE_SKILLS.md 4.8 recorded for the 2026-11-03 install. Suspension's info box shows the lock duration's change
      at 5 of 5 grades.
    - The lock duration is `5 + 0.5 g` at 6 of 6 grades, bit for bit. What this checks is the installed data (base 5,
      Suspension 0.5 g at its grades) through the installed `InstancedDesignerAttribute.SetBaseValue` and
      `calculate_modifier_value` (called directly, not through the VM); the stack rule is `ow-attr`'s on both sides,
      so it is not checked against anything but itself.
    - The bob: the installed `GetBobLocation`'s target before the smoothing equals the record's formula at 109 of 109
      times (independent of `VInterpTo`), and the smoothed bob, with `ow-vm`'s `VInterpTo`, equals the record's rule
      restated in the test (written apart from that binding) at 228 of 228 frames of an uneven frame sequence. The
      rule was matched live to RMS 0.001 uu; `VInterpTo`'s precision is UNVERIFIED.
    - Where the chains stop: the game's duration chain at the designer context resolver, the pawn's instanced
      attribute at `Object.PathName`, the lift at `Object.VLerp`, the cast at `ActionSkill.OnActionSkillActivated`.
    This shows that the installed script, the VM and these natives agree with the records; it is not a new real-game
    observation.
- **Open items.** The contract items above; reading `VLerp` and `PathName` (Core, cheap); a differential test of the
  oracles against the installed script once the kernel natives exist; the "Not here" list; the globals' proficiency
  list for `Initialize`; the drain's idle branch with a world; the record edits above (a later docs pass); deleting
  `tools/skill_stats.py` when `tools/prepare_skill_effects.py` and the two Python tests no longer import it (hub
  decision).
- **UNVERIFIED.** Every rule above as read from the executable or the script, except what the records confirmed live:
  the reload and put-down cases of the weapon constraint, the skill's end when Maya goes down while holding a target,
  the base first lock's release and end and a miss's end, the bob, the grade change reaching a running skill, and
  `Skill.UpdateGrade` keeping a skill at grade 1.
