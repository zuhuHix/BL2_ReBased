## 2026-11-19: Read the class starting values natives as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane ANALYST-CLASS-STARTING-VALUES, analyst only. Lane H6 (decision 2026-11-19-46) found that
`WillowPlayerController.ApplyCharacterClassStartingValues` ends with a parameterless script call that
NATIVE_COMBAT_GROUNDWORK.md section 1.10 left unnamed. The new own-words note
`docs/verification/NATIVE_CLASS_STARTING_VALUES.md` and two `NATIVE_INDEX.md` rows record, as behaviour only:

- The base `Controller.ApplyCharacterClassStartingValues`: delete every pool of the manager; then, with a class, its
  `ResourcePools` in order, the accuracy pool and the off-hand accuracy pool. Nothing else, and no role check.
- The player-controller override: the base first. Then, on the authority only: the experience, skill-cooldown and
  melee-cooldown pools when the controller has a `PlayerClass`, and after them the script event
  `OnPlayerClassChange`, with or without one.
- `OnPlayerClassChange` (script): the skill tree rebuilt; the character name and movement type through the
  replication info; the input devices.

NATIVE_COMBAT_GROUNDWORK.md section 1.10 step 2 now names the event and corrects its gate (the authority, not the
`PlayerClass`). Raw output stays under ignored `local/analysis/classsv/`; the Ghidra runs were read-only and took the
machine lock. Nothing is implemented: the natives stay unbound or `NotYet` in Off until a fresh lane codes them from
the note. Every rule is UNVERIFIED in the running game; the note lists the bl-sdk checks.
