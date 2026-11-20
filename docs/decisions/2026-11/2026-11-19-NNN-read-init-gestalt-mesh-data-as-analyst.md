## 2026-11-19: Read SkeletalMeshComponent.InitGestaltMeshDataFromOther for link 8 as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-INIT-GESTALT-MESH-DATA, analyst only, for slice link 8. The
decision number is pending (the hub numbers decisions at landing); NNN is a placeholder.

- **What:** a new own-words note, `docs/verification/NATIVE_INIT_GESTALT_MESH_DATA.md`, and one `NATIVE_INDEX.md`
  row. No code changes.
- **Sources:**
  - the script (`research/script_disasm.py`);
  - the installed `Engine.upk` export table (the declared `GestaltDataContainer` members, names only);
  - `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only, under the machine lock);
  - a local, uncommitted slice-gate probe.

  Raw output stays under ignored `local/analysis/igm/`. No bl-sdk session took place, so **every rule is
  UNVERIFIED** in the running game.
- **Clean room:** this lane's author will not implement this native.
- **Findings:**
  - **The state:** the component's declared `GestaltData` struct (`PartMeshNames`, `SkeletalMesh`, `LODModelData`,
    `SocketMappings`, `GestaltAccessoryList`, `GestaltPartMatricesContainer`, `ReferencePoseBounds`). No script
    function names it or any member; reflection reaches it.
  - **The rule:** a whole-value assignment of the argument's `GestaltData` to the receiver's. Arrays are copied,
    the two object references are shared, and nothing else is touched: no mesh change, no load, no draw.
  - **The layer:** `BuildCompositeMesh` fills this state and this native copies it, so the note names one layer for
    both, homed in the declared property slot. The socket natives reading it is UNVERIFIED.
  - **The proposed Off answer:** a write-free preflight (receiver, then argument; None or the wrong class is a
    `VmError::Script`, an unloaded handle `NotRead`), then `LayerAbsent` with the same layer name as #588 until the
    layer exists, and after that the slot copy.
  - **Review r1 of #591:** a fourth preflight check makes both `GestaltData` slots be held whole, with no decode or
    load (else `NotRead`, or the same `LayerAbsent`), and the copy is one atomic value replacement. A self-copy
    answers success with no writes, even while the layer is absent. A tests list for the implementer was added, and
    NATIVE_SET_SKELETAL_MESH.md is cited as #583's until it lands.
  - **The probe:** with a no-op stand-in, link 8 (still 1 of 5 at `init`) goes from `error` to `notyet` at
    `SetSkeletalMesh`'s mesh change (#583's `LayerAbsent`). Installed strict: 1 ok, 5 notyet, 8 error.
