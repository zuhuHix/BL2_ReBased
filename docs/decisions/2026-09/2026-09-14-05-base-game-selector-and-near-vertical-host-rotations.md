## 2026-09-14: Base-game selector and near-vertical host rotations

Added a command-line selector over the preparer's base-game package scope.
Installed package names are discoverable without pretending every map loads;
ambiguous names and mismatched saved manifests are rejected. DLC support and
an in-game selection menu remain separate work.

Southpaw Factory exposed a host-only rotation loss: assigning a transform
through UE5's actor API snapped a near-vertical collection pitch to 90 degrees.
The matrix-derived Euler rotation is now assigned to the unattached root
component after the actor transform. A synthetic near-vertical placement with
negative scale passes fresh-process saved-scene verification with the existing
axis tolerances. No serialization layout or bounds checks changed.

Verification and current limitations are recorded in
[map selector verification](docs/verification/MAP_SELECTOR_VERIFICATION.md).

