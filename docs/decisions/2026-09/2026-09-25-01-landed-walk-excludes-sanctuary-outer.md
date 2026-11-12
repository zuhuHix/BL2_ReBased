## 2026-09-25: landed walk excludes Sanctuary_Outer

`Sanctuary_Outer` and `Sanctuary_Land` are separate Kismet-streamed sublevels.
The landed walking view now tags all 29 `_Outer` placements (31 rendered mesh
sections) on import and hides them with collision disabled at `-owwalk` startup.
The saved inspection map retains them for scene-provenance work. This includes
the liftoff hull and antennas, skybox buildings, and invisible collision boxes;
it does not suppress the landed road or plaza geometry. The alternative
`Sanctuary_LandedComparison` map and captures are ignored local outputs.

Automated: UE 5.8.3 reimport and saved-scene, collision and UV verifiers passed;
the runtime log reports 31 hidden sections; 6 CTest cases and the nine-package
comparison passed. Visual: a capture from (8424, -2600, 3900) facing north
shows a clear road into the plaza. The maintainer identified the center object
in an overhead host capture as the intended monolith and accepted its current
appearance in that view. Its UModel 1590 `Open` frame-0 bake is present; exact
pose parity against the original-game reference has not been measured.
The maintainer's side-street framing was matched at approximately
(8150, -700, 3800), yaw -90 degrees, 106-degree FOV: one Resistance poster on
the left, two on the right and the road cover align with the report image.
In the landed comparison capture the tall slab and blocky wall are absent,
exposing Dr. Zed's building beyond the street. This is visual validation of
that view, not an original-game placement comparison. Runtime: the walking
pawn and Maya's first-person arms initialize; the reported street has not yet
been traversed after this change.

