## 2026-10-07: gun muzzle and eject points from the decoded gestalt sockets

AI-assisted (Claude), implementer lane I5, following lane R1's live cross-check. The gestalt array schema is now tracked
(`tools/gestalt-arrays.schema`: the two part-table lines plus socket mappings, part bounds and mesh sockets). The new
`tools/gestalt_sockets.py` decodes the six weapon families' sockets into ignored `local/gestalt/`, and the recipes carry them
(`weapon_refresh_fragments.py --sockets-only`). The host's muzzle flash and tracer now start at the held gun's decoded `Muzzle` socket
(bone transform times the cooked offset); the old "27 cm past the Barrel bone" estimate is a logged fallback, used 0 times in the suites
and gun-shot runs. The estimate was 14 cm too far on the slice pistol, and the SMG has no Barrel bone. `EjectPort` is exposed but the
host spawns no casings. Fixed: `slice_npc_assets.py` kept only the last range of a fragment name with several ranges. Tests:
`tests/gestalt_sockets_test.py` (9) and `tests/slice_npc_assets_test.py` (2, needs numpy). Checks: ctest 11/11, verify_packages 9/9, UE build
succeeded, quest 98/98 + 11/11, door 16/16, inventory 49/0. The socket values match the live game (R1); that the game's own flash starts
there, and our axis and scale conversion, are UNVERIFIED (an in-engine check against the offline mesh agrees to 0.00 cm).

