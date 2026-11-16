## 2026-11-16: Rust skills, skill trees, the action skill and Phaselock

AI-assisted (Claude Code). Lane W6-SKILLS of the Rust port fills `crates/ow-skills/src/{skills,tree,action_skill,phaselock}.rs`
and their tests (`skills/`, `tree/`, `action_skill/`, `phaselock/`), architecture section 8, Wave 6. No parsing
behaviour changes and no file outside the crate changed except the new, comment-only `tests/rust_parity.d/skills.txt`
(granted). The rules come from NATIVE_SKILLS.md, NATIVE_ACTION_SKILL.md, NATIVE_PHASELOCK_TARGETING.md,
NATIVE_ATTRIBUTES.md and PHASELOCK_STOCK_DATA.md; the Python references are `tools/skill_stats.py` and
`tools/prepare_skill_effects.py`. The installed script was read locally with `ow-package --disasm` (listings under
ignored `local/analysis/`, not quoted). There was no real-game session: every rule keeps its record's label, and
**everything not listed as seen live is UNVERIFIED in the running game**.

- **Shipped shapes.**
  - `skills`: `SkillError` (`NotYet`, `Attr`, `Vm`, `BadData`; causes through `source()`), the enums (`SkillType`,
    `SkillState`, `DurationType`, `EffectTarget`), `SkillDef` / `SkillEffect` / `Constraint` / `Bonus` and
    `read_skill`; the rules `passes_min_grade_test`, `update_grade`, `bonus_at`, `modifier_value`, `shown_value`; the
    tool views `Reader` (`constant_of`, `classify` into `InitValue`), `expression` / `conditions` (`Test`,
    `Condition`), `skill_entry`, `effect_rows`, `format_number`, `localization_key`, `read_localization`,
    `read_presentation`, `skill_lines`, and `local_output`.
  - `tree`: `TreeDef` / `BranchDef` / `TierDef` and `read_tree`; `SkillTree` (`initialize`, `reset`, `set_grade`,
    `upgrade`, `run_unlock`, `skill_state`, `branch_state`, `tier_state`, `points_spent`, `has_trained_of_type`,
    `grade`); the restated script `can_upgrade` (`UpgradeFailure`), `upgrade_cost`, `server_upgrade`; the `Trees` VM
    service and `Pack` (six `PlayerSkillTree` natives), and `register_player_skill_query`.
  - `action_skill`: `StartInputs` / `StartDecision` / `decide_start` and `cooldown_flags`; `state_from_constraints`,
    `calculate_state`, `evaluate`, `is_skill_active`, `calculate_modifier_value`; `Pack` (nine natives); `drain` (the
    cooldown pool's per-frame update, `Drained`); `run_skill_behavior` and `skill_behavior_handlers`.
  - `phaselock`: `lock_duration`, `maya_lock_duration`, `Schedule`, `repeat_target_scale`, `AutoAim` / `Candidate` /
    `Scored` / `magnetism` / `score` / `pick_target`, `bob_target_z`, `vinterp_to`, and `Pack` (`VInterpTo`).
- **Natives are bound by function path** (`ow_combat::hit::bind`): the key is the one the loader derives from the
  installed declaration, a function the VM does not declare is skipped, and the first binding of a key wins. So no
  key is hand-written and this crate never collides with `ow-combat`'s evaluator natives. Bound: `PlayerSkillTree`
  `Initialize`, `UpgradeSkill`, `SetSkillGrade`, `GetSkillPointsSpentInTree`, `GetActionSkill`,
  `HasTrainedASkillOfType`; `SkillEffectManager.IsSkillActive`; `SkillDefinition.DoesSkillPassMinGradeTest`;
  `Skill.CalculateModifierValue`, `CalculateModifierValueFromDefinitionEffectArray`,
  `CalculateStateBasedOnConstraints`; the `Evaluate` of `AttributeExpressionEvaluator`,
  `VehiclePassengerExpressionEvaluator`, `HealthStateExpressionEvaluator`, `WeaponActionAvailableExpressionEvaluator`;
  `Object.VInterpTo`. `AttributeExpressionEvaluator.Evaluate` and `VInterpTo` are not skill natives (the attribute layer
  and Core); they are bound here because no pack binds them and these rules need them. If their owners bind them
  later and register first, theirs win.
- **What runs as installed script** (architecture R2) and is not restated: the cooldown's start
  (`StartActiveSkillCooldown` through the pool's own `RefillPercentage`), `IsActionSkillOnCooldown`,
  `ActionSkillCallback`, `SkillEffectManager.ActivateSkill`, `Skill.Activate` / `Deactivate`, the skill behaviors'
  `ApplyBehaviorToContext`, and the lift's `GetLiftLocation` / `GetBobLocation` / `UpdateLiftedPawn`. Restated for
  callers without a VM, each a second implementation until a differential test exists (Open):
  `ServerStartActionSkill`'s decision list (`decide_start`), `CanUpgradeSkill`, `GetSkillUpgradeCost` and the point
  half of `ServerUpgradeSkill`. `Schedule` and `bob_target_z` restate script too, as oracles the installed-game run
  compares the script against.
- **Choices where the record is silent, each UNVERIFIED.**
  - The tree: the branch walk opens nothing while the branch's own first tier is locked (that tier opens only by the
    reset or by the parent, the record says; a first tier's cumulative requirement is 0, so a walk from it would open
    it on the first grade the load path sets). The child branches are checked in the same call as the walk. Branch
    points count every skill of its tiers; tier points count the first `cells` skills (the layout row's occupied
    cells, 3 without a layout). A skill outside every tier (a proficiency) is `NotYet { NotRead }` to upgrade, to
    report and to test for a trained type. `Initialize` builds the tree with no proficiencies (the globals' field is
    not named in the record).
  - The constraint check: each failing constraint sets the result and only a Deactivated one stops (a Paused failure
    followed by a Deactivated one gives Deactivated). Phaselock's three constraints all fail to Deactivated, so both
    readings of the record's sentence agree for it.
  - The evaluators: "in a vehicle" is the context's pawn being an `Engine.Vehicle` (that branch is `NotYet`); the
    health evaluator asks the pawn's own `IsAliveAndWell` / `IsInjured` / `IsDead` binding, and none or several flags
    are `NotYet`; the weapon evaluator needs a player controller and calls its installed `CanPerformWeaponAction(0)`.
  - The grade rule's bonus: the largest `GradeToApplyAt` at or below the grade, the first of equal grades.
  - The drain: `SetCurrentValue` is called only when the new value differs from the current one (the idle stamp is
    not moved by a step that changes nothing); a step that reaches or leaves an extreme is `NotYet` only when the
    definition lists behaviors for those events; a pool class other than `Engine.ResourcePool` and an attached
    regeneration pool are `NotYet`; idle regeneration needs the world time (`NotYet` without a level).
  - The target score is computed in doubles through the `ow_core::num` choke points; the native's precision is not
    read.
- **Where the Rust differs from the Python tools, on purpose** (each pinned by a test): single-precision numbers (the
  tools widen to doubles and round to 6 places); the grade rule's bonus term (`skill_stats.py` leaves it out, no
  Python case has one); an untagged start grade is the decoded struct default (`skill_stats.py` assumed 1, no Python
  case relies on it); a constant attribute is one whose value chain is exactly one `ConstantAttributeValueResolver`
  (the tool took the first child export of that name); an untagged effect target is `TARGET_None` (the tool's `None`);
  `local_output` resolves `..` lexically (the tool follows symbolic links).
- **No stand-ins.** No rule here has a `ParityMode::Reference` form: library code takes no mode, and nothing in
  architecture 6.1 is this lane's.
- **Findings for the maintainer** (records not edited by this lane).
  - The bob's constant: the installed `GetBobLocation` multiplies by the single nearest to pi (`0x40490FDB`, found in
    the export's bytecode). NATIVE_PHASELOCK_TARGETING.md section 5 quotes `3.14159`, which is how the listing prints
    that float (six significant digits); with `3.14159f32` the script's bob differs by one or more ulps. The live
    confirmation (RMS 0.001 uu) is unaffected.
  - The repeat-lock row: PHASELOCK_STOCK_DATA.md's "same target again within 25 s: 3.7 s" takes the target's time scale
    as `1 - 0.4 = 0.6`, the fitted form it labels UNVERIFIED. The stack rule read later from the executable
    (NATIVE_SKILLS.md 5.2, NATIVE_WEAPON_RULES.md section 1, `ow-attr`) divides a negative Scale: `1 / 1.4`, so about
    4.27 s. Neither was seen live; `repeat_target_scale` follows the stack rule.
- **Contract problems** (stopped, not patched).
  - `EngineQuery::PlayerSkill`: `ow-attr`'s resolver reads `Grade` on the object the query returns, but the record's
    value is the grade in the tree, the native tree's own memory. The active `Skill` object's grade is `max(g, 1)` and
    exists only while the skill runs, so returning it would answer a wrong grade for a trained skill between casts. The
    handler this lane registers answers `NotYet { NotPorted }` naming the gap; a grade-valued query is the fix.
  - Script context resolvers: `Att_Phaselock_Duration` (and `Cooldown_Phaselock`, the cooldown pool's maximum) resolve
    through `DesignerAttributeContextResolver`, which is script; `ow-attr`'s `ContextResolver` gets an `AttrWorld`, not
    the VM, so it answers `NotYet { LayerAbsent("ow-vm") }`. The game's duration chain stops there.
  - The behavior kernel's script-facing natives (`RegisterBehaviorConsumer`, `IntializeBehaviorProviderForConsumer`,
    `BeginNondeterministicProviderRegistration`, the `ActionSkill.On*` event senders) are bound by no pack (only
    `ow-mission`'s bridge binds some, for its run), so `Skill.Activate` and the cast stop at them.
  - `registry::HandlerFn` gives no consumer (decision 2026-11-15-30), so the skill behavior handlers are set per
    provider (`skill_behavior_handlers(consumer)`), as `ow-combat` does.
  - Unread Core natives the lift and the designer attribute reach: `Object.VLerp` (`GetLiftLocation`) and
    `Object.PathName` (`GetInstancedDesignerAttribute`, `InstancedDesignerAttribute.Initialize`). Neither is read in any
    record, so neither is bound.
- **Verified (automated, local, pc-hummer).**
  - 58 `ow-skills` unit tests on synthetic objects: every case of `tests/skill_stats_test.py` (5 tests: the seven
    number formats, the two localization keys, the effect values, the three traced line shapes, the effect rows) and of
    `tests/skill_effects_test.py` (5 tests: the skill fields, the effect values, the conditions, the output folder, the
    formula value) re-expressed with the Python's answers, plus a test of every other rule above and each `NotYet`.
  - The installed game (`installed_game_maya_skills_duration_and_lift`, ignored, `OW_BL2_ROOT`, Off mode; output
    `local/p2/w6-skills/maya_installed.txt`): Maya's tree reads as 4 branches (1 + 3 x 6 tiers, cumulative 0 to 25,
    every skill within its cells, 19 of 19), the action skill opens the 3 trees, Motion's second tier opens at 5 points,
    Suspension takes grades 1 to 5 and refuses a sixth, 11 points spent. The skill manifest gives 31 skills, 50 effect
    rows, 32 on an attribute and a target, 34 constraint rows: the counts NATIVE_SKILLS.md 4.8 recorded for the
    2026-11-03 install. Suspension's info box shows the lock duration's change at 5 of 5 grades. The lock duration,
    from the installed `InstancedDesignerAttribute.SetBaseValue` on the attribute's base, Suspension's modifier from
    this crate's `Skill.CalculateModifierValue` native on the installed effect data, and `ow-attr`'s stack, is
    bit-equal to `5 + 0.5 g` at 6 of 6 grades. The installed `GetBobLocation` with `VInterpTo`, over 228 frames of an
    uneven frame sequence, is bit-equal to the rule confirmed live at 228 of 228 frames (its target before smoothing at
    109 of 109 times). Where the chains stop: the game's duration chain at the designer context resolver, the pawn's
    instanced attribute at `Object.PathName`, the lift at `Object.VLerp`, the cast at
    `ActionSkill.OnActionSkillActivated`. This shows the installed script, the VM and these natives agree with the
    records; it is not a new real-game observation.
- **Open items.** The contract items above; reading `VLerp` and `PathName` (Core, cheap); a differential test of
  `decide_start`, `can_upgrade` and `server_upgrade` against the installed script once the kernel natives exist;
  `Skill.AddSkillEffect` / `AdjustModifiers` (the applied-effect record struct is not named in the record), the
  manager's per-frame refresh and expiry handshake, `NotifySkillEvent`, `GetActionSkillDuration`,
  `SetActionSkillTime`; the drain's idle branch with a world; the record edits above; deleting `tools/skill_stats.py`
  (architecture section 9 names this lane's gate; `tools/prepare_skill_effects.py` and both Python tests still use it,
  so it stays until the hub decides).
- **UNVERIFIED.** Every rule above as read from the executable or the script, except what the records confirmed live:
  the reload and put-down cases of the weapon constraint, the skill's end when Maya goes down while holding a target,
  the base first lock's release and end and a miss's end, the bob, the grade change reaching a running skill, and
  `Skill.UpdateGrade` keeping a skill at grade 1.
