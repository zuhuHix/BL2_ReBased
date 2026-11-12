## 2026-09-14: First source collision and placeholder walking slice

User explicitly approved collision-parser changes after the sensitive-area rule
was disclosed. The mesh reader now exposes the body reference it already reads;
the scene reader includes Engine.RB_BodySetup tagged properties. Reader limits,
container checks and property-size checks are unchanged.

Installed Ash_P body exports 11404 and 11405 provided initial box and convex
observations. Tagged Box is two XYZ float vectors and one validity byte (25
bytes); tagged Plane stores W,X,Y,Z, and Matrix has four such rows (64 bytes).
Identity boxes and asymmetric rotated/translated synthetic fixtures distinguish
this from XYZW. The observation is limited to BL2 832/46, not general UE3 parity.

Convex VertexData is retained in local centimeters; source FaceTriData and
ElemBox are checked when present. Box TM and dimensions produce eight corners.
Malformed, nonfinite, degenerate and unsupported geometry is rejected. Sphere,
capsule, cooked PhysX blobs, per-poly flags and general class inheritance remain
unsupported. Empty/absent body geometry receives no invented collision.

UE5 cooks these hulls using its installed FKConvexElem/UBodySetup API. Reference:
https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Engine/FKConvexElem
and the locally installed UE5.8 headers. No third-party implementation was copied,
no new dependency was added, and no game data is tracked. Full hulls attach only
to the first visual section to prevent duplicate collision for material sections.
Components with explicitly false BlockActors or CollideActors are disabled;
absent flags currently default to enabled, an inspection approximation without
CDO inheritance or original-game collision-channel parity.

The opt-in -owwalk controller uses UE5 CharacterMovement with a 34 cm radius,
88 cm capsule half-height, 450 cm/s walk speed, 420 cm/s jump velocity, 35 cm
steps and a 45 degree floor angle. These are placeholder values. Default free
flight remains available. See COLLISION_WALKING_VERIFICATION.md for test evidence
and the deliberately bounded Sanctuary acceptance claim.

