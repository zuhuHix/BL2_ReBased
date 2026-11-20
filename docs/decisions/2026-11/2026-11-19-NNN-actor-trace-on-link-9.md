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
- **On link 9:** `BeginLifting` traces the target's floor (extent 1, actors on, flag 8). A miss keeps the check
  location.
- **Proposed Off answer:**
  - one shared `ow-world` function, bound in the skill worlds and replacing go_down's bare refusal;
  - the level part through the collision service, the actor part through `trace_filter`'s walk;
  - refusals T1 to T5 (`LayerAbsent` without a collision world, and for unknown component bounds);
  - Reference unchanged; tests listed.
- **Probe** (local, never pushed; stand-ins named in the note): installed Off `--slice-gate`, strict, ok 2, notyet 9,
  error 3. Link 9 passes 1 of 2 steps and stops at the unbound `Actor.AttachComponent(object)` (the phaselock light).
- **Clean room:** a different lane implements this note.
