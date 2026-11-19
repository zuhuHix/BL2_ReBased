## 2026-11-19: Read how a pawn context answers `IAIInterface` (analyst only)

AI-assisted (Claude Code, Claude Opus 5.5). Lane ANALYST-PAWN-AI-INTERFACE. The prepared bl-sdk session could not run,
because the maintainer is away, so this closes NATIVE_AI_HOLDS.md's open row "How a pawn context reaches
`IAIInterface`" by native analysis instead. The script was read with `research/script_disasm.py`, and the natives with
Ghidra read-only under the machine lock. Raw output stays under ignored `local/analysis/`. The outcome is the own-words
note `docs/verification/NATIVE_PAWN_AI_INTERFACE.md`. No code changes; a fresh implementer codes it from the note.

- **The rule** (the note's section 2, which the other records point at). `Behavior_AIHold` asks its context with
  `Object.QueryInterface`, naming `IAIInterface`. A pawn answers by `Pawn`'s rule, which no pawn class below it
  changes for this interface:
  1. its own class's declared interfaces;
  2. else its `Controller`. Only when it has no `Controller`, the controller of a vehicle it drives. A controller that
     declines does not lead to the vehicle;
  3. else its `Mesh`;
  4. else none.

  A question that comes back to an object already answering gets none at once, and the asker goes on to its next
  step (so a pawn whose controller asks back goes on to its `Mesh`). `WillowAIPawn` adds one extra answer, for
  `IPlayerMaster` only, after that rule.
- **For the Fire dummy** (link 11), expectations, UNVERIFIED. The pawn's class lists no `IAIInterface`. Its
  controller, expected to be the mind, is expected to answer, and the mind's AI component then receives a demigod
  hold named `Safe` with the pawn as its tag. COMBAT_GOLDEN_TRACE.md records the demigod flip inside that behavior's
  call, but not its context object. With no controller and no driven vehicle, the `Mesh` is asked; its answer is not
  read, and if it declines, nothing is held.
- **The port's assumption** (`ow-combat`'s `ai_interface_for`, Reference only): it agrees that the mind answers for a
  possessed AI pawn. It differs in four ways: the rule goes through `Controller`, not `MyWillowMind`; the pawn's own
  class comes first; a declining controller leads to the `Mesh`; and with no controller the order is a driven
  vehicle's controller, then the `Mesh`, then none, where the stand-in returns the pawn.
- **Open, each UNVERIFIED** (the note's "Open"): `GearboxMind`'s own answer, `WillowPlayerController`'s answer, a
  skeletal mesh component's answer, whether `Controller` and `MyWillowMind` can differ, what `Controller` holds on a
  dead or unpossessed pawn, and the `PlayerMaster` field identification. The note's bl-sdk section has a check for
  each branch.
- **UNVERIFIED.** Everything above. The note's last section lists the bl-sdk reads that would confirm it.
