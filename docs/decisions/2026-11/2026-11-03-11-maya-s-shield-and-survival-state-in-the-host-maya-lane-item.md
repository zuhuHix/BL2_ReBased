## 2026-11-03: Maya's shield and survival state in the host (maya lane, item 2)

AI-assisted (Claude). Record: `docs/verification/COMBAT_DOWN_RESPAWN.md` "The host's state". Host behaviour change; no new live reads.

- **New `FOpenWillowMayaState`** (`OpenWillowMayaState.{h,cpp}`): health, shield and Fight For Your Life move out of
  `OpenWillowWalker.cpp` unchanged, with every stat in one struct (`FOpenWillowMayaStats`) for the skills lane's modifiers.
- **Shield (behaviour change).** Hits take the shield first, then health; the recharge waits the delay after each hit, then rises at
  the rate. Numbers come from the equipped shield's card, else the live read of one shield as a stand-in (a save value, replaced on 2026-11-12).
  No recharge while down, a swapped shield starts empty, the second wind sets the shield to 90 % (the last is live; the rest UNVERIFIED).
- **`OpenWillowApplyDamageToPlayer`** is the one damage entry point for enemies (range lane) and hazards.
- **Checks.** New automation test `OpenWillow.MayaState` (synthetic). In-engine results come from the hub's integration run.

