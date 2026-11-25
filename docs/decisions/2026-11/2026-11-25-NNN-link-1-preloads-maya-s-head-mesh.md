## 2026-11-25: Link 1's set-up walks Maya's body composition templates and their meshes

AI-assisted (Claude Code, Opus 5.5). Lane LINK1-HEAD-MESH, stacked on fix/rust-customhead-parent-link (#721, decision
2026-11-25-56; at c845ff85 after its review) on main 5d20247f. No session, no native analysis: the evidence is the
installed packages and the existing records. A harness set-up step, not a native rule. Every rule here is UNVERIFIED
in the running game.

- **The stop:** with the `Parent` link answered (#721), link 1 stopped in `maya:8` at "the skeletal mesh data layer: the
  unloaded skeletal mesh CubeForCode". This is Maya's head mesh, `CD_Heads_Assassin_MainGame.CubeForCode`, her
  `CustomHead` template's `SkeletalMesh` (NATIVE_INITIALIZE_BODY_COMPOSITION.md section 8).
- **What the packages hold** (read with `ow-package --exports` and `--payload-file`, output under ignored
  `local/analysis/headmesh/`):
  - there is no `CD_Heads_Assassin_MainGame` package file. The cube is a cooked-in export of four class streaming
    package files: `GD_Assassin_Streaming_SF`, `GD_Siren_Streaming_SF` (export 4214), `GD_Mercenary_Streaming_SF` and
    `GD_Soldier_Streaming_SF`, each 1,820 bytes. The Assassin and Siren copies differ in 70 bytes, not decoded
    (plausibly package-local name and object indices; **UNVERIFIED**);
  - Maya's file `GD_Siren_Streaming_SF` also holds her pawn archetype, its body class's 13 attachment templates, and her
    body mesh `Char_Siren.Mesh.Skel_SirenBody`;
  - **the store's handle for the cube is the `GD_Assassin_Streaming_SF` copy.** The store binds one object per path
    (`IdentityPolicy::FirstInScanOrder`), so the template's reference resolves to that file's export, not to Maya's.
- **What was decided, in two steps:**
  1. The first version preloaded the cube by its path, after the body mesh. Through the store's identity, that loaded
     the Assassin copy. So "an export of her pawn's package" did not describe what was loaded. That version
     (06a24bb3) moved link 1 to attachment 2's unloaded template, `GearLikenessMeshComponent_2780`.
  2. The hub's ruling replaces it with one set-up walk, the pattern of link 8's material walk (decision 2026-11-25-47),
     `ow-cli` `combat::preload_body_composition`, run after the archetype's same-package references:
     - from the archetype's `BodyClass`: the component templates its body composition makes (each attachment's
       `Data.ComponentData.Component`, in list order), and each template's `SkeletalMesh`;
     - an object is loaded through the store only if its heap origin is the archetype's own package file. One already
       resident is visited with no load;
     - anything else stops its branch with a logged line, and is left for the native to refuse;
     - each object is visited once, up to a cap of 64 objects (logged if hit). Each load is logged before and after,
       as an UNVERIFIED stand-in for the streaming load; a failed load is the step's error, kept;
     - no native refusal changes. The explicit cube preload is gone; `preload_mesh` keeps the name it logs.
- **Tests:**
  - `combat/tests.rs`: two templates and their meshes in the home package load, a template in another package is
    skipped with a line naming it, and a walk at the cap visits nothing more;
  - the `maya` step's harness test: the walk's skip line for a toy archetype with no `BodyClass`.
- **The skeleton source:** on this machine the `.psk` root is missing, so skeletons come from the mesh's own package
  export (`ow_assets::skeletal_lods`, #693's reader). With the first version, the Assassin copy's skeleton read, and
  link 1 got past the `Parent` link.
- **The slice gate** (installed Off strict, under the machine lock, run the same way):
  - first version, base 69a8063c and 3da6d356: 5 ok / 9 NotYet / 0 errors on both. Link 1 moved from the cube to
    `GearLikenessMeshComponent_2780` (attachment 2's socket attach refuses an unloaded new-component template);
  - the walk, base fc1a46cb (the first version rebased onto c845ff85) and 546b0fd1: 5 ok / 9 NotYet / 0 errors on
    both. The walk loads 11 objects: the 10 unloaded attachment templates and the body mesh. It visits 16 in all, and it
    stops at the cube with "CD_Heads_Assassin_MainGame.CubeForCode is not an export of GD_Siren_Streaming_SF
    (GD_Assassin_Streaming_SF); its branch stops";
  - so link 1 moves in `maya:8` from `GearLikenessMeshComponent_2780` back to "the skeletal mesh data layer: the
    unloaded skeletal mesh CubeForCode". Links 3, 4, 5, 13 and 14 report the same. Every other row is unchanged.
- **Open, for the hub:** the cube is a load refusal that the walk's own rule keeps. In the game, Maya's file holds its
  own copy at the same path. Whether the store's path identity should count that copy as hers is not decided here.
  Nor is whether the Assassin copy's data equals Siren's (70 differing bytes, not decoded).
- **Sensitive files:** none. `crates/ow-package` and `Cargo.toml` are unchanged.
- **Verified:** nothing in the running game.
