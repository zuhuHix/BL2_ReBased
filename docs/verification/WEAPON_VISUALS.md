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

- The game's weapon material instance carried exactly one parameter beyond the chain: `p_EmissiveColor` (4.02, 0.27, 0.0)
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
- Muzzle flash: the host uses a sphere at an estimated point 27 cm past the `Barrel` bone; the game uses the type's `Muzzle`
  socket, an orange/yellow particle flash for 0.33 s and ejected shell casings. Not changed. The emissive glow after shots is wired
  (Lane E's rule) but not captured.
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
right hue (real about 150/140/120, host 100-119/95-114/80-95 sRGB); the metal cells match. Not tuned per gun.

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

- **SMG flash at the rear of the gun.** The SMG's (and the shotguns') cooked muzzle socket hangs off `Root` with a -Y offset
  (`SMG_Barrel_Maliwan_Muzzle` at about (0, -27, 12)), while the pistol and AR sockets hang off `Barrel` with the offset on X. The host draws the
  SMG flash about 27 cm behind the receiver, which fits a Y mirror between the cooked bone frame and the imported mesh's `Root`. Suspected,
  not tested; the eye sockets are on `Root` too, so a fix must recheck the ADS fit (lateral alignment does not depend on that sign, depth does).
- **SMG hip size:** about 370 px wide against 242 px in the real idle frame with scale 1.0 and the correct arms set; cause not found.
- **Reload mid-poses** of the AR (fills too much of the view) and SMG (hand placement, cant) still differ although both clips start together.
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
