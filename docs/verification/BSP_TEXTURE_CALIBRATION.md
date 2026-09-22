# BSP texture calibration gate

This record covers the bounded calibration contribution on 2026-09-22. It
does not claim a recovered BSP texel scale or V orientation without matched
original-game and UE5 evidence. Generated captures, measurement inputs and
reports belong under ignored `local/` (or UE `Saved/`); no game-derived file
belongs in the repository.

## Facts established before calibration

- `tools/bsp_decode.py` preserves and range-checks the observed surface Base,
  TextureU and TextureV references. `tools/crosscheck_bsp_polys.py` compares
  those roles against editor Polys exports; this corroborates the axis fields,
  not their render scale.
- `tools/prepare_bsp.py` retains the existing 128 world-unit divisor as
  `UNVERIFIED`. Its historical OBJ V flip is now selectable as `--v-orientation
  flip|same`, and both choices remain `UNVERIFIED` until the gate below passes.
- UModel / UE Viewer build 1590 is an external local reference tool recorded in
  `docs/verification/EXTERNAL_TOOL_BENCHMARK.md` and `THIRD_PARTY.md`. Its
  tested exports do not include BSP `ModelComponent` surfaces, so UModel output
  cannot calibrate this value.

## Reproducible acceptance tool

`tools/calibrate_bsp_uv.py` consumes a local JSON measurement file and writes a
JSON report. Each measurement must include:

```json
{
  "id": "unique measurement",
  "surface": {
    "level": "Sanctuary_P",
    "model": "TheWorld.PersistentLevel.Model_3",
    "component": "TheWorld.PersistentLevel.ModelComponent_0",
    "node": 23,
    "material": "<resolved source material identity>"
  },
  "independence_group": "surface-or-view-group",
  "axis": "u",
  "axis_vector": [1.0, 0.0, 0.0],
  "anchors": {
    "a": {"world_cm": [0, 0, 0], "pixel": [10, 20]},
    "b": {"world_cm": [256, 0, 0], "pixel": [110, 20]}
  },
  "captures": {
    "original_game": "original/<capture>.png",
    "ue5": "ue5/<capture>.png"
  },
  "original_repeat_count": 4.0,
  "original_progress_sign": -1,
  "host_progress_sign_by_orientation": {"same": -1, "flipped": 1}
}
```

World anchors must agree between the two views and each capture must have a
nonzero screen-space anchor delta. The scale sweep compares projected distance
on the recovered axis with the original repeat count. The V sweep compares the
original progress sign with both independently rendered host orientation
candidates. A scale or orientation becomes `VERIFIED` only when exactly one
candidate is supported by at least two independent groups. Ambiguous,
conflicting or missing measurements remain `UNVERIFIED`.

For an already-prepared ignored scene, the report also inventories every BSP
section identity and material:

```powershell
python tools/calibrate_bsp_uv.py `
  --scene local/sanctuary/scene.json `
  --runtime local/sanctuary/bsp-runtime.json `
  --output local/bsp/calibration.json
```

The report inventories all 105 imported BSP sections and adds host collision
candidate anchors from `bsp-runtime.json`. It labels sections whose resolved
source path contains `TilingMaterials` as capture candidates, without claiming
that a name alone proves visual tiling. The command is intentionally valid
with no measurement file. That report is a concrete `UNVERIFIED` result and
lists the missing matched captures and measurements. A supplied measurement
file is run with, for example:

```powershell
python tools/calibrate_bsp_uv.py `
  --measurements local/bsp/measurements.json `
  --scene local/sanctuary/scene.json `
  --runtime local/sanctuary/bsp-runtime.json `
  --scales 32 64 96 128 192 256 `
  --output local/bsp/calibration.json
```

`--require-verified` is available for a gate that must fail until both values
are proven. The normal report command exits successfully while preserving an
explicit `UNVERIFIED` status so evidence collection is not mistaken for a
parser failure.

## Current evidence and missing proof

The existing runtime evidence identifies useful host-side BSP floor probes:
`Sanctuary_P` `ModelComponent_0` node 23 near `(4679.6, -5216.0, 2874.2)` and
`ModelComponent_19` node 5 near `(19580.1, -61287.7, 2688.9)`. Those points
prove host collision candidates only; they are not matched original-game
camera views and do not establish repeat density or orientation.

No paired original-game/UE5 capture measurements were available when this
tool was added. Therefore the current divisor and V orientation remain
`UNVERIFIED`. To close the gate, capture at least two clearly tiled BSP
surfaces or two independent anchored measurements, record their resolved
material and node identities, and retain the original-game and UE5 images plus
the camera/world-anchor record under `local/`.

## Synthetic checks

`tests/bsp_test.py` covers both V export choices and rejects unknown policies.
`tests/bsp_calibration_test.py` covers two-surface acceptance, single-surface
and conflicting-candidate rejection, missing captures, and scene identity
inventory. These fixtures contain no installed-game bytes or screenshots.
