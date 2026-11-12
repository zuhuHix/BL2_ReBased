## 2026-09-14: bounded native Sky_Dome import

The user authorized the next Sanctuary visitability slice. Scene preparation
marks only `Prop_Skybox.Meshes.Sky_Dome` placements whose effective material is
Unlit. The observed Sanctuary placement with a floor-material override is not
marked as native sky. UE5 imports the accepted dome as a static visual shell,
with no collision or shadow casting, and records `partial_unverified` graph
status. The temporary UE5 atmosphere remains in the map for unresolved sky
layers. This does not interpret Kismet state, outer sky meshes, time-of-day,
cloud/mask graph connections or visual parity. See
docs/verification/NATIVE_SKYBOX_VERIFICATION.md.

