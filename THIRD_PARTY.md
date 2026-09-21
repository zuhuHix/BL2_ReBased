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
