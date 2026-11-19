## 2026-11-19: The cast's worlds bind AttributeDefinition.GetValue, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-LINK9-ATTR-GETVALUE, slice link 9. No new rule and no native
read: the rule is the existing one, decision 2026-11-19-58 (`ow_combat::hit::attribute`, over `ow-attr`'s
`AttributeSystem::value_of`, NATIVE_ATTRIBUTES.md "AttributeDefinition.GetValue"). Synthetic fixtures only. Based on
fix/rust-link9-trace-filter (ccaf86f1).

- **What changes.**
  - Link 9's cast stopped in `cast` at "unbound native AttributeDefinition.GetValue(object,object,object)", a VM
    error. The Off binding existed, but only the combat pack registered it, and the cast's worlds (the slice gate's
    and the W6-SKILLS installed run's) install no combat pack.
  - `ow_combat::hit::attribute::register` is now public, and `ow_skills::action_skill::Pack` calls it, as it already
    reuses the combat pack's opinion natives. A native already bound is kept, so a world with both packs gets the
    same binding once.
  - Off only: under Reference it binds nothing, and the call stays the unbound native it was. Its preflight and
    partial-write rules are decision 2026-11-19-58's, unchanged.
- **Tests.** `ow-skills` `the_cast_pack_binds_get_value_in_off_only`: the pack binds the function's native in Off and
  not in Reference. The fixture gains a toy `AttributeDefinition.GetValue` declaration.
- **Checks.** `cargo test -p`: ow-skills 111 passed, ow-combat 187, ow-cli 101 + 2, ow-world 232; 0 failed each.
- **Installed `--slice-gate`, Off, strict, under the machine lock:**
  - before (fix/rust-link9-trace-filter): 0 ok / 4 not yet / 10 error; after: 0 ok / 5 not yet / 9 error.
  - Only link 9's row changed. It still passes 1 of its 2 steps (`spawn`) and stops in `cast`, now at a typed
    `NotYet` (`NotPorted`), marked "stopped after partial writes: NounAttributeValueResolver with ReplicationStrategy 2
    on a provider: whether the provider hands over a replicated array, and what the replicated-copy-changed call does,
    are not read" (NATIVE_NOUN_ATTRIBUTES.md).
  - That refusal is the noun resolver's existing one (FIX-NOUN-OFF). The note leaves both points expected, not read,
    so it is a question for an analyst, not for this lane.
- **The open question for an analyst** (NATIVE_NOUN_ATTRIBUTES.md section 4):
  1. Which arrays does each `INounAttributeProvider` hand the noun resolver? In particular, does the pawn the cast
     reads (expected a `WillowPawn` subclass) hand over `ReplicatedNouns`, so the table's "provider with a replicated
     array" row applies to strategy 2?
  2. What does the provider's "replicated copy changed" interface call do after a created entry gets a replicated
     slot? It is a notification body that was not read, on the same provider class.
- **UNVERIFIED.** Nothing here was checked in the running game.
