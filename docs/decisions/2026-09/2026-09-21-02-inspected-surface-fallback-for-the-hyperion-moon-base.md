## 2026-09-21: inspected surface fallback for the Hyperion moon base

The station in front of the moon (`Prop_MoonBase.Mesh.MoonBase02`, placed
material `Mati_MoonBase_02a`) imported as the neutral gray Lit fallback and
read as a black silhouette against the Unlit sky dome. The package explains
why: the base `Mat_MoonBase_02a` keeps no texture parameters, only tint,
fog, rim and emissive-multiplier constants, and its cooked texture list
holds `MoonBase02a_Dif/_Nrm/_Emis` plus a `Tiling_SmokePanner2_Dif` overlay.
The second `_Dif` defeats the sole-`_Dif` rule, which then refuses to guess.

`INSPECTED_COLOR_FALLBACKS` gains an entry for that placed instance naming
the hull's own `_Dif` as color, `_Nrm` as normal and `_Emis` as emissive
(`moon_base_color_fallback_v1`, `partial_unverified`). The table now allows
an entry keyed by a placed instance with an explicit required `base`
(refused with a recorded issue when the parent differs) and an optional
emissive texture. `MoonBase_Color`, `Emissive_Mult`, `Fog`/`Fog_Intensity`,
`RimLight_Color`, the smoke overlay and UV modulation are recorded as
omitted; the station is expected to read darker and less blue than the
original. Verified: exactly one manifest material changed; import, saved
scene, collision and UV verification pass; three game-viewer captures show
the plated hull, lens and lights. Not verified: the stripped graph, the
tint/fog combine, or parity with the original.

Same day, the moon behind it: `Mati_Moon` over the Unlit additive `Mat_Moon`
had already resolved `Moon_Dif`, but emitting that gray texture at 1x reads
as a pale smudge against the dome. The package keeps `p_moonColor`
(an HDR neutral grey) among `Mat_Moon`'s nine surviving named parameters, and
for an additive Unlit surface a color scale on the emitted value is the one
term whose meaning is not in doubt. A second table,
`INSPECTED_UNLIT_COLOR_MULTIPLIERS` (`unlit_color_multiplier_v1`,
`partial_unverified`), keyed by that placed instance with a required base,
reads the named vector through the chain (instance overrides last), refuses
a non-Unlit chain, a different base, or a missing/non-finite/negative value
with a recorded issue, and writes `unlit_color_multiplier` into the material
record. The host multiplies the recovered Unlit color by that constant before
the emissive input, on the existing Unlit branch only; `verify_level.py`
checks the `Multiply`/`Constant3Vector` pair against the recorded value and
reports `verified_unlit_multiplier_materials`. Recorded as omitted: the
`MoonBase02_GRP` H-shaped station shadow mask, `p_moonTimeBaseShadow`,
`Moon_Comp` relief, `p_Basecolor2`, `p_DarkColor`, the `Transition_Track`
time-of-day tint (with `Time_of_Day` and
`Horizion_track_color_multiplier`) and `p_moonTime`/`p_moonRotation` UV
motion. Verified: one manifest material changed; import and all saved-scene
verifiers pass with the new check; the same three captures show a bright
cratered disc instead of the wash. Not verified: the moon's absolute
brightness against the original, and the station shadow's placement on the
moon, which is deliberately not guessed until an in-game reference
screenshot exists. See
[MOON_BASE_SURFACE.md](docs/verification/MOON_BASE_SURFACE.md).

