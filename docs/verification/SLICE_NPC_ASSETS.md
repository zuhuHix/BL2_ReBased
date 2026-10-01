# Slice NPC and weapon assets: Marcus, target dummy, stock Maliwan pistol

AI-assisted (Claude), 2026-10-01. Extraction, import and visual-check record for the pieces listed in the
"Identity census for the pieces still to bind" section of `SANCTUARY_RPG_MISSION.md`. Generated exports,
logs, manifests and screenshots stay under ignored `local/`; imported assets live under the ignored
`host/ue5/OpenWillow/Content/`. Nothing game-derived is in Git.

What this record claims and does not claim:

- **Extraction** means UModel build 1590 wrote files for an object our reader identified. **Import** means
  UE 5.8 created an asset from those files and a fresh editor session loaded it back. **Visual check** means
  one still frame per preview level was looked at by eye. These are reported separately below.
- Nothing here is original-game parity. Materials are "textures bound by UModel's guess" in a minimal UE
  material, not verified material graphs. The MD5-to-UE animation mapping is UNVERIFIED beyond structural checks.
- Our reader decodes object identity and properties; it does not decode `SkeletalMesh`, `AnimSet` or
  `AnimSequence` payloads. For those the cross-check is **identity only** plus UModel-internal consistency.

## Summary

| Group | Extracted (UModel) | Imported (UE 5.8) | Visual check |
| --- | --- | --- | --- |
| Marcus | mesh (glTF + MD5), 2 MICs, 4 textures, 2 AnimSets (67 + 17 clips); 6/6 jobs ok | skeletal mesh (112 bones, 2 slots), 4 textures, 2 MIs, 5 AnimSequences | 2 stills (idle, walk) of the preview levels; character recognisable, textured, walk differs from idle |
| Target dummy | mesh (glTF + MD5), 1 MIC override + 4 textures, 1 AnimSet (21 clips); 4/4 jobs ok | skeletal mesh (50 bones, 1 slot), 2 textures, 1 MI, 4 AnimSequences | 1 still (idle); textured Hyperion worker with extra mechanical arms |
| Maliwan pistol | gestalt mesh (glTF), 1 MIC + 3 textures; 2/2 jobs ok | 2 skeletal meshes (rolled sample, 33-section candidates), 3 textures, 1 MI | 1 still (rolled sample); shape reads as a revolver, texture colours are the packed composite, not paint |

UE module build (`OpenWillowEditor Win64 Development`): **Succeeded**, 33 actions, 196.5 s, using the existing
`build/Release/*.lib` (log `local/slice/module_build.log`). No module source was edited.

## Tools added by this pass

- `tools/slice_npc_assets.py` host side: `identity`, `extract`, `crosscheck`, `anims`, `pistol`, `editor-job`, `manifest`.
- `tools/slice_npc_editor.py` UE editor Python: modes `npcs`, `anims`, `pistol`, `preview`.
- `tools/seed_slice_npc_assets.ps1` runner: takes `local/ue_run.lock` for every editor launch, refuses to run while
  an UnrealEditor process exists, kills only its own process. Paths come from `-UModel`/`$env:OPENWILLOW_UMODEL`,
  `-Game`/`$env:OPENWILLOW_BL2`, `-Engine`/`$env:OPENWILLOW_UE`; no machine path is stored in a tracked file.

Existing scripts were **not** run or modified: `host/ue5/import_character.py` deletes `/Game/OpenWillow/Characters/Maya`
and `host/ue5/import_weapon_items.py` deletes `/Game/OpenWillow/Weapons/Items`, and `prepare_character_anims.py` requires
the clip and mesh to have identical bone sets. Their logic was reused in new files instead (see "Animation conversion").

## 1. Identity, resolved with our reader

`ow-package --object-dump` (property prefix 4, then 8; prefix semantics remain UNVERIFIED per DECISIONS.md) over
`Sanctuary_Dynamic.upk`. Package/export index/size come from the reader's export lists (`tools/export_index.py` caches).
All objects below exist in `Sanctuary_Dynamic.upk`; the number of other packages holding the same path is `also_in` in
`local/slice/npc_identity.json` (e.g. `Skel_Marcus` is also in `SanctuaryAir_Dynamic`).

| Role | Object path | Class | Package | Export | Size |
| --- | --- | --- | --- | --- | --- |
| Marcus pawn | `GD_Marcus.Character.Pawn_Marcus` | WillowGame.WillowAIPawn | `Sanctuary_Dynamic` | 7243 | 846 |
| Marcus mesh component | `GD_Marcus.Character.Pawn_Marcus.SkeletalMeshComponent_2808` | Engine.SkeletalMeshComponent | `Sanctuary_Dynamic` | 14837 | 255 |
| Marcus skeletal mesh | `Char_Marcus.Meshes.Skel_Marcus` | Engine.SkeletalMesh | `Sanctuary_Dynamic` | 14798 | 261001 |
| Marcus AnimSet | `Anim_Generic_NPC.Anim_Generic_NPC` | Engine.AnimSet | `Sanctuary_Dynamic` | 1510 | 849 |
| Marcus AnimSet | `Anim_Generic_NPC.Gestures` | Engine.AnimSet | `Sanctuary_Dynamic` | 1511 | 649 |
| Marcus AnimTree | `GD_Marcus.Character.AnimTree_Marcus` | WillowGame.WillowAnimTree | `Sanctuary_Dynamic` | 16651 | 1123 |
| Marcus material slot `Mati_Marcus_Body` (in use: `Mati_Marcus_Body`) | `Char_Marcus.Materials.Mati_Marcus_Body` | Engine.MaterialInstanceConstant | `Sanctuary_Dynamic` | 7623 | 956 |
| Marcus material slot `Mati_Marcus_Head` (in use: `Mati_Marcus_Head`) | `Char_Marcus.Materials.Mati_Marcus_Head` | Engine.MaterialInstanceConstant | `Sanctuary_Dynamic` | 7624 | 956 |
| TargetDummy pawn | `GD_TargetDummy.Character.Pawn_TargetDummy` | WillowGame.WillowAIPawn | `Sanctuary_Dynamic` | 16471 | 637 |
| TargetDummy mesh component | `GD_TargetDummy.Character.Pawn_TargetDummy.SkeletalMeshComponent_333` | Engine.SkeletalMeshComponent | `Sanctuary_Dynamic` | 14847 | 311 |
| TargetDummy skeletal mesh | `Char_HyperionWorker.Mesh.Skel_HyperionWorker` | Engine.SkeletalMesh | `Sanctuary_Dynamic` | 14796 | 341918 |
| TargetDummy AnimSet | `Anim_Sanctuary.Anim_Fink` | Engine.AnimSet | `Sanctuary_Dynamic` | 1526 | 425 |
| TargetDummy AnimTree | `GD_NPCShared.Character.AnimTree_NPCIdleShared` | WillowGame.WillowAnimTree | `Sanctuary_Dynamic` | 16653 | 1107 |
| TargetDummy component material override | `Char_HyperionWorker.Materials.Mati_ZedSurgeryPatient` | Engine.MaterialInstanceConstant | `Sanctuary_Dynamic` | 7620 | 3403 |
| TargetDummy material slot `Mati_HyperionWorker` (in use: `Mati_ZedSurgeryPatient`) | `Char_HyperionWorker.Materials.Mati_ZedSurgeryPatient` | Engine.MaterialInstanceConstant | `Sanctuary_Dynamic` | 7620 | 3403 |

What the data says (all read through the reader, then confirmed by UModel finding the named objects):

- **Marcus**: `Pawn_Marcus.SkeletalMeshComponent_2808` -> `SkeletalMesh=Char_Marcus.Meshes.Skel_Marcus`, `AnimSets=['Anim_Generic_NPC.Anim_Generic_NPC', 'Anim_Generic_NPC.Gestures']`,
  `AnimTreeTemplate=GD_Marcus.Character.AnimTree_Marcus`, translation Z -82. The AnimTree's sequence nodes name
  `Idle`, `Walk_F`, `Run_F` and `ADD_Marcus` (additive, not imported).
- **Target dummy**: `Pawn_TargetDummy.SkeletalMeshComponent_333` -> `SkeletalMesh=Char_HyperionWorker.Mesh.Skel_HyperionWorker` (a Hyperion worker mesh),
  `Materials=['Char_HyperionWorker.Materials.Mati_ZedSurgeryPatient']` (slot override), `AnimSets=['Anim_Sanctuary.Anim_Fink']`, `AnimTreeTemplate=GD_NPCShared.Character.AnimTree_NPCIdleShared`,
  `PhysicsAsset=Char_HyperionWorker.PhAT.Phat_HyperionWorker`. That AnimTree only contains an `AnimNodeSpecialMoveBlend`; the
  idle clip is named by `GD_Z1_RockPaperGenocideData.Anims.SpecialMove_TargetDummyIdle.AnimName=Shield_Struggle_2` (reached from
  `CharClass_TargetDummy.BehaviorProviderDefinition_5.Behavior_SpecialMove_58`) and the death clips by
  `GD_TargetDummy.Anims.Anim_TargetDummy_Death{Fire,Corrosive,Shock}.AnimName` = `Death_Fire_var1`, `Death_Corrosive_var3`,
  `Death_Shock_var1`. `GD_TargetDummyBot` is a different pawn (battle-droid mesh) and was not extracted.

Not resolvable with our reader: the SkeletalMesh's own material slot list and the AnimSet's clip list (neither payload is
decoded). UModel reported them while exporting and those lists are marked as UModel-sourced in the manifest.

## 2. UModel extraction inventory

UModel build 1590 (`gildor2/UEViewer`, see `EXTERNAL_TOOL_BENCHMARK.md`), `-path=$COOKED -game=border -export ... -out=$OUT/<group>`
`<package> <object> <class>`, one bounded object per run (never a whole package). AnimSets add `-groups` because their
names are not unique (below). Formats: `gltf`, `md5`, `png`. Full command lines, logs and file lists: `local/slice/npc_extract.json`
and `local/slice/logs/*.log`.

| Job | Class | Format | Exit | Seconds | Objects found | Files | Bytes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `Marcus.mesh.gltf` | `SkeletalMesh` | gltf | 0 | 0.12 | 1 | 2 | 454,469 |
| `Marcus.mesh.md5` | `SkeletalMesh` | md5 | 0 | 0.24 | 1 | 11 | 16,371,715 |
| `Marcus.material.Mati_Marcus_Body` | `MaterialInstanceConstant` | png | 0 | 0.53 | 1 | 6 | 18,545,395 |
| `Marcus.material.Mati_Marcus_Head` | `MaterialInstanceConstant` | png | 0 | 0.17 | 1 | 6 | 4,550,946 |
| `Marcus.animset.Anim_Generic_NPC.Anim_Generic_NPC` | `AnimSet` | md5 | 0 | 0.98 | 1 | 67 | 12,607,120 |
| `Marcus.animset.Anim_Generic_NPC.Gestures` | `AnimSet` | md5 | 0 | 1.43 | 3 | 17 | 6,305,457 |
| `TargetDummy.mesh.gltf` | `SkeletalMesh` | gltf | 0 | 0.18 | 1 | 2 | 571,798 |
| `TargetDummy.mesh.md5` | `SkeletalMesh` | md5 | 0 | 0.41 | 1 | 7 | 10,849,910 |
| `TargetDummy.material.Mati_ZedSurgeryPatient` | `MaterialInstanceConstant` | png | 0 | 1.06 | 1 | 8 | 15,360,971 |
| `TargetDummy.animset.Anim_Sanctuary.Anim_Fink` | `AnimSet` | md5 | 0 | 0.6 | 1 | 21 | 3,028,288 |
| `Pistol.gestalt.gltf` | `SkeletalMesh` | gltf | 0 | 0.23 | 1 | 2 | 1,964,147 |
| `Pistol.material.Mati_MaliwanUncommon` | `MaterialInstanceConstant` | png | 0 | 4.23 | 1 | 12 | 47,718,283 |

Totals: 12 jobs, 0 failed, 10.2 s of UModel time, 138,328,499 bytes in 161 files.

Notes on the inventory:

- **Duplicates.** `Gestures` is three different AnimSets in `Sanctuary_Dynamic` (`Anim_Generic_NPC.Gestures`, `Anim_Lilith.Gestures`,
  `Anim_Tannis.Gestures`); UModel exported all three (`objects found = 3`). Without `-groups` they land in one folder and overwrite
  each other; with `-groups` they are separated and only the `Anim_Generic_NPC` one is used (the other two are the
  20 files / 10,028,530 bytes of `Anim_Lilith, Anim_Tannis` listed as unused). `Skel_HyperionWorker` exists in 19 packages
  (one per map that loads it); the export index lists them as the same path, and UModel was pointed at `Sanctuary_Dynamic`.
  A MIC export also writes the textures of the MIC's parent chain (the Maliwan pistol job wrote the 4096x4096
  `Weap_AssaultSubSniper_Comp` and `Weap_SMG_Nrm` for the parent `MasterMati_MaliwanUncommon`); those are in the byte totals but unused.
- **Warnings printed by UModel (not failures; exit 0 everywhere).** `unknown USkeletalMesh3 ReferencePoseBounds`/`SkelMirrorAxis`,
  `unknown FSkeletalMeshLODInfo bDisableCompressions`, `unknown UMaterial3 bAllowLightmapSpecular`, `UTexture2D <name>: dropping 4 bytes`
  for every texture, material-expression editor fields, and `Import(Common_Materials.Master_Creature)` /
  `Common_Textures.Stub*` not found (the master materials and stub textures are not resolved, so no master material graph exists).
  The same text is in each job's log; per-job counts are in the JSON.
- **Unsupported/not attempted.** UModel exports no material graph for these (`Ignoring Material3 ... due to empty parameters`; masters are
  stripped), so only MIC parameter lists (`*.props.txt`) and textures exist. Sounds (`-sounds`), FaceFX and cloth were not exported.
  `ADD_Marcus` (additive clip) and the `ADD_Bind`/`ADD_Hammerlock` clips were exported but not imported.
- **Textures.** The UE import uses the PNGs written by the MIC jobs (`-png`). The `-md5` mesh jobs also wrote TGA copies of the
  same textures (6 TGA files, counted in the byte totals, unused).

## 3. Cross-checks against our reader

### Textures: our own decoder vs UModel (independent decoders, same bytes)

`ow-package --texture` decoded each texture the MICs reference (mip 0, TFC streamed) and was compared with UModel's file:
dimensions exactly, and mean absolute difference over RGB (0-255 scale; DXT decoders differ in rounding).

| Group | Texture | Reader decode | Mean abs diff RGB | Result |
| --- | --- | --- | --- | --- |
| Marcus | `Char_Marcus.Textures.Marcus_Body_Dif` | 2048x2048 PF_DXT1 | 0.1085 | match |
| Marcus | `Char_Marcus.Textures.Marcus_Body_Nrm` | 2048x2048 PF_DXT1 | 0.0191 | match |
| Marcus | `Char_Marcus.Textures.Marcus_Head_Dif` | 1024x1024 PF_DXT1 | 0.1159 | match |
| Marcus | `Char_Marcus.Textures.Marcus_Head_Nrm` | 1024x1024 PF_DXT1 | 0.0103 | match |
| TargetDummy | `Char_HyperionWorker.Textures.HyperionWorker_Msk` | 1024x1024 PF_DXT1 | 0.0208 | match |
| TargetDummy | `Char_HyperionWorker.Textures.HyperionWorker_Dif` | 2048x2048 PF_DXT1 | 0.1128 | match |
| TargetDummy | `FX_GOR_Textures.Textures.Blood_Overlay_Dif` | 512x512 PF_DXT1 | 0.051 | match |
| TargetDummy | `Char_HyperionWorker.Textures.HyperionWorker_Nrm` | 512x512 PF_DXT1 | 0.0266 | match |
| Pistol | `Weap_Pistol.Tex.Weap_Pistols_Comp` | 2048x2048 PF_DXT1 | 0.0273 | match |
| Pistol | `Common_GunMaterials.CompTextures.Weap_LauncherShotgunPistol_Comp` | 4096x4096 PF_DXT1 | 0.094 | match |
| Pistol | `Weap_Pistol.Tex.Weap_Pistols_Nrm` | 2048x2048 PF_DXT1 | 0.0266 | match |

11/11 match (dimensions identical, mean RGB difference below 0.2 of 255; alpha difference 0 where compared).

### Meshes: identity only, plus UModel-internal consistency

Our reader exposes no `SkeletalMesh` payload (`--mesh` rejects it: "export is not StaticMesh"), so vertex/bone counts cannot
be compared with the reader. Compared instead: UModel's own glTF and MD5 exports of the same object, and UE's imported counts.

| Mesh | MD5 bones | glTF skin joints | MD5 verts per section | glTF verts per section | MD5 tris | glTF tris | UE bones after import | UE LOD0 vertices |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `Skel_Marcus` | 112 | 112 | [4204, 1825] | [4204, 1825] | [6653, 3214] | [6653, 3214] | 112 | 6029 |
| `Skel_HyperionWorker` | 50 | 50 | [7923] | [7923] | [8046] | [8046] | 50 | 7879 |

UModel's two formats agree on bones, vertices and triangles for both meshes. UE's vertex count equals the sum for Marcus (6029)
and is 44 lower for the dummy (7879 vs 7923; cause not investigated, possibly the importer merging identical vertices).

## 4. Animation conversion (UNVERIFIED mapping)

The AnimSets here animate only part of each mesh's skeleton (Marcus: 57 of 112 bones; dummy: 27 of 50) so
`tools/prepare_character_anims.py` (which requires identical bone sets) could not be used. `convert_subset` in
`tools/slice_npc_assets.py` applies the same fit: `T_i(t) = C W_i(t) D_i` with `C` the Y mirror fitted from bind positions and
`D_i = W_i(bind)^-1 C^-1 T_i(bind)`; bones without a track keep the UE reference pose. Frame values are treated as local to the
**mesh** parents (UModel's md5anim `hierarchy` block lists every bone under `Root`; the frames are treated as local to the mesh parents,
the same assumption `prepare_character_anims.py` already makes for Maya; supported here only by the plausible poses in section 6). Structural checks that passed: every clip bone exists in
the mesh and in the UE skeleton, every animated bone's parent is animated, the fitted linear map equals the Y mirror within
5.4e-06 (Marcus) / 3.4e-06 (dummy), all quaternions proper rotations.
Not checked against the original engine: root motion, additive/blend behaviour, `bUseTranslationBoneNames`-style retargeting, notifies.

| NPC | Role | AnimSet (group/set) | Clip | Frames | Source fps | Bones animated | UE asset |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Marcus | idle | `Anim_Generic_NPC/Anim_Generic_NPC` | `Idle` | 31 | 15 | 57/112 | `/Game/OpenWillow/Characters/Marcus/Animations/Anim_Marcus_idle` |
| Marcus | walk | `Anim_Generic_NPC/Anim_Generic_NPC` | `Walk_F` | 31 | 30 | 57/112 | `/Game/OpenWillow/Characters/Marcus/Animations/Anim_Marcus_walk` |
| Marcus | run | `Anim_Generic_NPC/Anim_Generic_NPC` | `Run_F` | 21 | 30 | 57/112 | `/Game/OpenWillow/Characters/Marcus/Animations/Anim_Marcus_run` |
| Marcus | gesture_casual | `Anim_Generic_NPC/Gestures` | `Casual_Var1` | 161 | 15 | 57/112 | `/Game/OpenWillow/Characters/Marcus/Animations/Anim_Marcus_gesture_casual` |
| Marcus | gesture_emphatic | `Anim_Generic_NPC/Gestures` | `Emphatic_var1` | 81 | 15 | 57/112 | `/Game/OpenWillow/Characters/Marcus/Animations/Anim_Marcus_gesture_emphatic` |
| TargetDummy | idle | `Anim_Sanctuary/Anim_Fink` | `Shield_Struggle_2` | 204 | 30 | 27/50 | `/Game/OpenWillow/Characters/TargetDummy/Animations/Anim_TargetDummy_idle` |
| TargetDummy | death_fire | `Anim_Sanctuary/Anim_Fink` | `Death_Fire_var1` | 98 | 30 | 27/50 | `/Game/OpenWillow/Characters/TargetDummy/Animations/Anim_TargetDummy_death_fire` |
| TargetDummy | death_corrosive | `Anim_Sanctuary/Anim_Fink` | `Death_Corrosive_var3` | 74 | 30 | 27/50 | `/Game/OpenWillow/Characters/TargetDummy/Animations/Anim_TargetDummy_death_corrosive` |
| TargetDummy | death_shock | `Anim_Sanctuary/Anim_Fink` | `Death_Shock_var1` | 107 | 30 | 27/50 | `/Game/OpenWillow/Characters/TargetDummy/Animations/Anim_TargetDummy_death_shock` |

Imported at the source rate (15 or 30 fps); no noninteger rate occurred. `gesture_*` are an extra from the second AnimSet the pawn
lists; the AnimTree itself plays only `Idle`/`Walk_F`/`Run_F` (+ the additive `ADD_Marcus`). Death clips follow the three
`GD_TargetDummy.Anims` definitions; whether the stock mission plays them was not checked.

## 5. UE import

Commandlet (`UnrealEditor-Cmd -run=pythonscript -nullrhi`), glTF via Interchange (same route as Maya's import), textures from
UModel PNG. Only these content roots are created/replaced: `Characters/Marcus`, `Characters/TargetDummy`, `Characters/Shared`,
`Weapons/MaliwanPistol`. Skeletons and physics assets are created by the glTF import (one per mesh, not shared with Maya).

| Group | Skeletal mesh | Skeleton | Bones | Bounds extent (cm) | Material slots -> instance |
| --- | --- | --- | --- | --- | --- |
| Marcus | `/Game/OpenWillow/Characters/Marcus/Meshes/Skel_Marcus/SkeletalMeshes/Skel_Marcus` | `/Game/OpenWillow/Characters/Marcus/Meshes/Skel_Marcus/SkeletalMeshes/Skel_Marcus_Skeleton` | 112 | [25.68, 80.71, 92.41] | `Mati_Marcus_Body` -> `MI_Mati_Marcus_Body`, `Mati_Marcus_Head` -> `MI_Mati_Marcus_Head` |
| TargetDummy | `/Game/OpenWillow/Characters/TargetDummy/Meshes/Skel_HyperionWorker/SkeletalMeshes/Skel_HyperionWorker` | `/Game/OpenWillow/Characters/TargetDummy/Meshes/Skel_HyperionWorker/SkeletalMeshes/Skel_HyperionWorker_Skeleton` | 50 | [35.55, 88.75, 93.15] | `Mati_HyperionWorker` -> `MI_Mati_ZedSurgeryPatient` |

Materials: master `/Game/OpenWillow/Characters/Shared/M_OW_NPC` (Diffuse + Normal texture parameters, constant roughness 0.7) with one
instance per slot. Marcus's MICs use only `p_Diffuse`/`p_Normal`. The dummy's MIC (`Mati_ZedSurgeryPatient`) also has `p_Masks`,
`p_CustomPattern` and 13 colour/3 scalar parameters (`p_A/B/C Color*`, `p_*Intensity`): the mask, pattern and colour zones are **not applied**
(master graph stripped; Maya's zone-tint approximation was not copied). Their values are kept in `local/slice/npc_identity.json` (`mesh_material_slots[].scalars/vectors`).

Fresh-session check (`preview` mode, a new editor process that only loads from disk): all meshes, skeletons and 9 AnimSequences
loaded; every AnimSequence's skeleton equals its mesh's skeleton; lengths: Marcus idle 2.0 s; Marcus walk 1.0 s; Marcus run 0.667 s; Marcus gesture_casual 10.667 s; Marcus gesture_emphatic 5.333 s; TargetDummy idle 6.767 s; TargetDummy death_fire 3.233 s; TargetDummy death_corrosive 2.433 s; TargetDummy death_shock 3.533 s.

## 6. Visual checks (one still per preview level, not parity)

`-game` launch of `Preview_<name>` levels (fixed front camera, one directional light, sky), window capture; files in
`local/slice/screenshots/` (ignored). What was seen:

- **Marcus** (`Marcus.png`, idle frame; `Marcus-walk.png`, walk clip): a heavyset man in a brown/ochre jacket and olive cargo trousers,
  face and clothing textured and in the right UV places, standing upright with arms at his sides. The walk still differs from the idle
  still (arm swing, narrower stance), so the clips drive the skeleton. Looks like Marcus; facing is toward the camera (+X).
- **Target dummy** (`TargetDummy.png`, idle `Shield_Struggle_2`): a grey-skinned bald man in tactical clothing with additional
  mechanical limbs hanging from the shoulders (the mesh's own geometry), textured, upright. This is what the stock pawn uses.
- **Pistol** (`pistol.png`, rolled sample): a revolver-shaped weapon, lying along Y. Its colours are the packed composite texture
  (`Weap_LauncherShotgunPistol_Comp`) used directly as base colour, so it looks like noise, not paint.
- The first Marcus capture (40 s wait) was a black frame because the window had not rendered yet; recaptured with a 60 s wait.
  The runner takes a fixed wait; there is no render-complete signal.

Not checked: lighting/shader parity, scale against Maya in the same scene, animation blending, gameplay use, any per-frame comparison.

## 7. Maliwan pistol

Existing pipeline state before this pass: `Content/OpenWillow/Weapons/Items` held 18 rolled demo weapons including `SK_smg_maliwan_1` but **no**
Maliwan pistol; `local/items` likewise. This pass adds `Content/OpenWillow/Weapons/MaliwanPistol` and does not touch `Weapons/Items`.

`tools/weapon_recipe.py` (rules UNVERIFIED: merge order, uniform pick at zero weight, manufacturer weights; stage 30, seed 1) over `Startup.upk`.
Merge chain: `Pistol_Maliwan` -> `Pistol_Maliwan_2_Uncommon` -> `Pistol_Maliwan_2_Fire`. `GD_Z1_RockPaperGenocideData.MW_RockPaper_Fire` (the lent weapon)
adds its own `PartList`; with this tool's rules its candidate set is identical to the base balance's (`slots` equal in `candidates.json`). Another
agent decodes the exact selection; the table lists what the balance allows.

Gestalt mesh `Weap_Pistol.GestaltDef_Pistol_GestaltSkeletalMesh` (Startup export 45445, 1,100,886 bytes) holds every pistol part in one
skeletal mesh; the fragment table comes from `GestaltDef_Pistol` (export 19832) decoded with `local/infinity/gestalt.schema`.

| Slot | Candidate part | Weight | Gestalt fragment |
| --- | --- | --- | --- |
| Body | `GD_Weap_Pistol.Body.Pistol_Body_Maliwan_2` | 1 | `Pistol_Body_Maliwan` |
| Grip | `GD_Weap_Pistol.Grip.Pistol_Grip_Bandit` | 1 | `Pistol_Grip_Bandit` |
| Grip | `GD_Weap_Pistol.Grip.Pistol_Grip_Tediore` | 1 | `Pistol_Grip_Tediore` |
| Grip | `GD_Weap_Pistol.Grip.Pistol_Grip_Vladof` | 1 | `Pistol_Grip_Vladof` |
| Grip | `GD_Weap_Pistol.Grip.Pistol_Grip_Dahl` | 1 | `Pistol_Grip_Dahl` |
| Grip | `GD_Weap_Pistol.Grip.Pistol_Grip_Torgue` | 1 | `Pistol_Grip_Torgue` |
| Grip | `GD_Weap_Pistol.Grip.Pistol_Grip_Maliwan` | 1 | `Pistol_Grip_Maliwan` |
| Grip | `GD_Weap_Pistol.Grip.Pistol_Grip_Jakobs` | 1 | `Pistol_Grip_Jakobs` |
| Grip | `GD_Weap_Pistol.Grip.Pistol_Grip_Hyperion` | 1 | `Pistol_Grip_Hyperion` |
| Barrel | `GD_Weap_Pistol.Barrel.Pistol_Barrel_Bandit` | 1 | `Pistol_Barrel_Bandit` |
| Barrel | `GD_Weap_Pistol.Barrel.Pistol_Barrel_Tediore` | 1 | `Pistol_Barrel_Tediore` |
| Barrel | `GD_Weap_Pistol.Barrel.Pistol_Barrel_Vladof` | 1 | `Pistol_Barrel_Vladof` |
| Barrel | `GD_Weap_Pistol.Barrel.Pistol_Barrel_Dahl` | 1 | `Pistol_Barrel_Dahl` |
| Barrel | `GD_Weap_Pistol.Barrel.Pistol_Barrel_Torgue` | 1 | `Pistol_Barrel_Torgue` |
| Barrel | `GD_Weap_Pistol.Barrel.Pistol_Barrel_Maliwan` | 1 | `Pistol_Barrel_Maliwan` |
| Barrel | `GD_Weap_Pistol.Barrel.Pistol_Barrel_Jakobs` | 1 | `Pistol_Barrel_Jakobs` |
| Barrel | `GD_Weap_Pistol.Barrel.Pistol_Barrel_Hyperion` | 1 | `Pistol_Barrel_Hyperion` |
| Sight | `GD_Weap_Pistol.Sight.Pistol_Sight_None` | 200 | `Pistol_Scope_BanditMade` |
| Sight | `GD_Weap_Pistol.Sight.Pistol_Sight_Bandit` | 10 | `Pistol_Scope_Bandit` |
| Sight | `GD_Weap_Pistol.Sight.Pistol_Sight_Tediore` | 10 | `Pistol_Scope_Tediore` |
| Sight | `GD_Weap_Pistol.Sight.Pistol_Sight_Vladof` | 10 | `Pistol_Scope_Vladof` |
| Sight | `GD_Weap_Pistol.Sight.Pistol_Sight_Dahl` | 10 | `Pistol_Scope_Dahl` |
| Sight | `GD_Weap_Pistol.Sight.Pistol_Sight_Torgue` | 10 | `Pistol_Scope_Torgue` |
| Sight | `GD_Weap_Pistol.Sight.Pistol_Sight_Maliwan` | 10 | `Pistol_Scope_Maliwan` |
| Sight | `GD_Weap_Pistol.Sight.Pistol_Sight_Jakobs` | 10 | `Pistol_Scope_Jakobs` |
| Sight | `GD_Weap_Pistol.Sight.Pistol_Sight_Hyperion` | 10 | `Pistol_Scope_Hyperion` |
| Elemental | `GD_Weap_Pistol.elemental.Pistol_Elemental_Fire` | 1 | `Acc_Barrel_Elemental2` |
| Accessory1 | `GD_Weap_Pistol.Accessory.Pistol_Accessory_None` | 200 | `Acc_Barrel_Elemental3` |
| Accessory1 | `GD_Weap_Pistol.Accessory.Pistol_Accessory_Bayonet_1` | 10 | `Acc_Barrel_Blade1` |
| Accessory1 | `GD_Weap_Pistol.Accessory.Pistol_Accessory_Laser_Accuracy` | 10 | `Acc_Barrel_Laser` |
| Accessory1 | `GD_Weap_Pistol.Accessory.Pistol_Accessory_Laser_Double` | 10 | `Acc_Barrel_Laser` |
| Accessory1 | `GD_Weap_Pistol.Accessory.Pistol_Accessory_Stock_Stability` | 10 | `Acc_Grip_Stock` |
| Accessory1 | `GD_Weap_Pistol.Accessory.Pistol_Accessory_Tech_1_Mag` | 10 | `Acc_Barrel_Tech1` |
| Accessory1 | `GD_Weap_Pistol.Accessory.Pistol_Accessory_Tech_2_Damage` | 10 | `Acc_Barrel_Tech2` |
| Accessory1 | `GD_Weap_Pistol.Accessory.Pistol_Accessory_Tech_3_Firerate` | 10 | `Acc_Barrel_Tech3` |
| Material | `GD_Weap_Pistol.ManufacturerMaterials.Mat_Maliwan_2` | 1 | (material part: `Common_GunMaterials.Materials.Pistol.Mati_MaliwanUncommon`) |

- Body: only `Pistol_Body_Maliwan_2` (fragment `Pistol_Body_Maliwan`). Elemental: only `Pistol_Elemental_Fire` (`Acc_Barrel_Elemental2`).
  Material: only `Mat_Maliwan_2` -> `Common_GunMaterials.Materials.Pistol.Mati_MaliwanUncommon` (Startup export 23971).
- The `*_None` sight and accessory parts name fragments (`Pistol_Scope_BanditMade`, `Acc_Barrel_Elemental3`) as "no part": `Pistol_Scope_BanditMade`
  does **not** exist in the gestalt's fragment table (the same unresolved-fragment case `tools/seed_inventory_demo.py` records), so the
  weapon without a sight has nothing to show for it. `Acc_Barrel_Elemental3` exists and is what the rolled "None" accessory selects
  in this recipe's data (it may be an unused/blank part; not visually inspected on its own).

UE assets (`/Game/OpenWillow/Weapons/MaliwanPistol/`):

- `SK_Pistol_Maliwan_2_Fire_seed1`: the seed-1 sample (5 fragments: `Acc_Barrel_Elemental2`, `Acc_Barrel_Elemental3`, `Pistol_Barrel_Jakobs`, `Pistol_Body_Maliwan`, `Pistol_Grip_Jakobs`), built with the existing
  `filter_gestalt_gltf.py` method; 2157 vertices (the full gestalt vertex buffer is kept, only indices are filtered, so bounds and vertex count do not describe the visible part).
- `SK_Pistol_Maliwan_Candidates`: one **section per candidate fragment** (33 sections, 9404 triangles), each with its own material slot named `Frag_<fragment>`. Show/hide
  a part with the section (material-slot) index; the fragment-to-slot map and the part-to-fragment map are in the manifest (`use.MaliwanPistol`).
  Candidates overlap in space by design (all grips, all barrels...). The section order follows the sorted fragment names, not any in-game order.
- Textures (imported, not combined into paint): `Weap_LauncherShotgunPistol_Comp` (4096x4096, `p_Diffuse`), `Weap_Pistols_Comp` (`p_Masks`, 2048), `Weap_Pistols_Nrm`
  (`p_NormalScopesEmissive`, 2048). Material `MI_Mati_MaliwanUncommon` on master `Shared/M_OW_GunComp` uses the composite as base colour only; the
  mask/tint/pattern/emissive logic and the `p_DecalRotate`/`p_DecalScalePosition` values are not applied (listed in the manifest).

## 8. Not resolved or not done

- Skeletal mesh payloads cannot be compared with our reader (no decoder; the layout has no public spec). Counts are UModel-vs-UModel and UModel-vs-UE.
- The AnimSet clip lists and the SkeletalMesh material slot lists come from UModel, not from our reader. The AnimNode graphs that pick clips
  (blend by speed/stance, special moves, slots, additive `ADD_Marcus`) are not decoded; only the leaf clip names the nodes carry are used.
- Talk/gesture clips: `Casual_Var1`/`Emphatic_var1` are chosen by name only; which gesture the dialog system plays for which line is not decoded.
- Marcus's own material slots in UE use default slot names `Mati_Marcus_Body/Head`; the dummy's slot is named `Mati_HyperionWorker` (the mesh's
  default) but holds the pawn's override `Mati_ZedSurgeryPatient`.
- `GD_TargetDummyBot`, `Pawn_TargetDummy_Target` (same mesh as `Pawn_TargetDummy`) and the dialog/FaceFX data were not extracted or imported.
- No facial/eye `SkelControl_LookAt` setup, no physics, no materials beyond diffuse+normal, no sockets.
- `ctest`/`tools/verify_packages.py` were not run: no file under `src/`, `tests/` or `CMakeLists.txt` changed and the `build/` directory was left alone.
- An UnrealEditor process that is not ours was seen running between my editor runs (pid 7956); it was not touched. None was running, and no lock file existed, when this pass finished.

## 9. Reproduce

```powershell
$env:OPENWILLOW_BL2    = "<Borderlands 2 folder>"
$env:OPENWILLOW_UMODEL = "<umodel.exe, build 1590>"
tools\seed_slice_npc_assets.ps1 -Steps all -CaptureWaitSeconds 60   # extract, import, preview, capture, manifest
```

Needs: a built `build/Release/ow-package.exe`, `tools/export_index.py` and `tools/mission_closure.py` already run (path index), `local/infinity/gestalt.schema`,
the built UE editor module. Last complete run (`-Steps all`): no error elapsed=562s, identity through manifest.

## 10. Manifest

`local/slice/npc_assets.json` (schema `openwillow.slice_npc_assets/1`): `npcs.<Marcus|TargetDummy>.{source_identity, import_status, ue, anims}`, `pistol.{source_identity,
candidates, sample, candidate_sections, import_status, ue, fresh_session_check}` and a flat `use` section with the UE asset paths to consume:

- Marcus: mesh `/Game/OpenWillow/Characters/Marcus/Meshes/Skel_Marcus/SkeletalMeshes/Skel_Marcus`; skeleton `/Game/OpenWillow/Characters/Marcus/Meshes/Skel_Marcus/SkeletalMeshes/Skel_Marcus_Skeleton`; anims idle `/Game/OpenWillow/Characters/Marcus/Animations/Anim_Marcus_idle`, walk `/Game/OpenWillow/Characters/Marcus/Animations/Anim_Marcus_walk`, run `/Game/OpenWillow/Characters/Marcus/Animations/Anim_Marcus_run`, gesture_casual `/Game/OpenWillow/Characters/Marcus/Animations/Anim_Marcus_gesture_casual`, gesture_emphatic `/Game/OpenWillow/Characters/Marcus/Animations/Anim_Marcus_gesture_emphatic`.
- TargetDummy: mesh `/Game/OpenWillow/Characters/TargetDummy/Meshes/Skel_HyperionWorker/SkeletalMeshes/Skel_HyperionWorker`; skeleton `/Game/OpenWillow/Characters/TargetDummy/Meshes/Skel_HyperionWorker/SkeletalMeshes/Skel_HyperionWorker_Skeleton`; anims idle `/Game/OpenWillow/Characters/TargetDummy/Animations/Anim_TargetDummy_idle`, death_fire `/Game/OpenWillow/Characters/TargetDummy/Animations/Anim_TargetDummy_death_fire`, death_corrosive `/Game/OpenWillow/Characters/TargetDummy/Animations/Anim_TargetDummy_death_corrosive`, death_shock `/Game/OpenWillow/Characters/TargetDummy/Animations/Anim_TargetDummy_death_shock`.
- Maliwan pistol: sample `/Game/OpenWillow/Weapons/MaliwanPistol/SK_Pistol_Maliwan_2_Fire_seed1`; candidate sections `/Game/OpenWillow/Weapons/MaliwanPistol/SK_Pistol_Maliwan_Candidates`; material `/Game/OpenWillow/Weapons/MaliwanPistol/Materials/MI_Mati_MaliwanUncommon`.

