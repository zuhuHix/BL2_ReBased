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
- **Proposed Off answer:** no deferral. The kernel registers in full, with a write-free, load-free preflight. The
  verdict and the store decision are computed in the preflight, read-only, over the bridge's existing status,
  objective and set rules, through one VM service the bridge implements (crate edges). The observer list is a
  service write that cannot fail. It gives typed refusals for other condition classes, unloaded objects, per-instance
  objectives and waypoint conditions.
- **Next** (a prediction, not a run): `InitializeBehaviorProviders` completes, unless the store holds a linked
  objective unloaded (then its `NotRead`). No stop is predicted in the rest of `ApplyBodyClassStartingValues`.
- **Review r1 (#771):**
  - the client net-mode test is a preflight step, with a test;
  - the Complete-and-not-repeatable decision and `bRepeatable`'s declaration check are in the preflight;
  - the verdict comes before the waypoint test, which covers every linked record, other consumers' too;
  - the hub's rulings are recorded:
    - kernel-registered conditions are kept in a list the script notification does not walk;
    - a later notification re-evaluates them by the same verdict, with a typed refusal, and gets a test;
  - the Fire slice's verdict difference on other missions is named, to be resolved toward `GetMissionStatus`.
- **Clean room:** a different lane implements this note.
