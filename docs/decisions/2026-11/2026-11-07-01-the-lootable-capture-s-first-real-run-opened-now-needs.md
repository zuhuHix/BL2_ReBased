## 2026-11-07: the lootable capture's first real run; opened now needs evidence

AI-assisted (Claude). `tools/real_game/loot_capture.ps1` ran once in the game: saves identical afterwards. Its first version counted a lootable as done when frames existed, and three of six had not opened. A lootable stage now needs one of the object's own state fields to have changed. The capture aims at the mesh's bounds, tries other sides, and tries a pickup in the open by touch and then by the use key. What the first run showed is in `docs/verification/VENDORS.md` section 9.

- **Verified:** with the game stubbed, tests that fail on the first version. Frames and live reads of the first run, by eye.
- **`UNVERIFIED`:** that any of those state fields changes on opening, since none has been observed. If none does, every lootable stage reports incomplete and the record shows what moved.

