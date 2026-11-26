## 2026-11-26: The body composition's actor attach publishes to the attach record

AI-assisted (Claude Code, Opus 5.5). Lane BODYCOMP-ATTACH-PUBLISHES, on #766's branch (fix/maya-pawn-from-archetype @
b6bd32ac). It implements the hub's ruling on decision 2026-11-26-25's open question: in the game these components are
attached. This lane did not read the executable. No session was run. UNVERIFIED in the running game.

- **What was decided**, in `ow-world`'s `body_composition` (Off only, as the whole build is):
  - each component the build attaches **to the actor** (`attach_to_actor`, no socket) is published to the attach
    record under the pawn (`actors::publish_attach`). That happens once every write of the build has succeeded, as
    the spawn's component update publishes its copies (`component_attach.rs`);
  - the preflight checks the record is not borrowed (`actors::record_ready`) whenever an actor attach is planned. The
    VM already refuses a borrowed service before any native call, so this check is a guard of the port's, as in the
    spawn;
  - a world with no scene layer publishes nothing;
  - **socket-attached components are not published.** The ruling names the actor attach. A socket attach leaves
    `Owner` alone (section 4.1), so the record's owner test would not hold for them anyway.
- **Checked first** (the hub's request): with the record published, a later `InitializePawnMeshes` finds the old
  `Mesh` or `CylinderComponent` recorded under the pawn. NATIVE_INITIALIZE_BODY_COMPOSITION.md section 4.5 and section
  7 ("set and attached (or the port cannot tell whether it is attached)") make that the release `NotRead`. #766's rule
  gives exactly that through its recorded arm; it is kept unchanged.
  - Its owned-but-unrecorded arm stays for other paths. Its comment no longer names this build's attach as a
    non-publishing path.
  - A second build as it stands meets the call-wide rebuild refusal first, because its entries are there.
- **Reference:** unchanged.
- **The slice gate** (installed, Off, strict, release build, under the machine lock). The trees are local merges and
  are not pushed. Before: #766 (b6bd32ac) merged with #768 (de809eb). After: that tree plus this change. 6 ok / 8 not ok,
  51.2% on both.
  - Links 1, 3, 4, 5, 13 and 14 pass `InitializeRBPhysics`. They now stop at **the unbound `Engine.GetPhysXLevel()`**,
    graded an error. That is NATIVE_LINK1_RIGID_BODY_LAYER.md section 4's prediction.
  - Before, they stopped at its refusal L ("Actor.GetBoundingCylinder: the colliding components' bounds").
  - Every other row is unchanged.
- **An open point for the hub:** NATIVE_LINK1_RIGID_BODY_LAYER.md 2.3 says Maya's socket attachments (the custom head
  and the gear-likeness meshes) each get `bSkelCompFixed` := true. The class table visits an `Attachments` component
  only when it is in the attach record under the pawn, and socket attaches are not published. So the port leaves their
  flag alone, with no refusal. Whether a socket attach records its component (and under which owner) is not ruled.
- **Tests** (`body_composition_tests.rs`, invented data):
  - the made `Mesh` and `CylinderComponent` are in the record under the pawn, and nothing else is;
  - a second build: the rebuild refusal; with the entries cleared, the release `NotRead` on the recorded old
    component (`Mesh` and `CylinderComponent`). Both are write-free;
  - a borrowed record refuses before any write.

  Mutation-checked: dropping the publish fails the first two. Dropping the preflight's `record_ready` fails none,
  because the VM's own check comes first.
- **Gates:** `cargo test --workspace -j 8` 3,830 passed / 0 failed / 59 ignored on this branch, and 3,849 / 0 / 59 on
  the merged tree. `crate_edges_test.py` OK.
- **Verified:** the tests on invented data and the installed rows above. **Not verified:** everything in the running
  game.
- **Sensitive files:** none.
