# Dependency provenance

## LZO decoder: lzokay (vendored)

- Author: Jack Andersen, copyright 2018.
- Source: https://github.com/jackoalan/lzokay
- Vendored files: `third_party/lzokay/lzokay.cpp`, `lzokay.hpp`, `LICENSE`.
  Compared byte-for-byte against upstream `master` at commit
  `db2df1fcbebc2ed06c10f727f72567d40f06a2be` on 2026-09-10; no local edits.
  SHA-256 of the vendored copies:
  `lzokay.cpp` `2f4ab24a24a65766f05e2aa60361c624d1a1b80e73164d2248300e0701d91e91`,
  `lzokay.hpp` `0a6165e6726f27c1abfc1b1bb0613b1a839f4285d5cd6108d62d63cc713645c7`.
- License: MIT, per the vendored `LICENSE` file.
- API used: `lzokay::decompress` with an explicit output capacity. The decoder
  reports input/output overruns as error results; the container reader treats
  any non-success result or short output as a malformed block.

Controlled by `OPENWILLOW_LZO` (default ON). `-DOPENWILLOW_LZO=OFF` builds a
decoder-free reader that accepts only decompressed packages and rejects
compressed inputs with an actionable message; the container, assets and
compressed-package tests are then skipped. lzokay is a clean-room LZO1X
implementation and is not a derivative of the GPL LZO library.

## Superseded: miniLZO 2.10 (removed)

The reader previously fetched Oberhumer's miniLZO 2.10 (GPL-2.0-or-later,
archive SHA-256 `eb4ce543aad19533c83550746e0e9d7bcf716b35a42429e3ba17d60fa0f3e47a`)
behind `OPENWILLOW_RESEARCH_LZO`. That option, the FetchContent download and
the GPL dependency are gone. The switch was made because lzokay decodes all
installed code packages byte-for-byte identically and removes the GPL
question from every enabled build. No miniLZO code was translated into
this repository.

## Format references (read, not copied)

Static mesh, `Texture2D` and bulk-data layouts in `src/assets.hpp` were
worked out against UE Viewer (umodel) sources by Konstantin Nosov,
https://github.com/gildor2/UEViewer (MIT), read locally as a format
reference (`UnMesh3.cpp`, `UnTexture3.cpp`, `UnPackage3.cpp`). No functions
were copied or translated; the C++ here is an independent implementation of
the serialization order those files document. If UE Viewer code is ever
copied in, its MIT notice must be added alongside lzokay's.

## Oracles (run, not copied)

UE Viewer (umodel) is additionally *run* as a comparison oracle against the
user's installed game: `-list` for export tables and `-export -gltf -png` for
meshes and textures. Its output tree is read and compared with ours by
`tools/crosscheck_umodel.py` and `tools/crosscheck_umodel_assets.py`. No code
is copied or translated, the reader does not link or depend on it, and it is a
developer tool rather than a build or runtime dependency. This is a different
relationship from the format reference above and is recorded separately so the
distinction stays explicit.

OpenBLCMM (https://github.com/BLCM/OpenBLCMM, GPL-3.0) ships the output of the
game's own `obj dump` console command: an SQLite index into per-class dump
files packed in a data jar. `tools/blcmm_dumps.py` reads that data from the
user's installed copy as an observed-game-behaviour reference, and
`tools/crosscheck_blcmm_dumps.py` compares it with our decode. No OpenBLCMM
code was read, copied or translated, and no dump text is redistributed: it is
game-derived data and stays under ignored `local/`, like every other extract.

## Python research decoder (resolved 2026-09-13)

`research/native_count.py` originally carried an LZO1X decompressor that
described itself as a faithful port of miniLZO (GPL-2.0-or-later) with no
recorded upstream version. That function was removed on 2026-09-13 and
replaced by an independent implementation written from the public LZO1X
instruction-format description, with lzokay (MIT) consulted for instruction
semantics only; no code from miniLZO, LZO, or lzokay was copied or translated
into it. The replacement was verified byte-for-byte against the C++ lzokay
path on all nine installed code packages and reproduces the recorded function
census exactly (20,119 functions; 7,141 native; 12,978 script; 2,453 events).
It remains a local comparison oracle and is never linked into the C++ reader.
The earlier function survives only in git history, which is noted here so the
provenance record is complete.

The container/table layouts are recorded in the existing workspace research.
No game binaries, leaked source, or third-party UE3 source were used.

## External community tools used locally

These tools are not dependencies of the project and are kept outside the
repository. They are used against the maintainer's own game installation; no
game-derived output is tracked.

### UModel / UE Viewer

- Source: https://github.com/gildor2/UEViewer
- Official project/download information: https://www.gildor.org/en/projects/umodel
- Local source commit: `a0bfb468d42be831b126632fd8a0ae6b3614f981`
- Local binary: UModel build 1590, SHA-256
  `13502E5A4D8F6B5F32252AFEBD6360F7302CCFACCF6B8DDA65BEFF0BE2D364A0`
- License: MIT, as supplied by the upstream `LICENSE.txt`.
- Use: external package listing, visual inspection and export benchmark.
- No source was copied or translated into this repository. If that changes,
  update this file and preserve the upstream notice before making the change.

### Gildor BuildTools

- Source: https://github.com/gildor2/BuildTools
- Use: external build helper for the UModel source tree.
- The repository contains public-domain helper scripts and GPL-covered MSys2
  binary components. It remains outside this repository and is not linked or
  vendored here.

### UPK Explorer

- Project/download page: https://www.nexusmods.com/site/mods/587
- Use: optional secondary UE2/UE3 inspection and conversion tool under
  consideration; it is not required for the first UModel path.
- The current distribution is authenticated on Nexus Mods and its standalone
  redistribution terms have not been established here. It is not vendored or
  silently redistributed by this project.

### GPL research references

UE Explorer (GPL-3.0) and UPKUtils (GPL-2.0) may be consulted as external
research references only. No code from either project is copied into this MIT
repository. Any future code reuse requires a separate maintainer license
decision before implementation.

### OpenBLCMM and its BL2 datapack

- Source: https://github.com/BLCM/OpenBLCMM (GPL-3.0), release v1.4.1,
  `OpenBLCMM-1.4.1-Windows.zip` SHA-256
  `bbe9d09a3373de7f20b2f138b865baed762f2a8e6ef9b50738966f4095bc4000`.
- Datapack: https://github.com/BLCM/OpenBLCMM-Data release 2023-04-21-01,
  `blcmm_data_BL2-2023-04-20-01.jar` SHA-256
  `8bf07971904ed9d511586e11adcc4e676fbeda546994b439456860fcce2457bc`.
- Local: `C:/Users/yorad/Tools/OpenBLCMM/`; `data.db` extracted to
  `%LOCALAPPDATA%/OpenBLCMM/extracted-data/BL2/`.
- Use: observed-game oracle read by `tools/blcmm_dumps.py` (see "Oracles").
  No code copied; no dump text tracked.

### Ruffle (Flash player) - benchmark candidate for BL2's UI movies

- Source: https://github.com/ruffle-rs/ruffle, license MIT OR Apache-2.0
  (upstream `LICENSE.md`).
- Local binary: `nightly-2026-09-26`, `ruffle-nightly-2026_09_26-windows-x86_64.zip`
  SHA-256 `a3f2a75b63a84f7a5600733f9c39ea9d6b9a4d6e703b26c5d92ead470184aa5b`,
  unpacked to `C:/Users/yorad/Tools/Ruffle/nightly-2026-09-26/`.
- On first run the desktop build downloaded Cisco's OpenH264 2.4.1 (its own
  BSD-2-Clause license and Cisco's binary terms) to
  `%LOCALAPPDATA%/ruffle/video/`; it is Ruffle's, unused by this project.
- Use: local execution only, as a timeboxed benchmark of running the game's
  converted HUD movie (maintainer approval 2026-09-26). Not a dependency, not
  linked, not vendored. Embedding it in the UE5 host is a separate decision.
- Web build `@ruffle-rs/ruffle` `0.7.0-nightly.2026.9.26` (MIT OR Apache-2.0,
  its `package.json`), unpacked under ignored `local/ui/run/ruffle/`. It is
  served locally to the browser bench and to the UE5 browser-overlay prototype
  (`tools/hud_overlay/`, maintainer approval 2026-09-26). The overlay uses
  UE's own `WebBrowser` module (CEF, shipped with the engine); no Ruffle code
  is in the repository.

### unrealsdk, pyunrealsdk and the willow2 mod manager (installed by the player)

- Sources: https://github.com/bl-sdk/unrealsdk and
  https://github.com/bl-sdk/pyunrealsdk (LGPL-3.0, per the `LICENSE` files
  installed with the SDK's `.stubs` and `mods_base`), mod manager
  https://github.com/bl-sdk/willow2-mod-manager.
- Local install, in the player's game folder (not ours):
  `Binaries/Win32/Plugins/unrealsdk.dll` v3.2.0 (b1852aa4) SHA-256
  `7921eca8e44d30db08d9741b70cfe8a19904cd52049f4a5ff98b0b0a88e2c05c`,
  `pyunrealsdk.dll` v1.10.0 (c72c5558) SHA-256
  `a1c4647367dd4828d6e3b87e9c8fad131d8c1fd6179512105c9631033aad652a`,
  mod manager 3.8 (9097107f), per `unrealsdk.log` and `unrealsdk.toml`.
- Use: external execution only, to observe the running game (docs/LEGAL.md,
  clean-room rule 3). `tools/sdk_trace/openwillow_uitrace` is our own mod
  (MIT); it imports the SDK's Python API at runtime inside the game and copies
  none of its code. Traces it writes are game data and stay under `local/`.

### gameswf - benchmark candidate (pending)

- tu-testbed gameswf by Thatcher Ulrich and contributors, public domain.
  Approved for the same benchmark; not yet downloaded or built.
