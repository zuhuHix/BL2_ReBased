## 2026-11-19: Actor.Trace on link 9: an own-words note and a proposed Off answer

AI-assisted (Claude Code)

Lane ANALYST-ACTOR-TRACE, analyst only, with Claude Opus 5.5. The file number is a placeholder until the maintainer
gives one. Docs only: no code, no dependency, and no bounds check changed.

- **What:** `docs/verification/NATIVE_ACTOR_TRACE.md` and one NATIVE_INDEX row, from the script, `Engine.upk`'s export
  table and a local, read-only Ghidra reading under the machine lock. Raw output stays under ignored
  `local/analysis/trc/`. It builds on `go_down.rs`'s `LayerAbsent` binding, `placement.rs`'s collision service and
  `trace_filter.rs`'s flag bits.
- **Findings** (UNVERIFIED in game):
  - the exec fills the defaults and builds one flag word from `bTraceActors` and `ExtraTraceFlags`; the blocking bit
    swaps projectile targets for blocking-only;
  - a `WillowAIPawn` traces as itself;
  - it makes one world check and writes the hit actor, location and normal, zero on a miss.
- **On link 9** (UNVERIFIED): `BeginLifting` traces the target's floor (extent 1, actors on, flag 8). A miss keeps
  the check location; a hit raises the target and traces again.
- **Proposed Off answer:**
  - one shared `ow-world` function, bound in the action-skill pack and replacing go_down's bare refusal;
  - a provable miss only; a level hit, a near primitive and unknown bounds refuse;
  - refusals T1 to T7 (layer-absent without a collision world and for unknown component bounds);
  - Reference unchanged; tests listed.
- **Probe** (local, never pushed; the stop holds only under the stand-ins named in the note): installed Off `--slice-gate`, strict, ok 2, notyet 9,
  error 3. Link 9 passes 1 of 2 steps and stops at the unbound `Actor.AttachComponent(object)` (the phaselock light).
- **Clean room:** a different lane implements this note.

## Edits: review r1 of #621

- **Narrowed Off answer.** Off answers only a provable miss. A level hit refuses, because the hit identity was not
  read and None would read as a miss to `BeginLifting`. A primitive whose bounds meet the swept box refuses, naming
  the unread rule, so there is no nearer-actor test. The source-aware filter, the actor-hit query and the BSP/actor
  comparator are listed as new work.
- **The binding sites.** There is one `ow-world` implementation keyed `Engine.Actor.Trace`. It is bound in the action
  skill pack (`phaselock_world` has no combat pack) and replaces `go_down.rs`'s closure at its source. Both sites check
  Off, and there are tests for the Phaselock-only set and for both combined orders.
- **The walk.** The walk runs from the flag word for both values of `bTraceActors`, with the outcomes ordered (T5,
  then the unread rules, then an unsupported sweep). There is a disjoint-bounds case with unloaded filter fields.
- **The preflight** is resident-only for the receiver, every candidate and every owner, and both outs and both values
  are validated before the first `set_out`.
- **Scope.** The no-write/no-load claim is limited to the exec work read. The probe's stop is conditional on its
  stand-ins. The refusal kinds are written in words.
