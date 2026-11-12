## 2026-11-06: a lootable material that did not bind now fails the run

AI-assisted (Claude). The first editor run of the materials step bound only the 12 pickups. The skeletal path changed a copy of the slot, every lootable stayed grey, and both the import and `test_interactives` still read as a pass. Now `import_lootables.ps1` fails when a planned mesh did not bind, after writing the report, and the host logs the material each lootable type renders, which `check_interactives.py` reports as `INCOMPLETE` unless the materials step bound it. The slot is written back, and a rerun updates instances in place instead of deleting what meshes use.

- **Verified:** synthetic tests, including one the old bind fails; the new check exits 1 on the failed run's own report. No parsing behaviour changed.
- **`UNVERIFIED`:** that the fixed bind holds in the editor (not run yet), and how a bound material looks.

