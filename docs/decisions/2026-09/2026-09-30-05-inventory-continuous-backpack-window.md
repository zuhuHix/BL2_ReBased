## 2026-09-30: Inventory continuous backpack window

AI-assisted presentation change reuses the existing UModel/Ruffle local payload
path; the project owns row selection, input routing and host validation. Seven
full backpack rows now scroll by one item instead of seven-item pages. Selection
crossing the window edge reveals the next item. Wheel input accumulates fractional
steps, reversals reset accumulation, and offsets clamp to the available list.
Page keys advance seven rows. Eight native cells are rendered under an AS2
scrollRect, leaving a clipped eighth-row preview; its HTML hit target is clipped
as well. Selecting the preview reveals that row fully.

Ruffle's existing scrollRect was checked with a small drawn-rectangle probe
before using it; no new tool or extraction architecture. Stock reference shows
seven rows and the next-row sliver. Browser measurement confirms about 9.5 px
of the eighth hit target remains visible. UE build succeeded (7.75 s), runtime
37/37 PASS (run-20260930-014550.log), including overlap ordering and top clamp.
CTest 6/6 (20.39 s); all nine decoded-package checks match. No extracted output
is tracked. UNVERIFIED: original-game wheel acceleration, bottom-edge physical
mouse interaction, stock horizontal focus navigation and independent critic.
The full 1:1 goal remains active.

