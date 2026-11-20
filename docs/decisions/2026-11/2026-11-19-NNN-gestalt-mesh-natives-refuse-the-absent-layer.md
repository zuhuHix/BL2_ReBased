## 2026-11-19: The gestalt mesh natives refuse the absent gestalt data layer in Off

AI-assisted (Claude Code, Opus 5.5).

Lane IMPL-GESTALT-MESH-LAYER, implementer only (clean room). It is coded from NATIVE_BUILD_COMPOSITE_MESH.md (#588 at
ce0ec917) and NATIVE_INIT_GESTALT_MESH_DATA.md (#591 at 67170f22). This lane read neither native.

`ow_inventory::gestalt` binds `WillowWeapon.BuildCompositeMesh(object,weapondefinitiondata)` and
`SkeletalMeshComponent.InitGestaltMeshDataFromOther(object)` in Off. Reference keeps the stub policy.

- **The preflights** run in the notes' order. Both answers take `&Vm`, so they write nothing, load nothing and draw
  nothing. An unloaded type, gestalt definition, part or component refuses `NotRead`. An unloaded mesh handle's name
  is read from the heap without loading it. A component that is None or of the wrong class, and a field of the wrong
  shape, are data errors.
- **`BuildCompositeMesh` answers:**
  - no weapon type: None;
  - the gestalt path: `LayerAbsent("a gestalt component data layer (a skeletal mesh component's GestaltData)")`;
  - no gestalt path, with zero meshes: None;
  - one mesh: that mesh;
  - two or more meshes: `LayerAbsent("a skeletal mesh merge")`.
- **`InitGestaltMeshDataFromOther`** answers a self-copy with success and no write, even while the layer is absent.
  Otherwise it checks that both `GestaltData` slots are held whole (#591 r1):
  - an object without its slots: the layer's `LayerAbsent`, since a slotless resident object is a stand-in that
    never loads; only a store definition whose slots were never filled is `NotRead` (review r1 of #596);
  - a malformed value: the layer's `LayerAbsent`.

  Then it refuses that same `LayerAbsent`.
- **Not built: the layer itself** (the fill and the copy). The notes leave unread:
  - which step writes `PartMeshNames`;
  - the socket fill;
  - the bounds growth;
  - the member each write lands in (matched by position only).

  Building it now would mean inventing those.

Verified: nothing in the running game. Every rule is UNVERIFIED. Parsing is unchanged.
