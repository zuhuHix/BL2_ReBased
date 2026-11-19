## 2026-11-19: Read GearboxPawn.GetSMComponent as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane GETSMCOMPONENT, analyst only. With PHASELOCK-CAST part B (#407), Maya's cast in Off stops at the unbound
`GearboxPawn.GetSMComponent()` (slice link 9). The new own-words note
`docs/verification/NATIVE_SPECIAL_MOVE_COMPONENT.md` and one `NATIVE_INDEX.md` row record:

- The native is a plain read of the pawn's `SMComponent`, reached through the pawn's special-move interface. It
  creates and checks nothing, and it answers None only when that property is None.
- The component is a default sub-object: the class defaults and Maya's archetype each hold one, and in stock UE3 a
  spawned pawn instances its own. Whether the port's class-default player should hold an instance is left to the port
  as an open question.
- The cast's next natives, in script order, sit inside `LiftActionSkill.GetPlayerAnimation`'s target tests and end at
  `SpecialMoveComponent.Play`. None of them is read here.

Raw output stays under ignored `local/analysis/getsm/`; the Ghidra runs were read-only and took the machine lock.
Nothing is implemented: the native stays unbound in Off until a fresh lane codes it from the note. Every rule is
UNVERIFIED in the running game; the note lists the bl-sdk checks.
