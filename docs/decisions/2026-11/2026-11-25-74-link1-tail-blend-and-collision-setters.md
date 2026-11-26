## 2026-11-25: maya:8's tail setters in Off (blend and collision)

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-LINK1-TAIL-SETTERS, on main 49810ce4. It implements
docs/verification/NATIVE_LINK1_TAIL_SETTERS.md section 2 (lane F's note, #734 with its review r1, on main). Clean room:
this lane did not read the executable. No real-game session was run. **Every rule is UNVERIFIED in the running
game.** Reference keeps the stub policy, and `death.rs`'s `SetRBChannel` no-op.

**Where.** `ow-world` `world::blend_setters` (2.A to 2.C) and `world::collision_setters` (2.D to 2.H), bound
keep-existing by key in the world pack's Off block. Every preflight is write-free and load-free.

**The rules**, as the note gives them:
- `FindAllAnimNodes`: the live component's `AnimTickArray`, when the receiver's `SkelComponent` is set and resident,
  the list is filled, and its `Animations` is the receiver. The matches by `NodeName` are appended to `Nodes`. Otherwise
  the `NO_TICK_LIST` `NotRead`; a component or node not loaded is `NotRead`;
- `SetBlendTarget`: the clamp, the immediate path's five writes, the timed path's two. With
  `bSkipBlendWhenNotRendered` and `bRecentlyRendered` clear, the unidentified engine flag's `NotRead`. Then the feather
  blend's `Children[0].Weight` := 1. The body is chosen by the exact class (`Engine.AnimNodeBlend` or
  `WillowGame.WillowAnimNodeFeatherBlend`); any other class, `AnimNodeBlendPerBone` included, is `NotRead` naming it;
- `SetActiveChild`: `NotRead`;
- `SetRBChannel`, `SetBlockRigidBody`, `SetNotifyRigidBodyCollision`: the field write. They are
  `NotYet { LayerAbsent }` ("the rigid-body collision filter update", layer "a physics layer (rigid bodies)") only
  when bodies are present: `PhysicsAssetInstance` on a `SkeletalMeshComponent`, `BodyInstance` otherwise, read as a
  handle;
- `SetTraceBlocking`: the two bits;
- `SetActorCollision`: `AlwaysCheckCollision`; on a change of `CollideActors`, the deferred reattach (the owner rule's
  `REATTACH_OWNER` or `REATTACH_SCENE` first) and, with both bits set and an owner, the unread owner step's `NotRead`;
  then `BlockActors`. All or nothing.

**`SetRBChannel` in Off.** `ow-combat` `death.rs` bound it as a no-op in both modes. The no-op loop is now
`bind_presentation_no_ops`, which skips it in Off, as it already skipped `DropCombatPerch`. Reference keeps the no-op.

**Not verified:** the native keys (`AnimNode.FindAllAnimNodes(array,name)`,
`AnimNodeBlend.SetBlendTarget(float,float)`, `PrimitiveComponent.SetActorCollision(bool,bool,bool)` and the rest) are
written in the loader's key form, but no installed run reaches them yet. A key that differs would show up as the next
link's "unbound native".

**Tests.** `blend_setters_tests.rs` (6) and `collision_setters_tests.rs` (5), on invented classes, cover the note's
tests, with each refusal asserting #518's counters, `class_code_loads` and the name table unchanged.
`death/tests.rs`: in Off the key answers with ow-world's rule, and in Reference the no-op.

**The slice gate** (installed Off strict, release build, under the machine lock): before and after the same, 5 ok /
8 NotYet / 1 error, `links ok 5/14`, `step share X = 5.783/14 = 41.3%`. Link 1 is still at `maya:8` on the aim-offset
profiles' `LayerAbsent` (the prediction: the profile reader, c28, has not landed), so it does not reach these natives
yet.

**Gates.** `cargo test --workspace -j 8`: 3,690 passed / 0 failed / 57 ignored. `crate_edges_test.py`: OK.
