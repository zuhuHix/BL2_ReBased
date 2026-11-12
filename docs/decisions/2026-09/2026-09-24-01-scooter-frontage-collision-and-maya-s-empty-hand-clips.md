## 2026-09-24: Scooter frontage collision and Maya's empty-hand clips

The Sanctuary scene records collision enabled on the placed
`SancScooterStairs` and `SanctuarySidewalk_ParkingLot_Low`, but their source
meshes have no `RB_BodySetup`. The latter is a source-hidden proxy that
overlaps the visible `Sanctuary_Land` parking-lot mesh about 8 cm higher.
For these two exact mesh identities only, when the body setup is absent, the
preparer records a render-triangle collision fallback. The placement's source
collision flag still controls whether it blocks the pawn. This is a host
walking approximation; native UE3 collision parity is `UNVERIFIED`. The
visible `Sanctuary_Land` parking-lot mesh is not changed.

The current Maya walker has no weapon. It now selects the UModel build 1590
`1st_Person_Unarmed` clips from `GD_Siren_Streaming_SF` instead of the pistol
clips. The existing converter and UE skeleton still own track conversion and
asset identity. Clip import and the UE host build are automated checks;
original-game pose and blending remain `UNVERIFIED` until a paired capture
is inspected.

Fresh checks in the isolated checkout: Release C++ build, 6/6 CTest, nine
package comparisons, UE5.8 host build, 31 level tests, the saved scene check
(4,888 section actors) and the saved collision check (615 sections, 123 active
triangle components, zero errors) passed. The imported unarmed idle also
logged active in a game capture. The saved UV0 check passed on 615 sections,
181,767 triangles and 545,301 corners. Live `-owspawnprobe` walking checks grounded
the pawn on the exact parking-lot proxy `StaticMeshActor_SMC_288` and the
Scooter stairs `StaticMeshComponent_970`; both captures show the player above
the floor. An earlier lower-stairs probe rested on an overlapping building
floor, so that probe was not used as stairs evidence. Commandlet editor-world
line traces returned no hits and were not used as runtime evidence. Native
BL2 collision shapes, animation blending and original-game visual parity
remain `UNVERIFIED`.

