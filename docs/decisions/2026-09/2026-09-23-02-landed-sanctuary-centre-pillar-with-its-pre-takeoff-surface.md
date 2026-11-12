## 2026-09-23: landed Sanctuary centre pillar with its pre-takeoff surface

The centre of the Sanctuary plaza is `SkeletalMeshActor_2` in
`Sanctuary_Dynamic`, using `Skel_SanctuaryCentralPillar`. The full mesh is the
tall spire seen after Sanctuary takes flight. Before that, the takeoff Matinee
has not run, and only the top of the spire shows above the plinth as an angular
monolith. `tools/prepare_sanctuary_pillar.py` bakes one frame of UModel build
1590's MD5 export of the mesh and its `Open` animation to OBJ. It places the
actor at the first world key of the `Spire` group's move track in
`Episode_8`'s `InterpData_0`, (8424, 632, 544), at the actor's 0.67 scale.
That key was read in an earlier session and is recorded in the script; it was
not re-derived for this entry. The reference state is a maintainer's own save
at *Welcome to Sanctuary* (Plan B, "Install first fuel cell") and its in-game
capture of the plaza. Neither is tracked.

The surface comes from `Master_SancSpire`. Its instances pass two textures:
`Color` (`SancSpire_Col`, a low-contrast tint in three horizontal-UV strips,
selected by `Color_UV_Scale & Offset`) and `Luminosity` (`SancSpire_Lum`,
three unrelated grayscale detail maps packed in R, G and B). The master's
`Luminosity_Channel` static component mask defaults to R. Each instance
overrides it, and the value is not in the tagged properties. We located it by
searching each instance's trailing static-parameter bytes for the mask node's
`ExpressionGUID` and reading the four flags before it: Top = G, Base = B,
Tile = R. `Emissive_Channel` uses the same pattern. Two independent checks
agree with that reading. First, Base's rasterized UV islands land on B-channel
detail, while the area outside them is B's flat filler (mean 138 outside vs 59
inside). Second, the coarse Col-strip/Lum-channel correlation is highest on the
same diagonal. Topinner samples `StubWhite_Gray` as Luminosity and carries its
detail in `SancPillarTopInner_Dif`. Our decode of `SancSpire_Lum` matches
UModel's DDS within 1/255.

What is approximated and UNVERIFIED: the cook stripped every non-parameter
node of `Master_SancSpire`, so how Color and Luminosity are combined is not
known. The tool bakes `2 x Luminosity[channel] x Color strip` in 8-bit sRGB
space per section, chosen so the roughly mid-grey tint (mean about 119) stays
near neutral. The baked sections then use the mesh's raw UVs. The emissive
channel is dropped: `p_emissive` defaults to 0 and no instance raises it (Topinner
sets it to 0 explicitly). The glow is assumed to belong to the takeoff
sequence; that is not checked. The pose is a frozen frame 0, collision is
absent, and the placement is compared by eye only. Before this change, the
host rendered the pillar with the raw Col atlas as diffuse and
`SancPillar_Emm` as emissive, giving an iridescent surface with coloured
streaks. The master's default `SancPillar_Emm` was used even though the
instances override Emissive with `SancSpire_Ems`.

Host result: the UE5 import passes its saved-scene, collision and UV
verifiers with no material compile failures. The four plaza Inspection
captures show a weathered grey-blue panelled monolith with no iridescence or
glow. Compared by eye with the maintainer's capture: the silhouette and
overall tone agree. The host shows more high-contrast grating and light panels
on the faces than the original, which could come from the unverified combine
step, the missing lighting and specular behaviour, or UV placement. That
difference has not been resolved.

UV fix (same day): the tool wrote UModel's MD5 V as-is, but OBJ V is bottom-up and
ow-package writes `1 - v` (`src/assets.cpp`). The earlier Base-channel UV-island
test only matched top-down V, which confirms the MD5 V is top-down. After flipping V,
close-up host captures show the vertical "SANCTUARY" face and the single grated
channel seen in the game close-ups; the diamond-grating "shard" look came from
sampling the texture upside down. Compared by eye only.

Open (pose not solved): close-up game captures show the same `Top` texture details
("SANCTUARY" lettering, screw hatch, grated channel), so the plaza
monolith is this pillar. But in the game it stands much taller through the
plinth hole, and its top block is tilted. No frame of the UModel-exported
`Open` / `Open_Idle` clips matches; frame 0 equals the bind pose. What was
checked: our skinning matches UModel's PSK bind pose to within 0.15 units
across all 5,811 vertices. The Y flip matches UModel's PSKX of
`SanctuaryPlatformInner` against our reader. Nothing else occupies the space
above the plinth. What is left to check: the Matinee movement and
anim-control tracks for `SkeletalMeshActor_2` (the actor is
`PHYS_Interpolating`), and whether UModel decodes this compressed animation
correctly.

