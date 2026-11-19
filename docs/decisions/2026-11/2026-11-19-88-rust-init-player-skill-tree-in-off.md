## 2026-11-19: InitPlayerSkillTree in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-INIT-PLAYER-SKILL-TREE, slice links 1, 3, 4, 5, 13 and 14. Coded
from docs/verification/NATIVE_INIT_PLAYER_SKILL_TREE.md alone, the note's text at 094415f3; #466 is accepted, rebased onto main unchanged and waiting to merge (clean room: lane
ANALYST-INIT-PLAYER-SKILL-TREE wrote it, decision 2026-11-19-78, PR #466). The executable and the analysis output were
not opened. Synthetic fixtures only.

- **What is bound (`ow-skills` `tree/init.rs`, Off only).** `WillowPlayerController.InitPlayerSkillTree()`, by its
  declaration's native key, in `--combat-run`'s world (and so the slice gate's links that use it). The note's
  section 5 is followed case by case.
- **Preflight, before any write, in the note's order. The preflight reads only resident definitions** (hub ruling: a lazy load is a write). A globals object or a
  player class that is still an unloaded store handle is a named `NotYet` (`NotPorted`), and nothing is loaded.
  - P1: `PlayerSkillTree` must be declared as an object property (otherwise a data error).
  - P2: a set tree is the named `NotYet` (`NotRead`) for the reset of an existing tree. The only installed caller
    clears the field first.
  - P3: the `WillowGame.PlayerSkillTree` class, looked up once at setup. The whole result is kept in a VM service,
    and P3 answers it, its original error included, without a second lookup. The tree field's declared
    class must accept it, else the heap's typed `WrongClass` error, before the tree is made.
  - P4: the world's globals (its own `NotYet` when there are none) and their `ThePlayerPawnDataManager`.
  - P5a: the controller's `PlayerClass`, read whether or not there is a manager.
  - P5b: its `SkillTreePath`, read only when there is both a manager and a player class.
  - Each is checked to be declared with its kind. A stored value of another kind is the note's data error ("holds a
    <kind>, not ...").
  - P6: a request exactly when P4 gives a manager and P5a a player class.
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
- **Tests.** `tree/init_tests.rs` has eight tests (eleven after review r1 of #475, thirteen after r2) on an invented `WillowGame.upk`:
  - the slice minimum (the tree, its outer, the seven-argument record);
  - the dotless, empty, no-manager and no-class cases;
  - an existing tree refused with nothing made;
  - a world without globals;
  - a tree field of another kind;
  - the partial-writes mark;
  - `SkillTreePath` read only for a request (P5b);
  - Reference.
- **Installed `--slice-gate`, Off, strict: 0 ok / 14 not yet / 0 error, as before.**
  - Links 1, 3, 4, 5, 13 and 14 no longer stop at the `InitPlayerSkillTree` check. They still stop at `maya:8`, now
    at "stopped after partial writes: value resolver class WillowGame.BlackMarketUpgradeAttributeValueResolver: not
    read from native code or script yet".
  - **That stop comes before the event.** `ApplyCharacterClassStartingValues` checks the event, then makes the
    override's pools, then runs `OnPlayerClassChange`. The installed run stops while making the pools, and a temporary
    local probe (not committed) saw no `InitPlayerSkillTree` call. So the native is not yet exercised on the installed
    run, and neither is the `IsInvalidName` and `GetLoginStatus` path after it. The synthetic tests are its only
    evidence so far.
  - **`UIInteraction.GetLoginStatus` is script, not a native** (the #474 review supersedes #466's wording). This lane
    binds and stubs nothing for it: the script dispatches through the VM, and `GameEngine.GetOnlineSubsystem` and the
    Steamworks `GetLoginStatus` are lane -95's.
  - The other eight rows are byte-identical against the same build without the new pack.
- **UNVERIFIED.** Nothing here was checked in the running game. Still open, from the note: residency (open question
  3), the immediate-load switch (2), the delivery receiver (4), the reset of a tree with no branch list (1), and the
  existing-tree reset.

**Review r1 of #475.**
- The preflight reads only resident definitions; P3's class is resolved at setup and checked for assignability.
- The request queue's mutable borrow is taken before W1, so a borrowed queue refuses before anything is made, and a
  retry then queues the request.
- The partial-writes mark is `ow_world::pools::after_partial_writes`, which checks the exact leading prefix.
- P2's `NotYet` carries the note's exact text.
- Tests: eleven. Three are new: an unloaded player class, an incompatible tree field, a borrowed queue. The P2 test
  asserts the full message.

**Review r2 of #475.**
- Setup keeps the tree class lookup's whole result, and P3 returns its original error. No lookup runs in the
  preflight, and the direct entry point reads the same setup result.
- P4 is back before P5a, in the note's order: with no globals and a bad `PlayerClass`, the world's `NotYet` comes out
  first, unchanged. The note's "PlayerClass first" is about the native's request test only.
