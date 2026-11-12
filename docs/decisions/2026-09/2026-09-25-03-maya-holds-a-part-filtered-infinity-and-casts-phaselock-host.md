## 2026-09-25: Maya holds a part-filtered Infinity and casts Phaselock (host prototype)

The Infinity visual is no longer the whole pistol gestalt. `ow-package
--properties` with a local array schema (element types only, no parser change)
decodes `Weap_Pistol.GestaltDef_Pistol.GestaltInfos[0].Parts`: 59 fragments,
each a `SkeletalMeshFragmentName`, `MaterialIndex`, `FirstIndex` and
`NumPrimitives`. Their per-material triangle totals (22,316 and 538) equal the
two UModel glTF primitives exactly, which supports the assumption that UModel
writes sections in material order. `tools/filter_gestalt_gltf.py` checks that
equality and keeps only named fragments.

Part choice: `Pistol_Vladof_5_Infinity` uses `EPRM_Selective` over base
`Pistol_Vladof_4_VeryRare`. Its barrel `Pistol_Barrel_Vladof_Infinity` maps to
gestalt fragment `Pistol_Barrel_Vladof`; the base body `Pistol_Body_Vladof_4`
maps to `Pistol_Body_Vladof`. The base sight list spans eight manufacturers and
the grip comes from data not traced here, so `Pistol_Scope_Vladof` and
`Pistol_Grip_Vladof` are one plausible roll, not the only one. Kept: 1,505
triangles. The UE mesh bounds still count the unused gestalt vertices.

Material: `M_OW_InfinityApprox` is an UNVERIFIED stand-in for the stripped
`Master_Gun` graph. `p_Masks` R/G/B select regions A/B/C. A `p_Diffuse` channel
lerps each region's shadow/midtone/highlight colors from the MIC. The pistol
detail is read from blue, inferred from the texture name
`Weap_LauncherShotgunPistol_Comp`. `p_HighlightsIntensity`,
`p_ShadowsIntensity`, the pattern and the decal are not used. The white body
with navy detail follows the MIC colors; it is not compared with a game capture.

Pose: the mesh attaches to `R_Weapon_Bone` with a 90 degree yaw, an observed
fit. A logged barrel axis (`WeaponOffset` to `Barrel` bone) is 0.89 forward
in view; its 0.45 up component matches the bones' 6.5 cm height difference.
`ADD_Fire_Recoil` has identity tracks at frame 0 on all 47 bones, so it is a
UE3 additive clip. The arms instance now layers it as clip(t) relative to
clip(0) instead of blending it as a full pose, which had collapsed the arm.
Its 29.1429 fps source imports at 30 fps (logged). A broken earlier import of
that clip crashed any load in animation compression; the generated asset was
deleted and re-imported.

Gameplay, all host prototypes: hitscan at 10 Hz after a 0.8 s spin-up, the
installed `SpinUpDuration`. The fire rate is not evaluated. A deterministic
figure-eight stands in for the undecoded `FiringPatternLines`. Damage is a
placeholder 87 per shot. Tracers, the muzzle flash, impact sparks and bullet
decals are engine shapes with host materials, not `FX_WEP_Pistol` particles.
The muzzle point is estimated from the barrel's gestalt bounds, 27 cm ahead
of the `Barrel` bone. Phaselock sweeps a 30 cm sphere. The host-made training
dummy lifts in the installed 0.7 s over an estimated 170 cm, hovers in a
violet shell and falls back at -500 cm/s^2. A violet beam runs from Maya's
`L_Hand` bone during `Phase_Lock_Lift`. A point light on the shell pooled
violet on the road under Lumen and was removed. The duration (5.5 s) and
cooldown (13 s) remain unevaluated. The HUD follows BL2's layout with canvas
shapes; it is not BL2's Scaleform.

`-owcombatshots` (with `-owwalk -owmaya -owcombattest`) runs an unattended
aim/fire/Phaselock sequence and writes five `OWCombat_*.png` captures.

Review: an independent reviewer agent scored the five captures from BL2
feel, not from data: 1/10 before this pass, then 4/10 and 4/10. Its open
items: the gun sits high and tilted in the view, the material is flat
white, the tracer and figure-eight spread are hard to see in stills, and the
Phaselock shell and beam look generic. It also found the dummy and HUD
placeholder-grade and saw arcs in the sky. The gun pose follows BL2's pistol
Idle clip on `R_Weapon_Bone`; the view has not been compared with a matched
game capture, so a socket or axis error is not ruled out. This is a working
prototype, not visual parity.

Automated: host build; import commandlets exit 0. Runtime: the unattended
sequence logs 4 target hits, Phaselock activation and release about 5.5 s
later. Visual: the captures were reviewed by eye and by the reviewer agent.
None was compared with the original game.

Follow-up after a maintainer play test (same day):

- Key 1 did not re-equip the Infinity. The map selector's controller
  bindings for 1-9 consumed the key before the pawn's `OWEquipInfinity`
  action. Those bindings no longer consume input. Pressing 1 with the map
  menu open now does both, which is harmless because the map changes.
- Phaselock timing is now read from `GD_Siren_Streaming_SF.upk`.
  `ActionSkill_Phaselock` (`LiftActionSkill`) has LiftDuration 0.7,
  LockFadeOutTime 1.1 and LiftSnapTimePct/HeightPct 0.5. Its
  LockDurationFormula is `Att_Phaselock_Duration`, base 5, scaled by
  `PhaselockTimeScale`, default 1. The host lock is now 5 s instead of the
  5.5 s placeholder, and the shell fades over the last 1.1 s. `Startup.upk`
  `Cooldown_Phaselock` resolves to a constant 13, which confirms the host
  cooldown. Skill-tree and class-mod modifiers are not applied. The lift
  snap is not modelled and the 170 cm lift height is still an estimate. The
  skill also names BL2's hand-orb, enemy-bubble and point-light effects; those
  are not hosted.
- Infinity material: `Pattern_Infiniti` is a 256x4 color ramp (black, navy,
  purple, pink, cream) that the first version ignored. It is now sampled
  through UV1. `p_PatternScalePosition` is read as UV1 scale (-1.4429, 30)
  and offset (0.3671, 0.03). It is weighted into regions A and B by
  `p_PatternChannelScale` (0.85, 1), tinted by `p_PatternColor` and shaded
  by the detail channel. Every one of those parameter meanings is an
  UNVERIFIED guess at the stripped `Master_Gun` graph. The gun now shows a
  multicolor gradient instead of flat white; it has not been compared with
  the game.

