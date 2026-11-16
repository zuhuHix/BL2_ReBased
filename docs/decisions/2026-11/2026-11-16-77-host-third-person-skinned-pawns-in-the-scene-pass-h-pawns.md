## 2026-11-16: Host third-person skinned pawns in the scene pass (H-PAWNS part 1)

AI-assisted (Claude Code). Lane H-PAWNS, part 1, from the host lanes of the slice run plan (PR #369,
`SLICE_RUN_PLAN.md` section 4). Number 77 was assigned by the hub. It builds on H6 stage 1 (decision 53), H7
(decision 56), H6-SKIN (decision 64), H8 (decision 58) and H9's light-mapped scene pass. There was no real-game
session. How the game places, poses and lights its pawns has not been seen in the running game, so everything below
is **UNVERIFIED** there.

- **What changed.**
  - New: `crates/ow-host/src/anim/pawns.rs` and `anim/pawns/tests.rs`, plus one `pub mod pawns;` line in
    `anim/mod.rs`.
  - `crates/ow-host/src/render.rs` (H2, H8 and H9's file, a small additive change): `SceneRenderer::encode_with`
    takes a callback that is called inside the scene pass, after the opaque and masked batches and before the
    blended ones. The scene's bindings are set again after it. `encode` calls it with nothing, so the scene draws as
    before.
  - `anim/gpu.rs`: `SkinnedMesh::set_view`, `draw_in` (a draw into a pass the caller owns) and `set_color`. `draw`
    keeps H6-SKIN's own pass for the arms.
  - `--pawn NAME DIR X Y Z YAW` for `--play`, repeatable, in `args.rs` (with parse tests) and `main.rs`. `Command`
    loses its `Eq` derive, since the pawn's location is a float; nothing compared commands with `Eq`.
  - `play.rs`: `PlayDrawer::with_pawns`, and one `draw_world` used by both the frame and the screenshot. It skins
    the pawns, draws the scene with the pawns inside its pass, then H6-SKIN's arms, then H4-WEAPON's overlay.
  - No dependency.
- **What a pawn is drawn from.**
  - **From the definition store:** the pawn archetype's `Mesh` component, and from it `SkeletalMesh`, `AnimSets`,
    `AnimTreeTemplate`, and the component's `Translation`, `Rotation`, `Scale` and `Scale3D`, read with the class
    defaults applied.
  - **From UModel's ActorX exports under `local/`** (as stage 1): the mesh and the clips. The clips' `RateScale` and
    notifies come from the store, through H7's library.
  - **Kinds:**
    - `bullymong` is `GD_PrimalBeastAdult.Character.Pawn_PrimalBeastAdult`, the adult bullymong archetype whose
      balance the Phaselock work found.
    - `maya` is `GD_Siren_Streaming.Pawn_Siren`.
  - The flat colours are host choices; H8 owns materials.
- **Placement (UNVERIFIED):** the component's transform, then the actor's (`frame::component_to_world`, H3's
  composition, confirmed against the running engine only for static components). The location and yaw come from
  `--pawn` for now; H-WORLD will feed a spawned actor's through the same `Pawn` and `PawnLayer`.
- **Pose:** H7's driver with its default inputs plus physics set to walking, as H7's report drives Maya's body. The
  clock is H6-SKIN's step-sized chunks of the host's frame-time clock, shared by all pawns.
- **Drawing:**
  - H6-SKIN's compute skinning, in model space then placed.
  - Inside the scene pass: depth-tested and depth-writing against the scene's opaque and masked batches, under its
    blended ones. These are world objects, not an overlay.
  - Lit by the scene's `Lighting` with the scene shader's rule. There is no light map: UE3 lights dynamic actors
    with light environments, which are not read.
- **AnimTree nodes the bullymong uses that H7 does not evaluate** (`GD_PrimalBeastShared.Character.AnimTree_PrimalBeastShared`,
  33 nodes: 28 evaluated, 5 `NotYet`).
  - The `NotYet` classes: `Engine.AnimNodeMirror` 1, `Engine.AnimNodeRandom` 1,
    `WillowGame.WillowAnimNodeBlendByStance` 2, `WillowGame.WillowStaggerAnimNodeBlend` 1.
  - Each is passed to its first child (`Unsupported::PassFirstChild`) and counted. On the idle path four `NotYet`
    evaluations pass a frame.
  - **The visible result:** both stance nodes' first child is their "injured" stance, so the bullymong stands in
    `Idle_Injured` where the stored active child ("Other") is `Idle_var1`. That is the labelled stand-in's effect,
    not a reading of the game.
  - Maya's body tree has H7's known seven: `WillowAnimNodeFeatherBlend` 5, `WillowAnimNodeAimOffset` 1,
    `WillowAnimNode_ClimbLadder` 1. It plays `Idle_var1`; `ADD_Idle` is not in her body's only set, `Base_Siren`, so
    it adds nothing.
- **Proof.**
  - **Synthetic** (`anim::pawns::tests`):
    - kinds by name;
    - the yaw rounded to rotator units (90 degrees to 16,384; -90 to 49,152; non-finite refused);
    - the `NotYet` census;
    - the `--pawn` parse (`args` tests).
  - **Drawn** (needs an adapter, `#[ignore]`d as `render::tests` are): `pawns_are_depth_tested_inside_the_scene_pass`.
    A skinned quad drawn through `encode_with` with a grey wall at x = 200 shows its own colour at the centre when
    placed at x = 100, and the wall's when placed at x = 300.
  - **Installed game** (`anim::pawns::tests::installed_pawns_load_and_gpu_matches_cpu`;
    `local/p2/h-pawns/installed.txt`):
    - Both pawns load through the store.
    - The bullymong: `Char_PrimalBeast.Mesh.Skel_PrimalBeast`, 70 bones, 5,620 vertices, 8,304 triangles. Its
      component is `SkeletalMeshComponent_978`, translated -96.25 in Z. Set `Anim_PrimalBeast.Unarmed`.
    - Maya's body: `Char_Siren.Mesh.Skel_SirenBody`, 27 bones, translated -86 in Z.
    - The bullymong skinned on the GPU along its idle (six poses 0.5 s apart) equals the CPU reference: worst
      3.05e-5 uu, against a tolerance of 1e-4 uu.
  - **UModel** (build 1590, under the machine lock): `-export -psk Ice_Dynamic Skel_PrimalBeast SkeletalMesh`
    (6.9 s) and `Ice_Dynamic Unarmed AnimSet` (1.1 s, a 14.7 MB `.psa`), under `local/p2/h-pawns/umodel/`.
  - **Window:** not run. The desktop had 4.1 GB of free commit memory, and a Sanctuary play run with materials and
    light maps could have starved the other lanes' processes. The hub asked for a screenshot only if memory allows.
- **Open items.**
  - A window screenshot of the pawns in Sanctuary.
  - The stance, mirror, random and stagger nodes (the bullymong) and H7's open Willow classes (Maya).
  - The pawns' materials (H8).
  - Light environments for dynamic actors.
  - Real placements and inputs from H-WORLD.
  - Drop actors and enemy kinds beyond these two.
  - Maya's weapon sets (her `ADD_` clips).
