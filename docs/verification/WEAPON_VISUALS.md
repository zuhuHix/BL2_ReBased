# Weapon visuals: what the running game showed and what the host does about it (lane C, 2026-10-04)

AI-assisted (Claude). Observations of the original game (Steam, 32-bit DX9, `-windowed` 1280x720) were made with the
SDK driver in `tools/real_game/`; everything the session recorded (screenshots, runtime dumps) stays under ignored
`local/realgame/orchC/`, and this file holds only rules, counts and paths into `local/`. Every host reading below stays
`UNVERIFIED` unless a line says how it was confirmed. Nothing here is a visual-parity claim: the matched frames
are listed in `local/orch/C/review_request_1.md` for an independent critic.

## 1. What was captured

One session (saves backed up first, `block_saves()` active, spawned weapons in memory only, driver removed afterwards,
the 22 save files byte-identical to the backup after the one `.bak` the game rewrites at start-up was restored).
Maya level 8 at Plan B. Six guns were spawned from exact parts: the host's Infinity (`infinity_3`, Vladof legendary
pistol), the Maliwan fire mission pistol, the Jakobs common pistol, a Hyperion SMG, a Jakobs assault rifle and a Bandit
shotgun (parts: the slice recipes in `local/items/slice` and `local/items/infinity_3.json`).

- First person: one frame per gun in the Crimson Raiders HQ, `local/realgame/orchC/fp_<id>_a.png`.
- Inspect view (inventory, F): `local/realgame/orchC/insp_<torment|widowmaker|smg|infinity|ar|shotgun>.png`.
- Fire burst of the Infinity and the Maliwan pistol, 40 ms frames: `local/realgame/orchC/burst_infinity`, `burst_torment`
  (an orange muzzle flash with a yellow core on the first frames, and bright orange shell casings flying out to the right).
- Runtime dumps: `gun_runtime.json` (mesh, anim set, material instance and its parent and parameters per gun),
  `gun_fragments.json` (the gestalt fragment names and triangle ranges of each live mesh), `gun_part_fragments.json`
  (what each part definition names).
- Camera: `PlayerController.FOVAngle` read 77.55 with the config's `FOVAngle=90`, `ForegroundFOV` 45 with `bForegroundFOV` true
  (the arms and gun use their own foreground FOV). Auto exposure is not used by the host (`r.DefaultFeature.AutoExposure=False`).
- Camera motion blur (2026-11-06): the host's config had no blur setting, so UE's default applied (`r.DefaultFeature.MotionBlur` defaults to 1, and the Epic scalability group gives
  `r.MotionBlurQuality` 4), and the first playtest's straight sprint showed heavy blur on the ground and the gun. The earlier fixes only turned off per-bone blur on two meshes, and two test paths
  (`OpenWillowQuest`, the Phaselock shots) set `r.MotionBlurQuality 0` at run time, which hid it from those captures. `DefaultEngine.ini` now sets `r.DefaultFeature.MotionBlur=False`. The installed
  game's own `WillowEngine.ini` on the zuhu PC has `MotionBlur=False` (a config read; no frame comparison). The sprint frame in the host after this change: UNVERIFIED until the batch runs it. If it still smears, UE's
  temporal upscaler is the next suspect (the same config has `bAllowTemporalAA=False`; the host uses UE's default anti-aliasing).
- The Sanctuary lighting seen outdoors in that save is a sunset set (orange), so no outdoor frame of the same daylight as the
  host could be taken; first-person pairs differ in lighting, which the matched-pair table says.

## 2. Part assembly: the host drew the wrong fragments (confirmed in game)

The running game lists the gestalt fragments each gun draws (`GestaltData.PartMeshNames`) and the triangle ranges of its
mesh. Compared with the host's recipes:

| gun | game fragments | host before | triangles game / host before / host now |
|---|---|---|---|
| Infinity (`infinity_3`) | Body_Vladof + `_Var1` + `_Var2`, Grip_Maliwan, Barrel_Vladof, Scope_Torgue | no variants | 2585 / 1571 / 2585 |
| Maliwan fire pistol | Body_Maliwan + `_Var1`, Grip_Jakobs, Barrel_Jakobs, Acc_Barrel_Elemental2 | extra `Acc_Barrel_Elemental3`, no `_Var1` | 1929 / 1865 / 1929 |
| Jakobs pistol | Body_Jakobs, Grip_Jakobs, Barrel_Torgue | same | 2275 / 2275 / 2275 |
| Hyperion SMG | Body_Hyperion + `_Var1`, Grip, Barrel_Maliwan, Scope_Tediore, Stock_Dahl | no `_Var1` | 3892 / 3736 / 3892 |
| Jakobs assault rifle | Body, Grip_Vladof, Barrel_Vladof_Alt, Stock | extra `AR_Scope_Bandit` | 2228 / 2800 / 2228 |
| Bandit shotgun | Body, FrontGrip_Hyperion, Barrel_Jakobs, Stock_Torgue | same | 3186 / 3186 / 3186 |

Rules that reproduce all six fragment lists (`tools/weapon_recipe.part_fragments`): a part draws its
`GestaltModeSkeletalMeshName` plus the non-`None` entries of the fixed array `AdditionalGestaltModeSkeletalMeshNames`
(the body variants), and a part whose definition is named `*_None` (no sight, no elemental, no accessory) draws nothing
although its fields name a fragment. The reader reports a fixed array one element per entry with `array_index`; the old
helper kept only the last. Confirmed by the six fragment lists and six triangle totals above, which the host now meets
exactly. Not confirmed: that the `*_None` rule is the game's own test (it could be a flag), only that it holds for these
six guns.

The Jakobs pistol and the shotgun still showed alternate magazine pieces in the bind pose. The weapon type hides bones
(`BoneToHideOnMesh`, `AdditionalBoneToHideOnMesh`: MoonClip and Bullet on the Jakobs pistol type, MoonClip on the Bandit
shotgun; Lane E's note `NATIVE_WEAPON_VISUALS.md` section 3). `tools/weapon_refresh_fragments.py` removes the triangles
skinned to those bones (404 on the Jakobs pistol; none on the shotgun, whose MoonClip has no triangles in its fragments).

## 3. Colour: what the numbers said

The earlier critic reading (host 2 to 8 times too dark, Maliwan orange hue 11 degrees against 32) was largely a geometry
problem: the host's "white body" was the stacked cream drums of the wrong fragment set and its zones therefore sampled the
wrong texels. With the corrected fragments the unlit model albedo (`tools/weapon_paint_model.py`, unchanged) compared with the
real Inspect frame of the Maliwan pistol, aligned by the gun's bounding box, has a median linear-light ratio of 0.82 (p10
0.55, p90 1.34) and a median hue difference of 2.7 degrees over six saturated cells
(`tools/weapon_visual_compare.py`, `local/orch/C/cmp_test.png`). So the colour model is sound; what was missing is the light.

Runtime facts that fix the material inputs:

- The game's weapon material instance carried exactly one parameter beyond the chain: `p_EmissiveColor` (an HDR red)
  for the fire pistol (`gun_runtime.json`), equal to the cooked `Pistol_Elemental_Fire` part's `MaterialVectorParameterValues`.
  Lane E's native note says the same: only elemental parts carry any such value, linear and unconverted.
  `tools/prepare_weapon_paint.py` now applies each part's `MaterialVectorParameterValues` in the native slot order.
- The compiled pixel shader adds `p_EmissiveColor` times the blue channel of the packed normal texture
  (`Weap_Pistols_Nrm`: red and green are the normal, blue is a sparse mask) after lighting, clamped to 4 (own reading of
  the token stream, `UNVERIFIED`; the real fire pistol shows the red window and slot glow this predicts).

## 4. The host material and lighting (pass 4)

`host/ue5/import_weapon_paint.py` (`BL2_ANALYTIC_LIGHTING`) builds an Unlit material that follows the shape of the original
base pass: `clamp(albedo x D x (ambient + key N.L1 + fill N.L2)) + emissive` (the original's D is 0.4; the host uses 1.25, section 5; the clamp divides by the peak when a colour exceeds 1, so HDR zone colours keep their hue), with the normal rebuilt from the packed
texture (z scaled by 0.8 as the shader does) and the lights fixed in view space. The game takes the light values from the
pawn's light environment (a dominant directional light plus SH ambient from baked data, read through the SDK), which the host
has no equivalent of; the constants are calibrated stand-ins (`OpenWillowGunLook.*`, command line `-owgunlook=`).
Why not PBR: the pass-3 inputs (`USE_SHADER_SHADING`) rendered 4 to 7 times darker than the references under the host's
scene lighting, and Lumen/sky would still make the gun look different in every scene.

Weapon glow after shots follows Lane E's native rule (+0.25 emissive scale per shot, cap 5, decay 3.5 per second after
0.2 s); `UNVERIFIED` in game.

## 5. Calibration and the numbers (host side view against the real Inspect view)

`tools/weapon_visual_compare.py` cuts the gun out of both frames (the host frame is keyed against its black clear colour, the
real frame is boxed by hand/automatically because the item card overlaps the barrel), scales both gun boxes to the same grid
and compares cell medians: the same cell is the same part of the same mesh. Median linear-light ratio host/real (p10 to p90) and
median hue difference over saturated cells, for `local/orch/C/shots/r13` against `local/realgame/orchC/insp_*.png`:

| gun | cells | ratio | p10 to p90 | hue diff |
|---|---|---|---|---|
| Maliwan fire pistol | 16 | 0.77 | 0.39 to 1.36 | 13 deg (2 cells) |
| Jakobs pistol | 21 | 0.72 | 0.50 to 1.12 | no saturated cell |
| Infinity | 12 | 1.06 | 0.48 to 1.75 | 8 deg (6 cells) |
| Hyperion SMG | 19 | 0.93 | 0.73 to 1.30 | 3 deg (8 cells) |
| Jakobs assault rifle | 17 | 1.08 | 0.58 to 1.50 | no saturated cell |
| Bandit shotgun | 14 | 0.64 | 0.42 to 0.88 | 5 deg (10 cells) |

Before this pass (host thumbnails against the real frames, from the 2026-10-02 critic table): value ratios 0.2 to 0.5 on most
zones, Maliwan orange hue 11 degrees against 32. One run of the old path (`OWGun_*_side.png` in `r6`, material Diffuse 0.4)
gives medians 0.20 to 0.51. The calibration constant is the material's `OW_Diffuse` (0.4 in the original's own pixel shader; the
host needs 1.25 because Unreal's film tone mapper darkens the same scene-linear value, the cell ratios against `OW_Diffuse` were
0.34, 0.82 and 1.47 for 0.4, 1.0 and 2.0 on the Maliwan pistol, so the response is steeper than linear). The shotgun and the
Jakobs pistol stay a quarter to a third dark and the Maliwan barrel is bluer than the real light steel. Open, not tuned further.

What still differs on the Infinity (the first-priority gun): the real handguard is dark wine where the host is a bright magenta,
the real receiver and grip are dark teal-green where the host's grip is black with white edges, and the infinity symbol is cream
pink where the host's is orange. The Python albedo render of the same model (no lighting, no reflection term;
`local/orch/C/inf_variants.png`) shows the real teal and wine, so the paint reading is right and the difference is in the Unreal
material: the environment reflection term. With the old reflection-vector lookup the whole gun got a purple wash (a render
without the term, `local/orch/C/inf_noref.png`, removed it); it now uses a view-space reflection of the camera ray, without the
per-object offset (UNVERIFIED stand-in).

## 6. Stats of the two shotgun recipes (reported by lane D)

The slice shotgun printed reload 4.4 and magazine 9 where the game's card reads 4.1 and 10, and the turn-in shotgun 3.7 and 13
where it reads 3.5 and 14. Two causes, both measured:

1. The stored `stats` of the slice recipes came from an older evaluator (no level line, double-precision values). Recomputing on
   the cooked packages with today's evaluator gives the same 4.4 / 9 / 3.7 / 13.
2. The running game's `WT_Bandit_Shotgun` differs from the cooked decode (ClipSize 10 against 9, ReloadTime 4.1 against 4.4, and
   also FireRate, InstantHitDamage, AttributeSlotEffects, StatusEffectDamage), the documented live-versus-cooked difference
   (`REALGAME_GROUND_TRUTH.md`, "Live weapon data"; the same objects are in lane W's `runtime_changes_filtered.json`). Evaluating
   with the live overlay (`tools/real_game/live_overlay.py`) gives 4.1 / 10 and 3.5 / 14.

Confirmed in game on 2026-10-04: the Inspect card of the exact shotgun parts reads reload 4.1, magazine 10
(`local/realgame/orchC/insp_shotgun.png`). The other four slice recipes are unchanged by the overlay. `tools/weapon_refresh_stats.py`
rewrites only `stats` (and `stats_source`) of the named local recipes; it was run on the six slice recipes. Where the live values
come from is still unexplained, so this chooses a data source for the host's cards; it does not change the evaluator.

## 7. Not done / open

- Foreground FOV 45 (read from the running game's `ForegroundFOV` through the SDK, `bForegroundFOV` true) is the default of the held
  weapon and arms since round 2 (UE 5.8 first-person FOV on the arms and gun; `-owfpfov=0` restores the old world-FOV view, `-owfpfov=<n>` another
  value). The default is a host choice: the value is the game's, using it by default follows both critics' preference. Lane A's Phaselock hand
  effects are first-person primitives as well, so they stay on the hand.
- The world FOV: the game reads 77.55 where the host assumes 90 (106 degrees horizontal at 16:9); a rotation test to
  settle it (`local/orch/C/rg2.ps1`) was not run: a person was using the machine when it was due.
- Muzzle flash and casings: drawn from the cooked particle data since 2026-11-03 (section 18); the emitter textures are not
  hosted, so the quads are host stand-ins. The world FOV rotation test is still not run (section 18, live items).
- First-person lighting: the real frames were taken in the HQ under cool light and the host's in Sanctuary daylight; the gun
  look is scene-independent in the host by design (analytic lights in view space), so only the inspect pairs are lighting-matched.
- The mission pistol's host mesh in the quest is Lane B's `MaliwanPistol/SK_Pistol_Maliwan_2_Fire_seed1`, a different rolled sample
  (it does not carry this pass's fragment rules). `SliceItems/SK_slice_mission_pistol_fire` is the corrected mesh; pointing the quest at it
  needs a change in `OpenWillowQuest.cpp`/`OpenWillowSliceData.cpp` (not mine).

## 8. Round 2 (2026-10-04, after the first independent critic): causes found

The critic's colour findings (shotgun and fire pistol oversaturated, accent red instead of orange, crushed blacks, Infinity shroud
hot pink, cream symbol salmon, blown-out Jakobs cylinder) had one common cause, which sections 4 and 5 above did not find: the
Unlit material's output goes through Unreal's film tone mapper, and that curve is far from linear. Measured with the material's own
debug ramp (`OW_Debug` 3: an exponential grey ramp across the screen, read back from a 1280x720 side capture of the shotgun mesh;
`local/orch/C/tone_ramp.json`, `shots/ramp2`):

| scene-linear input | displayed (linear) |
|---|---|
| 0.07 | 0.001 |
| 0.18 | 0.03 |
| 0.35 | 0.14 |
| 1.0 | 0.51 |
| 2.7 | 0.80 |
| 5.6 | 0.93 |

The toe crushes the darks and a low channel falls much faster than a high one, which is exactly "oversaturated, crushed blacks, red where
the real is orange". The main (first-person) view shows the same curve with the input four times smaller (world camera input 0.28 gives
0.58 where the capture needs 1.13; checked at six points), a 2 EV difference between the world camera and the SceneCapture
exposure that `AutoExposureBias` and the physical-camera switch did not move.

Fix (UNVERIFIED as a stand-in, measured as a transfer curve): the material now computes the *display* colour it wants
(`albedo x D x (ambient + key + fill)` plus emissive, clipped per channel) and applies the inverse of the measured curve
(`TONE_TT`/`TONE_LV` in `host/ue5/import_weapon_paint.py`, 28 points) times `OW_ViewScale` (0.25 for the held weapon, 1 for the
inventory preview capture). `D` is back near the original's own factor, 0.5 (was 1.25 against the tone mapper). The clip is per
channel (`OW_Clip` 1): the real infinity symbol is cream where the peak-divide variant gave salmon (zone colours reach 16 there; per-channel
clipping of a value at about 2 gives cream). The grey curve is applied per channel, an approximation (the mapper also desaturates highlights).

Side view against the real Inspect view, median linear ratio host/real (p10 to p90), `local/orch/C/shots/u1`:

| gun | cells | ratio | p10 to p90 | hue diff (saturated cells) |
|---|---|---|---|---|
| Maliwan fire pistol | 25 | 1.06 | 0.79 to 1.32 | 5.1 deg (6) |
| Jakobs pistol | 27 | 0.69 | 0.47 to 1.01 | none saturated |
| Infinity | 22 | 1.01 | 0.79 to 1.79 | 3.0 deg (11) |
| Hyperion SMG | 24 | 1.16 | 0.94 to 1.39 | 2.4 deg (9) |
| Jakobs assault rifle | 19 | 1.05 | 0.81 to 1.25 | none saturated |
| Bandit shotgun | 20 | 0.86 | 0.57 to 1.03 | 2.8 deg (14) |

(round 1: 0.77, 0.72, 1.06, 0.93, 1.08, 0.64; hue 3 to 13 degrees). The Jakobs wood cells read about 0.7 of the real brightness with the
right hue (host 100-119/95-114/80-95 sRGB); the metal cells match. Not tuned per gun.

Other round-2 changes:

- **Animation sets by weapon type.** The host held every gun in the pistol clips (the SMG, rifle and shotgun looked foreshortened). The
  game's `AnimSet Anim_1st_Person.<type>` (UModel MD5 export, `tools/prepare_character_anims.py`) is now imported for the assault
  rifle, SMG and shotgun (Idle, Run_F, Sprint, Jump_*, Draw, ADD_Fire_Recoil; `Anim_<Set>_<Clip>` beside the pistol ones) and
  `SelectSlot` picks the set from the item's type, falling back to the pistol set. The rifle's clips list the same 47 bones in another order;
  the converter now looks locals up by name and composes them along the mesh's parents, byte-identical to the old result for the pistol
  clips (checked). The launcher and sniper sets are not imported.
- **The weapon attach is data.** `Char_Siren.Hands_Siren` has `SkeletalMeshSocket_0` named `Weapon` on bone `R_Weapon_Bone` with a relative
  yaw of 16384 (90 degrees) and an offset under 1 unit: exactly the host's "observed" 90 degree fit. The arms mesh and material are the
  game's own too: `Char_Siren.Hands_Siren` with `Char_Siren.Mati_Siren_Hands` (parent `Master_Player`, no parameter overrides). So the real
  black glove and the host's bare forearm and yellow sleeve are the same asset seen through a different projection: the real arms are drawn
  with `ForegroundFOV` 45, which pushes the sleeve out of frame (compare the right panels of the first-person strips).
- **Mission pistol.** The quest now holds `SliceItems/SK_slice_mission_pistol_fire` (this recipe's own parts) when that asset exists;
  Lane B's rolled sample stays the fallback (`OpenWillowQuest.cpp`, one block).
- Environment term: the reflection lookup runs on the shaded normal in view space; the tan strip at the top rear of the Infinity receiver is
  still missing (the real strip may come from the glossy environment texture hitting that surface; not found).
- Not modelled: the real game's black ink outline around parts (a post effect), so the host has no outlines.
- First person: with the foreground FOV (now the default; in each strip `local/orch/C/review2/fp_<id>.png` the middle panel is the default, the right one the old view with `-owfpfov=0`) all six guns sit at about the real scale and the pistols lean left as in the real frames
  (`local/orch/C/review2/fp_<id>.png`, right panel); long guns are held in their own clip sets. Remaining: guns about 10 percent large, the
  muzzle flash, and the lighting differing by scene. The foreground FOV is the default in round 2 (see section 7).

## 9. Round 3 (2026-10-05): the first-person scale, cant and glove had one cause

Round 2's critic found the pistols about 20 percent too large and leaning 10 degrees where the game leans 25 to 30, the dark glove
missing and the long guns off. The candidates (FOV value, arms idle pose, camera offset) were tried in the cheapest order: the FOV.
Unreal's first-person FOV is a horizontal angle on the scene's axis (`FMinimalViewInfo::CalculateFirstPersonFOVCorrectionFactor`, a ratio of
half-tangents), so the question is what the game's `ForegroundFOV` 45 is an angle of. Three values were rendered for all six guns
(`local/orch/C/shots/u2` = 45 read as horizontal at 4:3, i.e. 57.9 degrees at 16:9; `fp_52`; `fp_57.9` = 45 read as the vertical angle, i.e.
57.9 degrees at 4:3 and 72.6 degrees at 16:9) and set beside the real frames (`local/orch/C/fpvar_a.png`, `fpvar_b.png`). Only the vertical
reading matches: gun size and screen position equal the real frame's on all six guns, the pistols lean toward the centre as the real ones
do (a wider FOV leans an off-centre barrel more), the left forearm of the long guns is in frame, and the dark glove appears at the bottom
edge. The glove was not culled and no material slot was wrong: with the narrower horizontal reading the hand simply sat below the bottom
of the frame (the legacy world-FOV view shows the same black glove). The arms idle pose and the camera offset were not changed. This is
`UNVERIFIED` as a statement about the game's code (a rotation test of the real world FOV, `local/orch/C/rg2.ps1`, was never run); it is
the reading under which the frames agree. By the same reading the world `FOVAngle` 77.55 would be a vertical angle too (110 degrees
horizontal at 16:9, where the host uses 106 from its by-eye 90); the world FOV was not changed.

`-owfpfov=<vertical degrees>` took the vertical angle in round 3. Superseded by section 10: with the real placement the 45 reads as a horizontal angle.

Colour claims of round 2's critic that the numbers do not support (side view, `local/orch/C/cells.py`, cell medians; hex is sRGB):

- Fire pistol barrel and slide: real `#474c59`, `#43454f`, `#3d404d`, `#343f4c`; host `#4a505e`, `#3d424d`, `#3e444d`, `#424751`. The critic's
  "light steel #8d9aa6" is the top highlight row only (real `#6b7a83`, host `#7c8a99`). The orange area as a fraction of the squared gun height is
  0.0425 real against 0.0409 host and the mean orange run across the grip is 0.046 against 0.0435 of the gun height: the stripes are not
  twice as wide; the review composite scales the host crop up. The Infinity shroud cells match within 25 percent (real `#583742`, `#492c33`,
  `#54323e`, `#4b2e3e`, host `#5b495d`, `#482b31`, `#58343d`, `#5f3a4e`), brightest toward the receiver.
- Hyperion SMG yellow: host cells are 10 to 15 percent lighter and warmer than the real (`#d4c684` against `#f9dd92`), not duller.
- Jakobs pistol metal matches (`#b6b3b4` against `#b7b9bd`); the wood cells are about 30 percent darker in the host with the right hue
  (`#8e8371` against `#615a50`). Not tuned: one global look, no per-gun correction; the different signs on different guns point at the albedo
  reading of each material (detail channel colour space), not at the light.

## 10. Round 4 (2026-10-05): what places the long guns differently from the pistols

Round 3's blind A/B (vertical-45 against the older horizontal reading) split by weapon type: the vertical reading was better on the two
pistols, the horizontal one on the SMG, rifle and shotgun. A single FOV value was the wrong thing to vary: the host never placed the arms
the way the game does, and the placement differs by weapon type.

Read from the live game (SDK driver, 1280x720, five weapons held in turn; 2026-10-04, `local/realgame/orchC/`) and from the cooked data:

- The arms mesh origin equals the view point plus the held weapon type's `PlayerViewOffset` (forward, right, up), exact for all five weapons
  probed. Values from `WeaponTypeDefinition` in Startup.upk: Vladof infinity pistol (20, 4, 2), Jakobs pistol and the Maliwan mission pistol
  (15, 4, 2), Bandit shotgun (12.5, 4, 2), Jakobs assault rifle (12, 2, 2), Hyperion SMG (10, 3, 0). Pistols sit about 3 to 8 cm further
  forward than the long guns, which is what moved the long guns 40 to 85 px in round 3.
- `ForegroundFOV` on the player controller equals the type's `FirstPersonMeshFOV`: 45 for five of them, 50 for the SMG.
- In the idle clip the arms' `Camera` bone sits (9.42, -2.15, -0.36) from the arms origin. The host's clips pin that bone to the component
  origin, so the host adds this offset. After adding both, the host's `R_Weapon_Bone`, weapon offset and barrel positions agree with the
  live ones to about 0.1 cm (`gunprobe` log lines from `-owgunshots`).
- With the placement right, the vertical-45 reading draws every gun too small (about half the area). Sweeping the foreground FOV for all six
  guns, an effective 25 degrees vertical (about 43 to 45 horizontal at 16:9) matches size, lean, glove and the long guns' left forearm on all
  six (`local/orch/C/fp5_a.png`, `fp5_b.png`); silhouette widths (solid-white renders, `-owgundebug=4`) at 25 degrees against the
  critic-measured real boxes: Infinity 200 against 205 px, Jakobs pistol 223 against 230, Maliwan pistol 239 against 270. At 35 degrees they
  are 103, 133 and 136 px. So the 45 is a HORIZONTAL angle on the view axis, and round 3's vertical reading was a compensation for the missing
  placement. One rule fits all six: foreground FOV = the type's `FirstPersonMeshFOV` as a horizontal angle, arms placed at
  `PlayerViewOffset` plus the Camera-bone offset.
- Status: the data values and the live equality are read facts; that the engine applies them this way is `UNVERIFIED` native behaviour; that a
  45-degree horizontal foreground FOV is the right projection is confirmed only by the frames agreeing, not by a code read.

Host: `tools/weapon_view_model.py` writes `weapon_view.json` (ignored, per items folder) from the cooked weapon types; `AOpenWillowWalker::
LoadViewModels` reads it and `ApplyViewModel` (called from `SelectSlot`) sets the arms location and the foreground FOV per weapon. A recipe
without an entry gets offset 0 plus the Camera-bone offset and 45. `-owfpfov=<horizontal degrees>` forces one value for every weapon;
`0` restores the old world-FOV view and the old unoffset arms. Frames: `local/orch/C/shots/u4`, composite `fp6_a.png`.

Exposure check (the maintainer saw all-white guns): inside the gun silhouette the 90th percentile luminance of the host against the real
first-person frames is 126/116 (Infinity), 209/211 (Jakobs pistol), 230/228 (Maliwan), 219/222 (SMG), 199/213 (rifle) and 114/127 (shotgun); the
99th percentile is 5 to 25 levels higher in the host on the pale guns (243 against 233 on the Jakobs pistol). The real Jakobs metal is also
near-white (cell median `#b7b9bd`), and the real first-person frames are lit blue where the host's Sanctuary street is neutral, so a pale
metal reads whiter next to a grey street. No exposure error found; a small highlight excess on the Maliwan pistol (6 percent of pixels at
250 or more, none in the real frame) is left alone. Light constants stay `UNVERIFIED`.

## 11. Lane E round 1 host bridge (2026-10-22)

The ignored per-recipe `weapon_view.json` may overlay fields read from the local live-measurement note; those measurements take precedence over
cooked weapon-type fields when present. The note, exports and generated manifests remain under ignored `local/`.

- Right mouse is the host hold-to-aim input; F remains the Phaselock input. Aim uses the live item zoom FOV where measured and the cooked type
  value otherwise. Converting the zoom value to the host camera's aspect convention is `UNVERIFIED`. The selected eye socket centres the
  host weapon during aim; the stock view-model motion formula is `UNVERIFIED`.
- Reload uses the last unconditioned type animation until the type expression conditions are evaluated. It plays on the imported weapon
  skeleton; a separate first-person hand reload gesture is not imported in this round. Scaling clip length to the item's full `ReloadTime` is
  `UNVERIFIED`; the host has not captured the stock animation choice or playback rate.
- The two-layer host muzzle flash uses the type duration when available. The host interpretation of that duration and the 3.5 cm fallback flash
  radius are `UNVERIFIED`. Shell casing actors are enabled only when the type names a casing template; their right/up impulse is `UNVERIFIED`.
  Tracers are enabled only when the type names a tracer template; the fallback tracer size and lifetime (0.8 cm and 0.09 s) are `UNVERIFIED`.
- Recoil reads each type's directional kick channels and ADS scale. Mapping the channel average to the old additive recoil clip's 0.45 baseline
  is `UNVERIFIED`; the figure-eight spread proxy remains unchanged.

## 12. Lane E round 2 implementation and host captures (2026-10-22)

This pass reused the existing local UModel MD5 exports; no new game package or capture was added to the repository. The host owns recipe
staging, reload-clip selection, socket resolution, the procedural scope, and repeatable captures. The 16 host frames are under ignored
`local/laneE/host_r2/`; `python local/laneE/make_pairs.py r2` produced 14 pairs because the real-game reference set has no mission-pistol ADS
or reload frames.

- **Capture identity and ADS state:** the mission-lent recipe was absent from the regular demo inventory. `tools/weapon_capture_items.py`
  creates a local-only copy that admits it as a capture fixture, normalizes the SMG type key for the existing ammo resolver, and leaves source
  recipes unchanged. `RunGunShots` verifies the equipped ID before capturing, releases aim and waits for the blend before hip-fire, and samples
  the muzzle frame about 0.12 s after the final shot. The capture log records all four named IDs and weapon names; the HUD labels in the host
  images match those IDs.
- **Flash and casings:** the round orb was replaced with four camera-facing tapered rays and a yellow core, capped at 0.16 s; muzzle and casing
  meshes use UE5's first-person primitive path. Casings remain small brass cylinders launched up and right. The selected cooked muzzle/ejection
  sockets and live `MuzzleFlashDuration` supply identity and timing; shape, scale, color, socket-parent fallback and casing impulse are
  `UNVERIFIED` host choices. The AR and SMG frames show a compact plume at the forward muzzle area; casing silhouettes remain only a few pixels.
- **SMG scope:** the selected Tediore sight has a live `FirstPersonParticle` field and `ZoomedEndFOV` 25.2809. A procedural dark side mask and
  yellow ring/ticks now appear only while aiming with that sight. The mask extent, reticle geometry, and interpretation of the particle field
  are `UNVERIFIED`; the host still shows more of the gun behind the ring than the real scope frame.
- **ADS framing:** the host now passes the measured `ZoomedEndFOV` directly as UE5's horizontal camera FOV (50 pistol, 45 rifle, 25.2809 SMG),
  replacing the previous 4:3 conversion. Base foreground FOV and arm placement continue to use the live `FirstPersonMeshFOV` and
  `PlayerViewOffset`. Selected cooked eye-socket transforms drive aim translation; if the imported mesh omits that socket's parent bone, the host
  composes it against `Root`. The horizontal-FOV reading and parent fallback are `UNVERIFIED`. The round-2 AR sight still sits low and small versus
  the real ADS frame, and the pistol remains left of its reference; these offsets need a better verified view-model rule.
- **Reload:** clip choice now compares the recipe's base card `ReloadTime` with the live `WeaponMaxFastReloadAnimTime` (1.5 pistol, 1.1667 Maliwan,
  2.0 AR, 2.5 SMG). The host selected the normal Jakobs/Maliwan clips for the first three items and `Reload_Hyperion_Fast` for the SMG. The
  sequences are time-scaled to the recipe's base card time and were captured at normalized fractions 0.501-0.502, independent of the live skill
  that shortened the original game's held reloads. Clip-condition interpretation and the length/card-time rule are `UNVERIFIED`. The weapon
  clip plays on the gun mesh only; the host lacks the matching first-person arm reload, so the pistol and long-gun midpoint poses still differ.
- **Arms and materials:** no arm mesh/material change was made. The imported Siren hands still show a narrower forearm and black glove than the
  real frames. Weapon material values were left alone; several host surfaces still read pale under the Sanctuary lighting.

Host evidence: `local/laneE/host_r2/` (16 PNGs), `local/laneE/pairs_r2/` (14 JPGs), and `local/laneE/host_r2/logs/capture-final-sockets.log`.
The capture log records four verified IDs, four reload starts, four normalized midpoint captures, no stale-item warning, and no ADS-release
warning. This is host runtime and visual validation against the existing real-game reference frames; no fresh original-game runtime validation
was performed in this pass.

## 13. Lane E round 3 implementation and host captures (2026-10-22)

The host owns the recipe fits, arms animation import, ADS positioning, scope drawing, and shot presentation. Cooked weapon and socket fields
continue to supply the selected weapon type, socket identity, zoom FOV, reload clip choice, and clip duration. All exported animation payloads,
manifests, captures, and pairs remain under ignored `local/`.

- **Extraction benchmark:** UModel / UE Viewer build 1590 (compiled 2022-12-23; source and downloads: [gildor.org/projects/umodel](https://www.gildor.org/en/projects/umodel)) was run against the local BL2 install using `umodel.exe -path=<BL2 CookedPCConsole> -game=border -export -md5 -out=local/laneE/umodel-arms-reloads-r3 GD_Siren_Streaming_SF 1st_Person_<Pistol|AssaultRifle|SMG> AnimSet`. Elapsed times for the three family exports were 0.465 s, 0.749 s, and 0.490 s; they produced 118 MD5Anim files. There were no export failures or unsupported entries; 42 animation names repeat across family sets, with no duplicate selected name within a family. Output is `local/laneE/umodel-arms-reloads-r3/`. The selected weapon and arms sequences were converted onto their respective host rigs and imported by the UE pipeline; import logs had no errors (only UE Python deprecation/reference warnings).
- **1. ADS alignment:** selected-fragment socket exports provide candidate socket identity and transform; the reference frames set the visual target. Cooked sockets selected by the host are `EyeSocket2` on the Jakobs and Maliwan pistol bodies, `EyeSocket2` on the Jakobs AR body, and `EyeSocket1` on the Tediore SMG scope. The host moves the selected socket to a point 18 px below the reticle on the 568 px reference panel. Final frames put the visible pistol and AR sight lines near the reference's 10–25 px-below-crosshair range. The target and per-recipe `zoom_position_fit_cm` are `UNVERIFIED` fits; the Jakobs pistol retains an extra 11 cm up / 3.2 cm lateral camera-local correction because its body socket does not coincide with the visible sight line. Live `ViewModelRotationOriginOffset` data exists (150, 0, 0 for the measured types), but its use in native zoom placement remains unconfirmed. `Sight_Idle` was inspected but showed negligible pose change after conversion and was not used.
- **2. Arms reload:** the selected normal/fast weapon clips now play with the corresponding first-person arms clips at a shared rate, captured at normalized fraction 0.500–0.502. The arm clip names and source rates are cooked/converted data; applying them to Maya's host rig and synchronizing to the weapon clip is a host interpretation. Selected clips were `Reload_Jakobs` and `Reload_Maliwan` for pistols, `Reload_Jakobs` for the AR, and `Reload_Hyperion_Fast` for the SMG. Midpoint poses now include the arm gesture; native blend and rate semantics remain `UNVERIFIED`.
- **3. Hip scale:** host recipe scale fits are 0.75 for the AR and 0.54 for the SMG; pistol and Maliwan pistol remain 1.0. These are `UNVERIFIED` fits to idle-frame dimensions, not cooked type fields. The SMG fit uses the supplied 205/380 pixel-width ratio; the AR was reduced because its round-2 frame read too wide. The SMG and AR remain somewhat small in the final side-by-side pairs, so these values are visual approximations rather than confirmed scale rules. Cooked/live `FirstPersonMeshFOV` and `PlayerViewOffset` still apply in hip mode; the aim transition only changes arms location/FOV and returns to the hip base on release.
- **4. Muzzle flash:** capture now occurs 30–37 ms after the last shot. A host eight-ray starburst, warm core and short point light replace the tiny glint; the Fire pistol receives a red-orange tint. The muzzle socket and live duration are data-backed; shape, dimensions, light strength, tint mapping, lifetime cap (0.12 s) and the 1.35 Fire size multiplier are `UNVERIFIED` fits. The result is visible in the captured firing frames but remains a simple host effect, not the native particle system.
- **5. SMG scope:** at full scope blend the host hides the weapon and arms, retaining the dark side mask and adding a yellow ring, arrows, interrupted inner arcs, and lower rangefinder marks. Scope selection is cooked/live data; threshold, mask, and reticle geometry are procedural `UNVERIFIED` fits. The comparison is close, though the reticle detail is not native particle/UI data.
- **6. Metal hue:** no material change was made. The host metal still reads more neutral than the cool blue real-game captures; the available comparisons point to scene lighting and did not reveal a clear material-only cause.

Validation is separated by type: UModel extraction and conversion completed with the counts above; host visual validation used 16 captures in `local/laneE/host_r3/` and `python local/laneE/make_pairs.py r3` produced 14 comparisons in `local/laneE/pairs_r3/` (the real reference set has no mission-pistol ADS or reload frame). The UE5 host build/import/capture run completed successfully; both weapon and arms imports completed without errors. Synthetic checks passed: `python tests/weapon_view_model_test.py` (12/12) and `python tests/weapon_reload_anims_test.py` (2/2). The real-game comparison uses previously captured reference frames; no fresh original-game runtime or gameplay parity validation was performed.

## 14. Lane E round 4 (2026-10-22)

This final pass reuses the existing UModel / UE Viewer build 1590 exports under ignored `local/`; it does not rerun extraction. The project owns manifest projection and socket resolution, host view-model math, reticle drawing, and shot presentation.

- **Hip placement and scale:** comparing `ApplyViewModel` with `917242f~1` shows the default path still uses `PlayerViewOffset + CameraBoneOffset` and `FirstPersonMeshFOV`; `UpdateAim` adds `ZoomOffset * AimAlpha`. Round-4 hip logs record `AimAlpha=0.000` and component scale `1.000` for all four guns, so no ADS offset or blend leaked into hip. The round-3 `first_person_scale_fit` fields are removed from the projection and host. Type offsets/FOV are live-measured overlays of the same cooked type fields; their application remains an UNVERIFIED native interpretation.
- **SMG idle-set mismatch:** the capture fixture changes its category from `Sub-Machine Gun` to `SMG` for ammo resolution. `SelectSlot` only recognized the display label, so the `slice_smg` fixture silently used the pistol idle set while its reload path used the SMG set. `SelectSlot` now accepts both type keys and logs the selected set; round-4 capture selects `SMG` for `slice_smg`. This fixes the host pose selection. In the 960×540 idle panels, approximate bounds are pistol host 170×225 at y≈274 vs real 180×225 at y≈276; Maliwan host 195×215 at y≈285 vs real 185×215 at y≈294; AR host 270×200 at y≈343 vs real 255×185 at y≈355; SMG host at least 370 px wide from y≈288 vs real 242×220 at y≈320. The AR returns close to the reference without a scale fit. The SMG remains too large/high in the no-scale capture; the baseline projection rule has not been confirmed against this current SMG reference.
- **Muzzle flash and casing:** a generated host-only additive material uses a noisy radial mask with six rotated/scaled quads and a warm core anchored at the selected muzzle socket. The socket and type duration are live/cooked data; shader mask, colors, sizes, light, and 0.12 s cap are UNVERIFIED fits. The AR flash reads softer than the old star; the Maliwan flash still has sharper points than its real frame. The casing is a short cylinder from the selected ejection socket; its 0.3 cm diameter, 1.2 cm length, and 0.22 s life are UNVERIFIED host values.
- **SMG scope:** the two side masks leave 580/960 of the panel width clear. The closed outer circle is replaced by four arc pieces and wider inward arrows. The aperture is measured from the real frame; reticle geometry, color, and inner marks remain UNVERIFIED fits.
- **Reload choreography:** the AR and SMG select matching weapon/arms clips (`Reload_Jakobs` / `AssaultRifle/Reload_Jakobs`, and `Reload_Hyperion_Fast` / `SMG/Reload_Hyperion_Fast`). Both start in the same `StartReload` call and receive the same `clip length / item ReloadTime` rate; the capture log records matching starts and rates (AR 0.908, SMG 1.123). The open-hand midpoint poses still differ from the real frames, so the remaining gap is clip shape/retargeting; native animation semantics are UNVERIFIED.
- **Metal and smaller items:** Jakobs metal is unchanged because the Inspect calibration has not established a safe material-only correction. The pistol ADS hold remains broad; the Maliwan paint accents remain pinker. `OpenWillowGunLook` exposes a shared emissive scalar, not a per-weapon hue control, so no hue override was added.

Validation: the UE5 Development module build succeeded; the generated material import succeeded with 0 errors and 0 warnings. Final host evidence is 16 PNGs in ignored `local/laneE/host_r4/`; `python local/laneE/make_pairs.py r4` produced 14 comparisons in ignored `local/laneE/pairs_r4/` (no mission-pistol ADS or reload reference). Synthetic checks passed: `python tests/weapon_view_model_test.py` (12/12) and `python tests/weapon_reload_anims_test.py` (2/2). UModel extraction was reused, with no extraction rerun. Visual checks compare the host captures to the existing real-game frames; there was no fresh original-game runtime or gameplay validation.

## 15. Lane E status after four rounds (2026-10-22)

Guns: the starting Jakobs pistol (`slice_pistol`), the lent Maliwan Fire pistol (`slice_mission_pistol_fire`), the Jakobs assault rifle
(`slice_assault_rifle`) and the Hyperion SMG (`slice_smg`). Real-game reference: one SDK session on 2026-10-22 (`tools/real_game/scripts/
weapon_parity_probe.py`; output, frames and the measurement note under ignored `local/realgame/laneE/`). In that session one live skill effect
doubled fire rate and damage and halved reload time on every held gun (identity not read), so the host keeps the card base values; the base
values read live equal the cards. The Fire pistol vanished from the inventory when the probe readied it (cause `UNVERIFIED`, probably mission-item
handling), so it has no live ADS or reload frames; its older idle and burst frames (`local/realgame/orchC/`) were used.

Read live and used: `ReloadCompletePercent` (ammo refills at 0.55 / 0.65 / 0.70 of the reload; matched by the refill tick), `ZoomTime` 0.2 s
(zoom-in measured 0.18-0.20 s), `ZoomedEndFOV` 50 / 45 / 25.28, `WeaponMaxFastReloadAnimTime`, the muzzle and eject sockets, `MuzzleFlashDuration`
(0.33 s in the data; the visible flash is gone 0.12-0.13 s after the last shot in the frames), no `TracerTemplate` on any of the four types.

Independent critic (a separate agent scoring only host/real pairs, 0-10): round 1 5.8, round 2 5.5 (round 1's Fire-pistol frames held the wrong
gun, so the two are not comparable), round 3 6.3, round 4 6.5 (Jakobs pistol 6.7, Maliwan 7.5, AR 6.3, SMG 6.0). The 8/10 target was not reached
in four rounds. Final pairs: `local/laneE/pairs_r4/` and the hub gallery; report `local/laneE/critic_r4.md`.

Open, most visible first:

- **SMG flash at the rear of the gun: fixed 2026-11-03 (host check; frames not yet compared with the game).** Cause confirmed:
  `WeaponSocketWorld` mirrored the cooked socket offset across Y, while the decoded-socket path (`GunSocketLocation`) and the
  mesh-space reference do not. Every decoded muzzle is at -Y in mesh space (pistol -22, AR -69, SMG -33, shotgun -93 cm) and every
  eye socket at +Y, so the barrel runs along -Y. On the `Barrel`-parented pistol and AR muzzles the mirrored component is about
  0.1 cm and did nothing; on the `Root`-parented SMG and shotgun muzzles it put the whole offset behind the gun. Gunprobe on
  pc-hummer (camera X forward, cm): SMG flash 51.0 before -> 116.9 after (decoded socket 116.9, gun root 84.3); shotgun -24.4 ->
  162.2 (decoded 162.2); pistol and AR unchanged within 0.3 cm. The mirror is removed. Firing suite after the change 14 PASS /
  0 FAIL (rockets and tracers start at the same point).
- **ADS fit after the mirror fix.** The eye sockets went through the same mirror. Two host rules changed with it, both UNVERIFIED
  against the game's zoom pose: (1) the fit projects the eye socket onto the plane through the gun's `Root` across the barrel
  axis, so only the sight's height and side offset count (the pistol's eye socket sits 83 cm along the axis, which turned the hip
  pose's yaw into about 9 cm of side error with the mirror or without it); (2) the "about 18 px below the reticle" drop is
  converted to cm at the gun body's depth (`Root`), where it is seen, not at the eye socket's (the mirror had put the pistol's at
  170 cm, which doubled the drop to about 35-40 px). Fitted offsets (right, up; cm) before -> after: SMG -18.8, 1.6 -> -18.9, 1.7;
  shotgun -18.7, -0.7 -> -18.7, 0.2; AR -16.9, 3.7 -> -16.9, 4.7; pistol -10.4, -1.2 -> -15.9, 4.7. The pistol fit does not
  converge: at full zoom its projected eye point stays 3.2 cm to the side (the same 3.2 cm as before the change), and the sight
  shows about 50 px left of and slightly above the reticle. Refit 2026-11-03 (section 18): the side term of the pistol's
  frame fit -3.2 -> -0.6 cm; the critic then put the sight within about 10 px of the real frame's.
- **SMG hip size:** about 370 px wide against 242 px in the real idle frame with scale 1.0 and the correct arms set; cause not found.
  Note (2026-11-03): that comparison used lane E's fixture SMG (a Maliwan barrel). This machine's `slice_smg` is a different roll
  (Dahl barrel, Hyperion body, Tediore scope, drawn from the pre-built `SK_slice_smg` because the Hyperion SMG type is not in the
  part library), so hip size needs the matched gun and frame before it can be judged.
- **Reload mid-poses** of the AR (fills too much of the view) and SMG (hand placement, cant) still differ although both clips start together.
  The slice SMG could not reload at all in the host until 2026-11-03: `UOpenWillowInventory::ResolveAmmoType` matched only the type
  `SMG`, while the recipes carry the game's long SMG type label (lane E's fixture had renamed it), so the gun had no ammo pool.
  The label now resolves to the SMG pool (inventory self-test check added).
  Reload clips on a fresh machine (pc-hummer, 2026-11-03; all output ignored): UModel 1590 `-game=border -export -md5` of the
  `Startup` weapon AnimSets (Pistol, AssaultRifle, SMG) and gestalt meshes into `local/laneE/umodel-*-r1`, of
  `GD_Siren_Streaming_SF` `1st_Person_<Pistol|AssaultRifle|SMG>` AnimSets (118 clips, as in section 13) and `Hands_Siren`; then
  `import_weapon_items.py` with `OPENWILLOW_WEAPON_REFERENCES=local/laneE/weapon-refs` (it rebuilds `Weapons/Items`, so the
  slice recipes are imported with the demo recipes), `tools/weapon_reload_anims.py`, and the UE imports
  `import_weapon_reload_anims.py` (per recipe and library) and `import_character_anims.py` (arms). Host check after it: the
  pistol (`Reload_Jakobs`), AR (`Reload_Jakobs`) and SMG (`Reload_Hyperion`) reloads play with their arms clips (rates 0.866 /
  0.718 / 1.852) and reach the midpoint frame; the slice shotgun has no reload clip (`tools/weapon_reload_anims.py` has no shotgun
  family) and the mission pistol is not captured (it cannot be equipped into a slot outside the quest). The pistol, AR and SMG
  now load `Items/SK_slice_*` instead of `SliceItems/SK_slice_*`; their muzzle points agree within 0.4 cm. The mid-poses still
  need the matched real frames.
- **ADS widths:** pistol about 20 percent too broad, AR about 20 percent too narrow.
- **Look:** Jakobs metal reads warmer and greyer than the real frames (different scene light; not changed without an Inspect-safe cause); the
  Maliwan accents read pinker; Maya's forearm reads narrower and paler than the real blue-white sleeve.
- **Not covered:** sound (no host gun audio), impact effects beyond the existing stand-in, recoil feel (kick channels are read; the mapping onto
  the additive clip is a fit).
- **Reload timing:** the source-labelled `ReloadCompletePercent` now transfers ammo once at its configured fraction; missing fields use `1.0` to
  preserve the old full-duration refill. The host check confirmed an early successful cast leaves the magazine empty and a late cast keeps the refill.
- **Phaselock abort:** a successful lift cancels reload before playing the cast action, and reload cleanup clears arms only while the reload clip is
  still active. The host check kept the cast clip past the old reload deadline with the test-only `-owcastrate=0.4`; failed/blocked cast behavior
  remains unconfirmed in the original game and does not abort reload in the host.

## 16. Part library and run-time assembly (2026-10-23)

The per-recipe meshes of section 2 are now the fallback: guns are assembled at run time from their part lists on one library
mesh per weapon type, with one material instance per skin. The fragment rule of section 2 and the material rule of
NATIVE_WEAPON_VISUALS.md section 1 were checked on 39 more spawned guns. See [WEAPON_PART_ASSETS.md](WEAPON_PART_ASSETS.md).

## 17. Gun sockets from the decode: the muzzle flash origin (lane I5, 2026-10-07)

Numbered 17 at the 2026-11-02 merge of main (it was section 11 on its branch; main's sections 11-16 kept their numbers).
AI-assisted (Claude). The gestalt definitions carry, per fragment, a socket mapping (the fragment's own socket name, for example
`Muzzle` or `EjectPort`, and the mangled name of that socket in the shared mesh), and the shared mesh carries the sockets themselves
(bone, bone-local location, optional rotation and scale). Our reader already decodes both exactly: 544 of 544 mappings and 541 of 544
sockets equal the live game's ([RTSE_GUN_DATA_CROSSCHECK.md](RTSE_GUN_DATA_CROSSCHECK.md) section 2). This section is the pipeline and
host use of that decode.

- **Schema.** `tools/gestalt-arrays.schema` (tracked) is the old two-line schema plus `GestaltSocketMappings`, `GestaltPartBounds` (a
  bounds struct with a nested bounds struct and vectors decodes without more lines) and `Sockets=ObjectProperty` for the mesh's socket
  list. It replaces the hand-made `local/infinity/gestalt.schema`; `tools/slice_npc_assets.py` now uses it.
- **Tool.** `tools/gestalt_sockets.py` decodes the six families the slice uses (pistol, SMG, assault rifle, shotgun, sniper, launcher),
  writes `local/gestalt/<Kind>.json` (the part table the glTF filter reads, now with bounds and mappings decoded) and
  `<Kind>.sockets.json` (per fragment: bounds and sockets by original name, with bone, location, rotation, scale and, from the UModel
  skeleton, `mesh_location`, the same point in mesh space in the reference pose). Counts agree with the record: 60, 52, 48, 40, 40 and 32
  mappings, none without a mesh socket.
- **Recipes.** `tools/weapon_slice_gear.attach_sockets` copies the sockets of the fragments a gun draws into the recipe as `sockets`
  (name order; when several drawn fragments define a name, the first is kept and the others listed under `also_from`).
  `tools/weapon_slice_gear.py --gestalt` and `tools/weapon_refresh_fragments.py` call it; `weapon_refresh_fragments.py --sockets-only`
  adds `sockets` to existing recipes and changes nothing else (fragments, meshes and hidden bones stay as they are). The muzzle always
  comes from the one barrel fragment; the eject port from the body fragment (the Maliwan pistol body has none, so the fire mission pistol
  has no `EjectPort`; the other bodies in the slice have one).
- **Host.** `AOpenWillowWalker::MuzzleLocation` returns the held gun's `Muzzle` socket (`GunSocketLocation`): the bone's world transform
  times the cooked bone-local offset. The flash and the tracer start there (the tracer 30 cm further along the shot). Shell casings are
  not spawned by the host, so `EjectPort` is only exposed (`GunSocketLocation(TEXT("EjectPort"), ...)`, logged by `-owgunshots`).
- **Fallback rule.** A gun whose recipe has no `Muzzle` socket (an older recipe, or no weapon mesh) uses the old estimate (27 cm past the
  `Barrel` bone along the view, else a point in front of the camera). The host logs `OpenWillow muzzle fallback for <id>` once per gun and
  counts fallback shots. Fallbacks seen: 0 in the quest run and the gun-shot runs below (all 13 local recipes carry a `Muzzle` socket).

What the numbers say about the old estimate: from the `Barrel` bone the decoded muzzle is 27.4 cm ahead on the Infinity's Vladof barrel
and 12.8 cm on the slice pistol's Torgue barrel (the old estimate was right for the first and about 14 cm too far for the second). The
SMG's skeleton has no `Barrel` bone at all (its muzzle socket is on the root bone), so the old code used the camera fallback for it.

**Conversion choices that are ours (UNVERIFIED against a capture of the live game).** The cooked data is in centimetres with the cooked
mesh axes. The UModel glTF is metres with y and z swapped (checked on one barrel fragment's vertices against its decoded bounds), and the
host's glTF import swaps them back, so the host's mesh space equals the cooked one. The bone-local offset is applied in the imported
bone's own frame without any axis change or scale. Two checks support this, neither is a live capture:

1. Offline, `tools/gestalt_sockets.py` computes every socket's mesh-space point from the glTF skeleton's reference pose. On all six
   families every fragment's `Muzzle` lies inside its fragment's reference-pose bounds box grown by 5 cm (the farthest is 3.3 cm past the
   front face, centred on the barrel axis).
2. In the engine, the host compares the bone-based point with that reference point once per gun and socket (log line `OpenWillow gun
   socket <id>/<name>: ... differ by x cm`). They agree to 0.00 cm for `Muzzle` and `EjectPort` on 8 guns: the slice pistol, the fire mission
   pistol (`Muzzle` only), the slice SMG, assault rifle and shotgun, and Infinity recipes 1, 3 and 5. If they ever differ by more than 1 cm the
   host uses the mesh-space point and says so. This confirms the bone frames and the axes inside the host's own import; it does not say the
   game's flash sits at that point (a live capture of the flash, or the live socket's world position through the SDK driver, would).

Not done: the socket rotation (flash direction), the alternate muzzle sockets of multi-barrel weapons (`Muzzle2` exists for the assault
rifle fragment in the slice recipe), the scope and sight sockets, the three live-versus-cooked eye-socket differences, shell casings and
a muzzle-flash particle. First-person flash placement under the foreground FOV projection is unchanged (the flash is a world actor at the
gun's world point).

## 18. Weapons parity 2 (2026-11-03): the cooked muzzle flash and casing, critic rounds

AI-assisted (Claude). Host frames on this PC (`tools/run_weapon_assembly_shots.ps1 -Items local/items/slice`, slice pistol, SMG
and assault rifle; the mission pistol cannot be equipped outside the quest) against lane E's real frames (section 15), pairs
by `local/wp2/make_pairs.py`, scored by an independent critic (Luna, a different model family) in `local/wp2/critic_r2.md`
and `critic_r4.md`. Nothing here was compared with new real-game frames: the planned live session
(`tools/real_game/weapon_parity2_capture.ps1`) stopped at a "Steam must be running" message box on this PC, so the live
items below stay open.

**Muzzle flash from the data.** The weapon type's `MuzzleFlashPSTemplates` is an `EffectCollectionDefinition` whose
`ParticleEffects` entries ({`Expression`, `ParticleEffect`}) pick a particle system per element (`WeaponDamage_Is_<X>` == 1;
the entry without an expression is the default; `Etec` systems are picked by `_Is_Amp`, slag). Pistols use the SMG
systems. `tools/weapon_muzzle_fx.py` reads each system's emitters (first LOD: required module, lifetime, size, location,
colour, alpha, axis lock, velocity) and `tools/weapon_view_model.py` writes them per element into `weapon_view.json`
(`muzzle_flash_fx`). Read values (cm in the muzzle's frame, X along the barrel; s; HDR colour):
- SMG/pistol default: side and top flame rectangles 35-48 x 18 centred 25 cm ahead, 0.034 s, an HDR orange;
  front and side glows with a modulate material; a smoke puff 0.28-0.37 s.
- Assault rifle default: side and top rectangles 85 x 45 centred 42 cm ahead, 0.055 s, a brighter HDR orange; an additive
  front sprite 48-68 cm square, 0.07 s.
- Shock: an HDR blue; incendiary: an HDR red-orange with a dark smoke. The flash is orange to yellow,
  not the host's earlier red, and each sprite lives 34-75 ms (the 0.33 s `MuzzleFlashDuration` is the component's life).
The lookup-table layout (two header floats, then chunks; a uniform distribution's chunk is a low and a high half) is fitted
to the data and `UNVERIFIED`; the values agree with what the burst frames show (gone 0.12-0.13 s after the last shot).

**Host drawing (UNVERIFIED stand-in).** `AOpenWillowShotFx::MuzzleFlashLayers` draws each emitter as one soft additive quad
in the gun's frame (barrel = the `Root` bone's -Y): `EPAL_Y` / `EPAL_Z` rectangles on the barrel's side / top plane, square
sprites facing along the barrel, smoke facing the camera. Host fits, from the critic rounds: quad size x 0.5
(`-owflashfill`), the offset along the barrel scaled with it (the flame starts at the muzzle), rectangle width x 0.6,
square additive sprites x 0.3, modulate glows as a faint tint (x 0.06), angle-fading glows by the view angle. Guns without
the data keep the older procedural plume.

**Casing.** Only the assault rifle type names a casing system (`FX_WEP_Assault.Particles.Assault_Shells`; the pistol, SMG
and shotgun types have none, so no casing is spawned for them, as in the data). Read: life 0.35 s, start velocity
x 125-225, y 100-140, z 120-175 cm/s in the socket's frame, acceleration z -650 to -400 in world space. The host uses the
life, the speed range and the gravity range; the direction stays the host's (right, up, a little forward) because the
socket rotation is not decoded.

**Critic scores** (0-10; lane E round 4 was 6.5 with other code and other captures):

| round | idle | ADS | firing | reload | overall |
|---|---|---|---|---|---|
| r2 (first data flash) | 7.0 | 6.5 | 6.1 | 5.6 | 6.3 |
| r4 (fits above, pistol ADS refit) | 7.0 | 6.8 | 6.5 | 5.6 | 6.5 |

Per gun at r4: pistol 7.1, assault rifle 6.2, SMG 6.1. AR flash 150x170 px -> 75x95 px (real about 95x80); SMG flash
55x70 px (real 60x75); pistol ADS sight 30-40 px left of the real -> within 10 px. The 8/10 target is not reached.

**Remaining, ranked by the critic (open):** AR reload framing (host gun and hand span about 480 px against 340 px);
SMG reload pose (host gun near vertical with the hand at the optic, real canted with the hand at the receiver; timing
sensitive); the pistol's firing forearm (real fills the lower right); AR ADS view 25-30 percent narrower and SMG ADS reticle
about 15 percent smaller than the real frames (both point at the zoomed foreground FOV, which was not read live: the
host keeps the hip foreground FOV during ADS); SMG hip size (roll-confounded, as before); the flash quads' noisy edges
(no hosted textures). Live items not done (blocked by the Steam dialog): the world FOV through the HUD canvas, the
particle reads in the running game, the status chance at an equal-level target, the reload mid-poses.

## 19. Slice gun look (gunlook lane, 2026-11-03/04): the part library on the second PC, the Inspect light fit

AI-assisted (Claude Opus 5.5; implementation sub-agents on Haiku 5.5; critic Luna, a different model family). Nothing here was
compared with new real-game frames; the real frames are the existing Widow Maker Inspect captures and lane E's held frames.

**The part library had never been built on this PC.** Every gun here was drawn from its pre-built per-recipe mesh (the
fallback of section 16). Rebuilt with the documented chain (WEAPON_PART_ASSETS.md sections 2-4; commands in TOOLING.md): census
904 parts / 301 skins; UModel 1590 skin export 301/301 resolvable (13.0 GB, about 9 min with 4 workers); library meshes 279
fragment slots in 6 types; skins 300/301 prepared (the BuzzAxe melee skin is unsupported, as before); UE import 6/6 meshes,
279/279 slots, 300 skins. The index was built without a live census, so the 22 Torgue parts whose `p_EmissiveColor` exists only
in the running game (DECISIONS 2026-10-23) have no glow colour here (open). Oracle on 33 guns, one per type x manufacturer
(`weapon_assembly_sample.py --sets matrix`): 33/33 pass against the cooked prediction (no live check on this PC), no fallback.

**Capture sets** (ignored `local/wpa/`): the 33-gun matrix; an elemental set (fire Bandit AR, corrosive/shock/slag Maliwan
pistols, fire Maliwan launcher: the matrix has no elemental gun); a rarity set (Jakobs pistol, Vladof AR, Uncommon/Rare/Epic
skins). `-owgunturn=<yaw>,...` (with `-owgunshots`) adds Inspect turntable frames `OWGun_<id>_turn<yaw>.png` (pitch 15).

**Inspect light fit (host choice, UNVERIFIED stand-in light).** The inventory preview capture now has its own light values
(`OpenWillowGunLook::CurrentInspect`, `-owinspectlook=D,A,K,F,E` overrides). The slice Jakobs pistol's side capture registers
with the real Inspect frame of the same gun to within about 4 px, so its luma can be compared percentile by percentile
(ignored `local/gunlook/measure_inspect.py`). Hue already agreed; the host was darker. Sweep (diffuse 0.5, emissive 1;
real minus host at the luma percentiles 5 / 25 / 50 / 75 / 95):

| ambient, key, fill | p05 | p25 | p50 | p75 | p95 |
|---|---|---|---|---|---|
| 1.6, 1.2, 0.5 (old, = the world look) | +7.8 | +21.4 | +28.0 | +25.6 | +18.1 |
| 2.2, 1.2, 0.5 | +12.4 | +11.4 | +16.2 | +10.7 | +1.0 |
| 2.6, 1.0, 0.4 | +16.3 | +7.9 | +12.5 | +6.0 | -3.9 |
| **3.2, 0.7, 0.3 (adopted)** | +16.4 | +2.7 | +6.4 | -1.5 | -10.7 |

One gun only; the thumbnails use the same capture and change with it. The darkest 5 percent stay about 16 too dark whatever the
light, so that gap is in the paint (the grip weave), not the light.

**Critic** (0-10, gun look only): round 0 (old light) mean 7.6 over six held/Inspect pairs, Inspect side 8.6; round 1 (fit)
Inspect side 8.0, turn pairs 6.6-7.7 (the real rotated frames' angles are unknown and the host turns of +-30 degrees do not
reach the real reverse view). The two Inspect side scores come from different critic runs and are not comparable; the measured
percentiles above are the evidence for the fit. The critic read the elemental set as correct (element-coloured glow part in
side and first-person views; high confidence for four of five guns, medium for the fire AR, partly hidden by its red paint)
and the six rarity skins as distinct and plausible (medium; no real rarity frames).

**Checked against the data instead of changed:** the round-0 critic's "Vladof guns lack red" (from general knowledge) does not
hold for the common skin: `Mati_VladofCommon` / `Mati_VladofCommonAR` zone colours are warm and cool greys, while the
Uncommon/Rare/Epic Vladof skins carry red decal colours (and show them in the host). Needs a real common Vladof frame to close.

**Open, most visible first:** metal highlights plateau near white on the Inspect render (host p99 luma 248 against 235) while
the wood mid-tones are darker than real: a soft shoulder in the paint material rather than one light level; the grip weave
(host median 31 against 68); the real Inspect's thin pale silhouette rim; turntable frames covering the reverse side; a
real-game reference session (Inspect and held frames of the 33 matrix guns, a dropped gun per rarity, the live Torgue glow
colours), which on this PC needs the Steam account that owns the game.
