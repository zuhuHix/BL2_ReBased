## 2026-10-23: Weapon part library, PR #58 review round 2

AI-assisted (Claude Opus 5.5; review GPT-6-Luna). Record: `docs/verification/WEAPON_PART_ASSETS.md` section 6c.

- Material instances are per component (a held gun's glow no longer reaches a pickup of the same gun); a gun needs Body, Grip,
  Barrel and Material parts and a known weapon type; unmapped or unloadable section materials fall back instead of taking the
  gun's skin; the oracle fails an empty or short index; the reload record is repository-relative; `-owgunnolook` clears old
  overrides.
- Checks: `OpenWillow.WeaponAssembly` and `OpenWillow.Skills` pass; gun shots with oracle 39/39 (`--expect 39`), no fallback;
  CTest 10/10, packages 9/9.
- After rebasing onto PR #57: eight generated guns (`ow.RollGun` from pools, every weapon type) assemble with no fallback and match
  the generator's parts exactly (fragments and triangles); inventory suite 49/49; CTest 12/12, packages 9/9.

