## 2026-09-30: Default Maya head preview correction and crop anchors

AI-assisted inventory-only material adjustment. Existing UModel 1590 external
binary (official gildor2/UEViewer source) exports `CD_Siren_Skin_Default_SF
Mati_Default_Head MaterialInstanceConstant` with `-game=border -export -png`.
Fresh representative export: 0.773 s, exit 0, eight files / 6,508,229 bytes:
two material descriptions, two property dumps, four PNGs (head diffuse/normal/
mask plus referenced Fire_Tile). Zero failed exports or duplicates in the fresh
output; Master_Player's cooked graph remains unsupported, not reconstructed.
Output: ignored `local/external/umodel/maya-menu-head-palette-20260930`;
log `local/ue-import/inventory-head-export.log`. Parent/texture references agree
with the earlier default head export; the head property dump SHA256 matches.

The look script reads that local palette and creates MI_InventorySirenHead,
parented to the imported head instance. The existing factor-two shader is
compensated for the face, while hair uses the default dark-blue shadow colour
at half gain. Only the inventory display actor applies this material; original
mesh assets retain their materials. This is an explicitly UNVERIFIED visual
translation of the missing stock shader, not proof of original ramp math.
Engine captures show dark-blue hair instead of violet and less pale face;
body colour/ramp and matched original-game lighting still need work.

A regression from the crop change was found in fresh captures: immediate
getBounds after setting scrollRect yielded transient displaced header anchors.
The page now captures row/header anchors before setting the crop. Browser
controls sit above row one (~169-193 px), rather than beside the tabs (~52-76).
Engine open/reopen verification now asserts that relationship. No independent
critic; 1:1 menu remains incomplete.

Verified final follow-up: UE build 7.95 s; runtime 37/37 PASS,
`run-20260930-015423.log`, including header placement on open/reopen/Skills return.
CTest 6/6 (15.60 s), all nine package checks match, Python/JS syntax/diff checks
pass. Fresh capture confirms corrected header and preview palette load.

