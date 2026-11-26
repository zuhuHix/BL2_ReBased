## 2026-11-26: InitializeRBPhysics and the player pawn's last natives in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-LINK1-RBPHYSICS, on main 59f0d2ea. It implements
docs/verification/NATIVE_LINK1_RBPHYSICS_AND_AFTER.md section 2 (lane F's note, #738 with its r1 fix, on main). Clean
room: this lane did not read the executable. No real-game session was run. **Every rule is UNVERIFIED in the running
game.** Reference keeps the stub policy.

**Where.** `ow-world` `world::player_pawn_tail` (2.A, 2.B, 2.D, 2.E, 2.F) and `world::weapon_hold` (2.C), bound
keep-existing by key in the world pack's Off block. The keys were read from the installed packages' declarations:
- `WillowPawn.InitializeRBPhysics()` and `WillowPawn.InitializeBehaviorProviders()` (not `WillowAIPawn`'s);
- `BodyClassDefinition.GetFirstPersonWeaponHoldDef(name,object,bool,bool)`;
- `WillowPlayerPawn.GetLinearColor(color)`;
- `MeshComponent.GetInstancedMaterialInstanceConstant(int)`;
- `WillowPlayerPawn.OnNativeApplyBodyClassStartingValuesHook()`.

**2.A `InitializeRBPhysics`**, write-free and load-free. With `bDeleteMe`: nothing. Otherwise the `Components` walk,
after the `PHYS_RigidBody` filter: None skipped; a lazy handle `NotYet { NotRead }`. An attached entry (the attach
record under the pawn, or `bAttached`; on a template receiver `bAttached` alone) is `NotYet { LayerAbsent }`,
"WillowPawn.InitializeRBPhysics: a component's rigid body", layer "a physics layer (rigid bodies)". A live entry
neither recorded nor `bAttached` follows refusal L's convention (its text and layer). The port applies that to every
such entry, colliding or not, the conservative side; the note's test names a colliding one. Then
`SetPushesRigidBodies`: clear is nothing, the same-value write skipped, so the native never writes and no
partial-writes prefix can arise; set, the `CollisionComponent` by the same test.

**2.B** `NotYet { NotRead }`, "WillowPawn.InitializeBehaviorProviders (the provider list)". **2.C** is
`GetWeaponHoldDef`'s rule on `FirstPersonWeaponHoldDefs` and `DefaultFirstPersonWeaponHoldDef`, with the same parent:
the lookup now takes its two fields and its refusal texts as a parameter, and the third-person behaviour is unchanged.
**2.D** is `byte as f32 * (1.0f32 / 255.0)` per channel. **2.E** is `NotYet { NotRead }`, "MeshComponent.GetMaterial":
both branches need routines not read. **2.F** writes `ImmuneToFallingDamage` := true.

**#736's two LOW findings.** `inventory_actors.rs`'s stand-in test is now `ow-attr`'s `slotless_refusal` shape (an
`Origin::Resource` object with no slots), and `manager.rs`'s long doc line is re-wrapped.

**Tests.** `player_pawn_tail_tests.rs` (8): 2.A's nine cases, 2.B and 2.E, 2.D with the byte-3 bits (`0x3C40C0C2`, the
divide's `0x3C40C0C1`), 2.F, and Reference. Each refusal leaves #518's counters, `class_code_loads` and the name table
unchanged, and `InitializeRBPhysics` is asserted never to write. `weapon_hold_tests.rs`: #728's cases on the
first-person fields (one test; the fixture now takes either native).

**The slice gate** (installed Off strict, release build, under the machine lock): before and after the same, 6 ok /
8 NotYet / 0 errors, `links ok 6/14`, `step share X = 6.583/14 = 47.0%`. Link 1 is still at the aim-offset rebuild's
`NotRead`, so it does not reach these natives yet (the expected result).

**Gates.** `cargo test --workspace -j 8`: 3,714 passed / 0 failed / 58 ignored. `crate_edges_test.py`: OK.
