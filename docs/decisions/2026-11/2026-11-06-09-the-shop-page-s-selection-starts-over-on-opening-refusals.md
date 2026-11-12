## 2026-11-06: the shop page's selection starts over on opening; refusals are shown in the stock labels

AI-assisted (Claude). From a playtest. The page kept its selection between machines; the host now counts openings and the page starts over on a new one. Refusals could go unseen: the host's one-second push wiped the page's own message, a press crossing that push was dropped as stale with no message, and the message was a small line under the lists. Now a refusal stays until the player moves, acts or the shop reopens, the timer push keeps the serial unless the stock changed, and a stale press says the stock changed.

- **Read from script, not from the running game (`UNVERIFIED`):** `VendingMachineExGFxMovie` shows one label for "cannot afford" (`INSUFFICIENT FUNDS`) and one for "cannot carry or use" (`FULL`), and the shop status that picks between them is native. So mapping a full ammo pool, full health and a full backpack to `FULL` is a reading. The label is plain page text, not the movie's feedback clip, and its sound is not played.
- **Verified:** synthetic page tests. The playtest's silent refusal was not reproduced, because the host path reads as sending its message; the causes above come from reading the code.

