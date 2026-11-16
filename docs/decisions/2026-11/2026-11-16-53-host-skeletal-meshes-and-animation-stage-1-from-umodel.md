## 2026-11-16: Host skeletal meshes and animation, stage 1, from UModel ActorX exports (H6)

AI-assisted (Claude Code). Lane H6-ANIM, stage 1, host track (architecture section 8, "Wave 7 and the host track";
the maintainer approved the host track on 2026-11-16). Number 53 was reserved for this lane by the hub. There was no
real-game session: every rule here is **UNVERIFIED** in the running game unless a line says how it was checked, and
the checks below are structural or cross-tool checks, not observations of the game.

- **What changed.**
  - `crates/ow-assets/src/actorx.rs`, `skeletal.rs` and `animation.rs` (new), plus three `pub mod` lines in
    `crates/ow-assets/src/lib.rs`.
  - `crates/ow-host/src/anim/` (new: `mod.rs`, `math.rs`, `pose.rs`, `skin.rs`, `report.rs`, `tests.rs`), plus one
    `pub mod anim;` line in `crates/ow-host/src/lib.rs`.
  - No dependency, no new tool, no edit to `ow-package` or `ow-container`, no bounds check or reader offset changed,
    no CLI flag (the installed-game check is an ignored test; a flag in `args.rs`/`main.rs` is for stage 2 or the
    hub).
- **The source: UModel's ActorX exports, not our own decoder.** Our decoders do not read `SkeletalMesh` or
  `AnimSequence` from the 832/46 packages: their native layout has no public spec and none has been recovered
  (AGENTS.md rule 4). UModel, the first-choice extraction backend already in `THIRD_PARTY.md`, exports them from the
  player's own install, and the exports stay under ignored `local/p2/h6-anim/`. **Native decoding of both is an open
  item.**
- **Why ActorX (`.psk` / `.psa`) and not MD5 or glTF.**
  - ActorX is fixed-size little-endian records behind 32-byte chunk headers, and each header carries its own record
    size. Readers of a few hundred lines, with no crate, check every size and count against the bytes.
  - The `.psk` stores the vertex positions, so the bind-pose check compares the skinned result with stored data.
    MD5 stores only weighted offsets, from which positions are derived.
  - The `.psa` stores full quaternions. MD5 drops W, and UModel's MD5 anim writer leaves an untracked bone's values
    unset.
  - glTF needs a JSON parser (a crate, or one of our own), for no gain here.
- **What was read of UModel's source, and how.** The desktop's local UE Viewer checkout (outside the repository) at commit
  `a0bfb46` (MIT, the commit `THIRD_PARTY.md` lists) was read to learn the export format, with no code copied or
  translated. These files were read:
  - `Exporters/Psk.h`: chunk header and record layouts, version 20100422;
  - `Exporters/ExportPsk.cpp`: chunk order, the mirror, the pskx switch above 65,536 wedges, and that every frame of
    a sequence is written as a full key per track;
  - `Exporters/ExportMd5.cpp`: the MD5 writer's root-bone and mirror handling, used only to read the cross-check
    export;
  - `Unreal/UnrealMesh/UnAnim3.cpp`: the one line where the exported rate is set to
    `NumFrames / SequenceLength * RateScale`;
  - `Unreal/Mesh/SkeletalMesh.h`: the bone translation modes behind the `.config` file.
  In our own words:
  - the exporter writes a mirrored frame (Y negated, quaternions as `(x, -y, z, -w)`) and swaps each triangle's first
    two corners;
  - a sequence's bone table carries names only;
  - a `.config` file carries translation-retargeting modes when an AnimSet has them.
- **The export's frame to the game's (`ow_assets::actorx`).** Positions get Y negated back, quaternions
  `(x, y, z, w)` become `(x, -y, z, -w)`, and the triangle corners are swapped back, so `ow-assets` returns the
  game's frame: uu, X forward, Y right, Z up. The engine frame is the game's (`frame.rs`, H3), so `ow-host` converts
  nothing further and model matrices reach the renderer through `Affine::to_engine`. This is **not** the retired UE5
  host's mapping (MD5 to UE5 space with a Y mirror, then a Camera-bone re-anchoring); it was derived again:
  - **Handedness check, from the game's bone names** (`anim::report`). With Z up and forward taken from the mesh
    (arms: `Camera` to the middle of the hands; body: feet to toes), the `R_` bone of each pair lies on the right
    (`up x forward`) and the `L_` bone on the left, for both meshes. A Y-mirrored reading would swap them. This rests
    on the names meaning what they say; no frame of the running game was compared. The report fails when the
    check does not hold (review r1 of PR #350).
- **The bone convention.** ActorX stores every non-root bone with its inverse rotation. The reader conjugates each
  non-root bone's quaternion after the frame conversion, so a `BoneTransform` maps the bone's frame to its parent's
  as `p = q p' q* + t` (Hamilton). How this was checked:
  - **Bind pose, cross-tool.** Composing both installed meshes' `.psk` bones this way gives the absolute joints of
    UModel's own MD5 mesh export of the same meshes to 3.3e-5 uu. The quaternions agree to a dot product of 1 - 5.4e-7
    (the MD5 writer stores the conjugate, for every joint alike). The stored quaternions used as they are miss by
    more than 100 uu.
  - **Clips, cross-tool.** The `.psa` keys bound with the same rule and sampled by the Rust sampler give the bone
    positions of UModel's MD5 anim export of the same clips, composed by the retired pipeline's MD5 reader
    (`tools/prepare_character_anims.py` `frames`). The worst difference is 1.04e-3 uu on frames 0 and 1 of nine clips
    (MD5 text keeps six decimals).
  - Both checks compare two UModel outputs, not UModel with the game.
  - **Which track is the root.** The rule is applied per mesh bone: the track bound to the mesh's root (bone 0) is
    kept as stored, every other track is conjugated. `AnimSet::bind` refuses a set whose first track is not the
    mesh's root (`ActorXError::RootTrack`), so a set that put another bone first cannot be bound under a rule meant
    for the root. All six installed sets (the 159 clips) have `Root` as track 0, and `Root` is bone 0 of both meshes.
- **Checks in the readers.** Every record size is the one the format has. Every count meets its chunk's bytes, and
  products use checked arithmetic. Every index is in range, every number finite, and every quaternion of unit length
  (squared length 0.99..=1.01). Bones are in parent order with one root (ActorX gives the root parent 0); this is the
  cycle guard, and composition walks bones in index order with no recursion, its depth bounded by the bone count.
  Bone names are unique (checked through an ordered set; track names are matched through an ordered map, so no
  lookup is quadratic in the bone count). Each point has one to four influences on distinct bones (one bone named
  twice in a point is refused; the installed meshes have none), with weights in 0..=1 summing to 1 within 1e-3. Each
  sequence's first frame is the sum of the frames before it, and the key table holds exactly frames x tracks.
  Nothing is clamped or renormalised.
- **Typed `NotYet`.** Unsupported inputs return `NotYet`:
  - a `.psa` with scale keys;
  - a LOD past the first.
  Unread chunks (`EXTRAUVS*`, vertex colours, morph targets) are listed in `ignored_chunks`. Normals are not in a
  `.psk` and are not computed.
- **Sampling and its end rules (`ow_host::anim::pose`), UNVERIFIED.** Keys sit at `k / frame_rate`.
  - `Wrap::Loop` takes the frame position modulo the frame count, and the interval after the last key runs back to
    key 0, so a clip lasts `frames / rate`. The installed tags agree with that length: for example, 91 frames with
    `SequenceLength` 3.0333 s at 30 fps.
  - `Wrap::Clamp` holds the first and last keys.
  - Translation is lerped, rotation nlerped on the shorter arc.
  - Not read from native code or seen in the game: the game's own end rule (`bLooping`, `bNoLoopingInterpolation`),
    its key interpolation, and its compressed keys (the export resamples them to one key per frame).
- **CPU skinning (`ow_host::anim::skin`).** Linear blend with the four stored weights: `sum w_i p S_i`, with
  `S_i = inverse_bind_i then pose_i`. It is the reference for the stage 2 GPU path. That the game skins this way is
  standard UE3, but it is UNVERIFIED here.
- **`RateScale`.** The export folds it into the frame rate. The report's rates match the package tags
  `NumFrames / SequenceLength * RateScale` to 6.1e-6 for all 159 clips, and its frame counts match `NumFrames`
  exactly. The tags were read with our own `ow-package --properties`.
  - Ten clips have `RateScale` other than 1: `ADD_Fire_Recoil` in two sets, `ADD_Fire_Revolver_Recoil`,
    `Sight_Idle` in two sets, `Sight_Jump_Idle`, `Run_F`/`Run_L`/`Run_R` (unarmed) and `Idle_Incap`.
  - Whether the AnimTree applies `RateScale` again on top is AnimTree work.
- **Additive clips.** The pistol set's `ADD_Fire_Recoil` and `ADD_Fire_Revolver_Recoil` and the rifle set's
  `ADD_Fire_Recoil` have `bIsAdditive` true. They load and bind, but they are deltas for the AnimTree's additive
  nodes, so the report does not pose them as absolute clips. The first-person fire clip pose-checked is
  `Fire_Fan_Recoil`, which is not additive. All six AnimSets have `bAnimRotationOnly` false and no
  `UseTranslationBoneNames`, so every tracked bone takes its translation from the clip; the empty `.config` files
  agree.
- **External-tool record (AGENTS.md "External tools and extraction").** UModel build 1590, SHA-256 `13502E5A...4A0`
  (`THIRD_PARTY.md`), run from that checkout's `umodel.exe` under the machine lock.
  - Commands (each with `-path=<CookedPCConsole> -game=border -nooverwrite`):
    - `-list GD_Siren_Streaming_SF` (0.63 s);
    - `-export -psk ... GD_Siren_Streaming_SF <Hands_Siren|Skel_SirenBody> SkeletalMesh` (0.22 s, 0.26 s);
    - `-export -psk ... GD_Siren_Streaming_SF <set> AnimSet` for `1st_Person_Pistol`, `1st_Person_AssaultRifle`,
      `1st_Person_Generic`, `1st_Person_Unarmed`, `Siren_1st`, `Base_Siren` (0.08-0.11 s each);
    - for the cross-check, `-export -md5` of both meshes (0.20 s, 0.22 s) and of `1st_Person_Pistol`, `Siren_1st`,
      `Base_Siren` (0.34, 0.10, 0.45 s).
  - One failure: `-export ... AnimSet` with the class and no object name exits 1 ("not found"); naming each set
    works.
  - Output: 2 `.psk` and 6 `.psa` (plus material `.props.txt`, `.mat` and `.tga` side files and six empty `.config`
    files), 35 files, 32.7 MB. MD5: 2 meshes and 107 anims, 122 files, 36.1 MB. All under
    `local/p2/h6-anim/umodel/`.
  - Warnings: four texture imports of the materials not found (`Common_Textures` stubs, `FX_CharacterAbilities`);
    no mesh or animation warning.
- **Installed-game results** (`local/p2/h6-anim/report.json`; the ignored test `anim::tests::installed_report`).
  - Arms, `Hands_Siren`:
    - 47 bones, 2,204 vertices (1,864 points), 3,692 triangles, 1 section, 1 material (`Mati_Siren_Hands`);
    - at the bind pose the skinned vertices equal the stored ones to 1.5e-5 uu;
    - half-extent (12.67, 46.62, 39.46) uu.
  - Body, `Skel_SirenBody`:
    - 27 bones, 5,368 vertices (3,801 points), 6,975 triangles, 1 section, 1 material (`Mati_Siren_Body`);
    - bind skin error 1.5e-5 uu;
    - half-extent (14.48, 53.90, 74.25) uu; it stands on Z = 0, about 148 uu tall, facing +X.
  - Every sequence loads and binds: arms 44 + 38 + 2 + 12 + 5 = 101, body 58.
  - Requested first-person clips: `Idle` 91 frames, 30 fps, 3.033 s; `Reload_Jakobs` 61, 30, 2.033 s; fire:
    `Fire_Fan_Recoil` 45, 30, 1.5 s (and the additive `ADD_Fire_Recoil` 17 frames at 29.14 fps, 0.583 s); Phaselock
    cast: `Phase_Lock_Lift` 26, 30, 0.867 s and `Phase_Lock_Fail` 16, 30, 0.533 s.
  - Body clips posed: `Idle_var1`, `Walk_F`, `Idle_Inventory`, `AA_PhaseLock`.
  - Every pose at the start, frame 1, the middle and the loop end is finite, with the farthest bone 133-181 uu from
    the root.
- **Against what the UE5 host imported** (`local/character/` logs of the main checkout).
  - Same bone counts (47, 27) and the same frame counts for every clip it imported: arms Idle 91, Run_F 25,
    Sprint 19, Jump_Start 7, Jump_Idle 61, Jump_End 10, Draw 14, ADD_Fire_Recoil 17, Phase_Lock_Lift 26,
    Phase_Lock_Fail 16; body Idle_var1 61, Idle_Inventory 271.
  - Half-extents within 0.05 uu of its import log (12.7, 46.6, 39.5 and 14.5, 53.9, 74.3), which prints one decimal.
  - The differences, each explained:
    - its space was UE5's, the game's mirrored in Y; here the frame is the game's own;
    - it re-anchored the arms on the `Camera` bone every frame; here bones stay in the mesh's model space, and where
      the view attaches is for the next lane;
    - it imported `ADD_Fire_Recoil` at 30 fps where the export says 29.14, because it rounded the rate (its log says
      so);
    - it imported `ADD_Fire_Recoil` as a normal clip; here additive clips are not posed as absolute clips.
- **The AnimTree, for the next lane (not ported).** Counted with our own reader in `GD_Siren_Streaming_SF`:
  - `Pawn_Siren`'s body component names `GD_PlayerShared.Character.AnimTree_Player` (class `WillowAnimTree`) with
    the AnimSet `Base_Siren`. Its 70 nodes and controls:
    - `AnimNodeSequence` 28;
    - `AnimNodeAdditiveBlending` 5, `AnimNodeScaleRateBySpeed` 5, `WillowAnimNodeFeatherBlend` 5;
    - `AnimNodeBlendBySpeed` 4, `AnimNodeBlendDirectional` 4;
    - `AnimNodeSpecialMoveBlend` 3, `WillowAnimNodeSequenceAdditiveBlend` 3;
    - `WillowSkelControlHandPlacement` 2, `WillowSkelControl_FootPlacement` 2;
    - 1 each: `AnimNodeBlendByPhysics`, `AnimNodeBlendByPosture`, `AnimNodeBlendPerBone`, `SkelControlLeftHandGripWeapon`,
      `WillowAnimNodeAimOffset`, `WillowAnimNodeBlendInjured`, `WillowAnimNodeBlendThirdPersonMenu`,
      `WillowAnimNode_ClimbLadder`, `WillowAnimNode_Condition`.
  - `BodyClass_Siren`'s first-person hands component names `GD_PlayerShared.Character.AnimTree_PlayerHands` (class
    `AnimTree`) with the AnimSets `1st_Person_Unarmed`, `Siren_1st`, `1st_Person_Generic`; the weapon sets are not
    on the component, and how they are added at run time is not read. Its 27 nodes:
    - `AnimNodeSequence` 14;
    - `AnimNodeBlendBySpeed` 2, `AnimNodeSpecialMoveBlend` 2, `WillowAnimNode_Falling` 2;
    - 1 each: `AnimNodeBlendByPosture`, `WillowAnimNodeBlendDirectional`, `WillowAnimNodeSequence`,
      `WillowAnimNodeSlot`, `WillowAnimNode_AddCameraBone`, `WillowAnimNode_AimState`, `WillowAnimNode_WeaponRecoil`.
- **Verified vs UNVERIFIED.**
  - Checked:
    - the readers' structural checks on the installed exports;
    - bind skin error 1.5e-5 uu;
    - the bone and clip convention against UModel's MD5 output (above);
    - frame counts and rates against the package tags;
    - the handedness check.
  - UNVERIFIED in the running game:
    - that UModel decodes the game's meshes and compressed tracks faithfully;
    - the end rules, interpolation and skinning;
    - which clips the game plays for idle, reload, fire and Phaselock.
- **Open items.**
  - Native `SkeletalMesh` and `AnimSequence` decoding from the packages, so UModel becomes a cross-check rather than
    the source.
  - Normals and tangents (not in `.psk`); LODs past 0; the second UV set (`EXTRAUVS0`).
  - The AnimTree (counts above), additive clips, `RateScale` as a separate play rate, `bNoLoopingInterpolation`,
    root motion and notifies.
  - Where the first-person view attaches (`WillowAnimNode_AddCameraBone`, the `Camera` bone): not read.
  - A `--anim-report` flag (or similar) in `ow-host`'s `args.rs`/`main.rs`, which this lane did not own.
  - A GATES entry for the anim checks (proposed in the lane handoff).
  - Stage 2: draw the skinned meshes in H1's window once H2 stage 2 lands, testing the GPU path against
    `anim::skin`.
