## 2026-10-01: progression in the quest save; health follows level

AI-assisted. Host work; no package parsing change; nothing compared against the original game.

- The quest save (`-owquestsave=`) gains an optional `progression` block: level, experience, action grade and skill
  grades (available points are written for checking only; they are derived from level and grades). A save that has
  the block replaces the `-owlevel` start level before weapons are equipped; saves without it load as before. A block
  that fails validation (experience outside its level, unknown skill, grade above its maximum, more points spent than
  earned) is rejected and the quest fails, as for a rejected mission state.
- Maximum health is recomputed with the same formula whenever Maya's level changes. Current health on a level-up is
  read from installed data: `WillowPlayerController.OnExpLevelChange` (script) calls the native
  `RecalculateAttributeInitializedState` and then runs `CharClass_Siren.OnLevelUp` =
  `GD_PlayerShared.Behaviors.PlayerBehavior_LevelUp`, whose skill definition adds `HealthMaxValue` x 1 to
  `HealthCurrentValue` (`MT_PostAdd`). The host refills to the new maximum.
- UNVERIFIED: that the pool caps the sum at the maximum; that the timed effect acts once as a heal; the 1 s guard
  between level-ups (not modelled); the argument order of `OnExpLevelChange` (taken from export order); level
  decreases (test fixtures only, treated the same). The health formula's own open constant is unchanged. The same
  skill definition also scales the action-skill cooldown; the host does not model that.
- Checks: four new first-run checks and three new resume checks (names in the record), a synthetic round trip in
  `OpenWillow.Skills` (not run in this pass). Record: `docs/verification/SANCTUARY_RPG_MISSION.md`, "Progression,
  Phaselock rules and dummy behaviours".

