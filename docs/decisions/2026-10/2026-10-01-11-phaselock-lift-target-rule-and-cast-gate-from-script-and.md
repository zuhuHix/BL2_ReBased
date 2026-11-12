## 2026-10-01: Phaselock lift, target rule and cast gate from script and data

AI-assisted. Host and tooling; no package parsing change; nothing compared against the original game.

- `LiftActionSkill.BeginLifting` (read, not run) computes the lift end from the pawn's centre: a trace down
  `HeightFromGround`; on ground the end is ground + collision half height + `HeightFromGround`; a trace up the
  centre's path lowers the end to a surface minus the half height. The host now does the same with a 1 uu box on
  `ECC_Visibility` and its targets' collision bounds standing in for the cylinder (UNVERIFIED). The rule only traces
  the centre's path, and the bob is not clamped.
- Correction to the earlier record: "Phaselock can lift the target into ceiling beams" was a misreading of the
  screenshot. Measured in the lane, the nearest surface is about 460 uu above the target's centre and the lifted
  target's top stays about 160 uu below it; a beam nearer the camera hides the upper part of the view. The suite
  exercises the clamp with a labelled test fixture (an invisible blocking box), because the lane has no low surface.
- `SelectTarget`/`CanPhaseLockTarget` are followed as far as the host has state: alive, not already locked, and a
  host property standing for `Flag_Skills_CanPhaseLock`. A blocked target is not lifted; its damage is not applied
  because the amount is not recovered. The targeting range is the auto-aim data's `MaxTargetDistance`
  (`GD_Autoaim.Default`); the native `GetPreferredTarget` is not reproduced and the view ray plus 30 cm sweep stay a
  host stand-in (UNVERIFIED).
- `Skill_Phaselock.SkillConstraints` evaluators are native. The host maps weapon action to "not reloading", healthy
  to "health above 0" and treats on-foot as always met (UNVERIFIED). While-active constraints, ladders, rider seats,
  friendliness, vehicles and Resurrect are not modelled.
- `tools/prepare_action_skill.py` writes format `openwillow.action_skill/2` (constraint evaluators, the
  `CanLiftTargetIf` flag chain, auto-aim data). A /1 manifest makes Phaselock unavailable with a message; regenerate.

