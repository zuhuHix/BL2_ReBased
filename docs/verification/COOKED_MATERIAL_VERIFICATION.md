# Cooked-material continuation - 2026-09-13

The user approved cooked-material serialization work and continuing into the
next validation step. This checkout is `t3code-df92eae1`.

## Automated checks

Release C++ build and UE5.8 OpenWillowEditor build succeeded.
`python tests/level_test.py`: 10 tests, OK.

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

Census: 2,008/2,008 packages, 4,751,329 serialized exports, two UHD sidecars.
The Phase 0 mesh/texture extraction and diffuse-parameter check passed again.
These establish parser consistency, not shader or visual parity.

## Local scene provenance

Reused previous scene geometry from `t3code-54150c5b/local/{ash,sanctuary}`
after 11 Ash and 334 Sanctuary fresh OBJ files matched byte-for-byte.
Stopped the redundant full extraction jobs. Refreshed materials in this isolated
checkout without `--reuse-textures`, so supported textures were decoded again
from the installed game. No prior UE import report counts as current proof.

The new resource reader found SancBuild1e_Dif in Mat_SancBuild1e's native
texture list. Master_Black's list is empty; its appearance remains unresolved.
The resource tail is not a recovered shader graph. See [DECISIONS.md](../../DECISIONS.md) for scope
and external format-reference provenance.

Ash's three unsupported diffuse candidates all refer to the same 256x256
PF_A8R8G8B8 texture, Prop_Skybox.Textures.Sky_TransitionBL2Default_Dif.
The current texture decoder supports DXT1/DXT5, so those candidates remain
fallbacks. Reading this texture and reconstructing sky shading are open steps.

## Refreshed material results

| Scene | Materials | New usable diffuse materials | Placed sections using them | Materials lacking diffuse, before -> after |
|---|---:|---:|---:|---:|
| Ash | 225 | 42 | 977 | 72 -> 30 |
| Sanctuary | 391 | 51 | 397 | 118 -> 67 |

Sanctuary's fully neutral fallbacks dropped from 115 to 64. Its total inference
count is 68: 14 previous unnamed-expression candidates plus 54 native-resource
candidates, three of which fail texture decoding. Ash has 45 native-resource
candidates, three unsupported. Candidate counts are not successful texture counts.
No native-resource parser errors occurred in these refreshes. All prior channel
assignments, scene meshes and actor placements compare unchanged. Section counts
include per-actor material overrides; unused material definitions count as zero.

Ash imported 5,235 sections and passed saved-scene verification with all four
texture channels. Import: zero errors, eight Interchange console-lookup performance
warnings. Reopen verification: zero errors/warnings. Standalone OpenWillow.Viewer
passed; diagnostic movement was 6,209.77 cm. The current Ash start screenshot
shows textured architecture but a black sky. This does not establish visual parity.

Local evidence: `local/material-audit/{targeted,comparison}.json`,
`local/{ash,sanctuary}-refresh.log`, `local/ash-import.log`,
`local/ash/viewer-4520c60382c24874961a9d435a6dd7c6.log`, and screenshots under
`host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor/`. All game-derived outputs
remain ignored. Physical keyboard/mouse input was not checked in this run.

Sanctuary imported and reopened all 4,768 sections, with all four texture
channels verified. Import: zero errors, eight Interchange performance warnings;
reopen verification: zero errors/warnings. OpenWillow.Viewer passed with
6,371.06 cm diagnostic displacement; log:
`local/sanctuary/viewer-b17f3c35c7bb4d518cd6596a6ab226c7.log`.

Visual inspection compared the previous Sanctuary_P_start00001.png in the
source checkout with this checkout's Sanctuary_P_start00000.png. The white
shop sign and large pipe now carry texture detail, along with more building
surfaces. The new overview still has large white fallback shapes; sky remains
black, and the shop sign appears mirrored. This is material-coverage progress,
not visual acceptance. UV orientation, shader semantics, sky rendering and
matched original-game viewpoints remain unverified/open. Phase 1 is not complete.

Reproduce material refresh and saved-scene verification using the existing tools:

```powershell
python tools/refresh_materials.py --reader build/Release/ow-package.exe --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --scene local/sanctuary
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' -Scene local/sanctuary -ImportOnly -SkipBuild
./tools/test_ue_viewer.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' -Scene local/sanctuary
```

`-SkipBuild` assumes the successful UE build documented above. These commands
require a prepared local scene and the user's own installed game.
