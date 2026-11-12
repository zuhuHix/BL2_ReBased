## 2026-09-30: Backpack-origin inventory transfer

AI-assisted behavioural correction using existing original-game UI Trace SDK
observations. `_220818` seq 23550 starts equip from InventoryListPanel, seq
24230 passes the backpack item to equipped-panel StartEquip with type-cell
selection, and subsequent TweenCards records bStartedFromLeftPanel=false.
This complements the already implemented equipped-origin transfer. No new
extraction tool or asset export; UModel/Ruffle payloads remain local.

E/Enter on a backpack item now starts a pending transfer rather than equipping
immediately. The source stable ID stays pinned while choosing an unlocked
weapon slot; comparison follows its current occupant. Gear can only target its
matching slot. Confirm submits the existing host-validated equip; Escape keeps
the source in the backpack and leaves equipment unchanged. Empty slots remain
valid pending targets and show Equip rather than Swap, with no invented
comparison item. Selected previews remain on the backpack source. Clicks on
equipped slots during left-origin transfer no longer silently change its
pinned destination. Snapshot refresh only cancels a transfer if its source
vanishes, rather than cancelling every backpack-origin transfer.

Panel/card tweens now distinguish transfer direction and keep the transfer
layout for empty destinations. Both panel focus arguments follow observed
comparison calls. Existing action checks were updated to start then confirm;
new checks cover pinned source, destination comparison, cancel and empty slot.
Stock initial analogue selection, exact keyboard grid traversal and click vs
hover timing remain unverified. Full 1:1 goal remains active.

Final checks: UE build 7.84 s; runtime 45/45 PASS
(`run-20260930-021213.log`), occupied/empty right-origin captures reviewed;
CTest 6/6 (21.64 s), all nine package checks match, JS syntax/diff pass.
Animated preview can overlap the backpack; full-cycle pose/framing remains
unverified, alongside the other full-parity gaps.

