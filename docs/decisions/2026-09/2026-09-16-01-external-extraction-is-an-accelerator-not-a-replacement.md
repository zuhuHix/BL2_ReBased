## 2026-09-16: External extraction is an accelerator, not a replacement

The project will evaluate mature community exporters before expanding every
custom visual decoder. UModel / UE Viewer is the first candidate because its
official compatibility data includes Borderlands 2 and it recognizes this
installation as package version `832/46`.

The verified local candidate is UModel build 1590 from the upstream
`gildor2/UEViewer` checkout at commit
`a0bfb468d42be831b126632fd8a0ae6b3614f981`. The executable SHA-256 is
`13502E5A4D8F6B5F32252AFEBD6360F7302CCFACCF6B8DDA65BEFF0BE2D364A0`.
It scanned 920 files, listed `Ash_P.upk` with 21,834 exports and exported
`Ash_Road01` as glTF in 0.1 seconds. Follow-up smoke runs exported the
TFC-streamed `MetalRoadConcrete_Dif` texture as a 1024x1024 DDS in 0.09
seconds and `Skel_BugMorph` as glTF in 0.08 seconds. The skeletal run emitted
unknown-field warnings that remain recorded as benchmark limitations. All
output was written only under ignored `local/external/`.

This is an acquisition and smoke result, not an importer or compatibility
decision. The Phase 0.5 gate in `ROADMAP.md` must test textures, static and
skeletal meshes, animations, sounds, materials, batch failures, duplicates,
output size and UE5 importability. Until that gate passes, UModel remains an
external visual oracle and optional payload source; `ow-package` remains the
project’s metadata, reference and verification path. No UModel source was
copied, and no game-derived output is tracked.

