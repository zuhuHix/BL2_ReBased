## 2026-11-19: Bind the skill manager natives in Off

AI-assisted (Claude Code / Claude Opus 5.5).

Lane FIX-SKILLMGR-B, the implementer for `docs/verification/NATIVE_SKILL_MANAGER.md` (analyst lane FIX-SKILLMGR-A,
decision 2026-11-19-11). Implemented from that note alone; no analysis output was opened. Off `--combat-run maya:8`
stopped on the unbound `WillowGameInfo.GetSkillManager()`, the first statement of `ApplyCharacterClassDefaults`
(slice link 1).

New module `crates/ow-skills/src/action_skill/manager.rs` with its pack, bound by function path in `ParityMode::Off`
only:

- `WillowGameInfo.GetSkillManager()` answers the game info's `GlobalSkillManager` when it is set, without checking
  whether it was destroyed. Otherwise, if the game info's `Role` is 3, it spawns a manager through
  `ow_world::actors::spawn`. The game info is the spawner and the owner (the note reads it as the owner without proving
  the argument position: UNVERIFIED), there is no template, and the spawn is at location and rotation (0, 0, 0). The
  result is stored only if it is of the manager class, otherwise None is stored, and the stored value is returned. The
  class spawned is the one `GlobalSkillManager` is declared with (`SkillEffectManager` in the installed package), so no
  class path is hard-coded. A game info that is not the authority, with no manager yet, gets None. A world that cannot
  spawn answers the spawn's own `NotYet` (no actor list, no `WorldInfo`, the collision layer), never None.
- `SkillEffectManager.DeactivateAllSkillsForInstigator(Controller)` is a no-op when no set entry of `ActiveSkills` has
  that controller as its `SkillInstigator`. That covers an empty list (the case the note settles), empty entries and
  other instigators' skills, per the note's walk. The first matching entry is `NotYet { NotRead }`, before anything
  runs: the note did not resolve the name of the function called on it.
- Reference: the C++ has no stand-in for either native, so both stay unbound and the stub policy answers them as
  before (R3). No NotYet guard changed.

`ow-cli`'s `--combat-run` world registers the pack after `ow_combat::hit::Pack` (one line). In Off, link 1's next stop
moves from `WillowGameInfo.GetSkillManager()` to `WillowGlobals.GetPlayerPawnDataManager()`. The Reference `maya:8` run
equals the one before this change in every field but `elapsed_ms`. On `--slice-gate` (SLICE-RUN-OFF's branch with this
change on top, not stacked), link 1 and the links blocked behind it (3, 4, 5, 13, 14) now stop at
`GetPlayerPawnDataManager()`. Links 10 to 12 are unchanged at `AIComponent.ActivateEvent(name)`, and the five links
with no driver are unchanged. The gate passes 0 of 14 before and after.

Verified: synthetic tests on two invented packages (`Engine` and an importing `WillowGame`) in both modes: the spawn and
its cache, a stored manager returned as it is, non-authority roles, the `NotYet` of a world without `WorldInfo`, the
deactivate no-ops and its `NotYet`, and Reference unbound. UNVERIFIED: every rule in the running game (the note's live
checks are not run). Gate results are in the PR.
