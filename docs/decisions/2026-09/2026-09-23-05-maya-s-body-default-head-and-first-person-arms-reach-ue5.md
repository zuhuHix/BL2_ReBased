## 2026-09-23: Maya's body, default head and first-person arms reach UE5

First Maya asset slice for the vertical slice: meshes and a default-skin
surface only, with no animation, skeleton merge or gameplay.
UModel build 1590 exported `GD_Siren_Streaming_SF` `Skel_SirenBody`
(5,368 vertices, 27 joints) and `Hands_Siren` (2,204 vertices, 47 joints),
plus `CD_Siren_Head_Default_SF` `Skel_Siren000` (2,803 vertices, 7 joints),
as glTF. Each mesh run exited 255 after writing the mesh but before any
texture, following missing `Common_Textures` stub imports. Exporting each
`Texture2D` alone as PNG succeeded. UE 5.8 rejects UModel's BC1 DDS
(`DXGIFormat not supported : 71`), so PNG is the texture path.
`host/ue5/import_character.py` imports them as UE5 skeletal meshes, and
`tools/run_ue_character.ps1` imports and captures a `-game` preview. Height
comes out at about 172 cm (glTF metres, Y-up converted by UE). The head's
lowest point meets the body's neck in bind pose.

Surface: the default skin (`CD_Siren_Skin_Default` ->
`CD_Skins_Siren_MainGame.Mati_Default_Body` / `Mati_Default_Head`, parent
`Common_Materials.Player.Master_Player`) passes `p_Diffuse`, `p_Normal`,
`p_Masks` and Shadow/Midtone/Hilight colours for zones A, B and C, read with
`ow-package --properties` and `level-arrays.schema`. `Master_Player`'s graph is
stripped, so the combine is not known. Observed: each `_Msk` holds two
half-width copies of the UV layout. In the right half, the head's hair is R
(zone A, whose midtone is blue), the face is B and the collar is G. The host
therefore samples the right half at `u/2 + 0.5`, takes A = R, B = G, C = B, and
multiplies the diffuse by `2 x Midtone` inside each zone.

UNVERIFIED: that combine, the factor 2, the unused Shadow/Hilight colours and
intensity scalars, the `_Msk` left half (it may drive the tattoo glow; the
emissive colours are not used), the hands reusing the body colours, and
left/right handedness. Result compared by eye only, with no original-game
capture: the preview shows blue hair, a yellow-orange top with grey panels,
black trousers, and a tattooed arm opposite a yellow sleeve, the arms matching
the body. The preview's fixed exposure and 2.5 lux sun are an inspection aid.
No C++ or parser code changed.

