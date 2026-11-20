## 2026-11-19: Link 9's cast asks the AI component resolver's question read-only, before the evaluator's first write

AI-assisted (Claude Code, Opus 5.5). Lane PREFLIGHT-ORDER-LINK9-CAST, implementer. The decision number is pending
from the hub; NNN is a placeholder.

- **The stop:** link 9's `cast` stopped "after partial writes" at a pawn's `QueryInterface(GearboxFramework.IAIInterface)`.
  A traced throwaway probe (on main df0970e with #628) placed the mark: `FlagExpressionEvaluator.Evaluate`
  snapshots #518's counters on entry, then loads its definitions and their resolvers before the definition's first
  context resolver, `GearboxFramework.AIComponentAttributeContextResolver`, runs. That resolver's
  `GetAttributeContext` is script. Read with `research/script_disasm.py`, its first act is to ask its source context
  for `IAIInterface`. The answer was not readable, so the refusal came after the evaluator's loads and was marked.
- **What changed** (`ow-skills`, `action_skill/flag_expression.rs`): before the evaluator's next load, it now asks
  that question through the read-only dispatch #602 added (`Vm::query_interface_read_only`). This covers:
  - a cold receiver, whose chain is unknown until its load, with a context;
  - a reached definition with a context, when the definition is a handle not loaded yet or its chain's first set
    entry is the AI component resolver.
  A `NotYet` from that dispatch is returned as it is, text and kind kept, so a question with no bound, read-only
  answer refuses before any write and is not marked.
- **Left to the dispatch, so there are no false refusals:**
  - an answer, yes or no, since the resolver asks again when it runs;
  - a rule with no read-only form (`NotPorted`), because the ordinary rule may answer;
  - any other error;
  - an `IAIInterface` class that is not loaded;
  - an AI component resolver after another resolver, which asks a context the earlier resolvers have changed;
  - a nested evaluator, which checks its own definitions.
- **GearboxMind's answer is not invented.** #602 (decision 2026-11-19-238) answers it read-only now. This lane only
  asks the existing dispatch earlier.
- **Verified by:** two `ow-skills` tests on invented classes with an invented rule:
  - an open read-only answer refuses before the first write, its text unmarked, with #518's three counters and the
    name table's length unchanged, on a loaded evaluator and on a cold receiver that stays unloaded;
  - a bound answer (yes or no), a rule with no read-only form, and a later AI resolver entry each go through.
  The slice result is in the GATES row.
- **UNVERIFIED** in the running game, as every rule is. These are the port's choices, not readings:
  - asking early is safe because the question has no effect. The read-only dispatch's own guarantee is what makes
    asking it twice harmless;
  - asking for an unloaded definition or a cold receiver is conservative: its chain is unknown, so it may refuse
    where the definition would not reach the resolver.
- **No dependency, no bounds check and no sensitive file changed.** Link 9's world registration in `slice_gate.rs`
  is untouched (lane d2b6's).
