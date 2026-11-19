## 2026-11-19: InitPlayerSkillTree in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-INIT-PLAYER-SKILL-TREE, slice links 1, 3, 4, 5, 13 and 14. Coded
from docs/verification/NATIVE_INIT_PLAYER_SKILL_TREE.md alone, the note's text at 4700171f (clean room: lane
ANALYST-INIT-PLAYER-SKILL-TREE wrote it, decision 2026-11-19-78, PR #466). The executable and the analysis output were
not opened. Synthetic fixtures only.

- **What is bound (`ow-skills` `tree/init.rs`, Off only).** `WillowPlayerController.InitPlayerSkillTree()`, by its
  declaration's native key, in `--combat-run`'s world (and so the slice gate's links that use it). The note's
  section 5 is followed case by case.
- **Preflight, before any write, in the note's order.**
  - P1: `PlayerSkillTree` must be declared as an object property (otherwise a data error).
  - P2: a set tree is the named `NotYet` (`NotRead`) for the reset of an existing tree. The only installed caller
    clears the field first.
  - P3: the `WillowGame.PlayerSkillTree` class, with the store's own error unchanged.
  - P4: the world's globals (its own `NotYet` when there are none) and their `ThePlayerPawnDataManager`.
  - P5: the controller's `PlayerClass` and its `SkillTreePath`, each checked to be declared with its kind.
  - P6: a request exactly when P4 and P5 both give one.
- **Writes.**
  - W1: a new tree, with the controller as its outer and the VM's generated name.
  - W2: the controller's `PlayerSkillTree`.
  - W3: one pending request, with all seven arguments, recorded in the VM service `PendingSkillTreeLoads` for a
    dotted path. A dotless path, the empty string included, records nothing.
  - W4: the no-op reset of the new tree.
- **The note's Off assumptions, each UNVERIFIED and each one named function, as the note asks for the fork.**
  - A1: the definition is never already loaded.
  - A2: the immediate-load switch is off.
  - A3: nothing is delivered. A later lane delivers the pending request to its `PlayerContext` through
    `RunStreamingDataEvent`.
  - A4 and A5: the reset of the new tree changes nothing, and no client event is sent.
  - Where the game has Maya's definition already loaded at the class change (the note's open question 3), it fills
    the tree inside the call. Off fills it later or never. That changes when the tree fills, not what
    `OnPlayerClassChange` does next.
- **After a write.** With A1 to A5, no `NotYet` can follow W1. Should one surface, it stops the call with the "stopped
  after partial writes: " prefix, once, keeping its kind and record. Nothing is rolled back, and other errors
  propagate unchanged.
- **Tests.** `tree/init_tests.rs` has seven tests on an invented `WillowGame.upk`:
  - the slice minimum (the tree, its outer, the seven-argument record);
  - the dotless, empty, no-manager and no-class cases;
  - an existing tree refused with nothing made;
  - a world without globals;
  - a tree field of another kind;
  - the partial-writes mark;
  - Reference.
- **Installed `--slice-gate`, Off, strict: 0 ok / 14 not yet / 0 error, as before.**
  - Links 1, 3, 4, 5, 13 and 14 no longer stop at `InitPlayerSkillTree`. They still stop at `maya:8`, now at "stopped
    after partial writes: value resolver class WillowGame.BlackMarketUpgradeAttributeValueResolver: not read from
    native code or script yet".
  - `UIInteraction.GetLoginStatus`, the note's likely next stop, is not the stop on this run.
  - The other eight rows are byte-identical against the same build without the new pack.
- **UNVERIFIED.** Nothing here was checked in the running game. Still open, from the note: residency (open question
  3), the immediate-load switch (2), the delivery receiver (4), the reset of a tree with no branch list (1), and the
  existing-tree reset.
