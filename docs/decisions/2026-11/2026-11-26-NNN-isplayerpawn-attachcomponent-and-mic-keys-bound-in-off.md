## 2026-11-26: IsPlayerPawn, AttachComponent and MIC keys bound in Off

AI-assisted (Claude Code, Opus 5.5). Lane FIX-IS-PLAYER-PAWN-AND-UNBOUND-KEYS (a code chore of the hub's, from #776's
list), from main 0847259. Each rule is an existing one on main; this lane only binds it where its key was unbound. It
did not read the executable, and no session was run.

- **What was done:**
  - **`Pawn.IsPlayerPawn()`** (`ow-world` `render_hooks`): the native answers by hook 2's pawn test, the rule of
    NATIVE_BODYCOMP_RENDER_HOOKS.md section 6 that `owner_no_see` already uses. A pass is true, a decided failure is
    false, and an undecided test is its `NotYet`. A receiver the store has not loaded is a `NotRead`. Off only,
    keep-existing, registered with the world's Off natives;
  - **`Actor.AttachComponent` in the combat world** (`ow-cli` `combat::world`): `ow-inventory`'s rule
    (NATIVE_ATTACH_MESH_TO_PICKUP.md part 1), bound through `bind_engine_attach` as the cast's worlds bind it. The
    world's empty scene layer was already installed. Off only;
  - **`MaterialInstanceConstant.SetParent(object)` and `.SetVectorParameterValue(name,linearcolor)`** (`ow-inventory`
    `material_instances`): `MaterialInstanceConstant`'s own declarations are separate native keys with
    `MaterialInstance`'s exec. They are bound to the same two rules (NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md section 3).
    `MaterialInstanceTimeVarying`'s keys stay unbound. Off only.
- **Checked first:** no placeholder or duplicate binding of the keys in the tree. The all-packs test
  (`pack_wiring_tests`) now declares `Actor.AttachComponent` and still finds no shadow beyond the three allowed. The
  installed shadow census finds 0 shadows in all five slice worlds.
- **Reference:** unchanged. None of the keys binds there (a test each).
- **Tests** (invented data):
  - `is_player_pawn_answers_by_the_pawn_test`;
  - `the_material_instance_constant_keys_answer_by_the_material_instance_rules`;
  - `the_combat_world_binds_the_three_once_unbound_keys_in_off_only`.
- **Gates:**
  - `cargo test --workspace -j 8`: 3,863 passed, 0 failed, 59 ignored;
  - `crate_edges_test.py`: OK;
  - the installed Off strict `--slice-gate` (release build, under the machine lock): 6 ok / 14, X = 51.2%, on base
    0847259 and on this change.
  - **Rows that moved:** links 1, 3, 4, 5, 13 and 14 were an error at the unbound `Pawn.IsPlayerPawn()` on base. They
    are now `NotYet { NotRead }` in `maya:8` at `InitializeBehaviorProviders`' "a definition the store has not loaded",
    the refusal #776's probe traced to `RunStance0` (the text does not name the object). Every other row is unchanged.
- **Verified:** the tests on invented data and the installed rows above. **Not verified:** everything in the running
  game.
- **Sensitive files:** none.
