## 2026-11-19: Read Pawn.IsFriendly as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane ANALYST-IS-FRIENDLY, analyst only. With sub-object instancing (decision 2026-11-19-44), the Off Phaselock cast of
slice link 9 stops at the unbound `Pawn.IsFriendly`, which `LiftActionSkill`'s can-be-phaselocked test asks of the
caster's pawn about the target. The new own-words note `docs/verification/NATIVE_IS_FRIENDLY.md` and two
`NATIVE_INDEX.md` rows record, as behaviour only:

- `IsFriendly` is "the pawn's opinion of the other is Friendly". No class on link 9's path (`GearboxPawn`,
  `WillowPawn`, `WillowPlayerPawn`, `WillowAIPawn`) replaces it.
- `WillowPawn` replaces `GetOpinion` with a charm layer (`CharmedChild` Enemy, `CharmedParent` Friendly, and an Enemy
  case through the other pawn's `CharmedParent`) over the generic rule of NATIVE_COMBAT_GROUNDWORK.md section 6, which
  this read confirms.
- A controller and None have no allegiance on the other side and answer Neutral. A controller is not read through its
  pawn, unlike today's Reference binding in `ow-combat`.

NATIVE_COMBAT_GROUNDWORK.md section 6 gets a pointer that names the two charm fields. Raw output stays under ignored
`local/analysis/isfriendly/`; the Ghidra runs were read-only and took the machine lock. Nothing is implemented:
`Pawn.IsFriendly` stays unbound in Off until a fresh lane codes it from the note. Every rule is UNVERIFIED in the
running game; the note lists the bl-sdk checks and two open questions (the allegiance data, what sets the charm
fields).
