## 2026-09-30: Local paint pass for the selected Maliwan SMG

AI-assisted bounded visual improvement using the already benchmarked UModel
1590 exports, not another extraction backend. prepare_weapon_paint.py resolves
the known recipe's two-MIC parent chain and four existing textures; the UE
importer assigns an approximate material to its existing mesh without mesh
reimport or directory deletion. One glTF primitive has UV1. No new exports,
duplicates or external binaries. Preparation succeeded; UE commandlet completed
with zero reported errors/warnings (script execution 1.00 s). Output remains
under ignored local/ and UE Content. Leaf-name resolution is bounded to this
known export set, not proof of general cross-package material identity.

The in-engine screenshot shows blue/pale metal paint replacing neutral grey.
The missing Master_Gun graph, inferred detail channel, HDR palette compression,
packed normal/emissive semantics, pattern placement, roughness and metallic
response remain UNVERIFIED. Other weapons still use their previous materials;
this does not establish stock shader parity.

Fresh runtime run local/inventory-actions/run-20260930-023458.log: 46/46 PASS,
including 2049 live head samples across the full 13-second armed idle. CTest
6/6 (17.39 s); all nine package checks match decoded bytes/counts/export fields.
Python syntax checks pass. Native computer-use pipe was unavailable, so no fresh
original-game trace or physical input validation occurred. Self-review only;
no independent critic rerun. Full menu projection, Inspect and appearance parity
remain open.

