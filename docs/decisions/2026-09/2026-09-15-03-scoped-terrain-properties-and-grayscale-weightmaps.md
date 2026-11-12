## 2026-09-15: scoped terrain properties and grayscale weightmaps

The user's continued terrain work authorizes a bounded new reader route.
`--terrain-records` uses the observed Terrain actor prefix (26), component
prefix (8), and resource prefix (4), without offset scanning. Its class scope
is separate from `--scene-records`: TerrainLayerSetup.Materials is a struct
array and must not share the mesh Materials object-reference schema.
Individual unsupported objects retain explicit errors.

PF_G8 decoding now requires exactly width*height bytes and expands each value
to opaque grayscale RGBA. TerrainWeightMapTexture is accepted only for PF_G8;
existing dimensions, mip, TFC, LZO and allocation guards remain unchanged.
Synthetic pixel/bounds tests and the five installed Terrain_10 24x28 weightmaps
pass. This proves grayscale extraction, not layer assignment/blending parity.
No terrain triangle/hole semantics or root BSP render buffers are inferred
from this decoder extension.

