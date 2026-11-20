## 2026-11-19: GearboxMind.GetAIComponent in Off

AI-assisted (Claude Code). Lane IMPL-GEARBOXMIND-GETAICOMPONENT, slice link 9. The decision number is pending (the
landing run numbers it). Coded from docs/verification/NATIVE_GEARBOXMIND_GETAICOMPONENT.md alone (PR #594, the note at
4b83f1c5; clean room: lane ANALYST-GEARBOXMIND-GETAICOMPONENT wrote it). The executable and the analysis output were
not opened. Synthetic fixtures only.

- **What changes:**
  - `ow-combat` `holds` binds `GearboxFramework.GearboxMind.GetAIComponent` in Off to the note's answer
    (`holds::get_ai_component`), in place of the `NotYet { NotRead }` it bound since decision 2026-11-16-83.
  - Reference is unchanged: the `hit.rs` stand-in (the mind's `AIComponent`) still answers there.
- **The answer:** the receiver's `AIComponent`, read by name straight from the heap and returned as it is, the
  component handle or None.
  - `WillowMind` and any other `GearboxMind` subclass answer the same.
  - The native writes, loads and draws nothing. The returned handle is not loaded.
  - The class test uses the class table's `by_path` only, never `find_class`.
  - No receiver, or a receiver outside `GearboxMind`, is a `VmError::Script`. So is a class without `AIComponent`, or
    a value that is not an object.
- **Pending the note's review r1 of #594:** two cases wait for the note's clarified rule, and neither is special-cased
  yet:
  - how to detect the template refusal (an `AIComponent` that is the class default's template component) without a
    load;
  - what a receiver that is itself an unloaded handle answers.
- **Pins updated:**
  - `ow-ai`'s `the_pack_binds_the_hold_kinds_in_off_only` now expects the mind's component;
  - the module and constant doc comments in `ow-ai` `awareness::holds`, `ow-combat` `holds` and `hit.rs` describe the
    new answer.
- **Verified:** host unit tests on invented classes. Each answer leaves every heap object and the store's load state
  unchanged. Nothing is checked against the running game, so every rule is **UNVERIFIED** in game.
