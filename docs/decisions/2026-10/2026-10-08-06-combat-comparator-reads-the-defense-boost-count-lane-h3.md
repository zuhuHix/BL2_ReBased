## 2026-10-08: combat comparator reads the Defense Boost count (lane H3)

AI-assisted (Claude). `tools/verify_combat_pawn.py --live` now reads `L6/q7_rows.json` (boost count per hit, ignored local data) and computes the
applied-ratio rows from unboosted hits only. Adult level 6 has no unboosted melee hit in the dump, so that row is MISSING, not FAIL. New
`ratio_boost1`/`ratio_boost2` rows check the `1/(1 + 0.5 n)` factor lane L6 observed live (adult L5 and L6 melee and ranged L5 grenade PASS at 1e-3).
Checks: self-test on invented inputs passes; the live run gives 24 PASS, 0 FAIL, 5 MISSING of 29. Open: no 2-boost melee sample; status-effect and
empty-type hits at adult L6 with one boost show no reduction in 39 hits (adult L5 status hits do show it), so the boost rule for non-melee sources stays UNVERIFIED.

