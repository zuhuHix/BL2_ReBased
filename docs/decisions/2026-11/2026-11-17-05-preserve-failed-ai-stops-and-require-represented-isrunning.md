## 2026-11-17: Preserve failed AI stops and require represented IsRunning setup

AI-assisted (Codex/GPT-6.1 Sol). PR #374 review fix, implemented from the own-words
`NATIVE_AI_ACTION_SEQUENCE.md` and `NATIVE_AI_AWARENESS.md` notes only. No analysis outputs,
Ghidra projects or REA results were opened. No game or host run was made. Native behaviour remains
**UNVERIFIED** in the running game.

This corrects decision 2026-11-16-72's IsRunning implementation and limits; the historical decision
and analyst notes are preserved unchanged. The remaining atomic, range and Reference policies stand.

- A failed Stop must leave the Rust tree's current leaf and the adapter's running action consistent.
  The adapter marks the transition while Stop runs, then clears its record only after Stop and the
  rest-state check both succeed. Any error restores the running action and preserves the original
  error. Script side effects are not rolled back. No service borrow crosses a VM call. The unread
  default-state operation still returns NotYet when Stop leaves a different state.
- IsRunning requires the action's AI interface and the AI's component/current-state link, as the note
  says. Rust ownership alone cannot establish them. This port has no represented setup for owned
  actions, so they return NotYet even when a caller supplies an AI object. A known unowned action
  with null AI returns false; one with AI set returns NotYet because its tree is unknown. Absence of
  the brains service is unknown ownership and returns NotYet without creating an empty service.
- Reference keeps all seven action-sequence natives unbound. No dependency, tool, hook, gate or
  roadmap change is made.

Synthetic regressions exercise a Stop error after a script state change, wrong rest state, successful
retry, owned current and noncurrent actions with null and assigned AI, the represented unowned null-AI
case, missing service, and an IsRunning call from the action's actual Start callback. They establish
Rust behaviour only; no game parity or completed independent correctness review is claimed.
