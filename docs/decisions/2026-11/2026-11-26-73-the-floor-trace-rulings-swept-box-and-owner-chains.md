## 2026-11-26: The floor trace's rulings: the swept box and the Owner chains

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-FLOOR-TRACE-RULINGS, the implementer half of the clean room, from
`docs/verification/NATIVE_FLOOR_TRACE_FLAGS.md` section 6.1 alone (#809, as landed), on main f80b68e7. 6.2, the
map-backed path, is not approved and not done. **Every rule is UNVERIFIED in the running game.** Reference is
unchanged.

- **What was decided (Off only, under the existing `NoLevelBaseSearch` setting):**
  - **the guard is kept:** a service holding geometry, or `NoLevel` without the setting, is the trace flags' `NotRead`,
    decided first;
  - **the actor part** walks every live actor's primitives (`CollisionComponent`, then `Components`), in the game
    walkers' order (the note's section 3). A primitive is passed over when its owner is the actor, the skip flag is set,
    `BlockNonZeroExtent`, `CollideActors` or the owner's `bCollideActors` is clear, its bounds miss the swept box, or
    its owner is in the actor's `Owner` chain or holds the actor in its own;
  - **the swept box (ruling 2):** the actor's cylinder (radius, radius, half-height) from `Location` plus its
    component's `Translation`, swept down 8 units, against a candidate's bounding-cylinder box, per axis, touching
    counted as meeting. It is read at the first candidate that reaches the test. A non-cylinder `CollisionComponent` is
    the extent's `NotRead`. A start, extent or bounds that is not finite, or a negative extent, is the swept box's
    `NotRead`, so a source radius of -5 cannot clear a radius-10 candidate 8 units away. Bounds the rule does not
    answer keep the rule's own `NotYet`;
  - **the Owner chains (ruling 1):** `trace_filter::owner_chains` walks the source's `Owner` chain and the owner's,
    resident objects only, bounded by the links seen. An unloaded link that decides a candidate is `NotRead`, nothing
    loaded. This replaces #798's R2 "common convention" label;
  - **any primitive left** is the candidate's `NotRead`: the owner filter's blocked-by test and the primitive's own
    test are not ported;
  - **the touch trace (ruling 3):** its source exclusion uses the same two `Owner` walks in place of the `Base` chain.
    The unread rule is renamed `Unread::OwnerChainLink` ("an Owner-chain link the store has not loaded").
- **Verified (host only):**
  - `ow-world` set_physics, 4 new tests: the swept box, meets and misses on each axis and at the touching bounds; the
    extent and the malformed cases (the -5 radius, a NaN and an infinite `Location`, a negative height, an Absolute
    flag); the `Owner` chains both ways, two links deep, a cycle, `Base` alone not passed over, and an unloaded link
    with nothing loaded; the read flags passing a candidate over before its bounds; a service with geometry;
  - `ow-inventory` touch tests moved to the `Owner` chains, plus the pickup's `Base` alone now tested and an actor the
    pickup owns passed over. Removing the downward walk fails one test in each crate (checked by hand);
  - `cargo test --workspace -j 8 --no-fail-fast` 4,083 passed / 0 failed / 59 ignored;
  - the installed Off strict `--slice-gate` under the machine lock, base f80b68e7 against this head: both **8 ok /
    14, X = 8.5/14 = 60.7%, every row's stop the same**. Links 6 and 7 have the same steps and harness lines. Links 3,
    4, 5, 13 and 14 stay past the floor trace (the stand-in's line, with its new text, on links 3 and 4) and stop at the
    spawn's `SetBaseValue`.
- **UNVERIFIED:** every rule; the trace's own blocked-by test and the primitive's swept-box test are not ported.
