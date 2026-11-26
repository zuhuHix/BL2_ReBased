## 2026-11-26: A socket attach records its component under the pawn

AI-assisted (Claude Code, Opus 5.5). Lane SOCKETS-PUBLISH-AND-PHYSX-LEVEL, part 1, stacked on #770 (f489ac7b). It
implements the hub's socket ruling: in the game, the head and gear-likeness meshes belong to Maya's pawn, attached to
the body mesh's socket. This lane did not read the executable. No session was run. UNVERIFIED in the running game.

- **What was decided**, in `ow-world`'s `body_composition` (Off only, as the whole build is):
  - each **socket-attached** component is published to the attach record under the pawn, with the same publish as an
    actor attach, once every write of the build has succeeded;
  - **its `Owner` first takes its parent's `Owner`** (the pawn, since the parent is actor-attached). The record's owner
    test (`actors::attached_to`) counts a component only while its `Owner` names the recorded owner. A local diagnostic
    (not committed) showed all eight of Maya's socket-attached meshes (one `CustomSkeletalMeshComponent`, seven
    `GearLikenessMeshComponent`) with `Owner` None after the build: the port's socket attach leaves `Owner` as the
    template has it. So the publish alone would not have let `InitializeRBPhysics` visit them;
  - the source is NATIVE_BODYCOMP_SOCKET_BONE_TABLES.md section 2.1, step 6. The game attaches the child "with the
    parent's Owner", which sets its `Owner` field. **That one field is the port's stand-in.** The rest of step 6 (the
    bone-transform layer: the world placement and the scene) is still not read and not done. UNVERIFIED, including the
    timing: the port writes `Owner` after the build's other writes, where the game sets it during the attach;
  - the preflight checks that every made component's class declares `Owner`, before the first write.
- **`InitializeRBPhysics`** then visits a recorded `Attachments` component by the class table (decision
  2026-11-26-25). For Maya's skeletal subclasses with their flags off, that writes `bSkelCompFixed`. The rows do not
  show that write.
- **Reference:** unchanged.
- **The slice gate** (installed, Off, strict, release build, under the machine lock; #770's head f489ac7b and this
  commit): 6 ok / 8 not ok, 51.2% on both, and **every row unchanged**. Links 1, 3, 4, 5, 13 and 14 still stop at the
  unbound `Engine.GetPhysXLevel()`. So `InitializeRBPhysics` passes with the socket children visited, with no refusal.
- **Tests** (`body_composition_socket_tests.rs`, invented data):
  - a socket-attached head and likeness component take the pawn as `Owner` (their template's is None) and are in the
    record under the pawn, as their parent is;
  - `InitializeRBPhysics`' half is `player_pawn_tail::tests::an_attached_attachment_takes_the_same_rule`.

  The earlier test that told a socket hit from the owner retry by `Owner` now uses the pawn's `Components`, since
  both now end with the pawn as `Owner`. Mutation-checked: dropping the `Owner` write, or the socket publish, fails the
  new test.
- **Verified:** the tests on invented data and the installed rows above. **Not verified:** everything in the running
  game.
- **Sensitive files:** none.
