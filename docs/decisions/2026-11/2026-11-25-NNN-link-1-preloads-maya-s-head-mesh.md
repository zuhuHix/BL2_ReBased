## 2026-11-25: Link 1's set-up preloads Maya's head mesh

AI-assisted (Claude Code, Opus 5.5). Lane LINK1-HEAD-MESH, stacked on fix/rust-customhead-parent-link 69a8063c (#721,
decision 2026-11-25-56) on main 5d20247f. No session, no native analysis: the evidence is the installed packages and
the existing records. Every rule here is UNVERIFIED in the running game.

- **The stop:** with the `Parent` link answered (#721), link 1 stopped in `maya:8` at "the skeletal mesh data layer: the
  unloaded skeletal mesh CubeForCode". This is Maya's head mesh, `CD_Heads_Assassin_MainGame.CubeForCode`, her
  `CustomHead` template's `SkeletalMesh` (NATIVE_INITIALIZE_BODY_COMPOSITION.md section 8).
- **Where the game has it** (from the packages, read with `ow-package --exports`, output under ignored
  `local/analysis/headmesh/`):
  - there is no `CD_Heads_Assassin_MainGame` package file. The mesh is a cooked-in export of Maya's character streaming
    package file, `GD_Siren_Streaming_SF`, under a package export of that name;
  - the same file holds her pawn archetype `GD_Siren_Streaming.Pawn_Siren`, its `CustomHead` template, and her body
    mesh `Char_Siren.Mesh.Skel_SirenBody`. The class packages carry other copies of the cube (decision 2026-11-25-09);
  - so the mesh is resident when her pawn's package is. The combat run's set-up already preloads the body mesh and the
    archetype's same-package references as UNVERIFIED stand-ins for that streaming load (lanes SLICE-HARNESS-MAYA-MESH
    and SPAWN-COMPLETE). The head mesh stayed a lazy handle only because it is one level further away: the
    same-package preload reads the archetype's own object slots, not its templates' fields.
- **What was decided:** option (a). The `maya` step's set-up preloads exactly that mesh, after the body mesh and its
  sockets. It goes through the same `preload_mesh`, logged as "Maya's head mesh, cooked into her pawn's streaming
  package; UNVERIFIED stand-in for the streaming load", with its outcome. `preload_mesh` now takes the name it logs.
  Which exports the game's streaming load makes resident is still not read.
- **The skeleton source reads it:** on this machine the `.psk` root is missing, so the skeleton came from the mesh's
  own package export (`ow_assets::skeletal_lods`, #693's reader). Link 1 got past the `Parent` link with it.
- **Tests:** the harness test of the `maya` step's inputs lists the head preload (two more lines, logged and skipped in
  the toy packages); the mesh preload tests pass the logged name.
- **The slice gate** (installed Off strict, under the machine lock, base 69a8063c and this head 3da6d356, run the same
  way): 5 ok / 9 NotYet / 0 errors on both.
  - Link 1 moves in `maya:8` from the CubeForCode refusal to "InitializeBodyComposition: the mesh of the unloaded
    component template GD_Siren_Streaming.Pawn_Siren.GearLikenessMeshComponent_2780".
  - That is attachment 2 (identifier `None`, base 0, the body). Its socket matches on the body mesh, and the socket
    attach's gate (NATIVE_BODYCOMP_SOCKET_BONE_TABLES.md) refuses an unloaded new-component template.
  - The template is an export of the same package file. So are the other nine `BodyClass_Siren` attachment templates
    that the same-package preload does not reach (seven more `GearLikenessMeshComponent`s, two particle system
    components and a static mesh component). Extending the stand-in to them is a hub call. This lane does not.
  - Links 3, 4, 5, 13 and 14, behind link 1, report the same new stop. Every other row is unchanged.
- **Sensitive files:** none. `crates/ow-package` and `Cargo.toml` are unchanged.
- **Verified:** nothing in the running game.
