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
  that question through the read-only dispatch #602 added (`Vm::query_interface_read_only`). It asks only for a
  reached, resident definition with a context whose chain's first set entry (None entries skipped) is the AI
  component resolver. Only a rule's own `NotYet` is returned, text and kind kept: one whose record is not the
  read-only dispatch's own (`ow_vm::api::DISPATCH_RECORD`, new in review r2 of #648: the rules share
  `READ_ONLY_RECORD`, so filtering on that dropped them). So an open answer refuses before any write
  and is not marked.
- **Left to the dispatch** (review r1 of #648), where a refusal stays marked as before:
  - a cold receiver or a definition not loaded yet: their chains are unknown, and asking anyway refused player
    contexts whose chains never reach the AI resolver;
  - a chain that does not read as resolvers (the reads after the load report it);
  - an answer, yes or no, since the resolver asks again when it runs;
  - the read-only dispatch's own refusals (an entry not resolved yet, an unloaded stand-in, no code layer, a rule
    with no read-only form), now under `DISPATCH_RECORD` in `ow-vm`'s `interface_read_only.rs`, and the two
    read-only-only refusals of `ow-combat`'s `pawn_interface/read_only.rs` (an unloaded object's field, an interface
    class with no layout), which carry it too: the ordinary path may load and answer there;
  - any other error;
  - an `IAIInterface` class that is not loaded;
  - an AI component resolver after another resolver, which asks a context the earlier resolvers have changed;
  - a nested evaluator, which checks its own definitions.
- **GearboxMind's answer is not invented.** #602 (decision 2026-11-19-238) answers it read-only now. This lane only
  asks the existing dispatch earlier.
- **Verified by:** four `ow-skills` tests on invented classes with an invented rule:
  - an open answer refuses before the first write, its text unmarked, with #518's three counters and the name
    table's length unchanged and the resolver not run, also after a leading None entry;
  - a bound answer (yes or no), a rule with no read-only form, and a refusal carrying the dispatch's record each go
    through, and the resolver runs;
  - on an open context carrying the real rules' record, a chain without the AI resolver answers as before, a cold
    receiver and a reached unloaded definition are not refused with the open answer, a later AI resolver entry is left to the
    dispatch, and a short-circuited cold definition stays unloaded with no refusal.
  The slice result is in the GATES row.
- **UNVERIFIED** in the running game, as every rule is. These are the port's choices, not readings:
  - asking early is safe because the question has no effect. The read-only dispatch's own guarantee is what makes
    asking it twice harmless;
  - not asking for an unloaded definition or a cold receiver: their refusals stay after their loads, marked.
- **No dependency, no bounds check and no sensitive file changed.** Link 9's world registration in `slice_gate.rs`
  is untouched (lane d2b6's).
