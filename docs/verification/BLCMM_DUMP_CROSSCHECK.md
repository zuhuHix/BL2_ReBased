# Cross-check against the game's own object dumps

AI-assisted verification pass, 2026-09-18. Evidence comes from the user's
installed game and from OpenBLCMM's copy of the game's `obj dump` output on the
user's machine. No binary layout changed in this pass except the collection
scale rule recorded below. Dump text and all reports stay under ignored
`local/`.

## What the oracle is

OpenBLCMM ships the output of the game's own `obj dump` console command for
every object it indexes: an SQLite index (`data.db`) mapping an object name to a
byte range inside per-class dump files packed in a data jar. A dump is the full
property state the *running engine* reports for an object after it has loaded
and cooked it, defaults included.

That makes it a different kind of oracle from umodel. umodel is a second reader
of the same bytes, so agreement means two decoders agree. A dump is what the
engine itself concluded, so agreement means our decode matches observed game
behaviour. It is still not a specification: the dumps show state at the moment
they were taken, they omit anything the property system does not print (see the
BSP limitation below), and nothing here establishes how the engine *renders*
what it reports.

`tools/blcmm_dumps.py` locates and parses a dump; `tools/crosscheck_blcmm_dumps.py`
compares a prepared scene against it. Neither reads game packages: package data
comes through our own reader.

## Coverage

Every object the Sanctuary scene names has a dump: 15 `TerrainComponent`,
24 `ModelComponent`, 381 materials and all 4469 actor entries.

## Terrain: 15 of 15 agree

Per component the dump reports `SectionBaseX/Y`, `SectionSizeX/Y` and a
`Bounds` box, plus the component's `_LocalToWorld`.

- Section base and size match exactly on all 15.
- Mapping our decoded vertices through the reported `_LocalToWorld` reproduces
  the reported `Bounds` origin to within 0.003 cm on every axis, and the extent
  to exactly the one-unit expansion Unreal applies to a component's reported
  bounds (constant on all 15 components and all three axes; subtracted before
  comparison).

Because the box is reproduced rather than merely similar in size, this
corroborates the height convention (row-major +X/+Y, local `Z=(sample-32768)/128`),
the per-terrain cell scale, and the section placement — against the engine, not
against a second reader.

One convention had to be recovered first: a `TerrainComponent`'s own
`_LocalToWorld` already carries its section base translation, while our terrain
vertices are terrain-local. Comparing without shifting them back produces
offsets of exactly `SectionBase x cell scale` (131072, 6144, and rotated
equivalents), which is what the first run showed.

Face orientation, native terrain blending and layer weights are untouched by
this check and remain `UNVERIFIED`.

## BSP: 24 of 24 agree, on counts only

Per `ModelComponent` the node count, the element count and the owning `Model`
reference match. `ModelComponent_0` reports 35 nodes and 15 elements, matching
our recovered node set and section count; `ModelComponent_16` reports 22 and 16.

The limitation is in the dump format, not in the comparison: the property system
prints `Nodes(N)=` and `Elements(N)=` with empty values, so the arrays give their
length and nothing else, and a `Model` dump carries no geometry. Node contents,
BSP UVs, `PolyFlags` and lighting are therefore not testable this way and stay
`UNVERIFIED`.

## Actor placement: 4209 exact, 35 movers, 1 disagreement (fixed)

Our recorded placement is composed into a matrix and compared with the
component's `_LocalToWorld`. Both recorded shapes are handled: the cooked 4x4
matrix plus separate draw scale that `StaticMeshCollectionActor` entries carry,
and the actor/component location-rotation-scale pairs everything else carries.

This confirms the rotator decode: Unreal rotator units scaled to degrees and
read as (Pitch, Yaw, Roll) through `FRotationMatrix`, with draw scale
multiplying each row, reproduces the engine's matrix to within 1e-3 on rotation
entries and 0.05 cm on translation across 4209 placements.

35 `InterpActor` placements differ. Those are matinee-driven, so a dump reports
wherever the actor had moved to, not its cooked placement; they are reported as
`mover` and are not counted as disagreements. That they differ is not evidence
of a decode problem, and that the other 80 `InterpActor`s match is not evidence
that they never move.

One genuine disagreement, now fixed:
`Sanctuary_P.TheWorld:PersistentLevel.StaticMeshCollectionActor_10.StaticMeshActor_SMC_1802`
(a `Prop_Bank.BankTeller`). We placed it unscaled; the engine reports an X row
scaled by 0.97 and a `_LocalToWorldDeterminant` of 0.97. The component export
carries `Scale3D=(0.97,1,1)` as an ordinary property, while the collection
actor's cooked per-entry tail records `(1,1,1)` for it. A survey of all 3153
collection children in `Sanctuary_P` found 2037 that declare their own
`Scale3D`/`Scale`; the tail agrees with the property in 2036 of them. The tail
is a cache that is stale in exactly this one case, so `prepare_level.py` now
prefers the component's own property where it exists. See the DECISIONS entry.

## Material texture picks: 353 confirmed, 0 contradicted

For each material the effective `TextureParameterValues` are collected by
walking the `Parent` chain, with a child's override beating its parent's, and
compared with the texture we chose per channel. Parameter names are mapped to
channels through a narrow allow-list with a rank, so a master's canonical
`p_Diffuse` wins over a variant such as `p_DiffuseVertexPaint` when a material
carries both.

| Status | Count | Meaning |
| --- | --- | --- |
| `agree` | 353 | the parameter the game reports for that channel is the texture we chose |
| `differ` | 0 | it reports a different texture |
| `other_parameter` | 3 | our texture is in the material's parameter set, under another name |
| `unparameterised` | 348 | the material has texture parameters but none for that channel |
| `no_parameters` | 97 | the dump lists no texture parameters at all |

By channel: diffuse 246 agree / 0 differ, normal 85 / 0, emissive 20 / 0,
specular 1 / 0.

The `unparameterised` bucket is the honest limit of this check, and it is
concentrated exactly where the project already reports approximations: 197 of
our emissive picks and 149 of our normal picks are not backed by a texture
parameter the game reports, so the oracle neither confirms nor contradicts them.
The same is true of the 97 `no_parameters` materials, which are base `Material`
objects whose expression graph a cooked dump does not print.

16 parameter names are not in the allow-list and are reported rather than
guessed at: `p_Masks` (22), `p_VertexPaintNoise` (12), `p_Masked` (11),
`Color` (3), `Luminosity` (3), `P_SimpleReflect` (2), `Diffuse_A/B/C` (2 each),
and singletons including `top_layer`, `CubeMap`, `Tex_Opac`, `Transition_Track`,
`clouds` and `p_CustomPattern`. Deciding what any of them contribute is
material-graph work this pass does not attempt.

## Not established

Nothing here says how the engine renders what it reports. Shader graphs,
terrain layer blending, HLS packing, lightmaps, BSP UVs, `PolyFlags`, terrain
face orientation, sky behaviour and time of day are all untouched. Count and
field agreement is not visual parity, and no in-game comparison was made.

## Reproduction

```powershell
python tools/crosscheck_blcmm_dumps.py --scene local/sanctuary/scene.json `
  --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 `
  --output local/blcmm/crosscheck.json
```

`--reader`/`--game` are only needed for the material comparison, which resolves
our texture PNG file names back to the texture objects they came from. The tool
exits non-zero if any terrain, BSP, actor or material comparison disagrees;
`mover`, `unparameterised` and `no_parameters` are not disagreements. A single
object can be inspected with
`python tools/blcmm_dumps.py "<Level>.TheWorld:PersistentLevel.<Name>" --raw`.
