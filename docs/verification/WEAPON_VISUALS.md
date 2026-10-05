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
