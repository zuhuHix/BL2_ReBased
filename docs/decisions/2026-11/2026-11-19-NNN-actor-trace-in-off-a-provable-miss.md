## 2026-11-19: Actor.Trace in Off: a provable miss over the collision service, bound at both sites

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-ACTOR-TRACE, implementer. The decision number is pending from the hub;
NNN is a placeholder.

- **What:** `Engine.Actor.Trace` in `ParityMode::Off`, one shared function, `ow_world::world::actor_trace::trace`,
  coded from `docs/verification/NATIVE_ACTOR_TRACE.md` alone. That is #621's note at its accepted head 860e5492,
  copied onto this branch because it is not on main yet. Clean room: no native code or analysis output was opened.
- **The answer:** only a miss it can prove, answered as None with both out vectors zero. Everything else is a typed
  refusal, in the note's order:
  - no receiver, or a malformed argument, is a `VmError::Script`;
  - T1, an unresident receiver, candidate or owner, is `NotRead`;
  - T2, an unread option (`HitInfo`, bullet listeners, an `ExtraTraceFlags` bit other than 8), is `NotRead`;
  - T3, a source class other than `WillowAIPawn` and its script-only children, is `NotRead`. The two native
    descendants NATIVE_IS_STAGGERED.md records refuse;
  - T4, no collision service, is `LayerAbsent`, with go_down's text and layer;
  - the out destinations are checked before the first write;
  - the level sweep: `Unsupported` is `NotRead`, and a hit is T6;
  - T5, a primitive without bounds, is `LayerAbsent`;
  - T7, a primitive whose bounds meet the swept box, is `NotRead`, naming the terrain handling or the source-aware
    owner filters.
- **Bound at both sites, keep-existing:**
  - `ow_skills::action_skill::Pack` binds it in Off. That is the pack set link 9's `phaselock_world` uses.
  - `ow_combat::go_down` binds the same function in place of its bare `LayerAbsent` closure, replaced at its
    source, so neither pack order gives the old refusal.
  - Reference binds nothing new and is unchanged.
- **Verified by:** `ow-world` tests 1 to 8 of the note's section 3.6 and the binding-site test 9 in `ow-skills`, on
  invented packages and an invented collision service. Each refusal leaves #518's three counters and the name table's
  length unchanged. A candidate handle is checked to stay unloaded.
- **UNVERIFIED** in the running game: every rule. Some are the port's choices, not readings:
  - leaving the source actor's own primitives out of the walk (the note's open question 1);
  - naming the source-aware filter, not the hash admission, as T7's rule for a non-terrain owner, since nothing gets
    past the unread filter.
- **No dependency, no bounds check and no sensitive file changed.**
