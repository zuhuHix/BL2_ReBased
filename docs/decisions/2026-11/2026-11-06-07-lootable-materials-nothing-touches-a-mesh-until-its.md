## 2026-11-06: lootable materials: nothing touches a mesh until its materials are saved; no lootables fails the suite

AI-assisted (Claude). From Luna's second review of PR #120. The materials step now saves textures, instances and this run's folders first, and only then binds and saves meshes; if a folder save fails, no mesh is touched. A mesh whose own save fails changed only in memory, and the commandlet exits without saving it. A texture the instance refuses fails the row. `test_interactives.ps1` no longer passes on a machine with no imported lootables unless told `-AllowNoLootables`, and the summary then says so. Frame names now carry the full balance, and the checker requires each frame on disk.

- **Verified:** synthetic tests, with the Unreal calls faked, looking at the fake meshes' slots. The current manifest has no two balances with one short name.
- **`UNVERIFIED`:** none of this has run in the editor yet.

