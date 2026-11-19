## 2026-11-19: Rust AttachMeshToPickup part 1 in Off: SetForceRefPose to AttachComponent, with an empty scene layer

AI-assisted (Claude Code). Lane IMPL-ATTACH-MESH-TO-PICKUP, part 1 (the file number is a placeholder until the
maintainer gives one). Coded from `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md` part 1 alone (analyst lane
ANALYST-ATTACH-MESH-TO-PICKUP, the note at fefadd6, PR #505), with the hub's ruling on the note's section 5 and the
hub's points from review r1 of #505. This lane did not read the natives. No real-game session was run. No dependency
was added and no bounds check was loosened. **Every rule is UNVERIFIED in the running game.** Stacked on #495.

- **What was added** (`crates/ow-inventory/src/attach_mesh.rs`, Off only; Reference binds nothing):
  - **The deferred reattach** (the note's section 2), shared by the next three natives. Its preflight runs before the
    calling native writes: an `Owner` set is `NotYet { NotRead }` (the owner's component update); no `Owner` and a
    non-zero `Scene` is `NotYet { NotRead }` (the detach and reattach). Otherwise its only write is
    `bNeedsReattach` = true.
  - `SkeletalMeshComponent.SetForceRefPose(bool)`: the preflight, then `bForceRefpose` = 1 or 0 (an int,
    **UNVERIFIED (fitted)** against `bOldForceRefPose`), then `bNeedsReattach`. It always writes.
  - `PrimitiveComponent.SetHidden(bool)` and `SetOwnerNoSee(bool)`: an unchanged flag writes nothing; a changed one
    runs the preflight, writes the flag, then `bNeedsReattach`. The flags, `HiddenGame` and `bOwnerNoSee`, are
    **UNVERIFIED (fitted)**, and are read from the component instance.
  - `SkeletalMeshComponent.HideBoneByName(name,byte)`: a None name, or the instance's `SkeletalMesh` None
    (**UNVERIFIED (fitted)**), does nothing; otherwise `NotYet { NotRead }` before any write.
  - `Actor.AttachComponent(object)`:
    - the guards' "nothing", as plain returns: no component, the actor's `bDeleteMe`, or the actor destroyed on the
      heap (`Heap::destroy`), the port's stand-in for the pending-kill flag, **UNVERIFIED (fitted)**;
    - after the guards, a world without the scene layer is `NotYet { LayerAbsent }`, "a scene that components attach
      to: render, collision and physics registration (the port has none)";
    - with the layer: an attached component (`bAttached`) is `NotYet { NotRead }` before any write. Otherwise the
      writes, unconditional, in the note's section 5 order: `bNeedsReattach` and `bNeedsUpdateTransform` cleared,
      `Owner` = the actor, then the component appended to `Components` with no duplicate check. `Scene` and
      `bAttached` are not written, and nothing is registered.
- **The hub's ruling on section 5: a port Off answer, UNVERIFIED.** A slice world in Off gets an explicit, empty
  scene layer (`SceneLayer`, a VM service), the pattern of #495's `ComponentRegistry`. The pickup world's set-up
  (`ow-cli`, `pickup_world`) adds it next to the registry. Every other world is unchanged and keeps the named
  `LayerAbsent`. A later lane that adds a real scene feeds the same layer.
- **Partial writes.** A preflight `NotYet` is returned unmarked; the `make` step's write tracking adds the prefix.
  An error after a native's first write is marked once, its kind and record kept.
- **Expected after the attach:** `Owner` is the pickup, so a later deferred reattach on that component answers
  `NotRead` (the owner's update). That is the note's rule, not a regression.
- **Tests** (`attach_mesh_tests.rs`, 8, on an invented `Engine` package): the slice's `SetForceRefPose` writes and
  no short cut; the reattach preflight's two refusals for all three natives, nothing written; unchanged and changed
  flags; `HideBoneByName` without a mesh, with a mesh set, and with a None name; `AttachComponent` without the layer
  after the guards; with the layer: the guards, an attached component, the writes in order, `Scene` and `bAttached`
  untouched, and a second append; Reference binds none of the five; the partial-writes mark.
- **Installed slice gate (Off, strict):** links 6 and 7 move from the unbound `SetForceRefPose` to the unbound
  `PrimitiveComponent.SetLightEnvironment(object)` in `make` (part 2 of the note, section 6). The other 12 rows are
  byte-identical. Numbers in `docs/verification/GATES.md`. This is what the host did, not parity.
- **Open:** part 2 of the note (`SetLightEnvironment`, `InitializePickupForRBPhysics`, `SpawnPickupParticles`); the
  note's open questions 1 to 5, and its three live checks.
