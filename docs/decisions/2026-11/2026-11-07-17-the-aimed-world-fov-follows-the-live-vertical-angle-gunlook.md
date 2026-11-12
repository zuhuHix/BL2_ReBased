## 2026-11-07: the aimed world FOV follows the live vertical angle (gunlook)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/WEAPON_VISUALS.md` section 24.

- **Change:** `OpenWillowMovement::AimedFov` scales the zoom value in tangent by the live default over 70, then widens it by the aspect, for the world camera only. It replaces passing the zoom value as a horizontal angle. Hip FOV, foreground FOV, the gun's size and position are unchanged. Every aimed world frame moves, and so does the view kick while aiming.
- **Confirmed:** on the real game, by a measured 4-degree view turn and by magnification from frames, on two guns at 1280x720 and FOV setting 90. UNVERIFIED on other zoom values, aspects and FOV settings. In the host (all-in-one build): compiled, firing suite 14 of 14 with no row moved, hip-to-ADS world scale on frames as the rule gives.

