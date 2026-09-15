# Unlit host color routing, 2026-09-14

AI-assisted continuation in `t3code-dc91995f`.

The host previously selected Unlit shading from `MLM_Unlit` but connected
recovered diffuse textures and constant fallback colors only to Base Color.
UE Unlit displays Emissive Color. The importer now also connects that color
to Emissive when there is no explicit emissive texture. An explicit emissive
channel keeps its existing RGB-times-alpha policy, regardless of dictionary
order. Opacity behavior is unchanged. This remains a Material v1 approximation.

Reference: Epic's [shading model documentation](https://dev.epicgames.com/documentation/unreal-engine/shading-models-in-unreal-engine).
No external implementation code was used.

The installed `Sanctuary_Light` material
`Prop_Skybox.Materials.Mati_Sky_Dynamic_INST` reproduces this condition: Unlit,
a recovered `Sky_TransitionBL2Default_Dif` diffuse, and no explicit emissive.
That establishes relevance to Sanctuary, not recovery of time-of-day shading,
clouds, masks, sky UVs, or the native sky composition.

## Resource investigation boundary

The previous Ash/Sanctuary diagnostic census contains 495 native records.
Testing the hypothesis that each record's second word directly indexes the
resource texture list produces 13 out-of-range values. The observed sky
records also contain repeated indices with differing candidate float pairs.
These direct package observations do not justify applying those records as
rendering UV assignments. No guessed field semantics entered the importer.

## Verification

The synthetic material fixture now includes six Unlit cases: diffuse-only,
null emissive, constant color, neutral fallback, and explicit emissive before
and after diffuse in dictionary order. The saved-scene verifier checks visible
color connections, texture identity, constant values, shading model and
explicit emissive precedence after reopening the assets.

Reproduce the synthetic import and saved-scene checks:

```powershell
python tests/prepare_ue_smoke.py
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' -Scene local/material-smoke -ImportOnly
```

Automated package checks: CTest 5/5 passed; all nine `verify_packages.py`
comparisons passed. Level tests: 15 passed; resource tests: 4 passed.
UE5.8 host build succeeded. Synthetic import, saved-scene reopen and saved UV
verification all succeeded with zero errors/warnings. All six Unlit material
cases passed; the existing diffuse/normal/specular/emissive checks also passed.

The actual Sanctuary sky material was freshly prepared from Sanctuary_Light
and applied to a synthetic triangle probe in a separate `SanctuaryUnlitProbe`
map. Its import, reopen and saved UV checks also passed with zero errors or
warnings. The reopened Emissive input references its recovered diffuse texture.
This probe tests the actual material definition without claiming to test the
native dome, its placement or sky UV selection.

Local evidence: `local/material-smoke/ue-verify.json`,
`local/material-smoke-run.log`, `local/sanctuary-unlit-probe/ue-verify.json`,
and `local/sanctuary-unlit-probe.log`. The local probe preparation script is
`local/prepare_sanctuary_unlit_probe.py`; all game-derived outputs remain ignored.

No full Sanctuary reimport, rendered comparison, original-game comparison,
or physical-input validation is established by these checks.
