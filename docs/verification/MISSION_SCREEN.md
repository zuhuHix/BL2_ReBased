# Marcus's mission screen in the host (2026-11-06)

AI-assisted (Claude), mission UI lane. A pass here means the host did this, not that it matches the game. Nothing below has been compared with a frame of the real game.

## What was wrong

The first scripted playtest showed one E accepting the mission with no text, no reward and no button, and one E turning it in with no reward screen, then a second E, with nothing prompting it, closing a page nobody could see. From the code, the cause is that `OpenWillowQuest::TryUse` runs Marcus's chain on the VM, finds what the screen would offer, and confirms it itself. The screen was logic with no page behind it (`UI_Mission.MissionInterface_Definition` was logged and nothing drawn).

## What the evidence says, and how good it is

| Rule | Source | Status |
|---|---|---|
| E at Marcus opens a screen; Enter on the offer accepts; Enter on the turn-in entry turns in; Enter again on a reward page takes the reward; the reward came 14.9 s after the turn-in, on that press | Live trace, [FIRE_MISSION_GOLDEN_TRACE.md](FIRE_MISSION_GOLDEN_TRACE.md) | Observed in game |
| Accepting closes the screen by itself, because the entry list is then empty | Live trace plus the installed script | Observed in game |
| After the reward the screen stayed open, listing the next mission | Live trace | Observed in game; the host closes instead, because the slice has one mission |
| The offer takes Enter (confirm) and Escape (close), and the page-up and page-down keys scroll the description | Installed script (`QuestAcceptGFxMovie.HandleInputKey`), read with `research/script_disasm.py` | UNVERIFIED in game |
| E does nothing on either page; Escape does nothing on the reward page, which takes Enter or the left mouse button | Installed script: no such key is bound there | UNVERIFIED in game |
| The prompts' text | The install's localization and `DefaultGame.ini`, through `tools/prepare_mission_screen.py` | The strings are the game's; where they appear is from the script |
| Whether the world keeps running behind the screen, whether the player can move or look, what the reward page shows for a mission with no item | Not known | Open: `tools/real_game/mission_screen_capture.ps1` reads these |

The host's second-E close was a stand-in, and the script says it is not how the screen works. So E now does nothing on these pages. If the live read disagrees, the rule changes in one place, `UOpenWillowQuest::OnScreenKey`.

## What the host does now

- **Page:** `tools/hud_overlay/mission_screen.html`, BL2's own movie `UI_Mission.MissionInterface` under Ruffle, created hidden at the start of play (`FOpenWillowMissionPage`).
- **Keys:** the host owns them. While the page is shown, every key press goes to `UOpenWillowQuest::OnScreenKey`, so the page needs no focus and a broken page cannot take input away from the keys that close it.
- **Maya:** the page uses the status pages' transition, `SetInventoryPresentation`: arms and gun put away, aim let go, and every close restores it (`CloseScreenPage`). The trigger's release and the firing gates that make this hold come with PR #118, so the row `held_trigger_does_not_fire_under_the_page` fails until it is merged.
- **One page at a time:** with a status page or a shop open, `TryUse` refuses before Marcus's chain runs. No key takes that route, since those pages hold the input, but closing the mission page over them would hand the game the input.
- **Flow:** E opens the offer or the turn-in, with nothing accepted yet. Enter on the offer accepts, and the VM's own close ends the page. Enter on the turn-in turns in and shows the reward page. Enter on the reward page takes the reward and closes. Escape closes the offer or the turn-in and is refused on the reward page. If Maya goes down, the page closes, and a reward still waiting is shown again at the next use.
- **The page only draws:** it writes the fields and calls the movie functions listed under "The movie's interface", from the host's mission log snapshot.
- **No page, old path:** without a page URL, or before the page is ready, `TryUse` confirms as it always did. That is what `tools/test_quest.ps1` and the slice gate run (both use `-owcanvashud`), so their flow is unchanged.

## A missing movie

The page loads through `movie_guard.js` (PR #122), so Ruffle's error panel never appears. Without the movie, the page shows a plain panel with the mission's name, description, reward and the prompts, and Enter and Escape work the same. Without the server at all, the page never reports ready and the quest takes the old path, with a warning in the log.

## The movie's interface

Read from the installed script and the converted movie. No script logic is transcribed; these are the names the movie answers to.

- **Frames:** the root plays `intro` into `missions` and stops there. `reward` plays into the frame that places the reward clip, one frame past the label. A write sent before the movie settles on its frame is lost, because the fields are created again on the way there.
- **Offer and turn-in:** under `missions`: `header` (frames `new`, `turnin`) with `header.description`, `description`, `tooltips`, `OptionalRequired` (frames `required`, `optional`) with `textField`, `selections`, `reward_weap`, `reward_other`. On the root: `SetRewardsXP`, `SetRewardsTotalCredits`.
- **Reward:** the `reward` clip is imported from the library `SharedWillowMission`: `SetNumItemChoices`, `rewardsHeader`, `missionTitle`, `description`, `rewards`, `xpReward`, `credits`, `cashHeader`, `reward_other`, `norewardtext`, `OptionalRequired`, `tooltips`.
- **Ruffle:** the library needs the nested-import workaround the inventory uses (`tools/prepare_inventory_movie.py`), or the reward clip never appears. Without the movie's own 3D the panels are drawn flat, left of centre.

## Local data

```powershell
python tools/prepare_mission_screen.py --reader build/Release/ow-package.exe --game "$env:OPENWILLOW_BL2"
```

It writes the two converted movies, the wrapper and `missionscreen.json` under ignored `local/ui/run`. It needs `SharedComponents`, `SharedWillowComponents` and `SharedWillowInventory` converted already (`tools/check_overlay_data.py` lists what is missing). Five of the movie's seven images are not in `Startup.upk`: they are named as weapon and player render targets, and the page draws without them.

## Checks

- `node tests/mission_screen_test.js` (the real page against a stub movie, DOM and guard), `python tests/prepare_mission_screen_test.py`, `python tests/check_mission_page_test.py`: synthetic.
- Headless Edge with real Ruffle, on `?demo=offer`, `turnin` and `reward`: each page draws its text and prompt with the movie; with the movie folder absent, each shows the plain panel and no error panel. Sample data only.
- `tools/test_mission_page.ps1` and `-NoMovie`: not run yet. E goes through the player's input binding, keys for the page through Slate, each no-op row checks the press arrived, and `tools/check_mission_page.py` requires the exact rows once each.

## The stall on the first E (2026-11-06)

H14 showed one frame of about 16 s on the first E, and none on the second; the page itself drew 0.4 s after it opened. The stall is not the page: it is in the VM's first `useMarcus`, which runs on the old path too. Timed on the zuhu PC with the core CLI (Release): the first use costs about 26 s and a second use almost nothing; no package is loaded for it. A timing trace puts all of it in two object loads in the dialog code, which caches them afterwards (`DialogSystem::load`, `instantiateExport` with depth 4): Marcus's dialog group about 15.5 s, then a talk act of that group about 17 s (this run's clock). Not fixed here.

## Open

- The live read above; until then the key rules and the flat layout are the host's reading of the script.
- The mouse: the script takes the left button on the reward page and on a one-mission offer, and the movie has a close button. The host takes keys only, and the page shows no cursor.
- The movie's 3D turn, the post-process override and the focus camera the real screen uses; the reward page's gear heading on a page with no gear; the difficulty bar and the player's XP and money (the movie's `SetLevelAndDifficulty`, `SetPlayerXP`, `SetPlayerTotalCredits` are not called yet); the screen's sounds, which wait for the audio lane.
- **Dialogue:** Marcus's lines and subtitles stay out of scope until audio has a decoder. What they need: decoded WEMs for the lines the slice names (`tools/audio_slice_chain.py` already resolves them, but the dismissal line is not in its manifest), the two-step dialog dispatch that is only reported today, and a subtitle source and a place to draw it.
