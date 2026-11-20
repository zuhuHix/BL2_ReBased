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
  - a query with a non-finite endpoint or extent, or a negative extent, is `NotRead` before the service is asked
    (review r1 of #633);
  - the out destinations are checked before the first write: each out argument, and the write through its alias,
    is checked with the new `NativeCall::check_out` before either out is written (review r1 of #633);
  - the level sweep: `Unsupported` is `NotRead`, and a hit is T6;
  - T5, a primitive without bounds, is `LayerAbsent`; bounds that are not finite and ordered are `NotRead` and never
    prove a miss (review r1 of #633);
  - T7, a primitive whose bounds meet the swept box, is `NotRead`, naming the terrain handling or the source-aware
    owner filters.
- **Bound at both sites, keep-existing:**
  - `ow_skills::action_skill::Pack` binds it in Off. That is the pack set link 9's `phaselock_world` uses.
  - `ow_combat::go_down` binds the same function in place of its bare `LayerAbsent` closure, replaced at its
    source, so neither pack order gives the old refusal.
  - Reference binds nothing new and is unchanged.
- **Vectors by type:** a vector argument or out must be a `Core.Object.Vector` by its reflected type, not only by its
  X, Y and Z float fields; a look-alike struct is a `VmError::Script` (review r1 of #633).
- **New VM entry points** (not sensitive files, named for review): `NativeCall::check_out` in `ow-vm`'s `api.rs`,
  backed by `interp::check_alias_write` and `aggregate::check_store_in`, and `Heap::check_write_in` in `ow-object`'s
  `heap.rs`, which `write_in` now calls before it writes. Each runs the checks the matching write runs, without
  writing.
- **Verified by:** `ow-world` tests 1 to 8 of the note's section 3.6 and the binding-site test 9 in `ow-skills`, on
  invented packages and an invented collision service. Each refusal leaves #518's three counters and the name table's
  length unchanged. A candidate handle is checked to stay unloaded.
- **UNVERIFIED** in the running game: every rule. Some are the port's choices, not readings:
  - leaving the source actor's own primitives out of the walk (the note's open question 1);
  - the invalid-input policy: refusing non-finite or negative query values and unsound primitive bounds as
    `NotRead`. How the game treats them was not read;
  - naming the source-aware filter, not the hash admission, as T7's rule for a non-terrain owner, since nothing gets
    past the unread filter.
- **No dependency, no bounds check and no sensitive file changed.**
