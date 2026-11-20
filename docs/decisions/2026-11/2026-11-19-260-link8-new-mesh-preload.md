## 2026-11-19: Link 8 preloads the gestalt definition's mesh as a harness input

AI-assisted (Claude Code). Lane LINK8-NEWMESH-PRELOAD. The decision number is pending (the landing run numbers it).
Stacked on #645 (fix/rust-skelmesh-mesh-change-b6d8 @ 44992baa). Synthetic fixtures only.

- **The stop:** with #645, link 8 passes `make` and stops in `init` at "SetSkeletalMesh: the new mesh is an unloaded
  handle (a load is a write)" (`NotRead`, NATIVE_SKELMESH_MESH_CHANGE.md row 6).
- **Which mesh:** `BuildCompositeMesh`'s gestalt path answers the weapon type's gestalt definition's
  `GestaltSkeletalMesh`, a shared mesh (NATIVE_BUILD_COMPOSITE_MESH.md). `InitMeshes` hands it to `SetSkeletalMesh`
  for both weapon components. On installed data it is `Weap_AssaultRifles.GestaltDef_AssaultRifle_GestaltSkeletalMesh`,
  an unloaded store handle in the same package as the gestalt definition.
- **Why a stand-in, not a port of the real load:** in the game the mesh is resident with the gestalt definition that
  references it, because the definition is loaded with its references. The port's definition store loads every object
  lazily, by handle. Porting load-with-references would change the store's loading model, which belongs to another lane
  and is not this harness question. So link 8's set-up preloads it, **as an UNVERIFIED stand-in** for that residency,
  as #592 did for Maya's mesh and HARNESS-LINK8-GESTALT-PRELOAD for the gestalt definition.
- **What changes (`ow-cli`, Off only):**
  - `equip_run::preload_gestalt_skeletal_mesh` runs right after the gestalt preload, both in the equip run's own set-up
    and in the slice gate's link 8 set-up;
  - when the weapon type's gestalt definition is resident and its `GestaltSkeletalMesh` is an unloaded handle, the
    mesh is loaded through the store;
  - it is logged before ("harness input: preload the gestalt definition's GestaltSkeletalMesh <path> (... UNVERIFIED
    stand-in)") and after (": loaded", or ": failed (...)" keeping the error);
  - otherwise nothing is written or loaded. `SetSkeletalMesh`'s rules (#645) are unchanged.
- **Verified:** host tests on an invented package:
  - the cold mesh is loaded after the gestalt preload, and the two lines are logged;
  - a warm mesh, a cold definition, no gestalt, and Reference write and load nothing.

  On the installed Off strict slice gate, link 8 now passes the new-mesh row and first stops at the mesh change's
  LOD row (`LayerAbsent`, "the skeletal mesh data layer's LOD models").
- **Not verified:** that the mesh is resident with its definition in the running game is **UNVERIFIED**. Every rule
  stays UNVERIFIED in game.
