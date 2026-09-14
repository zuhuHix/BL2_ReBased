# UV and winding investigation — 2026-09-14

All assets and screenshots referenced here remain in ignored local folders.

## Numeric UV checks

`host/ue5/verify_uv.py` compares saved UE LOD0 vertex-instance positions and
UV0 bindings with prepared OBJ corners. It accounts for OBJ V conversion,
checks every imported corner against the expected bindings and requires every
source binding to appear. It also compares oriented triangles, ignoring cyclic
start vertex and triangle-list ordering but retaining winding and multiplicity.
UV tolerance is 0.00001. Material graph UV operations
and original-game parity are outside this check.

Southpaw Factory passes for 185 sections / 213,297 corners. The material
fixture passes for one section / three corners. A separate four-corner color
fixture passes for one section / six corners. Its initial captured frame shows
red top-left, green top-right, blue bottom-left and yellow bottom-right.
This rules out an extra U/V flip in those export/import paths.

## Sign investigation

The recorded Sanctuary start screenshot in checkout `t3code-9935cb53` shows
the Scooter sign mirrored. Current-install extraction identifies
`prop_signs.ScootersGarageSign` at Sanctuary export 10034, with 826 vertices,
656 triangles and two UV sets. Its diffuse texture is export 13988,
1024x512 DXT1, streamed from `Textures.tfc`. The atlas contains two sign faces.
The material's 24 expression references are null in the cooked properties;
no graph behavior is inferred from them.

All 656 source triangles have geometric cross products opposite the stored
vertex normals. The earlier synthetic fixtures used the opposite convention.
The host adapter reflected Y and also reversed triangle indices. Removing the
extra reversal preserves game index order through UE's importer. A newly
rendered isolated sign at the recorded Sanctuary placement now reads
"Scooter's" correctly, with the same source texture and UVs. The mirrored sign
was a back-face/winding error, not a reason to flip texture coordinates.

A new synthetic regression requires the adapted game's index order to produce
an outward standard-OBJ face. The isolated sign passes saved geometry, UV and
winding verification (1,968 corners / 656 triangles), and its runtime test
passes. The pre-fix Southpaw map is a negative control: the new verifier fails
on `triangle winding/topology differs from source OBJ`. Corrected Southpaw
verification passes for 185 sections, 213,297 UV corners and 71,099 oriented
triangles. Its runtime movement/camera test passes again. The new start frame
shows wall/ceiling faces absent from the old capture; large floor gaps and
fallback surfaces still remain. This is not a matched original-game comparison.

Corrected Southpaw runtime sample: mean frame 46.96 ms, p95 49.30 ms,
physical process memory 3,175.7 MB; not a controlled benchmark. Evidence:
`local/southpawfactory/winding-import.log`, `winding-negative-control.log`,
`winding-viewer-test.log`, and `viewer-1f0f741992114028b6aec5f717d34788.log`.
The reviewed corrected start capture ends in `start00001.png`; the sign probe
capture is `ScooterProbe_P_start00000.png` in the host's ignored screenshots.

Synthetic fixture triangle order now matches the game convention. The earlier
fixture's opposite convention had hidden this regression; its prior color
orientation result did not establish correct game face visibility.
Both corrected synthetic scenes now pass saved geometry, UV and winding checks.
The near-vertical/negative-scale material fixture retains both actors after
reopening. The corrected color fixture's runtime test passes and its new start
capture (`UVSmoke_P_start00001.png`) retains all four expected corner colors.

Local evidence: `local/southpawfactory/ue-uv-verify.json`,
`local/material-smoke/ue-uv-verify.json`, `local/uv-smoke/ue-uv-verify.json`,
`local/scooter-probe/records.json`, `diffuse.png`, `sign.obj` and
`ue-uv-verify.json`. These files are not distributed.
