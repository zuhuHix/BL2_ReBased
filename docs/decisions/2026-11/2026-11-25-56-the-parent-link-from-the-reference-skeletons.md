## 2026-11-25: The body composition's Parent link, from the two reference skeletons

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-CUSTOMHEAD-PARENT-LINK, on main 5d20247f (the hub rebased it there
after #714 and #717 landed; written stacked on #717 at 85687ab6). Coded from NATIVE_INITIALIZE_BODY_COMPOSITION.md section 8 alone (#716, analyst lane
ANALYST-CUSTOMHEAD-PARENT-FLAG, decision 2026-11-25-51). Clean room: this lane is not its analyst, and no native code
or analysis output was opened. Every rule is UNVERIFIED in the running game.

- **What was decided:** hub ruling 3 on #529 (the `Parent` link kept `NotYet { NotRead }`) is lifted. Section 0's
  ruling line says so. `body_composition::PARENT_LINK` is gone. The gate is unchanged: identifier `Parent`, the
  parent a made `SkeletalMeshComponent`, and the new component one too. The preflight (`parent_link_plan`) reads
  only and loads nothing:
  1. the new class declares `ParentAnimComponent` (taking the parent), `ParentBoneMap` (an array of ints) and
     `bIsGestaltAccessoryMesh` (a bool), else `VmError::Script` naming it;
  2. the meshes as made: the new component's `SkeletalMesh` from its template, the parent's from the parent's
     template. Either None: the map is empty;
  3. the same mesh, or the flag set: the identity over the new mesh's reference skeleton;
  4. otherwise slot `k` is `skeletal::find_bone` on the parent mesh with bone `k`'s name, or -1. A name the name table
     does not hold is no match, and nothing is interned. For a duplicated bone name, `find_bone` takes the first, an
     **UNVERIFIED** choice;
  5. a skeleton that cannot be read refuses with its reader's own `NotYet`, before any write.

  The write phase sets `ParentAnimComponent` (the made parent), then `ParentBoneMap`, after the attachment's attach and
  before the pawn's `PostInitBodyComposition` writes for it.
- **One guard the note does not list:** the link reads the new component's mesh from its template. So a child
  template the store has not loaded is the walk's existing `lazy_template` `NotRead`, with nothing loaded, as the parent
  template already is. Step 1's declaration checks, which need only the class, come before this guard (review of
  #721).
- **Tests** (`body_composition_parent_tests.rs`, section 8.2):
  - the same mesh gives the identity, and different meshes map by name, `[1, -1, 0]` (tests 1 and 2);
  - the flag gives the identity without reading the parent's skeleton (3);
  - a missing mesh empties a template map (4);
  - no source installed, or no data for the parent's mesh, is the reader's `LayerAbsent`, write-free (5);
  - an uninterned bone name is -1, with nothing interned (6);
  - malformed `ParentBoneMap` and flag declarations are script errors (7);
  - a non-skeletal child or another identifier makes no link (8);
  - the write order, through an injected `HeadMesh` write failure: the attach and the link are in, and hook 2's write
    is not (9). The other side (review of #721): an injected failure of the child's actor attach leaves both link fields
    unwritten. Moving the link before the attach fails that test;
  - an unloaded child template is refused, as above.

  The old `PARENT_LINK` case of the named-refusals test is replaced by tests 1 to 4 (10), and Reference is unchanged
  (11). The hook-3 lane's Maya-shaped test now stops at the link's skeleton reader. The fixture gained the link's two
  fields, a second mesh export and three `Opts` switches (synthetic).
- **The slice gate** (installed Off strict, under the machine lock): this head at 807231fd against the base reading
  of 9ca895aa (the hook-3 lane's run, which 85687ab6 changes only in tests and docs). 5 ok / 9 NotYet / 0 errors on
  both.
  - Link 1 moves in `maya:8` from "InitializeBodyComposition: the Parent link's parent-bone map" to "the skeletal mesh
    data layer: the unloaded skeletal mesh CubeForCode". That is the reader's own `LayerAbsent`, unprefixed
    (write-free), as section 8.1 predicted: her map is the by-name one, and `CubeForCode`'s reference skeleton is not
    resident, and the reader loads nothing.
  - Links 3, 4, 5, 13 and 14, behind link 1, report the same new stop.
  - Every other row is unchanged.
- **Sensitive files:** none. `crates/ow-package` and `Cargo.toml` are unchanged.
- **Verified:** nothing in the running game.
