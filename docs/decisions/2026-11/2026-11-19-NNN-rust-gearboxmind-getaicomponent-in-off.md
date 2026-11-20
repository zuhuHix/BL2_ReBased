## 2026-11-19: GearboxMind.GetAIComponent in Off

AI-assisted (Claude Code). Lane IMPL-GEARBOXMIND-GETAICOMPONENT, slice link 9. The decision number is pending (the
landing run numbers it). Coded from docs/verification/NATIVE_GEARBOXMIND_GETAICOMPONENT.md alone (PR #594, the note as
final at 660d0b32; clean room: lane ANALYST-GEARBOXMIND-GETAICOMPONENT wrote it). The executable and the analysis output were
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
- **The two clarified cases (the note as final at 660d0b32):**
  - **An unloaded receiver** (`vm.defs.is_unloaded`, a read) is `NotYet { NotRead }` naming the mind. It is not
    loaded first.
  - **A template component** is `NotYet { NotRead }`: "the mind's AIComponent is its class default's template, not an
    instance". The test reads only the heap. The component's `outer` is a class default when its origin is
    `Origin::Default`, or when its name starts with `Default__`. Nothing is decoded, and `class_default` is never
    called.
  - **Both checks also run in the Off preflight** (`preflight_hold_contexts`), for each context whose
    `IAIInterface` answer is a `GearboxMind`, so a `Behavior_AIHold` run refuses before any script runs. A refusal
    later in a run goes through the existing "stopped after partial writes" handling.
- **Pins updated:**
  - `ow-ai`'s `the_pack_binds_the_hold_kinds_in_off_only` now expects the mind's component;
  - the module and constant doc comments in `ow-ai` `awareness::holds`, `ow-combat` `holds` and `hit.rs` describe the
    new answer.
- **Verified:** host unit tests on invented classes. Each answer leaves every heap object and the store's load state
  unchanged. Nothing is checked against the running game, so every rule is **UNVERIFIED** in game.
