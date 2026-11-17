## 2026-11-16: Native AI hold lists with unread path refusals

AI-assisted (Claude Code; Codex recovery and completion). Implements only the base movement, demigod, god and dialog list contracts from NATIVE_AI_HOLDS.md and its own-words NATIVE_AI_AWARENESS.md reference. The implementer did not open raw analysis, Ghidra or REA output. All native rules remain UNVERIFIED in the running game.

Off binds twelve list natives and an explicit NotYet for GearboxMind.GetAIComponent. Exact base AIComponent instances append unique name/tag pairs, ignore null tags and query list occupancy. Tail release is supported; non-tail release refuses the unread remaining-entry order before mutation. Subclass virtual dispatch, including Willow, is refused before list access. Reference keeps the project C++ stand-ins unchanged.

A new movement pair is appended before the owner lookup. With no owner there is no motion side effect; with an owner the call returns NotYet after that append: the controller virtual and vector constant are unresolved. No controller property or zero vector is invented. Duplicates return before that side effect. Pawn interface routing and the damage reader remain unread; these lists do not prove damage behavior or Fire-dummy parity.

Synthetic fixtures exercise list separation, caller and tag identity, append order, duplicate and null behavior, tail release and non-tail refusal, subclass refusal, movement partial effects and script error propagation. Gate logs and the installed headless stop are recorded under ignored local/p2/fix-aihold-natives; no game payload is published. No live, window, host, GPU or benchmark validation was performed.
