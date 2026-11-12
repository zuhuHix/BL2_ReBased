## 2026-11-11: The unforced slice gate claims only what happened (review round 1 of #218)

AI-assisted (Claude Opus 5.5). Follows the entry above; test behaviour only. Record: `docs/verification/SLICE_GATE.md`, "The unforced variant".

- **Grading.** In an unforced run every NOT_AVAILABLE is now a FAIL, so the run exits 0 only with all 14 links passed. Before, a mission skipped while Maya was down graded 11 PASS, 3 NOT_AVAILABLE and exited 0.
- **Mission page.** `how=mission_page` is printed only when the page did the work. At the accept that means status 1 after an Enter on the offer page. At the turn-in it means status 2 to 3 after an Enter sent while the turn-in page was up, told apart from the reward page by the status. If the offer page does not accept within 8 s, the driver accepts with a direct `Accept()` call, named as a new forcing fixture, so the objective and turn-in links still run.
- **Down.** A down counts as an enemy's only with a hit whose causer is a range enemy. The count starts at the end of Recover for a down that came before link 13, and at the start of the wait for a down during it.
- **Loot.** A gun near any range enemy that died during the link counts as a kill drop (a stray shot's kill included), and a far gun fails the link only after the drop wait.
- **Other.** The runner's limit for `-Unforced` is 2,100 s. The skills-page token names the last key kind (`key=slate` or `key=page_event`).
- **Verified (automated):** `tests/check_slice_gate_test.py` 58 of 58. The older variants still grade the same as with the checker on main over 400 random logs.
- **Not verified:** still no compile (build slot held) and no engine run. Whether `Accept()` accepts after the page was closed with Escape is UNVERIFIED.

