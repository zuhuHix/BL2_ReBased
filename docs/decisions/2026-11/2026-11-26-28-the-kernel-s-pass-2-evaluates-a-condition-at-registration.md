## 2026-11-26: The kernel's pass 2 evaluates a condition at registration

AI-assisted (Claude Code)

Lane ANALYST-BEHAVIOR-KERNEL-PASS2, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** link 1's predicted stop (#767) is `InitializeBehaviorProviders`' pass-2 `LayerAbsent` for
  `PlayerEvents_Siren`, whose sequences carry a `CustomEnableCondition`.
- **What:**
  - the new note `docs/verification/NATIVE_BEHAVIOR_KERNEL_PASS2.md`;
  - two NATIVE_INDEX rows and a GATES row.
- **Evidence:**
  - a local read-only Ghidra reading under the machine lock: the registration routine, the condition class's link
    and after-link virtuals (from its class table), the base link and the tracker's register;
  - the layout tool;
  - `ow-package --def-dump` of the definition and its four conditions;
  - NATIVE_BEHAVIOR_POPULATION.md sections A to C, re-checked.

  Raw output stays under ignored `local/analysis/pass2/`.
- **Findings** (UNVERIFIED in game):
  - pass 2 runs after all of pass 1. Per conditioned record it calls the condition's link and, on success, its
    after-link;
  - `BehaviorSequenceEnableByMission` links unless the net mode is the client's or `bInstanced` is set. It then
    registers with `WorldInfo.GRI.MissionTracker`, whose immediate level-load reaction applies the verdict;
  - without a tracker, nothing is evaluated or stored;
  - Maya's four conditions are false on a fresh character. Pass 2 disables `Ep10_MordecaiChatter`, which pass 1 had
    enabled;
  - side finding: the port's direct `IntializeBehaviorProviderForConsumer` registers an empty definition, which the
    game's gate skips (`SetDeathDefinition` with `Death_Siren`).
- **Proposed Off answer:** no deferral. The kernel registers in full, with a write-free, load-free preflight and a
  read-only verdict over the bridge's existing status, objective and set rules, read through a VM service the bridge
  registers (crate edges). It gives typed refusals for other condition classes, unloaded objects, per-instance
  objectives and waypoint conditions.
- **Next** (a prediction, not a run): `InitializeBehaviorProviders` completes, unless the store holds a linked
  objective unloaded (then its `NotRead`). No stop is predicted in the rest of `ApplyBodyClassStartingValues`.
- **Clean room:** a different lane implements this note.
