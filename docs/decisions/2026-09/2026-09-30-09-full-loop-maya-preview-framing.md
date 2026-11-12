## 2026-09-30: Full-loop Maya preview framing

AI-assisted presentation refinement reuses existing UModel 1590 body/animation
payloads and the project-owned MD5-to-UE converter. No extraction/import or
animation-track edit. Forward kinematics on the local converted Rifle idle
(381 frames / 30 fps) finds a stationary Root, authored Hips ranges
x -4.70..8.21 / y -11.21..16.65 cm and Head x -5.12..5.84 /
y -10.16..11.69 cm. Thus the earlier sideways movement is authored animation,
not accumulating root drift. The base unarmed idle's head y range is
-10.19..2.14 cm; these measurements do not prove exact original-game playback.

Default horizontal preview anchor moves from .79 to .86 to fit the stock
(default outfit) reference's right-side composition over the whole armed loop.
The authored motion is retained. The action runner now projects the live Head
bone throughout 13 seconds, slightly longer than the 12.7-second source clip,
and checks its normalized position stays to the right of the backpack region
and on screen. This measures the head anchor, not all silhouette pixels or
weapon-specific hold definitions.

Verified: UE build 11.49 s; runtime 46/46 PASS,
`run-20260930-021751.log`: 2,110 head samples, x .806.. .906 / y .319.. .360.
Fresh open and empty-target captures show the face clear of the backpack at
the sampled poses. CTest 6/6 (22.01 s), all nine package checks match, diff
check passes. No independent critic or matched original full-loop capture;
exact body/weapon clipping, stock initial pose/hold selection and full visual
parity remain incomplete. All local payloads/screenshots remain ignored.

