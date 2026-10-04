# Sanctuary ambient NPCs: census, assets and host walk (2026-10-04)

AI-assisted (Claude), 2026-10-04. Extraction, import and host-run record for the civilians who stand and wander in Sanctuary.
Everything game-derived (census JSON, UModel exports, manifests, screenshots) stays under ignored `local/` and the ignored
host Content folder; this file holds identities, counts, rules and pass/fail only. No dialogue text, no listings.

What this record claims and does not claim:

- **Census** means our reader (`ow-package`, `--properties` for placed actors, `--object-dump` for definitions) decoded the
  object and the numbers below came out of it. **Extraction** means UModel build 1590 wrote files for an object our reader
  identified. **Import** means UE 5.8 created an asset and a fresh editor session loaded it back. **Host run** means the
  OpenWillow module spawned the pawns from the manifest and the log/self-test says what they did. **No original-game parity
  capture exists for any of it**; the movement rules are stand-ins built on the installed data and on lane E's own-words
  reading of the native code (`NATIVE_AMBIENT_NPC.md`, `UNVERIFIED` in game). Materials are "textures bound by UModel's
  guess" in a minimal UE material, not verified graphs.
- Our reader decodes object identity and tagged properties; it does not decode `SkeletalMesh`, `AnimSet` or `AnimSequence`
  payloads. For those the cross-check is identity only plus UModel-internal consistency (as in `SLICE_NPC_ASSETS.md`).

## 1. Where the ambient NPCs live

`Sanctuary_P` streams ten levels. The population data for the civilians is in **`Sanctuary_Combat`** (the streaming level that
holds the AI population); the named story NPCs and their definitions are in `Sanctuary_Dynamic`. Counts of placed
actors (`tools/census_ambient_npcs.py`, `local/slice/ambient_npcs.json`):

| Class | `Sanctuary_Combat` | `Sanctuary_Dynamic` | `Sanctuary_Side` |
| --- | --- | --- | --- |
| `WillowGame.PopulationOpportunityDen` | 32 | 17 | 0 |
| `WillowGame.WillowPopulationPoint` | 123 (112 used by a den) | 23 | 0 |
| `WillowGame.WillowPopulationEncounter` | 2 | 1 | 0 |
| `WillowGame.WillowAIMoveNode` | 50 | 195 | 1 |
| `WillowGame.Perch` | 148 | 87 | 10 |
| `WillowGame.WillowAIPawn` (placed) | 1 | 10 | 0 |
| `GearboxFramework.SeqEvent_PopulatedPoint` | 36 | 26 | 0 |
| `WillowGame.WillowSeqAct_AIScripted` | 13 | 49 | 0 |

The banners in `Sanctuary_Px` are 26 `Engine.SkeletalMeshActor` cloth props, not people.

### Civilians (`Sanctuary_Combat`)

Four population definitions in `GD_Population_NPC.Population` spawn them. Each has a male and a female factory (the
`MaleOnly` one has only the male), both with weight 1; the pawn balance names a pawn archetype and one playthrough-1 name
"Overlook Citizen" (the in-game `DefaultDisplayName` of the class is "Sanctuary Citizen"):

| Population definition | Dens | Population points | Flag set on the pawn at spawn (`FlagsToSet`) | Meaning (lane E, `UNVERIFIED`) |
| --- | --- | --- | --- | --- |
| `PopDef_NPC_ScriptedIdle` | 13 | 69 | `Flag_IdleNPC` | "Perch Only AI": snapped to a perch, stays |
| `PopDef_NPC_ScriptedWalking` | 9 | 16 | none | load-balanced scripted walker |
| `PopDef_NPC_MaleOnlyScriptedWalking` | 3 | 4 | none | same, male only |
| `PopDef_NPC_NoThrottle` | 7 | 23 | `Flag_NPCDoNotThrottleMovement` | not load-balanced; sent around town by Kismet |

All pawns come from two archetypes:

| Role | Object path | Class | Package | Export | Size |
| --- | --- | --- | --- | --- | --- |
| Male citizen pawn | `GD_GenericNPCMale.Character.Pawn_GenericNPCMale` | WillowGame.WillowAIPawn | `Sanctuary_Combat` | 1672 | 874 |
| Male mesh component | `GD_GenericNPCMale.Character.Pawn_GenericNPCMale.SkeletalMeshComponent_9` | Engine.SkeletalMeshComponent | `Sanctuary_Combat` | 2610 | 295 |
| Male skeletal mesh | `Char_GenericMale.Mesh.Skel_GenericMale` | Engine.SkeletalMesh | `Sanctuary_P` | 9578 | 707,064 |
| Male head / body material (in use) | `Char_GenericMale.Materials.GenericMaleHead01_Mati` / `GenericMaleBody_Mati` | Engine.MaterialInstanceConstant | `Sanctuary_P` / `Sanctuary_Combat` | 2500 / 1733 | |
| Female citizen pawn | `GD_GenericNPCFemale.Character.Pawn_GenericNPCFemale` | WillowGame.WillowAIPawn | `Sanctuary_Combat` | 2992 | 874 |
| Female mesh component | `GD_GenericNPCFemale.Character.Pawn_GenericNPCFemale.SkeletalMeshComponent_702` | Engine.SkeletalMeshComponent | `Sanctuary_Combat` | 2609 | 295 |
| Female skeletal mesh | `Char_GenericFemale.Mesh.Skel_GenericFemale` | Engine.SkeletalMesh | `Sanctuary_Combat` | 2608 | 733,419 |
| Female head / body material (in use) | `Char_GenericFemale.Materials.GenericFemaleHead_Mati` / `GenericFemaleBody_Mati` | Engine.MaterialInstanceConstant | `Sanctuary_Combat` | 1731 / 1730 | |
| Shared AnimTree | `GD_NPCShared.Character.AnimTree_NPCShared` | WillowGame.WillowAnimTree | `Sanctuary_Combat` | 3009 | 1,191 |
| Shared AnimSet | `Anim_Generic_NPC.Anim_Generic_NPC` | Engine.AnimSet | `Sanctuary_Combat` | 337 | 849 |

Facts from the data: both classes have `GroundSpeed` 294, `WalkingPct` 1, yaw rotation rate 16384 units/s (90 deg/s), body
`DefaultStance` Patrol with `SpeedScale` 0.51 (the female body's `DefaultStance` is written as 0); the AnimTree names `Idle`,
`Walk_F`, `Jog_F`, `Run_F` and `Scared_Run`; the mesh component translation is Z -79.75 (male) / -79.5 (female). Both
meshes have 112 bones in UE, the same count as Marcus. The pawns carry 50-odd `StaticMeshComponent`s each (hairs, hats,
glasses, gear) that an instance-data switch chooses per spawn; those are **not** imported here (see section 6).
`ResistanceFighter` (`Pvt. Jessup`) uses the same mesh and body textures as the male citizen with different colour
parameters; without the colour-zone shading it would look identical, so it is not one of the kinds below.

### Named NPCs (`Sanctuary_Dynamic`)

Ten `WillowAIPawn` actors are placed individually: Daisy, Marcus, Van Owen, Tannis, Hammerlock, Claptrap, Roland, a
Resistance fighter, Zed and Lilith (archetypes `GD_<Name>.Character.Pawn_<Name>`). Marcus is the Fire mission pawn
(`SLICE_WORLD_PLACEMENT.md`). The dens there are the Resistance fighters (7 `Pop_ResistanceFighter_Pistol`, 1 `_NoGun`), the
target dummies, Scooter, One Winner and a ten-point John Mamaril den. They are mission/story actors, not the wandering
crowd; this pass does not touch them.

## 2. The route data (what "walking their real routes" means here)

Everything below is decoded, not guessed; what the game does with it is `UNVERIFIED`.

- **Perch** (148 actors in `Sanctuary_Combat`, 17 `PerchDefinition`s in use): a node with a pose and `PerchDef`. A definition's
  `AnimMap` entry for `BodyTag_Human` names a start, an idle and a stop special move; the idle one is a
  `SpecialMove_PerchRandomLoop` with weighted variants (e.g. `Perch_ArmsCrossed_Loop` 1.0 / `_var2` 0.1). The special moves name
  clips of `Anim_Generic_NPC` (44 perch and idle clips in total across the definitions; 42 of them exist in the exported set).
  `LoopTime` (3-5 s, 5-10 s for chairs) and `LerpTime` (1.0 for the shared perches) are on the definition; 46 perches override
  the loop time. Perches per definition in this level: ChairSit 45, HandsOnHips 15, LookIntently 11, LookAtGround 11, KickGround 11,
  ArmsCrossed 10, LeanOnWall 10, BangOnSomething 8, PeerUnder 7, LeanOnCounter 5, SittingDrinking 4 (+3 Var2), ArmsCrossedForever 3,
  BarrelSit 2, LeanOnWallNonRandom 1, DartsHit 1, DartsMiss 1.
- **Perch chains** (`NextNodes` with weights, all 1 here): the walking dens' spawn points name a perch in
  `InitialActionDestinations`, and a perch's `NextNodes` lead to further perches; chains are 2-4 perches long.
- **Town loops** (the 50 plain `WillowAIMoveNode`s, single successor each, weight 1): the node graph is three closed circuits and
  their approaches. One circuit around the middle (nodes 40, 43-49), one around the east side and Moxxi's
  (50, 52-62) and one around the south and west (16, 24-26, 29-39). A `SeqEvent_PopulatedPoint` for a `NoThrottle` point leads (through
  `SeqAct_ApplyBehavior`) to a `WillowSeqAct_AIScripted` whose last `Destination` is one of those nodes (51, 33, 21, 41, 1...);
  the lane E note says a scripted walk then follows `NextNodes` from there, so a closed circuit is walked for ever.
- **Population limits:** a den's `MaxActiveActorsIsNormal` (class default 1) says how many of its points are live at once;
  summed over the NPC dens that is 53 pawns (20 idle, 11 + 4 walkers, 18 crowd), against 112 points.
- **Enabling:** 31 of the 32 dens and both encounters are saved with `IsEnabled = False`; no Kismet in the town enables them
  (the only den variables in the Sanctuary Kismet are mission ones). **Observed in the real game** (section 3a): at mission Plan B
  the persistent `Sanctuary_Combat` encounter 0 and 23 of the 32 dens are enabled (12 of 13 idle, 8 of 9 walking, 3 of 3
  male-only walking), encounter 1 and all 7 `NoThrottle` dens are not. What turns them on is native and not read.

## 3. Assets

Pipeline (new, own files; the Marcus/dummy/pistol pipeline is untouched): `tools/ambient_npc_assets.py` (identity, UModel
extraction, clip conversion, editor job, manifest) and `tools/seed_ambient_npc_assets.ps1` (the lock-taking runner), with the
unchanged editor half `tools/slice_npc_editor.py` run on `local/slice/ambient/editor_job.json`. UE content root:
`/Game/OpenWillow/Characters/Ambient/<Kind>` (the shared `M_OW_NPC` material is reused).

### 3a. Real-game observations (2026-10-04, `UNVERIFIED` only where marked)

Read with `tools/real_game/scripts/ambient_npcs.py` through the SDK driver (a level-8 Maya at mission Plan B, saves blocked
and restored byte-identical afterwards; `REALGAME_GROUND_TRUTH.md` method). Raw dumps and frames: ignored
`local/realgame/ambient/`.

- **Live pawns right after loading:** 33 "Sanctuary Citizen" (16 male mesh, 17 female mesh) plus 9 "Pvt. Jessup" Resistance fighters
  and the named NPCs, 52 dens/encounters read, 36 of them enabled. 20 of the 33 citizens stand on a `Perch` node (within 80 uu, not moving
  during a 100 s sample), 4 moved more than 150 uu, 9 stand elsewhere (the `ApplyBehavior`-only points).
- **The wandering crowd of the town is not the citizens:** over a 100 s sample the 7 pawns that cover the town (4,600-9,300 uu
  walked each) are all Resistance fighters. Three of them have 90-96% of their samples within
  150 uu of a `Sanctuary_Combat` node edge (median 39-65 uu from the polyline); the other four are 4-69%, so part of their
  routes lie on nodes this pass does not use (the `Sanctuary_Dynamic` nodes). Speed by displacement is 94-95 uu/s while moving
  (190 x 0.51 = 97 from the data). Citizens that walk do so on perch chains, a few hundred uu. This is why the host manifest's
  Kismet-routed `NoThrottle` crowd (disabled in game at Plan B) is left out of the observed spawn set.
- **Citizen walking speed:** the `Velocity` property of one walking citizen read 150.0 = 294 x 0.51 (GroundSpeed x patrol
  SpeedScale), one sample; the host uses 149.94.
- The Resistance fighters wear red berets, armour and carry guns (frames in `local/realgame/ambient/c1`); they share the male
  citizen's mesh and textures and differ by colour-zone vectors and attached static meshes, which this pass does not
  reproduce, so **the wandering patrols are the main thing the host does not show** (section 6).

### 3b. Imported assets (UE 5.8, fresh-session check)

Two kinds, same pipeline as Marcus (UModel glTF + PNG textures bound by UModel's guess; clips converted with the same fit):

| Kind | Skeletal mesh (UE) | Bones | Extent (cm) | Materials | Clips imported | Fit error |
| --- | --- | --- | --- | --- | --- | --- |
| CitizenMale | `/Game/OpenWillow/Characters/Ambient/CitizenMale/Meshes/Skel_GenericMale/SkeletalMeshes/Skel_GenericMale` | 112 | 18.1 x 74.9 x 93.1 | `MI_GenericMaleHead01_Mati`, `MI_GenericMaleBody_Mati` (4 textures) | 42 | 5.8e-06 |
| CitizenFemale | `/Game/OpenWillow/Characters/Ambient/CitizenFemale/Meshes/Skel_GenericFemale/SkeletalMeshes/Skel_GenericFemale` | 112 | 16.9 x 73.7 x 93.1 | `MI_GenericFemaleHead_Mati`, `MI_GenericFemaleBody_Mati` (4 textures) | 42 | 6.1e-06 |

Clips per kind: `Idle` (2.0 s), `Walk_F` (1.0 s), `Kick_Object_on_Ground` and 39 perch clips (start, loop, variants, end) of the 15 perch
definitions that map completely (rates 7.5, 9, 15, 22.5 and 30 fps; the 7.5 and 22.5 fps ones needed a half-integer frame-rate
branch in `tools/slice_npc_editor.py`). Two definitions (`Perch_NPC_SittingDrinking`, `_Var2`) name clips that are not in the exported
set and are dropped (their perches are not used). 12 jobs for UModel in 3 s; 36 MB of UE content. All 42 clips of both kinds load in a
fresh session and share their mesh's skeleton. The material slot `GenericMaleBody_HodunkGrunt` holds the pawn's override
`GenericMaleBody_Mati` (the component's Materials array), as for Marcus. UModel could not export the male body MIC from the package
that holds the MIC (its textures live in `Sanctuary_P`), so the textures named by our reader's decode of the MIC (`p_Diffuse`, `p_Normal`)
are exported one by one instead.


## 4. Host implementation

`OpenWillowAmbient.h/.cpp` (new): `AOpenWillowAmbientNpc` (one pawn, a small state machine) and `AOpenWillowAmbientDirector`
(spawns the manifest, runs the load balancer, the self-test and the capture tour). The game mode creates the director when
`-owambient=<ambient_world.json>` or the environment variable `OPENWILLOW_AMBIENT` names a manifest; otherwise nothing is
spawned. `tools/prepare_ambient_world.py` writes the manifest from the census and the asset manifest. Rules, each a
stand-in unless it says it comes from data:

- **Realisation** (two modes, both stand-ins for the native population's own choice): (a) default, each NPC den realises
  `MaxActiveActorsIsNormal` (default 1) of its points by a seeded shuffle and the kind is a seeded weighted draw over the factories'
  weights, every den treated as enabled (39 pawns); (b) `--observed`, the 33 live citizens of the real-game capture at Plan B become the
  spawns (kind from the mesh, pose as found; 20 on a perch, 9 held where they stood, 4 walkers). The capture mode below uses (b).
- **Route** (data): the first node is `InitialActionDestinations[0]`, or the last `Destination` of the `AIScripted` the point's
  `SeqEvent_PopulatedPoint` reaches. A point with neither (the `ApplyBehavior`-only points) is not realised.
- **Idle pawns** (`Flag_IdleNPC`): snapped to the perch, perch cycle for ever (script `Action_ScriptedNPC.IdleNPC`, lane E).
- **Walkers:** straight lines between nodes at `GroundSpeed x WalkingPct x Patrol SpeedScale` = 294 x 1 x 0.51 = 150 uu/s
  (the product is `UNVERIFIED`; the native speed function was not read), arrival at the node's `PawnArrivalRadius` (128 for a
  plain node, 32 for a perch, as written on the node when set), next node by weight, yaw turned at the class rate,
  feet on a visibility trace.
- **Perch cycle** (script, lane E): ease onto the perch pose over `LerpTime`, start clip, idle variants by weight for a random
  time in `LoopTime` (or the node's override), stop clip, next node. Perch cooldown (a second user may not take the perch) is
  not implemented; no two pawns here share a perch.
- **NPCLoadBalancer** (native, read by lane E, `UNVERIFIED`): pawns that are not marked `Flag_NPCDoNotThrottleMovement` wait standing
  until admitted; at most 7 path at once, one admission per 0.5 s, the waiter with the largest `seconds waited - (distance to
  the player / 1000)^2` first.
- Fixed random seeds (per pawn name) so a capture repeats; the original's random choices are native and not reproduced.

## 5. Checks

Automated, host (no original-game parity):

- `tools/test_ambient.ps1 -Seconds 60` (observed manifest, 33 pawns: 16 male, 17 female; 198 nodes, 15 perch definitions):
  `OWAMBIENT SUMMARY result=PASS pawns=33 reached_a_node=33 walked_50cm=3 at_perch=24 seconds=60`. The test passes when every
  pawn reached a node (a pawn held in place counts) and at least one walked 50 cm. It does not check that the poses are right.
- Capture tour (`-Shots`): 8 stops (6 idle pawns on different perches, one walker, and a stop for which no walker was found), 3 frames each
  to `local/ambient/<tag>/`.
- Fire mission suite with the ambient manifest switched on (`OPENWILLOW_AMBIENT`), `tools/test_quest.ps1`: first run **79 checks, 0 errors**,
  resume **11 checks, 0 errors**. An earlier first run with ambient on failed check 61 `phaselock_stock_presentation_running`
  (Phaselock light read 32.00 against data 4.00 at the 1.30 s sample) and passed on the repeat; the same suite without ambient
  passed 79/0 and 11/0. The cause of the single failure is not known (it is lane A's presentation check; timing is the suspect).
- `ctest --test-dir build -C Release`: 10/10 passed. `tools/verify_packages.py`: 9/9 packages match.
- Floor traces: pawns logged `floor jump` three times in the first manifest (a walker crossing a ramp, 150-380 uu steps); the observed
  manifest has no walker on that route.

Real game (observations, section 3a): enabled-den table, 33 live citizens, walking speeds, loop-following by the Resistance patrols.

Visual: frames are in `local/ambient/` and `local/realgame/ambient/`; the independent critic's review request is
`local/orch/B/review_request_1.md` (round 2: `review_request_2.md`). **I have not graded these frames.** Known visible differences from the real
game (round 1 text; round 2 added heads, hair, hats and an ink line, section 8): the body is one outfit (the real ones show several garments), hair colour is a stand-in tint, the host map is lit by day while the real
capture was at night, the real citizens have a "Sanctuary Citizen" name tag, and the host has no Resistance patrols.


## 6. Not done / `UNVERIFIED`

- Since round 2 the observed pawns' heads, hair, hats and gear are applied (section 8). Still missing: FaceFX, head-look, body variants (the live body clones'
  `p_HidePart` / `p_MuscleFat` and the garment layers of the body atlas), goggles/masks/beards other than those seen on the 33 captured pawns.
- Colour-zone shading of `Master_NPC` (the live material clones' zone vectors are read but only used as a hair tint), so Resistance
  fighters and other recoloured bodies are not a separate kind.
- Special moves on nodes (`SpecialMoves`, `HoldTime`: none are set on the 198 nodes of this level), Moxxi's bar `RunCustomEvent`
  chains and `LeavingMoveNode` events, the `ApplyBehavior`-only crowd points (poses applied by Kismet behaviours), talking and
  look-at-player, the placed `WillowAIPawn_15` (sent to `Perch_2` by an `AIScripted`), the dart players' darts, the Dynamic named NPCs.
- Everything about when the native population enables a den, how many pawns are live at once, the next-node choice, the walking
  speed and the load balancer is `UNVERIFIED` until a real-game capture exists (`tools/real_game/scripts/ambient_npcs.py` is the
  prepared sampler).

## 7. Reproduce

```powershell
python tools/census_ambient_npcs.py --definitions-from Sanctuary_Combat        # ~25 min, local/slice/ambient_npcs.json
tools\seed_ambient_npc_assets.ps1 -Steps all                                   # extract, import, preview, manifest
python tools/prepare_ambient_world.py                                          # local/slice/ambient_world.json
# host: -owwalk -owmaya ... -owambient=local/slice/ambient_world.json  [-owambienttest | -owambientshots]
```

## 8. Round 2 (2026-10-04): critic findings, causes and changes

Round 1 scored 5/10 from the independent critic (mesh/outfit 4, poses 5, walk 6, scale 7). Findings and what was found:

1. **"Duplicate pawn" at stop 1 (Perch_66) was two real pawns, not one placed twice.** Perch_66 and `Perch_140` are 240 uu apart and both
   were occupied in the real game (two live `WillowAIPawn`s at 6113,4023 and 6358,4048); the round-1 camera stood on the line through
   both. Each spawn point is realised once (the generator drops a second pawn with the same kind and position). Fix at the cause of the
   overlap: the capture camera rejects any spot where another pawn is within 130 uu of its line of sight to the subject.
2. **Perch alignment was a missing root motion plus a wrong floor, not the clips.** The stock perch clips carry root travel: the `Root`
   bone of `Perch_BangOnWall_Start` moves +25.8 uu forward (toward the wall), `Perch_ArmsCrossed_Start` +11.1/-2.1, `Perch_PeerUnder`
   starts 3.5/-3.2; the following loop clip starts its root at zero again. In the game the pawn is snapped onto the node and the travel
   walks it to where it was found (observed pawn minus node: 24.1 uu at Perch_66 against the clip's 25.8, 14.6 at Perch_186, 19.9 at
   Perch_4). Round 1 started the pawn at the observed point and then showed the start clip's own root travel on top, and snapped the
   next clip back, so pawns hung away from walls. Now an idle pawn starts at the node (x, y, yaw) and the actor takes each clip's root travel when a clip ends (`root_end` per role in
   the manifest, computed from the converted tracks). Height: the visibility floor trace differed from the real pawn height by more than 10 uu (-128 to +75) on
   19 of 33 pawns (it hit counters, steps, the ground below); idle and held pawns now keep the **observed** height (`fixed_z`), walkers still trace. This is what cut the
   lower legs off at stop 2 (the feet were below the counter-side floor) and floated stop 5.
3. **Heads, hair, hats, gear.** A live pawn's mesh component carries its attachments (static meshes on the `Head`, `Jaw` or `Spine3` bone)
   and its two materials are per-pawn clones whose parent and texture overrides are readable (`amb_compose`). 33 live citizens carried
   88 attachments: 20 distinct static meshes (`GenericMale_Hair_01..04`, `GenericFemaleHairstyle_01..04`, `MaleGear1/4/5`, `FemaleGear1/2`, `Hardhat`, `PrisonerMask`,
   `Scarf`, `SheriffHat`, `ScooterCap`, `BanditPsychoMohawk`, the Marauder pack on 17 of them) and 8 head textures (male 01-05, female 01-03). All are exported with UModel (glTF, PNG),
   imported as static meshes and material instances (`Characters/Ambient/Attachments`), attached to the bone of the live pawn with the transform
   that carries the UE3 bone frame to the imported bone (identity within 2e-4 for Head, Jaw, Spine3; UNVERIFIED by the eye except where a frame shows a head). Hair gets
   a tint: the pawn's zone-A midtone times 1.5 (UNVERIFIED stand-in for the original's three-zone colour from `p_Masks`, whose shader was not read).
   Body variants are not done: the body atlas holds several garments (white tee, tan vest, grey vest) and the live body clone's `p_HidePart` / `p_MuscleFat`
   vectors pick which are shown; how they act on the mesh is unknown.
4. **Shading and outline.** Marcus, the dummy and the citizens all use the same minimal `M_OW_NPC` (Diffuse, Normal, roughness 0.7, no ink line); the only ink line
   in the project is Maya's inverted-hull material (`host/ue5/import_character_menu_look.py`, `M_OW_CharacterOutline`, not present in this worktree's Content).
   The citizens now use that same recipe (`M_OW_AmbientOutline`, same nodes, `ThicknessCm` 0.5) on a leader-pose copy of the body, and Maya's matte
   constants (specular 0.15, roughness 0.85) in `M_OW_NPC_Tint`. This is an art-direction approximation, as it is for Maya: the original shader is not read.
5. **Female pawns do use the female mesh.** The female kind's manifest mesh is `.../CitizenFemale/Meshes/Skel_GenericFemale/...` (extent 16.9 x 73.7 x 93.1 against 18.1 x 74.9 x 93.1),
   its materials `MI_GenericFemaleHead_Mati` / `MI_GenericFemaleBody_Mati`, and the host spawn log names the kind per pawn; the round-1 impression came from the bald head, the
   shared outfit and the grey colour path. Female head textures 01-03 and `GenericFemaleHairstyle_01..04` are now applied.
6. **Resistance patrols** and matched real-game close-ups: not done (Resistance needs the zone shading and their attachments; the matched close-up needs a population that stays put).

Real-game capture used: 33 citizens (20 male + 13 female in the third session, 16 + 17 in the first); the population differs between sessions, so the review stops are
the round-1 perches, filled with a pawn of the round-1 kind at the first capture's pose when nobody stands there (looks borrowed from an observed pawn).
