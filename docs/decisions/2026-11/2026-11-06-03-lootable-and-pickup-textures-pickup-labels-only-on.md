## 2026-11-06: lootable and pickup textures; pickup labels only on placeholders

AI-assisted (Claude). The imported lootable and pickup meshes rendered as plain pale surfaces. `tools/lootables_editor.py` gains a `materials` step: diffuse and normal from UModel's export, one instance per material of a minimal material, bound to each mesh's single slot. A mesh with no answer stays unbound with a reason; a similar name alone never binds a material. No parsing behaviour changed.

- **Verified:** synthetic tests of the selection rules; the plan resolves all 18 meshes on the local export, with `Skel_StorageLocker` taking `Mati_StorageLocker` from its own import, read from the package's import table and UModel's log. Editor binding and rendering are not yet run.
- **`UNVERIFIED`:** how the result looks (UModel's material output is heuristic, and this is not the game's material graph); the two meshes' slots in the running game; whether the game shows text on cash and ammo pickups. The host now hides the stand-in label when the real mesh loads.

