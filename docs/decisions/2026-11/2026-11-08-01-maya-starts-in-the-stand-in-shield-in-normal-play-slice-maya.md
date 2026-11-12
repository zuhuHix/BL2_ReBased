## 2026-11-08: Maya starts in the stand-in shield in normal play (slice, maya lane)

Maintainer decision (2026-11-08): Maya may start with a shield, for the slice only; AI-assisted (Claude). Record: `docs/verification/COMBAT_DOWN_RESPAWN.md`, "Start shield".

- **Change:** normal play equips the stand-in shield at start, with its capacity from the local rule file (not tracked). `-ownostartshield` opts out, and the launchers that need no shield pass it (listed in the record).
- **Why:** the slice needs her shield bar full from the start.
- **UNVERIFIED:** whether the real game starts Maya in a shield; the stand-in rule's numbers.
- **Watch:** a shield swapped after the start starts empty (the first-equip rule), and the stand-in sits in the backpack under its test name.
- **Checked:** `tests/start_shield_test.py` (source text only) and the UE module build. Not run in the editor.
