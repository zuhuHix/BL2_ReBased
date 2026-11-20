## 2026-11-19: The equip world preloads the weapon type's gestalt definition

AI-assisted (Claude Code, Claude Opus 5.5). Lane HARNESS-LINK8-GESTALT-PRELOAD, from the hub's ruling on stop 3 of
ANALYST-LINK8-NEXT (`research/rust-link8-next`): a logged harness preload in a follow-up PR, not in #596. No native was
read and no real-game session was run. **The preload is a harness input, UNVERIFIED in the running game.** Off only.

**What it does.** `--equip-run`'s and link 8's weapon set-up (`equip_run.rs`, `prepare_equip`, and the slice gate's
link 8 `make`) now runs `preload_gestalt_mesh` after #573's muzzle-flash preload. It reads the weapon's
`DefinitionData.WeaponTypeDefinition`, then that type's `GestaltMesh`. When `GestaltMesh` is set and is one of the
store's unloaded handles, the store loads it. Before the load, the harness output gets "harness input: preload the
weapon type's GestaltMesh <path> (BuildCompositeMesh loads nothing; the game keeps it resident: UNVERIFIED stand-in)",
and after it, "...: loaded" or "...: failed (<error>)". A failed load keeps its error, so `make` stops there, as #573's
preload does. No weapon type, no `GestaltMesh`, or one already loaded: nothing. Reference: nothing.

**Why a harness input.** `BuildCompositeMesh`'s preflight (#596, NATIVE_BUILD_COMPOSITE_MESH.md section 4, step 3)
refuses a gestalt definition that is not resident, because a lazy load is a write (#518). The preflight is right to
refuse. The gap is that nothing in the equip world's set-up loads the definition, which the game keeps resident with
the weapon type. This load stands in for that, the way #573 preloads the muzzle-flash collections. It needs no #596 or
#613 code, so the branch is on main.

**Tests** (`inventory/equip_run_gestalt_tests.rs`, 3, on an invented package, rule 9):
- Off loads a cold gestalt definition and logs it before and after; a second call adds nothing;
- a type with no `GestaltMesh`, and a weapon with no type, load and log nothing, and #518's heap-write and
  load-transition counts stay where they were;
- Reference preloads nothing.

**Gates.** `cargo test --workspace -j 2`: 2,981 passed / 0 failed / 51 ignored. `crate_edges_test.py` OK. Clippy has
no hits in the touched files. ctest and verify_packages were not run (no C++ change). Installed Off strict
`--slice-gate` under the machine lock, release build:
- **this branch alone** (main 4c4522d7 plus this lane): 4 ok / 9 NotYet / 1 error. Link 8 passes `make` and stops in
  `init` at the unbound `GetExpLevelFromManufacturerData`, as on main;
- **a probe merge** (this branch, #596 at 491a1ba5, #613 at 62e3877b, #577 at 0cfb99a7 and #599 at 5d03d98a,
  conflicts resolved by hand, link 8 given #577's `Link8` weapon route; never pushed): 4 ok / 10 NotYet / 0 errors.
  Link 8 passes `make` (1 of 5 steps). Its harness lines show the gestalt definition
  `Weap_AssaultRifles.GestaltDef_AssaultRifle` loaded, and `BuildCompositeMesh` passes. Link 8 then stops in `init` at
  #599's `SetSkeletalMesh` mesh-change refusal (LayerAbsent: a component's bone transforms), after partial writes.
  That stop stays NotYet until the layer exists, per the hub's ruling; an analyst lane reads what the layer needs.

This is what the host did, not parity.

### Review r1 of #632 (FIX)

AI-assisted (Claude Code, Claude Opus 5.5).
1. **`--equip-run` reports its harness lines.** A successful cold preload changes what `init` sees, so each weapon's
   line in the output file now carries a `harness` array: the `harness input:` lines its set-up logged. The summary
   carries the distinct lines in the order they were first seen. When a weapon's set-up fails, the command still returns
   its original error, but it first writes the lines so far and that weapon's line, with the set-up error under `make`
   and its `harness` array. Link 8 already captured both lines.
2. **The counter checks cover every no-op path.** The tests now snapshot #518's heap-write and load-transition counts
   around the warm-handle call and the Reference call too, and assert that they do not move.

An installed `--equip-run` (50 weapons, under the machine lock) shows the GestaltMesh harness lines in the per-weapon
output. This is what the host did, not parity.
