## 2026-09-26: HUD movie renders in Ruffle with game fonts, library imports and localized text

Continues the HUD benchmark above. All of this is in the locally converted
copies under ignored `local/ui/run`; the install is untouched.

Observed and implemented (layouts read from the bytes; no Scaleform code or
SDK consulted):
- `SharedWillowComponents.SharedWillowComponents` and
  `SharedComponents.ConsoleComponents` are in `WillowGame.upk`, not
  `Startup.upk`.
- Packed atlases: a DefineExternalImage2 whose bytes 2-3 are `09 00` is read as
  an atlas, with bytes 0-1 a u16 atlas index. DefineSubImage's second u16 is
  that index. UNVERIFIED reading, but consistent across UI_HUD (index 1),
  SharedWillowComponents and ConsoleComponents (0 and 1). This replaces the
  earlier `--pack-texture` guess.
- DefineCompactedFont (1005) is decoded by `tools/gfx_compacted_font.py` and
  written as DefineFont3. The layout is documented in that file's docstring.
  Glyphs were checked by contour closure and bounds on every ASCII glyph, and
  by rendering. From `FontsEn`: WillowBody 293 glyphs, 28 skipped; "Compacta
  Bd BT" 232, 10 skipped; "Chintzy CPU BRK" 40, none skipped. Skipped glyphs
  use edge-word bit 0 (accented Latin Extended and some quotes). That encoding
  is not understood, so they are emitted empty and listed in the report.
- Font aliases: the font library's sample texts read `$Alias = Font Name`, and
  each font is also exported under its alias (`$WillowBody`, `$WillowCompact`,
  `$WillowTechNumbers`).
- `--localization`: `$File.Section.Key` tokens are replaced from the install's
  `.int` files, with `Patched*.int` overriding its base file. None of the HUD's
  static DefineEditText strings use tokens. All 40 HUD tokens are ActionScript
  ConstantPool/Push strings (for example
  `$WillowMenu.HUD.EnemyLevelAbbreviation`), which Scaleform translates when
  script assigns them. They sit in straight-line frame scripts with no
  branches, functions, `with` or `try`, so each string is rewritten in place
  and only its action's length changes. Streams containing any of those actions
  are left alone and counted as `script_skipped` (0 in the three movies). In
  Ruffle, the XP bar's level label now reads `LV` instead of a clipped `$Willo`.
- `--inline-font-imports` works around a Ruffle limitation read from its
  source (`core/src/loader.rs` `load_asset_movie`,
  `core/src/display_object/movie_clip.rs` `preload`, nightly 2026-09-26). An
  imported movie is preloaded once. If it has its own ImportAssets, preload
  stops there and never resumes, so its later exports never register.
  SharedWillowComponents imports its fonts from gfxfontlib, so every HUD import
  from it (value clip, eridium counter, item cards, manufacturer logos) failed
  with "non-registered character ID". The option replaces a font-only import
  with the DefineFont3 from the already converted library, under the
  importing id, plus an ExportAssets under the import name so HTML
  `<font face="$WillowBody">` still resolves. Scaleform resolves nested imports
  itself; this changes only our converted copies. After this change the HUD
  loads with no missing characters or unknown-font warnings. `$WillowHead` is
  not exported by gfxfontlib and remains a plain import.
- `tools/hud_harness_swf.py` writes a small AVM1 wrapper, our own bytecode
  assembled from the public SWF spec. It loads a movie into `_level1` and
  exposes `ow(target, op, a, b)` to JavaScript through ExternalInterface.
  `gotoAndStop("16_9")` on the root shows the 16:9 layout: vitals, XP bar,
  ammo and grenade bars, minimap and crosshair.

Not yet verified or still open:
- Nothing has been compared to the game beyond eyeballing the layout against
  `local/ui/ref`. Bars show authoring-time fill; no host data is driven yet.
  103 "Stack underflow" warnings during the first frames are unexplained.
- About 10 "Character ID collision" errors remain. Ruffle fetches
  SharedWillowComponents once per import tag (5 times) and registers the
  exports each time. This looks harmless but is UNVERIFIED.
- No count or render agreement here claims full Scaleform compatibility.

