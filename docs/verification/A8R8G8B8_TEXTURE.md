# PF_A8R8G8B8 texture decoding (2026-09-14)

Checkout: `t3code-6bf0ae5d`. AI-assisted implementation; synthetic fixtures only.

Release configuration and build succeeded. Pixel tests cover unequal RGB
channels, zero/partial/full alpha, a 3x2 image, a 1x1 mip, selected/all mip
output, raw and LZO-compressed inline/TFC payloads. Negative tests cover short
and oversized pixel data, bulk size mismatch, zero/oversized dimensions,
unsupported format/flags, truncation and cache bounds. Existing DXT and mesh
checks also pass.

`ctest --test-dir build -C Release --output-on-failure`:

```text
1/5 Test #1: package-synthetic ................ Passed
2/5 Test #2: properties-synthetic ............. Passed
3/5 Test #3: runtime-synthetic ................ Passed
4/5 Test #4: container-synthetic .............. Passed
5/5 Test #5: assets-synthetic ................. Passed
100% tests passed out of 5
```

`python tools/verify_packages.py --reader build/Release/ow-package.exe`:

```text
Core: 234397 bytes, 1621 exports; decoded bytes, counts and export fields match
Engine: 5878264 bytes, 33166 exports; decoded bytes, counts and export fields match
GameFramework: 61714 bytes, 258 exports; decoded bytes, counts and export fields match
GearboxFramework: 1224040 bytes, 7098 exports; decoded bytes, counts and export fields match
WillowGame: 13054200 bytes, 56443 exports; decoded bytes, counts and export fields match
GFxUI: 136680 bytes, 841 exports; decoded bytes, counts and export fields match
IpDrv: 230751 bytes, 1364 exports; decoded bytes, counts and export fields match
OnlineSubsystemSteamworks: 265760 bytes, 1709 exports; decoded bytes, counts and export fields match
AkAudio: 39503 bytes, 176 exports; decoded bytes, counts and export fields match
```

## Installed texture extraction

The installed `Ash_P.upk` contains export 21482,
`Prop_Skybox.Textures.Sky_TransitionBL2Default_Dif`.
Using `--texture 21482 --property-offset 4 --tfc <CookedPCConsole>` with
`--output local/argb/sky.png --all-mips local/argb/mips` succeeds:
PF_A8R8G8B8, 256x256, selected mip 0, streamed false, one serialized and one
resident mip. Outputs remain under ignored `local/argb/`.

This verifies extraction of one real asset, not its appearance against the
original game. Scene materials were not refreshed or imported into UE5.
Native sky shading and visual parity remain UNVERIFIED. This supersedes the
texture-decoder limitation in the historical cooked-material verification;
its recorded scene counts are unchanged.
