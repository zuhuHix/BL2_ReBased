## 2026-11-06: weapon patterns use their installed address modes; the pre-built fallback paints guns wrong (gunlook)

AI-assisted (Claude Opus 5.5; critic Luna). Record: `docs/verification/WEAPON_VISUALS.md` section 21.

- **Behaviour change (host look):** the paint tools, both importers and the thumbnail renderer use the pattern texture's installed
  address modes; patterns all wrapped before (48 of 226 patterned skins clamp or mirror). Needs `weapon_library.py skins`, then a skins
  re-import. Placement confirmed on one gun against its real Inspect frame (offline and in the engine); the others are UNVERIFIED.
- **Finding:** the white guns in Inspect came from the pre-built fallback in a worktree without the library. It logs one line; nothing
  fails. Open: suites should count those lines.
- **Critic:** 6.5 / 10 on that gun, target 8 not reached; highlights clip, while the same light fits the Widow Maker. Open.
