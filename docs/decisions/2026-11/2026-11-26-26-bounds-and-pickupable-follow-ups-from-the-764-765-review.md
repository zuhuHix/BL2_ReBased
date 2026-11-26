## 2026-11-26: Bounds and pickupable follow-ups from the #764/#765 review

AI-assisted (Claude Code, Opus 5.5). Lane BOUNDS-AND-PICKUPABLE-FOLLOWUPS (queue 0ae), from main 4d2fd17. It fixes the
LOW findings of the #764/#765 review; it did not read the executable, and no session was run.

Edits: docs/decisions/2026-11/2026-11-26-21-a-uniform-positive-scale-is-answered-in-the-skeletal-bounds.md

That entry gains one sentence: on the parent source (source b), a uniform non-unit scale is now the simplest
transform, so it answers `PARENT_ROOT` there instead of `WORLD_TRANSFORM` (review of #764, item 3).

- **`Pickupable_IsEnabled`** (`ow-inventory` `pickup.rs`, review of #765, item 1): the `bPickupable` declaration is
  checked from the receiver's class before the loaded test. A class that declares none, or declares it as other than
  a bool, is `VmError::Script` even for an unloaded receiver, as the module documentation already said. Two tests:
  no field, and an int field, each also on an unloaded receiver.
- **The skeletal stored box** (`ow-world` `skeletal_bounds.rs`, review of #764):
  - item 2: a test that "uniform" means exactly equal. `Scale3D` (1, 1, 1.0000001) is the world transform's
    `NotRead`;
  - item 3: the parent-source test asserts `PARENT_ROOT` at a uniform `Scale` 2 (the sentence above);
  - item 4: the owner's `PrePivot` is now read. A non-zero `PrePivot` leaves the simplest transform: the world
    transform's `NotRead`, on source f and on source b. **UNVERIFIED:** the port's reading that a pre-pivot moves the
    owner's frame, not read from the native. A new test, and the toy `Actor` declares `PrePivot`.
- **Reference:** unchanged. Both natives are bound in Off only.
- **Gates:**
  - `cargo test --workspace -j 8`: 3,830 passed, 0 failed, 59 ignored;
  - `crate_edges_test.py`: OK;
  - the installed Off strict `--slice-gate` (release build, under the machine lock): 6 ok / 7 not ok / 1 error,
    X = 51.2%. That is the same on base 4d2fd17 and on this change, all 14 rows identical.
- **Verified:** the tests on invented data, and the unchanged slice rows. **Not verified:** everything in the running
  game.
- **Sensitive files:** none.
