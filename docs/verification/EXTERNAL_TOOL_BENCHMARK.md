# External extraction benchmark

This record separates tool acquisition and extraction evidence from UE5 import,
visual parity and gameplay evidence. Generated exports and logs remain under
the ignored `local/` directory.

## UModel / UE Viewer smoke check - 2026-09-16

- Source checkout: `https://github.com/gildor2/UEViewer`
- Source commit: `a0bfb468d42be831b126632fd8a0ae6b3614f981`
- Executable: UModel build 1590, compiled 2022-12-23
- Executable SHA-256:
  `13502E5A4D8F6B5F32252AFEBD6360F7302CCFACCF6B8DDA65BEFF0BE2D364A0`
- BL2 detection: `border` game tag
- Install scan: 920 game files, 15 skipped, in one
  `WillowGame/CookedPCConsole` directory
- Package listing: `Ash_P.upk`, version `832/46`, 21,834 exports
- Static mesh command: UModel `-export -gltf -lods -dds -nooverwrite` for
  `Ash_Road01` (`StaticMesh`) from `Ash_P`
- Static mesh result: exit code 0, 0.1 seconds, one `.gltf`, one `.bin` and one
  log file, 34,616 total bytes
- Texture result: `MetalRoadConcrete_Dif` (`Texture2D`) exported from `Ash_P`
  as a 1024x1024 DDS streamed from `Textures.tfc`; exit code 0, 0.09 seconds,
  526,172 total bytes including the run log
- Skeletal result: `Skel_BugMorph` (`SkeletalMesh`) exported from `WillowGame`
  as glTF; exit code 0, 0.08 seconds, 234,220 total bytes including the run
  log. UModel reported unknown fields while reading the mesh/material data;
  the export still completed and those warnings remain part of the benchmark.
- Output directories: `local/external/umodel/`

An unbounded no-object export of the whole `Ash_P` package was also attempted
with the same flags. It produced no output during several minutes and was
manually stopped; this is not enough evidence to call UModel incompatible, but
it is enough evidence to require bounded package/object runs before attempting
a full-install export.

This proves that the candidate recognizes this BL2 install and can export a
static mesh, texture and skeletal mesh from it. It does not prove complete
package extraction, animation or sound export, material graph translation, UE5
import, visual parity or gameplay compatibility.

## Gate still open

Run the following before making UModel the default asset backend:

- export representative textures, static meshes, skeletal meshes, animations,
  materials and sounds;
- run a bounded multi-package batch and inventory successes, failures,
  unsupported objects, duplicates, elapsed time, output size and logs;
- import at least one representative mesh/texture path into UE5;
- compare the UModel inventory with the project's `ow-package` census and scene
  manifests;
- decide whether glTF, PSK/PSA or another format is the most reproducible UE5
  adapter.

The full-install export is deliberately not marked complete until those
measurements exist.
