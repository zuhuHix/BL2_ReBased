## 2026-11-19: Read SpecialMoveComponent.Play as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane SM-PLAY, analyst only (slice link 9). After `GetSMComponent`, Maya's cast plays the Phaselock hit or fizzle
definition through `SpecialMoveComponent.Play`. The new own-words note `docs/verification/NATIVE_SPECIAL_MOVE_PLAY.md`
and four `NATIVE_INDEX.md` rows record, as behaviour only:

- What `Play` does: resolve the definition (it answers itself), two gates (`IsLocalAuthority` and the script
  `AuthorityCanPlay`), then start: stop any current move, record the request and start time, queue follow-ups, and on
  the authority replicate and run `ServerStarted`. Last, `ClientStarted`, whose answer is the duration `Play` returns.
- What `ClientStarted` and `ServerStarted` do on the Phaselock definitions' class chain: behaviors, the animation's
  play rate, movement stop and left-hand IK, weapon blocking, and the first-person animation through `PlayLocal`.
- Before `Play`, the target tests of `LiftActionSkill.GetPlayerAnimation`, with `Pawn.IsAliveAndWell` read. Also the
  component queries `GetSMNode` (the mesh's `SpecialMoveBlend` node, cached) and `IsPlayingClass`.
- How a move ends: the animation node, then `AnimFinished`, then the stop steps, the callback and the next queued move.
- For the port: without an animation tree, `AuthorityCanPlay` is false, so `Play` would start nothing. That is left
  to the port as a decision.

Raw output stays under ignored `local/analysis/smplay/`; the Ghidra runs were read-only and took the machine lock.
Nothing is implemented: everything named stays unbound or `NotYet` in Off until a fresh lane codes it from the note.
Every rule is UNVERIFIED in the running game; the note lists the bl-sdk checks and the natives not read.
