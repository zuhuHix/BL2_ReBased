## 2026-11-19: UpdateAnimations and ForceSkelUpdate, bound in every slice world

AI-assisted (Claude Code, Claude Opus 5.5). Lane BIND-UPDATEANIMATIONS-FORCESKELUPDATE, on
`fix/rust-bone-transform-layer` (0c260d56). It follows docs/verification/NATIVE_BONE_TRANSFORM_LAYER.md at 94ab4339,
section 6.4. No real-game session was run. **Every rule is UNVERIFIED in the running game.** Off only; Reference
binds nothing.

**The binding.** IMPL-BONE-TRANSFORM-LAYER (0c260d56) already binds `SkeletalMeshComponent.ForceSkelUpdate()` and
`UpdateAnimations()` keep-existing per key in Off, from the world's own pack:
- `ForceSkelUpdate()` is nothing when unattached, `NotRead` when attached;
- `UpdateAnimations()` is nothing with no tree, `NotPorted` with one.

`World::new` registers that pack in every slice world: combat, equip, pickup and cast. So no world needs its own
binding. This lane adds one test: the world pack binds both keys in Off and neither under Reference, and the bound
natives answer nothing on an unattached, tree-less component. Their refusals and counters are already covered by
`bone_transform_tests.rs` (#518's three counters and the name table's length on every call).

**Which slice links reach them** (the script, through `research/script_disasm.py`, and a measurement-only probe):
- **`UpdateAnimations`** is called only from `WillowPawn.UpdateAnimSets` and `WillowPlayerPawn.UpdateAnimSets`, on
  `Mesh`, and on the player's `Arms`. Their slice-side callers are `ApplyBodyClassStartingValues` (Maya's spawn path:
  links 1, 3, 4, 5, 13 and 14), `WeaponChanged`, `SetPawnAnimInfo` and the environment-tag paths;
- **`ForceSkelUpdate`** is called only from `WillowWeapon.PlayWeaponEquip`, on the player's `Arms`. Its callers are
  `WillowPawn.WeaponChanged`, `WillowPlayerPawn.OnPlayArmAnimation` and `WillowWeapon.TimeWeaponEquipping`, which are
  the equip paths;
- **today no link reaches either.** A throwaway probe printed every call of the two natives, and the installed Off
  strict `--slice-gate` gave 4 ok / 9 NotYet / 1 error with **zero** calls:
  - link 1 stops at `maya:8`, before Maya's body-class step, at the skeletal mesh data layer's missing skeleton (no
    `.psk` export on this machine). Links 3, 4, 5, 13 and 14 are blocked behind it;
  - link 8's equip run calls no `WeaponChanged` or `TimeWeaponEquipping`, and stops in `init` at #599's mesh change;
  - when those stops lift, Maya's mesh and arms have trees, so `UpdateAnimations` would answer `NotPorted` there
    (UNVERIFIED), and `ForceSkelUpdate` would answer nothing on an unattached arms mesh.

**Gates.** `cargo test --workspace -j 2`: 3,227 passed / 0 failed / 51 ignored. Installed Off strict `--slice-gate`:
4 ok / 9 NotYet / 1 error, every row as main's. Link 1 is 0 of 2 at `maya:8`, link 8 is 1 of 5 at #599's mesh
change, and link 9 is an error at the unbound `GearboxMind.GetAIComponent()`. This is what the host did, not parity.
