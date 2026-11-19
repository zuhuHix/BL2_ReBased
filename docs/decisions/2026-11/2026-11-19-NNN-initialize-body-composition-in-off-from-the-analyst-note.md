## 2026-11-19: InstanceDataHelper.InitializeBodyComposition in Off from the analyst note

AI-assisted (Claude Code). Lane IMPL-BODY-COMPOSITION, implementer (clean room). This lane coded
`InstanceDataHelper.InitializeBodyComposition` for the pawn wrapper from `docs/verification/NATIVE_INITIALIZE_BODY_COMPOSITION.md`
alone (analyst lane ANALYST-BODY-COMPOSITION, PR #529 at its revised head 87c8b9cd) and the hub's rulings. This lane
did not read the executable. There was no real-game session, so every rule stays **UNVERIFIED** in the running game.
No dependency or crate edge was added, no bounds check was loosened, and Reference is unchanged. The lane is stacked
on #522. The decision number is a placeholder.

- **The write tracker:** #518's counters (`Heap::committed_writes` and `note_write`, `GameRng::draws`,
  `DefStore::load_transitions`, and the VM's `note_write` calls) are copied unchanged from `fix/rust-483-followup` @
  211484a5 in one commit, so the rebase folds. A `NotYet` returned after any counter moved is marked "stopped after
  partial writes", its kind kept.
- **Scope** (the note's section 7): the pawn wrapper only, mode 0, the pawn's own `InstanceDataState` and
  `ReplicatedInstanceDataState`. The script native, for the other callers, is `NotYet { NotPorted }`,
  "InitializeBodyComposition called from script (its destinations)".
- **`ow-world` `world/body_composition.rs`, Off only. The preflight** (write-free and load-free), in order:
  - shapes (`VmError::Script`); a rebuild (`Data` not empty) and a client-side apply (`Role` not 3 with
    `bNeedToApplyThisState`), both `NotRead`;
  - the walk (section 3): a baseless attachment's parent is the owner; a base is read as its record stands (no "base
    first"); a switch base makes its child when the switch's `Int` is the child's `SwitchIdentifier`, and the child's
    parent is the switch's parent; a custom head or skin counts as not made; a removed attachment is skipped;
  - for each attachment the walk makes, in list order:
    - object and template types (`NotRead`);
    - a hit region (`NotRead`);
    - the attach: an actor parent with a socket name (`NotRead`, the actor's own socket components); a skeletal
      parent with a socket name and a mesh asset (`LayerAbsent`, its socket and bone tables); with no mesh asset or no
      socket name the skeletal attach answers false and the owner retry (an actor attach with no socket name) takes it;
    - the `Parent` link, identifier `Parent` with both components skeletal (`NotRead`, hub ruling 3);
    - the pawn's `PostInitBodyComposition`: identifier `Mesh` on a skeletal component is `LayerAbsent`, "the new
      Mesh's animation nodes (...)", layer "an animation-tree instance for the skeletal mesh";
    - the hook's release of the pawn's old `Mesh` or `CylinderComponent` (a detach, then removal from a global list),
      when the pawn already holds one, is `NotRead`, "InitializeBodyComposition: releasing the old <field>" (hub,
      20:03Z: stock pawn defaults give a cylinder, so the cylinder branch is not plain writes). Since review r1 of
      #542 the `Mesh` release comes before the animation nodes (section 4.5.1 releases first), and a second
      `CylinderComponent` attachment in one walk is a release too.
- **The writes**, in the note's order, per made attachment in list order:
  - a component: made from its template (outered to the owner, the template's properties); its entry appended (4.1's
    table: `Name`, `Type`, `DeleteByOwner`, `ComponentData` with the new component, `BodyCompositionIndex` copied, the
    rest zero); its collision flags (4.1's table, on a `PrimitiveComponent`, the owner's flags too); the actor attach
    (`Owner`, and `Components` if not there); then `PostInitBodyComposition`: `CylinderComponent` and, unless the
    body class's `bUseMeshForCollision`, `CollisionComponent` for identifier `CylinderComponent`; `HeadMesh` for a
    `Data.Name` of `CustomHead` (the light environment is no longer written: ruling 4 below);
  - a switch: the whole `Data`, `Float` 0;
  - a custom head or skin: `Name`, `Type`, `BodyCompositionIndex`, the rest zero (`Replication` and `Object`
    included);
  - after the walk, on authority: the switch packing exactly as section 5 (positions used up by unmade switches, the
    width from `BitsRequiredForMaxSwitchValue` or the truncated `Float`, no mask, a width of 0 packing at the current
    offset, at most 32 switches), then `bNetDirty` and `bForceNetUpdate`.
- **The hub's rulings:**
  1. a component made from a template counts as valid;
  2. a custom head or skin entry is appended and nothing is applied;
  3. the `Parent` link stays `NotYet { NotRead }`;
  4. (19:58Z, with the note's r2 at 8b324f22 and review r1 of #542) the render-side hooks answer nothing only where
     they write no field the script or a slice step reads. The gear-likeness step, the mode-0 rendering setting and
     the light environment were not read far enough to list their fields, and their conditions are unread (taken as
     allowing the hook, UNVERIFIED), so **every made component** is `NotYet { LayerAbsent }`, "InitializeBodyComposition:
     the body's rendering hooks", in the preflight before any write, after the `Mesh`, `CylinderComponent` and
     `HeadMesh` checks. **This makes the component write path unreachable on installed data until the hooks are
     read**; switches and custom entries still write. The component writes stay covered by tests through a
     test-only switch that answers the hooks with nothing on the test's thread.
- **This lane's narrower answers** (named `NotRead`): a component parent that is not a skeletal component; a `Mesh`
  attachment whose component is not skeletal; a skeletal parent whose template is not loaded (its mesh asset cannot be
  read without a load); a template whose load already failed; an instance-data type outside the note's list; a mode
  other than 0. The release refuses whenever the old field is set, while the note's r2 says "set and attached (or the
  port cannot tell)": the port cannot tell, so this is the narrower answer (review r1 of #542, item 3).
- **Port answer, UNVERIFIED:** the global "attaching later" state is never set (section 7, item 5).
- **The order** (hub, 20:03Z): with an animation-tree layer, Maya's attachment 1 meets its socket attach (the
  socket-table `LayerAbsent`, the mesh-data reader) before its `Parent` link, as section 7 lists them.
- **`InitializePawnMeshes`** (#522) runs the build instead of its `LayerAbsent`, with one more write-free check after
  the build's preflight succeeds and before its first write (review r1 of #542, item 7): a body class that sets
  `UseRootMotion` or `UseRootRotation` is `NotYet { NotRead }` (the two mesh fields are named by fit only).
- **Tests:** 21 in `world/body_composition_tests.rs`, on invented classes, structs and packages (the component
  classes in an invented `Engine.upk`, templates in an invented `GD_ToyBody.upk`):
  - production: a made component is the rendering hooks' `LayerAbsent`, write-free, while switches and custom entries
    write; the animation-node `LayerAbsent`; the old cylinder's and old `Mesh`'s release `NotRead` (the `Mesh`
    release first); a second cylinder attachment's release; the root-motion check after the preflight; the packing
    fields probed before any write; a failed template refused before any write; eight named refusals, write-free; the
    tracker; the script native's `NotPorted`;
  - the component writes, with the hooks answered through the test-only switch: the cylinder made, entered, collided,
    attached and posted; `bUseMeshForCollision`; the collision table for three types; a switch's copy and its gating;
    the owner retry with `HeadMesh` (no light environment write); an unloaded skeletal parent template's `NotRead`; a
    template loaded by the writes counted by `load_transitions`;
  - the switch packing's edge cases and the 32-switch limit; a custom head; a missing template.
- **The slice** (installed, Off, strict, with #528 @ 9cd49c14 merged and `Engine.IsSplitScreen()` bound false, both
  locally only): 1 ok / 10 NotYet / 3 errors. Links 1, 3, 4, 5, 13 and 14 now stop in `maya:8` at the
  animation-node `LayerAbsent` on Maya's attachment 0, found in the preflight (no prefix, nothing written).

**Superseded:** the first version of this lane implemented the note at cdf199be (the special refresh on `Mesh`, every
collision type but 0 refused, entry index written as `i` + 1) and then held the parts the revision would change as
"(pending the revised note)" refusals behind a test-only switch. Both are replaced by the above; the switch is gone.
