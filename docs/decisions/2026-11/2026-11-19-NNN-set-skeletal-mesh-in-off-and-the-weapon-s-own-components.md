## 2026-11-19: SkeletalMeshComponent.SetSkeletalMesh in Off, and the equip harness's own weapon components

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SET-SKELETAL-MESH, slice link 8. Coded from
docs/verification/NATIVE_SET_SKELETAL_MESH.md alone (#583 at 231bbaed). Clean room: lane ANALYST-SET-SKELETAL-MESH
read the native, and this lane did not open the executable or the analysis output. Synthetic fixtures only.

- **What changes (`ow-world` `world::skeletal_mesh`, bound by the world's pack in Off only; Reference keeps the stub
  policy).** `SkeletalMeshComponent.SetSkeletalMesh(NewMesh, optional bKeepSpaceBases)`, the note's section 4, in
  order:
  1. the receiver must be set and resident. An unloaded handle is `NotYet { NotRead }`, "SetSkeletalMesh: the
     component is an unloaded handle (a load is a write)";
  2. the current mesh is read straight from the component's `SkeletalMesh` slot. A class with no `SkeletalMesh`
     object field is `VmError::Script`;
  3. `NewMesh` is an object argument, and `Object(None)` is null. `bKeepSpaceBases` is false when omitted
     (`NativeCall::has`) and must be a bool when supplied. Any other shape is `VmError::Script`;
  4. identity comes first, before either mesh's residency is looked at. Equal handles (both None, or the same handle,
     even an unloaded one) do nothing. Unequal handles (a replacement, or a clear to None) are
     `NotYet { LayerAbsent("a skeletal mesh component's bone transforms") }`, "SetSkeletalMesh: a mesh change (bones,
     the animation tree's per-bone state, physics and render state)", raised before any write.

  The rule takes the VM by shared reference, so it writes nothing, loads nothing and draws nothing on every path.
- **The harness input (the hub's ruling on #583, a separate commit), UNVERIFIED.** In the game every spawned weapon
  owns its mesh components. The equip run (`ow-cli` `inventory::equip_run`, shared by `--equip-run` and slice-gate
  links 7 and 8) makes its weapon from the class defaults, which leaves the shared template components in its slots.
  In Off, the weapon now gets its own copy of each component the class default owns, through #435's spawn-path
  instancing (`ow_world::actors::instance_components`). The run logs "harness input: the weapon's components
  instanced through the spawn path (UNVERIFIED harness input): <n> component(s)". The native's rule 1 stays for any
  other caller that hands it an unloaded handle.
- **Tests** (invented classes and an invented package, rule 9), `world::skeletal_mesh::tests`:
  - the same mesh does nothing: None to None with the flag omitted or supplied, the same handle, and the same
    unloaded handle, which stays unloaded;
  - a change is `LayerAbsent`: None to a mesh, a mesh to another, a mesh to None, and a mesh to an unloaded handle,
    which is not loaded. The slot is unchanged each time;
  - an unloaded receiver is `NotRead`. A class with no `SkeletalMesh`, a non-object `SkeletalMesh`, a non-object
    `NewMesh` and a non-bool `bKeepSpaceBases` are `Script`.

  Each call is checked to make no object.
- **Slice probe** (installed Off `--slice-gate`, strict, under the lock, on a throwaway merge with #572, #534 and #573;
  #552 conflicts with #534 and was left out): 1 ok / 5 NotYet / 8 errors. Link 8 is 1 of 5 driver steps. `make`
  passes with 6 components instanced, and `init` stops at "stopped after partial writes: value resolver class
  WillowGame.DamageTypeAttributeValueResolver". The probe's output does not show whether the native's None resets
  ran before that stop, and the mesh-change path (`InitMeshes`) was not reached.
- **Verified:** the host tests and the slice probe above only. **UNVERIFIED in the running game**: every rule
  here, and the harness input (the note's live check 1 would confirm per-weapon components).
