## 2026-11-06: the lootable material bind has run, 18 of 18

AI-assisted (Claude). With the slot written back, the editor run bound all 18 meshes through 10 instances, and `test_interactives.ps1` passed with its new `lootable_materials` row (`passed=11`). No code changed in this entry's commit.

- **Verified:** that the host binds and renders these materials. A passing suite is not parity.
- **`UNVERIFIED`, host frames only and not compared with the game:** how the materials look; the storage locker's hinge, which reads as correct from the frames; the strong box's hinge, which stays open until `tools/real_game/loot_capture.ps1`'s side frame and its real-game frames decide it. `docs/verification/VENDORS.md` names the frames nobody read and the untriaged blue streak in the existing world.

