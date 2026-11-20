## 2026-11-19: The bone-transform layer in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-BONE-TRANSFORM-LAYER, coded from
docs/verification/NATIVE_BONE_TRANSFORM_LAYER.md at #646's final head 94ab4339 alone. The branch's first commit copies that
note in, and the copy drops out when #646 lands. Clean room: lane ANALYST-BONE-TRANSFORM-LAYER read the native, this
lane did not. No real-game session was run. **Every rule is UNVERIFIED in the running game.** Off only; Reference is
unchanged and binds nothing.

**What changed** (`ow-world` `world/skeletal.rs`, the new `world/bone_transform.rs`, `ow-cli` `cmd/skeletal_source.rs`):
- **The data (6.1).**
  - `SkeletonBone` gains `bind: Option<BoneBind>`, the bone's transform relative to its parent in `ow-assets`'
    convention. `None`, per bone, means unknown. `SkeletonBone` and `ReferenceSkeleton` drop `Eq` and keep
    `PartialEq`.
  - The `.psk` source fills `bind` from the reader's `Bone::bind`. That it equals the reference skeleton is
    UNVERIFIED. The two test literals take `None`.
  - `reference_space_bases` composes root first: `q = q(p)·q_local`, `t = t(p) + q(p) t_local q(p)*`, scale 1, every
    bone required (UNVERIFIED). Any unknown bind is `LayerAbsent` with #569's layer text, and nothing is cached. The
    composed pose's cache is a field of the `SkeletalMeshes` service, so it lives and is cleared with it.
- **The read (6.2).** `stored_space_bases` returns the buffer as stored, after one check: `SpaceBases` declared as an
  array of `Core.Object.BoneAtom`. It never computes a pose. No consumer is switched over in this lane.
- **The pose update (6.3).** `pose_update` returns `Skip` or `Write(pose)`, for the fill paths only, none of which is
  ported. So it ships with its tests only and nothing calls it.
  - All seven field kinds are checked up front, before rule 1, on every call (`Skip` answers included), each by its
    declaration and in 6.3's list order: `SkeletalMesh`, `Animations` and `ParentAnimComponent` objects;
    `bSkipUpdateSkelPose` and `bForceDiscardRootMotion` bools; `bForceRefpose` an int (non-zero is set); `SpaceBases`
    an array of `Core.Object.BoneAtom`.
  - Then the note's rules 1 to 6. The mesh's residency check comes first, so an unresident mesh is `NotRead`.
- **The two natives (6.4).** `ForceSkelUpdate()` and `UpdateAnimations()`, bound keep-existing per key in Off from
  the world pack. Each checks:
  - its receiver: set, and resident (`NotRead`, the port's check, as `SetSkeletalMesh`'s);
  - its field's declaration (`bAttached` a bool, `Animations` an object);
  - then the note's answer: nothing, `NotRead` attached, or `NotPorted` with a tree.
- **Not built.** The mapping of the pose onto `Object.BoneAtom` values (6.1's last item) is for a fill path's write,
  and no fill path is ported. It comes with the first one.

**Tests** (`bone_transform_tests.rs`, 8, on invented packages and an invented source): the note's tests 1 to 10. Test
6 runs on components with `SkeletalMesh` None, with one variant class per field of the seven declared with a wrong
kind, one per field left undeclared, and one with two wrong (the first in the list reported). Test 2's fresh service
is a new fixture whose source knows the bind pose. Every `pose_update` and native call checks #518's three counters
and the name table's length unchanged.

**Gates.**
- `cargo test --workspace -j 2`: 3,226 passed / 0 failed / 51 ignored. `crate_edges_test.py` OK. Clippy has no hits in
  the touched files.
- Installed Off strict `--slice-gate` (release, under the machine lock): 4 ok / 9 NotYet / 1 error, every row the same
  as main's. No slice link reaches the layer (the note's section 4):
  - link 1 at the skeletal mesh data layer's missing skeleton (no `.psk` export on this machine);
  - link 8 at #599's mesh change (1 of 5; #645 is not on main);
  - link 9 an error at the unbound `GearboxMind.GetAIComponent()`.
- This is what the host did, not parity.
