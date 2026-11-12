## 2026-09-30: Equipped transfer state and stock-sized comparison cards

AI-assisted, same local SWF/Ruffle payload path. Recorded callbacks show
equipped-to-backpack transfer, `TweenPanel` compare positions and left-origin
`TweenCards`. Enter/E on equipped now pins that source, filters compatible
backpack candidates and preserves the comparison while moving selection.
E/Enter confirms through the existing validated host equip path; Escape
restores source selection/category without closing inventory. Backpack Enter
remains a host direct-equip shortcut, not a verified stock focus sequence.

A newly consulted [stock comparison screenshot](https://www.thatgamesux.com/borderlands-2-can-there-be-too-much-loot)
confirms that large cards intentionally overlap the upper equipment/backpack
panels. The previous host-chosen 55% cards were incorrect. Recorded
75/81/81/75 scales and positions are restored; equipped source is the left
highlight card and the candidate is the right comparison card. Numeric fields
must be reasserted after the movie tween because its native completion callback
is absent. Host overlays covered by cards are hidden, including category arrows.

Verified: UE build, 35/35 in-engine actions, source preserved on selection,
268.1/289.6 card widths and both first-stat fields visible after tween; settled
capture reviewed. Browser E produces the expected candidate-ID/slot request.
CTest 6/6 (8.47 s); all nine package byte/count/export checks match. Details in
the inventory verification record. No extracted payload or reference image is
tracked. Not verified: exact 3D perspective, full stock focus behaviour,
physical mouse interaction, other sort/inspect states or independent critic.

