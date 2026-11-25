## 2026-11-25: UpdateAnimations and ForceSkelUpdate, bound in every slice world

AI-assisted (Claude Code, Claude Opus 5.5). Lane BIND-UPDATEANIMATIONS-FORCESKELUPDATE, on main d3c7a13b. It follows
docs/verification/NATIVE_BONE_TRANSFORM_LAYER.md as landed (#646), section 6.4. No real-game session was run. **Every
rule is UNVERIFIED in the running game.** Off only; Reference binds nothing.

**The binding.** IMPL-BONE-TRANSFORM-LAYER (#656, decision 2026-11-19-267) binds `SkeletalMeshComponent.ForceSkelUpdate()`
and `UpdateAnimations()` keep-existing per key in Off, from the world's own pack (`bone_transform::register`):
- `ForceSkelUpdate()` is nothing when `bAttached` is clear, `NotYet { NotRead }` when it is set;
- `UpdateAnimations()` is nothing with no `Animations` tree, `NotYet { NotPorted }` with one, nothing written.

That matches section 6.4 of the note. `World::new` registers that pack unconditionally, before the caller's packs, in
every slice world built through it: combat, equip, pickup and cast (`phaselock_world`). No other pack binds either key,
so no world needs a binding of its own and first-wins has nothing to settle. This lane adds two tests:
- `world_new_binds_force_skel_update_and_update_animations_in_off_only` (world/tests.rs): `World::new` itself binds
  both keys in Off and neither under Reference;
- `the_world_pack_binds_both_natives_in_off_only` (bone_transform_tests.rs): the pack alone does the same, a second
  registration keeps the bound natives, and they answer nothing on an unattached, tree-less component.

The refusals and counters are already covered by `bone_transform_tests.rs` from #656.

**Which slice links reach them** (the game's script, read with `research/script_disasm.py` on 2026-11-20):
- **`UpdateAnimations`** is called from `WillowPawn.UpdateAnimSets` and `WillowPlayerPawn.UpdateAnimSets`, on `Mesh`
  and on the player's `Arms`. Their slice-side callers are `ApplyBodyClassStartingValues` (Maya's spawn path: links 1,
  3, 4, 5, 13 and 14), `WeaponChanged`, `SetPawnAnimInfo` and the environment-tag paths;
- **`ForceSkelUpdate`** is called from `WillowWeapon.PlayWeaponEquip`, on the player's `Arms`. Its callers are
  `WillowPawn.WeaponChanged`, `WillowPlayerPawn.OnPlayArmAnimation` and `WillowWeapon.TimeWeaponEquipping`, the equip
  paths (link 8).

**No call in the instrumented run** (installed Off strict `--slice-gate`, 4 ok / 10 NotYet / 0 error, as main d3c7a13):
- link 1 stops at `maya:8`, at the skeletal mesh data layer's missing reference skeleton, before Maya's body-class
  step. Links 3, 4, 5, 13 and 14 do not run their own steps: they are blocked behind link 1;
- link 8 stops in `init`, at `SetSkeletalMesh`'s LOD info, before any `WeaponChanged` or `TimeWeaponEquipping`;
- a throwaway probe (an `eprintln!` at the entry of each native, not committed) counted **zero** calls of either
  native in the instrumented Off strict run on main d3c7a13, which again gave 4 ok / 10 NotYet / 0 error. That result
  covers that run only.

What the natives do once those blockers lift is not known. From section 6.4, Maya's mesh and arms would have trees
there, so `UpdateAnimations` would answer `NotPorted`, and `ForceSkelUpdate` would answer nothing on an unattached arms
mesh. Both are UNVERIFIED. This is what the host did, not parity.

**Gates.** `cargo test --workspace -j 8`: 3,365 passed / 0 failed / 52 ignored. `crate_edges_test.py`: 19 OK. Installed
Off strict `--slice-gate` (release build, under the machine lock): 4 ok / 10 NotYet / 0 error; link 1 is 0 of 2 at
`maya:8`, link 8 is 1 of 2 steps (`make`) and stops in `init` at `SetSkeletalMesh`'s LOD info, link 9 stops in `cast` at
`Actor.Trace`'s component bounds.
