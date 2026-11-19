## 2026-11-19: Read the InitPlayerSkillTree native as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane ANALYST-INIT-PLAYER-SKILL-TREE, analyst only. `WillowPlayerController.InitPlayerSkillTree` is the native that
links 1, 3, 4, 5, 13 and 14 of the slice gate stop at (`maya:8`, inside the installed `OnPlayerClassChange`).

**What was added.** An own-words note, `docs/verification/NATIVE_INIT_PLAYER_SKILL_TREE.md`, plus `NATIVE_INDEX.md`
rows: one row updated and two added. The note records, as behaviour only:

- **The native.** With no tree yet: it makes a `PlayerSkillTree` with the controller as its outer and stores it. With
  a `PlayerClass` and the globals' pawn data manager both set, it asks the manager to load the class's
  `SkillTreePath` with the completion event `SkillTreeDefinitionLoaded`. Then, always, it runs the controller's reset
  with both flags false. It refuses nothing.
- **The manager's request.** A definition that is already loaded is delivered at once, inside the call. Otherwise the
  request is queued for a streaming load, or loaded at once under an engine switch that was not read. A path without
  a dot is dropped silently.
- **The completion (script).** `PlayerSkillTree.Initialize`, then the listener registration, then a one-shot timer for
  `NotifyReadyToLoadPendingSavegame`.
- **What `OnPlayerClassChange` reaches next (script).** The movement type and the default name are script only. The
  likely next stop for a local player is `UIInteraction.GetLoginStatus`, through `IsInvalidName`. The input devices'
  native is not read.
- **An Off answer list.** Preflight reads first, then writes, under the hub's write rule.
- **A slice minimum.** Make the tree, record the request as pending without delivering it, and reset the empty tree.
  This is the game's own state right after the call when the definition is not loaded yet. Whether Maya's definition
  is already loaded at a class change is an open question, and the minimum is labelled UNVERIFIED for that reason.

**How it was read.** The Ghidra runs were read-only and took the machine lock. Raw output stays under ignored
`local/analysis/init-player-skill-tree/`. The note passed the clean-room scan.

**Not read.**
- The controller's reset routine (its virtual slot could not be located for `WillowPlayerController`).
- The manager's immediate-load switch.
- The input devices' native.

**Status.** Nothing is implemented; the native stays unbound in Off until a fresh lane codes it from the note.
Every rule is UNVERIFIED in the running game. The note lists the bl-sdk checks. No session was run.
