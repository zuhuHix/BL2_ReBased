## 2026-11-25: Link 1's set-up walks Maya's body composition, and a file's own copy of a path

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
- **What was decided, in three steps:**
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
  3. **The store rule** (the hub's ruling on step 2's stop, a third commit): in the game only Maya's streaming package
     is loaded, and her head template's `SkeletalMesh` is an export index of that same file, so the object at that path
     in her game is the Siren copy. `ow-defs` gains `DefStore::load_export_of_file(obj, file)`:
     - when `file` holds an export at `obj`'s path and `obj` is still an unloaded handle, the handle is re-pointed to
       `file`'s export and loaded. It stays the one object for the path;
     - when `obj` is already loaded from another file's copy, it is a typed `NotYet { NotRead }` naming both files
       ("... is resident from A, and B holds its own copy (an identity conflict)"), with nothing changed;
     - a file with no export at the path does nothing, and a handle whose load failed returns that error again;
     - every other lookup (`find`, `resolve_ref`, `ensure_loaded`, `load`) keeps the identity policy's holder. The
       policy itself is unchanged;
     - **UNVERIFIED** that the game resolves a path so; the reason is the one above.

     The walk calls it for an object the store holds from another file, logging the attempt and its outcome; a file
     with no copy keeps the old "its branch stops" line.
- **Tests:**
  - `ow-defs` `store/tests.rs`, on the existing two-holders fixture (`GD_Test.Part_A` in `GD_Test.upk` and
    `Startup.upk`): a handle reached from `Startup.upk` loads Startup's copy and stays the one object for the path; a
    second call is a no-op; a call from `GD_Test.upk` with Startup's copy resident is the conflict `NotRead`; a file
    without the path does nothing; and an ordinary load keeps the policy's holder, after which a call from Startup is
    the conflict;
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
  - the store rule, base 0b426f30 (the walk; its reading is the walk's above, same code) and 25cd9b07: the walk loads
    12 objects, Siren's own `CubeForCode` among them ("loaded GD_Siren_Streaming_SF's own copy"). Link 1 then gets
    through the body composition and stops in `maya:8` at **an error**: "unbound native
    BodyClassDefinition.GetWeaponHoldDef(name,object,bool,bool)". Links 3, 4, 5, 13 and 14 report the same. So the
    gate reads 5 ok / 3 NotYet / 6 errors, against 5 / 9 / 0. The six are unbound-native errors, the gate's grading of
    a native no lane has bound: a non-load rule, so this lane stops there. Every other row is unchanged. The native has
    no note and no `NATIVE_INDEX.md` row; its script caller was not looked for.
- **Sensitive files:** none. `crates/ow-package` and `Cargo.toml` are unchanged. `crates/ow-defs` (not on the list)
  gains one public method and its result type; no existing behaviour changes.
- **Verified:** nothing in the running game.
