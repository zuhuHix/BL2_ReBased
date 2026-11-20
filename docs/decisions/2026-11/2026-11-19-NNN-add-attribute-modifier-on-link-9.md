## 2026-11-19: AddAttributeModifier on link 9: the records settle it; bind it where skill scripts run

AI-assisted (Claude Code)

Lane ANALYST-ADD-ATTRIBUTE-MODIFIER, analyst only, with Claude Opus 5.5. The file number is a placeholder until the
maintainer gives one. Docs only: no code, no dependency, and no bounds check changed.

- **What:** `docs/verification/NATIVE_ADD_ATTRIBUTE_MODIFIER.md`, a short note, plus one NATIVE_INDEX row.
  NATIVE_ATTRIBUTES.md section 1 and NATIVE_NOUN_ATTRIBUTES.md sections 5 and 10 already settle the rule: the value
  chain picks the list, and the bool suppresses only a property's change notification. An existing entry and a property
  add load and draw nothing; a missing noun entry evaluates its `DefaultValue`, which can load, run script and draw.
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

## Edits: review r1 of #607

- **Loads and draws corrected.**
  - A missing noun entry's `DefaultValue` evaluation can load and draw.
  - A0 is added for an unloaded context.
  - The load-free probe covers `entry_with`, `preflight_init` and `preflight_read`.
  - #604's terminal-`NotYet` rule is adopted word for word.
- **#523 is a prerequisite:** on main, replicated nouns refuse `NotPorted`. The noun test is split into strategy 0 and
  replicated.
- **The binding follows #604's final rulings:**
  - the logic in `ow-attr`, the wrapper in `ow_inventory::equip::natives`;
  - keep-existing in the skill pack and in the equip pack;
  - the equip-run gates;
  - a both-orders test.
- **Hub rulings:**
  - `RemoveAttributeModifier` stays unbound in skill worlds (open item);
  - the None checks come before A1;
  - a non-bool flag is treated as false.
- **Refusal order:** A2 for every entry, then A3, then the probes, with a mixed-chain test.
