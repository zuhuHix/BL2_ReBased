## 2026-11-04: the quest's on-screen text hides behind the status pages; mission log states checked (inventory lane)

AI-assisted (Claude). Record: `docs/verification/MENUS_PARITY.md` "Mission log". Host behaviour change.

- `UOpenWillowQuest::UpdateHints` no longer posts its tracker and talk lines (on-screen debug text) while a status page is
  open; they drew over the Inventory, Skills, Missions and Map pages under `-owquest`.
- The Missions page shows the Fire mission's four states (accepted, at the range, ready to turn in, turned in) in BL2's
  mission log with the expected category row and header (host captures from resumed saves; UNVERIFIED against the game).
- New capture switch `-owstatusshots=<page>[,<name>]` (`OpenWillowStatusShots`): opens a status page, captures it, quits.


