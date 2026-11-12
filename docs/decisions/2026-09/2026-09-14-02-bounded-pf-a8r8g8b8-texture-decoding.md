## 2026-09-14: Bounded PF_A8R8G8B8 texture decoding

The user explicitly requested this format addition. AI-assisted implementation
adds little-endian BGRA-to-RGBA conversion after the existing bulk decoding,
without changing Texture2D serialization offsets or bulk flags. Alpha and row
order are preserved; no premultiplication or color-space conversion is applied.
The shared dimension guard retains the DXT limits (16384 per axis, 256 MiB per
mip); exact width * height * 4 bytes are required before channel conversion.
Existing decoded bulk-size and aggregate mip limits remain in force.

Format reference: Microsoft's public
[D3DFORMAT documentation](https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dformat)
defines A8R8G8B8 channel significance and memory byte order. No reference
implementation code, dependency or game-derived data was added.

All five CTest suites and nine installed code-package comparisons pass.
Ash_P export 21482, Prop_Skybox.Textures.Sky_TransitionBL2Default_Dif,
extracts as 256x256 with one resident mip. Synthetic tests verify exact pixels,
alpha, inline/TFC and LZO paths, small mips and corrupt-input rejection.
Native sky shading and in-game visual parity remain UNVERIFIED.
See [verification](docs/verification/A8R8G8B8_TEXTURE.md).


