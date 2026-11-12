## 2026-09-26: Run BL2's real HUD movie (option 1), benchmark players first

The maintainer chose to run the game's own HUD movie over rebuilding it,
"as true to game as possible even if harder". Observed in `Startup.upk`
(export 46064, `GFxUI.SwfMovie UI_HUD.HUD`):
- `SourceFile` `..\..\WillowGame\Flash\UI_HUD\HUD.swf`, timestamp 2012-08-01.
  `RawData` holds a 131,240-byte `CFX` file: zlib-compressed Scaleform SWF,
  version 9, 336,607 bytes uncompressed.
- The stage is 1280x720 at 24 fps.
- Tag census: 353 sprites, 13,446 PlaceObject2, 389 frame labels, 314 shapes,
  81 edit texts, 737 DoAction plus 22 DoInitAction and no DoABC, so the
  scripts are ActionScript 2. It also has 25 Scaleform DefineExternalImage2,
  164 DefineSubImage and one DefineCompactedFont, and 12 ImportAssets2 tags
  (`gfxfontlib.swf` fonts `$WillowBody`, `$WillowHead`, `$WillowCompact`, and
  `SharedWillowComponents.swf`).
- Its art is 17 separate `UI_HUD` `Texture2D`s: DXT1/DXT5, power-of-two
  padded, including the 1024x1024 atlas `texture1`. All 17 decode with
  `ow-package --texture` to ignored `local/ui/tex`.

Observed tag layouts (from the bytes; no Scaleform code consulted):
- DefineExternalImage2 (1009): u32 character id, u16 format (13 in every
  record), u16 target width, u16 target height, u8-length export name, then
  u8-length file name. Example: id 0xB8 -> `HUD_IB8.tga`, 389x14; the
  matching texture is `UI_HUD.HUD_IB8`, 512x16.
- DefineSubImage (1008): u16 id, u16 image id, then u16 x1, y1, x2, y2.
- UNVERIFIED: the first 1009 record, the atlas `texture1.tga`, reads
  `01 00 09 00` where the others hold a u32 id. The sub-images reference
  image 1. How that id relates to the `-nopack` weapon placeholders, which
  also start at id 1, is unresolved.

Policy recorded in docs/LEGAL.md, "UI movies", with maintainer approval: the
movies run from the install; disassembled script listings stay local; no
transcription into project code; no Scaleform SDK or source. Next, a
timeboxed benchmark of Ruffle (MIT/Apache-2.0) and of public-domain gameswf
on a standard SWF converted locally from the installed movie. Integration
into UE5 waits for those results.

Benchmark progress (same day):
- `tools/extract_swfmovie.py` writes a `SwfMovie`'s RawData. The value starts
  24 bytes (the UE3 tag header) plus a u32 count after the property's
  reported offset; checked against the CFX signature at byte 256 of
  `UI_HUD.HUD`.
- `tools/gfx_to_swf.py` turned the installed HUD into a 1,067,262-byte
  standard SWF under `local/ui`. It converts 16 external images and 164
  sub-images to `DefineBitsLossless2`; the 8 `-nopack` weapon-icon slots,
  filled by the game at runtime, become transparent. It drops
  `ExporterInfo` and `DefineCompactedFont`.
- Ruffle nightly-2026-09-26 loads it (`Loaded SWF version 9, resolution
  1280x720 @ 24 FPS`). It then fails to fetch `../gfxfontlib.swf`: the
  `--base` argument must be a `file:///` directory URL.
- The font library is `UI_FontsEn.FontsEn` (`FontsEn.swf`, CFX version 8). It
  holds three `DefineCompactedFont` tags only: WillowBody, "Compacta Bd BT"
  and "Chintzy CPU BRK". Those fonts are licensed to the game and load from
  the install only. Text needs this Scaleform font format decoded into
  standard `DefineFont3`.
- `SharedWillowComponents.swf`, which the HUD imports, is not in
  `Startup.upk`; its package is not yet found.
- Nothing rendered yet. Window capture of Ruffle's Vulkan surface via
  PrintWindow came back blank, and a screen-copy capture was discarded
  because it caught other desktop windows. The HUD's clips are also expected
  to stay hidden until game code drives them, so a visual check needs a host
  harness that calls into the movie.

