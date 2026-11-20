## 2026-11-20: Read WillowWeapon.BuildCompositeMesh for link 8 as analyst

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
    (the next native copies it) and for a merge. A hub alternative treats the gestalt data as render-side.
  - **The probe:** with that alternative as a stand-in, link 8 next stops at the unbound
    `SkeletalMeshComponent.InitGestaltMeshDataFromOther(object)`.
