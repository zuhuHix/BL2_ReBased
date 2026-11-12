## 2026-09-15: distinguish lower ice geometry from visual helpers

The Sanctuary artifact pass retains all four IcePlate placements and their
recovered collision. WorldTransition remains a narrowly hidden translucent
helper pair; it does not explain the separate omitted terrain/BSP geometry.
Mat_FrozenLake now uses its inspected FrozenLake resource as an explicit UV0
color approximation instead of the generic Snow_Dif selection. Native snow,
noise, reflection, normal, glow and UV modulation remain unverified.
Mat_IceRoadSanctuary, the single SanctuaryRoad_01 placement at the town gate,
follows the same rule with its inspected BrokenRoad_Dif resource; its cooked
list carries three `_Dif` overlays, so the sole-`_Dif` heuristic had left the
road white. Its p_Normal expression survives with a stripped texture and no
normal exists in the cooked list, so no normal is approximated. Both inspected
color fallbacks share one scoped table; the unplaced Env_Ice Mat_IceRoad is
untouched.

The existing HLS regular-diffuse fallback now requires the inspected direct
parent and concrete atlas, records texture/UV provenance, and retains other
supported channels. Material refresh reapplies the placement-derived native
dome interior policy. No binary layout or bounds checks changed. AI-assisted
source inspection and validation are recorded in
`docs/verification/SANCTUARY_ARTIFACT_PASS.md`.

