## 2026-10-08: Script swap 11, Marcus's mission screen closes at the accept and the reward page waits for the player

AI-assisted (Claude), implementer lane I10b (continuing lane I10).

- **What:** the VM keeps a stand-in `QuestAcceptGFxMovie` (plus a `WillowGFxUIManager` on the controller) while Marcus's screen is open. `GetPlayingMovie`, the movie's native `Close` (raising an emulated `OnClose`:
  `ServerPlayerEndUseNPC`, then `OnPlayerClosedMissionUI`), `PlayUISound` (no-op), `DisplayRewardsPage` (waits) and `DetermineQuestEntries` (rows built in C++) are the host's; the controller's and the movie's
  other script runs unchanged. Accept order: `dismissal_dialog`, `dialog`, `mission_interface_closed`, `status Active`, `mission_weapon_granted`. Turn-in with the screen open: `experience`, `reward_page`, `status Complete`,
  weapon removed, `reward`, `turn_in_dialog`; `MissionRewardsReceived` waits for the player (`--slice-run` step `confirmreward` gives `reward_received`; `close` is Escape). Host: the use key confirms the page and closes the screen.
- **Bug fixed on the way:** the row builder iterated a temporary's elements (freed memory), so the list stayed empty and the screen never closed. The VM's untyped-empty-array limitation was not changed.
- **Verified (live, lane L2/L4 and installed script):** the close order inside the accept, the dismissal tag, the reward confirmation after the turn-in, the screen not closing by itself after it. **UNVERIFIED:** the
  VM's emulation of `OnClose` (one `EndUse` against two live), the dismissal line's talker (live: another NPC; VM: no talker), that the use key closes the screen after the confirmation (a host choice), everything not about order.
- **Checks:** ctest 11/11; verify_packages 9/9, 3,339/3,339; verify_fire_event_order self-test and 24 of 24 (18 old, two reworded for the new events, 6 new); UE module build succeeded; test_quest first run 107/107 (105 +
  `accept_closes_mission_screen_with_dismissal`, `reward_received_on_confirmation`), resume 11/11; test_mover 16/16; test_inventory_actions 49 pass / 0 fail. FIRE_MISSION_GOLDEN_TRACE.md: 45 matched, 2 out of order, 0 missing, 4 extra.

