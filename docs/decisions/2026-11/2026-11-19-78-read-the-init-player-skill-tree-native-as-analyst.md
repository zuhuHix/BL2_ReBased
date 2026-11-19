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
- **A slice minimum.** Make the tree, record a pending request for a dotted path (nothing for a dotless one), deliver
  nothing, and reset the new tree as a no-op with no client event. This is the game's own state right after the call
  when the definition is not loaded yet.

**How it was read.** The Ghidra runs were read-only and took the machine lock. Raw output stays under ignored
`local/analysis/init-player-skill-tree/`. The note passed the clean-room scan.

**Not read.**
- The controller's reset routine (its virtual slot could not be located for `WillowPlayerController`).
- The manager's immediate-load switch.
- The input devices' native.

**Status.** Nothing is implemented; the native stays unbound in Off until a fresh lane codes it from the note.
Every rule is UNVERIFIED in the running game. The note lists the bl-sdk checks. No session was run.

**Edits: review r1 of #466.** Section 5 now gives exactly one Off answer for every case. Five Off assumptions, all
UNVERIFIED, stand where the native or the game leaves the case open:
- the definition is never already loaded when the request is made;
- the immediate-load switch is off;
- delivery is a later lane's, to the requesting controller;
- the reset of a new tree is a no-op;
- no client event is sent.

The other changes:
- One table gives each combination of loaded state and path shape: the game's two answers, and Off's one.
- An existing tree is a named `NotYet`, returned before any write. The only installed caller never meets it.
- A missing tree class is the store's own error, unchanged.
- The field, kind and no-request checks now run before the allocation, and the full rule after a write is spelled
  out.
- The calls `OnPlayerClassChange` makes next go through the VM's existing dispatch. The `GetLoginStatus` stop is
  conditional on a local owner.
- The bl-sdk checks now record residency before the request, the callback's receiver, and state snapshots at each
  boundary, for a new tree and for an existing one.
