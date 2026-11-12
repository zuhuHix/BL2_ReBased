## 2026-09-27: Skill-tree branches slide in a row, as traced (no wrap-around)

Maintainer report: the Skills trees "rotate fully" instead of the selected
tree popping forward. The page had cycled the three trees modulo 3. The
real-game UI trace (local, ignored) shows what the game does on each arrow:
`extGenericButtonClicked("arrowright"|"arrowleft")`, then the movie's own
`BubbleSortBranchDepths(n)` with n = the new front branch (1-based), then
`TweenBranch` for all three branches. For offset d from the selected branch:
X = 15 + 330d, Y = 17, Z = -5500|d|, scale 100, alpha 100 - 15|d|, duration
0.3 s. Observed for Harmony and Cataclysm in front (Motion at X -645,
Z -11000, alpha 70 in the latter) and for Motion in front. So the trees keep
one row order and slide as a unit; the front tree comes forward in depth.

`tools/hud_overlay/skills.js` now sends those values and calls
`BubbleSortBranchDepths`. Selection is clamped at the ends. The trace never
pressed an arrow at an end, so wrapping there is unobserved (UNVERIFIED).

Ruffle ignores the Z coordinate, so the page projects it in 2D: scale by
f = 13200 / (13200 + |Z|) about a parent-local centre (-209, 12). Both
constants were fitted to the maintainer's real-game capture, after removing
the global offset and 1.087x scale of the game's 3D menu plane (fitted from
two Harmony cells at Z = 0, residual under 2 px). That gives Cataclysm at
70.6%, tucked partly behind Harmony as in the capture. The fitted Cataclysm
cells land within 4 and 9 px of the capture, but three samples of the
horizontal centre scatter by about +/-20 px. The game's 3D plane may also
rotate the side trees, which a 2D scale cannot show. Two steps back (f = 0.545)
has no reference capture.

Checked in headless Edge with Playwright (local bench under ignored `local/`)
at 1280x720: arrow keys slide the row, the front tree has the highest depth
after each move, and a second right arrow at Cataclysm changes nothing. The UE
5.8 check was attempted but not completed: the OS-input helper could not bring
the UE window forward, so its keys went to another application. That run was
stopped and its captures deleted. The UE view of this change is UNVERIFIED
until the maintainer checks it by hand.

