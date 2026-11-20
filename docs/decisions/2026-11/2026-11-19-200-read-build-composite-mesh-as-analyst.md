## 2026-11-19: Read WillowWeapon.BuildCompositeMesh for link 8 as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-BUILD-COMPOSITE-MESH, analyst only, for slice link 8. The decision
number is pending (the hub numbers decisions at landing); NNN is a placeholder.

- **What:** a new own-words note, `docs/verification/NATIVE_BUILD_COMPOSITE_MESH.md`, and one `NATIVE_INDEX.md` row.
  No code changes.
- **Sources:**
  - the script (`research/script_disasm.py`);
  - the installed packages (`ow-package --def-dump`, `tools/ghidra/class_layout.py`);
  - `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only, under the machine lock);
  - a local, uncommitted slice-gate probe.

  Raw output stays under ignored `local/analysis/bcm/`. No bl-sdk session took place, so **every rule is
  UNVERIFIED** in the running game.
- **Clean room:** this lane's author will not implement this native.
- **Findings:**
  - **The rule:** None without a weapon type. The gestalt path, taken when the type's gestalt definition has a mesh,
    collects each part's gestalt names, writes the component's gestalt section data and returns the shared gestalt
    mesh. Otherwise it returns none, the one part mesh, or a merged mesh. Name parts are excluded; it loads and
    draws nothing of its own.
  - **The slice gun** takes the gestalt path with 4 names (barrel, body, grip, stock).
  - **The proposed Off answer:** a write-free, load-free preflight, then `LayerAbsent` for the gestalt data write
    and for a merge.
  - **Review r1 of #588:** the hub's render-side ruling was overturned. `InitMeshes` copies the gestalt data at
    once (first to third person, then third person to each extra slot, both under the `GestaltMesh` and
    `GestaltSkeletalMesh` checks), and later script queries weapon sockets (that dependency is UNVERIFIED). So the
    gestalt component state (`GestaltData`) is a port layer. The preflight order is defined: residency before
    fields, shapes as read, the component checked only on the gestalt path (None or the wrong class is a
    `VmError::Script`), and the part-name fallback read from handle name metadata without a load. The definition's
    `GestaltInfos` already decode; what Off lacks is the component state and its fill.
  - **The probe:** with a probe-only stand-in that skips the component write, link 8 next stops at the unbound
    `SkeletalMeshComponent.InitGestaltMeshDataFromOther(object)`.
