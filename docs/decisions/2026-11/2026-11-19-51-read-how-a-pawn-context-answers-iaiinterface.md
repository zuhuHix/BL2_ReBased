## 2026-11-19: Read how a pawn context answers `IAIInterface` (analyst only)

AI-assisted (Claude Code, Claude Opus 5.5). Lane ANALYST-PAWN-AI-INTERFACE. The prepared bl-sdk session could not run,
because the maintainer is away, so this closes NATIVE_AI_HOLDS.md's open row "How a pawn context reaches
`IAIInterface`" by native analysis instead. The script was read with `research/script_disasm.py`, and the natives with
Ghidra read-only under the machine lock. Raw output stays under ignored `local/analysis/`. The outcome is the own-words
note `docs/verification/NATIVE_PAWN_AI_INTERFACE.md`. No code changes; a fresh implementer codes it from the note.

- **The rule.** `Behavior_AIHold` asks its context with `Object.QueryInterface(IAIInterface)`. A pawn answers by
  `Pawn`'s rule, which no pawn class below it changes for this interface:
  1. its own class's declared interfaces;
  2. else its `Controller`, or, with none, the controller of the vehicle it drives;
  3. else its `Mesh`;
  4. else none.

  `WillowAIPawn` adds one extra answer, for `IPlayerMaster` only, after that rule.
- **For the Fire dummy** (link 11): the pawn's class lists no `IAIInterface`, so its controller, the mind, answers,
  and the hold goes to that mind's `GetAIComponent()` with the pawn as the tag. With no controller (and no driven
  vehicle), the hold does nothing.
- **The port's assumption** (`ow-combat`'s `ai_interface_for`, Reference only): confirmed that the mind answers for a
  possessed AI pawn. It differs in three ways: the rule goes through `Controller`, not `MyWillowMind`; with no
  controller it answers none, not the pawn; and the pawn's own class comes first.
- **Not read:** `GearboxMind`'s own answer, `WillowPlayerController`'s answer for `IAIInterface`, a skeletal mesh
  component's answer, and whether `Controller` and `MyWillowMind` can differ on a possessed AI pawn.
- **UNVERIFIED.** Everything above. The note's last section lists the bl-sdk reads that would confirm it.
