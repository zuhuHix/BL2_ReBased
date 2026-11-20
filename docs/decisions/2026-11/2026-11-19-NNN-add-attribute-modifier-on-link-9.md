## 2026-11-19: AddAttributeModifier on link 9: the records settle it; bind it where skill scripts run

AI-assisted (Claude Code)

Lane ANALYST-ADD-ATTRIBUTE-MODIFIER, analyst only, with Claude Opus 5.5. The file number is a placeholder until the
maintainer gives one. Docs only: no code, no dependency, and no bounds check changed.

- **What:** `docs/verification/NATIVE_ADD_ATTRIBUTE_MODIFIER.md`, a short note, plus one NATIVE_INDEX row.
  NATIVE_ATTRIBUTES.md section 1 and NATIVE_NOUN_ATTRIBUTES.md sections 5 and 10 already settle the rule: the value
  chain picks the list, the bool suppresses only a property's change notification, and nothing is loaded or drawn.
  `ow-attr`'s `add_modifier` implements it. No new reading of the executable was needed.
- **For link 9:** the equip pack's binding fits the cast's call, a noun add on `IsPhaselocked` to the target pawn. It is
  not bound in the cast's world, and its resolver reads can load.
- **Proposed Off answer:** bind it next to `ResolveContext` (#604) where skill scripts run, first-wins, and leave
  Reference unchanged. A load-free preflight comes first, with refusals A1 (unloaded definition), A2 (unloaded resolver)
  and A3 (unknown resolver class). Tests are listed.
- **Probe** (local, never pushed; stand-ins named in the note): installed Off `--slice-gate` strict, ok 2, notyet 9,
  error 3. With the equip body, link 9's add succeeds, with 3 writes, no load and no draw. Link 9 then passes 1 of 2
  steps and stops at the unbound `WillowAIPawn.IsStaggered()`.
- **Clean room:** a different lane implements this note.
