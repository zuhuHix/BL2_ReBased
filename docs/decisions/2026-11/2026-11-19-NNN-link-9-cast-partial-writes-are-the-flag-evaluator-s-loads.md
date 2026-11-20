## 2026-11-19: Link 9's cast: the partial writes before the mind's QueryInterface are the flag evaluator's own loads

AI-assisted (Claude Code)

Lane ANALYST-GEARBOXMIND-QUERYINTERFACE (second pass), analyst only, with Claude Opus 5.5. The file number is a
placeholder until the maintainer gives one. Docs only: no code, no dependency, and no bounds check changed.

- **Scope.** The mind's `QueryInterface` rule was already recorded by #589 (decision 2026-11-19-201,
  NATIVE_GEARBOXMIND_QUERYINTERFACE.md). This pass re-read the interface virtual's entries for `AIController`,
  `GearboxMind` and `WillowMind`, and `WillowMind`'s pawn accessor, and agrees with that note. It changes no rule.
- **What it adds:** one section of that note, on link 9's "stopped after partial writes" mark (queue item
  PREFLIGHT-ORDER-LINK9-CAST). The mark is `FlagExpressionEvaluator.Evaluate`'s. Inside that one call a local probe
  saw 8 lazy definition loads, the Phaselock flags and their resolver sub-objects, and 40 committed heap writes, with
  0 draws. These are the evaluator's own loads, not an earlier native's writes. Two options are recorded for the hub:
  preload them at set-up, or a residency check first.
- **Probe** (local, never pushed, on #628's head 77ff5e7c): with the mind answering by the plain rule, installed Off
  `--slice-gate`, strict, gives ok 4, notyet 8, error 2 of 14. Link 9 passes 1 of 2 steps and stops at the unbound
  `GearboxMind.GetAIComponent()`, as #589's probe found.
- **Correction:** this lane's first push (f5c7807c) replaced #589's landed note with a duplicate rewrite. This version
  restores #589's note and NATIVE_INDEX unchanged and only appends the section above.
- **UNVERIFIED** in game.
