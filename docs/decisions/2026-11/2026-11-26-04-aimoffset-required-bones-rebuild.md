## 2026-11-26: the aim-offset node's required-bones rebuild in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-AIMOFFSET-REBUILD, rebased onto main 1709b8a8 (with #742). It implements
docs/verification/NATIVE_AIMOFFSET_REBUILD.md section 3 (lane E's note, #741 with its r1 fix, on main). Clean room:
this lane did not read the executable. No real-game session was run. **Every rule is UNVERIFIED in the running
game.** Reference keeps the stub policy.

**Where.** `ow-world` `world::aim_offset`: the `REBUILD` refusal (a mesh on `SkelComponent`) is replaced by the
rebuild, and `world::aim_profiles`' source answer gains each profile's component bone names (`SourceProfile`, read by
`ow-cli`'s source from `ow_assets::aim_profiles`' `components[..].bone_name`).

**The rule**, in the preflight (write-free and load-free), the first refusal the answer:
1. the gate: no `SkelComponent`, or a resident one with `SkeletalMesh` None, writes both arrays empty; a component
   not loaded is `COMPONENT_NOT_LOADED`;
2. the mesh's reference skeleton through `skeletal::reference_skeleton`, its refusals standing;
3. more than 256 bones: `NotYet { NotRead }`, "AnimNodeAimOffset: a required-bones rebuild past a byte";
4. the current profile's components' bone names (a decoded `AimComponents` with a name `BoneName`, else
   `VmError::Script`; or the layer's text names); more than 255 components: the same `NotRead`;
5. the match, interning nothing: text through `ReferenceSkeleton::bone_index`, a name id by each bone's
   `names.find`; no bone or a `None` name is skipped. A match appends the bone and sets its table entry (one 255 per
   bone) to the component's position;
6. the ascending sort, then the parent pass (each missing ancestor directly before the first bone needing it; bones 3
   and 4, with 1 and 3 children of the root and 4 a child of 1, give 0, 3, 1, 4).

Then the writes: `CurrentProfileIndex`, `RequiredBones`, `AimCpntIndexLUT`, as byte arrays. Every refusal comes
before the first write.

**Tests.** `aim_offset_rebuild_tests.rs` (6), on invented classes, invented `Engine.upk` mesh exports and a toy
skeleton source, cover the note's section 5. Test 8 compares the layer's text names and the decoded ids directly, and
`aim_offset_tests.rs` runs it through the profiles layer itself (the source's bone names, a mesh export's skeleton).
Test 7 includes the literal 257-bone, 256-component case; both limits share one text, so it shows only that the case
is refused past a byte, and the malformed-list case shows the bone limit comes first. The skeleton-layer cases are in `aim_offset_tests.rs`, whose
toy mesh class is now `Engine.SkeletalMesh` (with a property): no source, and, with a source installed, a mesh not
loaded (the layer's "unloaded skeletal mesh" text). The gate's cases are there already. The tests cover section 5
(review of #745, closed by lane REBUILD-TEST-GAPS), with the limit order shown by the malformed-list case (review of
#750).

**The slice gate** (installed Off strict, release build, under the machine lock, on main 4c01c9a1, without #742):
- before: 6 ok / 8 NotYet / 0 errors, `links ok 6/14`, 47.0%, with link 1 at the rebuild's `NotRead`;
- after: 6 ok / 2 NotYet / 6 errors, `links ok 6/14`, `step share X = 6.583/14 = 47.0%`.

Links 1, 3, 4, 5, 13 and 14 pass the rebuild and `FindAllAnimNodes` (#739), and stop at an **error, the unbound
`WillowAnimNodeFeatherBlend.SetBlendTarget(float,float)`**. The feather blend declares its own native, which shares
`AnimNodeBlend`'s exec (NATIVE_LINK1_TAIL_SETTERS.md 2.B), and #739 bound only `AnimNodeBlend.SetBlendTarget`'s key.
That key was the one #739 had not verified: **#739's binding missed the subclass key.** This lane binds
`WillowAnimNodeFeatherBlend.SetBlendTarget(float,float)` to the same `blend_setters` rule, in Off only (its own
commit, with a test that a feather-blend receiver reaches the rule through its own key).
With the feather key bound and the branch on main 1709b8a8 (with #742): 6 ok / 8 NotYet / 0 errors, `links ok 6/14`,
`step share X = 6.583/14 = 47.0%`. Links 1, 3, 4, 5, 13 and 14 pass `SetBlendTarget` and stop at `maya:8` on
`NotYet { NotPorted }`, "UpdateAnimations: the tree nodes' ticks" (NATIVE_BONE_TRANSFORM_LAYER.md), in
`Mesh.UpdateAnimations`, before `InitializeRBPhysics`.

**Gates.** `cargo test --workspace -j 8`: 3,738 passed / 0 failed / 58 ignored (after the rebase). `crate_edges_test.py`: OK.
