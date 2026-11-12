## 2026-11-06: lootable materials: a failed save fails; each type must render its planned instance

AI-assisted (Claude). From Luna's review of PR #120. A failed asset save in the materials step now fails that mesh, or every mesh for the folder, instead of reporting a bind that is not on disk. An instance's name now includes a hash of its textures, so two exports with one name and different textures do not share one. `check_interactives.py` compares the host's path with the exact instance planned for each type, by full balance, and neither the lid nor the materials row can pass without the content folder and plan that say what was expected. A rerun clears an instance's parameters first, and `import_lootables.ps1` drops the old `import_report.json` when it starts.

- **Verified:** synthetic tests, with the Unreal calls faked. No parsing behaviour changed.
- **`UNVERIFIED`:** these paths have not run in the editor. The mesh and clip saves of the other editor modes still ignore the result: they are outside the materials step and were not changed.

