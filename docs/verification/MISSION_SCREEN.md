# Marcus's mission screen in the host (2026-11-06)

AI-assisted (Claude), mission UI lane. A pass of the host's suite means the host did this, not that it matches the game. "What the real game does" below reports the live captures; the host's own frames were seen by the runner only and have not been compared with the real ones side by side.

## What was wrong

The first scripted playtest showed one E accepting the mission with no text, no reward and no button, and one E turning it in with no reward screen, then a second E, with nothing prompting it, closing a page nobody could see. From the code, the cause is that `OpenWillowQuest::TryUse` runs Marcus's chain on the VM, finds what the screen would offer, and confirms it itself. The screen was logic with no page behind it (`UI_Mission.MissionInterface_Definition` was logged and nothing drawn).

## What the evidence says, and how good it is

| Rule | Source | Status |
|---|---|---|
| E at Marcus opens a screen; Enter on the offer accepts; Enter on the turn-in entry turns in; Enter again on a reward page takes the reward; the reward came 14.9 s after the turn-in, on that press | Live trace, [FIRE_MISSION_GOLDEN_TRACE.md](FIRE_MISSION_GOLDEN_TRACE.md) | Observed in game |
| Accepting closes the screen by itself, because the entry list is then empty | Live trace plus the installed script | Observed in game |
| After the reward the screen stayed open, listing the next mission | Live trace | Observed in game; the host closes instead, because the slice has one mission |
| Enter accepts, turns in and takes the reward, each on the key's release | Live capture, one run each | Observed in game, on the game's own handler. The mod below does not act on Enter here |
| Escape closes an offer | Live capture, one press, on the next mission's offer | Observed in game |
| Escape does nothing on the reward page: both handler calls return false and the page stays | Live capture, one press | Observed in game |
| The page-up and page-down keys scroll the description | Installed script (`QuestAcceptGFxMovie.HandleInputKey`), read with `research/script_disasm.py` | UNVERIFIED in game |
| What E and WASD do on these pages | The script binds no such key. One read at the offer, with the mod below loaded: the game's handler returned false on E's press and release, and the mod's own hook then sent `Enter`, so the press as a whole accepted. WASD was not pressed | Without the mod: UNVERIFIED until a stock run. The host's rule that they do nothing comes from the script |
| The prompts' text | The install's localization and `DefaultGame.ini`, through `tools/prepare_mission_screen.py` | The strings are the game's; where they appear is from the script |
| The world keeps running behind the screen; the reward page shows the heading, the XP and an empty gear row for this mission | Live captures, see "What the real game does" | Observed in game, with the mods loaded |
| Whether the player can move behind the screen | Not read: WASD were not pressed, and the mouse moved nothing though `IsLookInputIgnored` reads false | Open |

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
- `tools/test_mission_page.ps1` and `-NoMovie`: not run yet. E goes through the player's input binding, keys for the page through Slate, each no-op row checks the press arrived, and `tools/check_mission_page.py` requires the exact rows once each. With the movie and without, on main + #118 + #122 + fb613c5 (H14c, hummer, the page's rows then 20): both passed. The suite stops early if OWQUEST READY comes with no `page_ready` row in 120 s, which is what a stale DLL looked like.

## The reward page's gear row (2026-11-07)

The H14c frames, as described by the runner, show the gear heading with nothing under it. The real game's `reward_page` frame shows the same for this mission: the heading with nothing under it (one capture, the frame viewed by the author). Why, on the evidence:
- The Fire mission has no item reward: the live trace has `GetItemRewardsForPlayer` empty, and the lent pistol is taken back in the same step. So the pistol is not the row's content.
- The installed `SetUpRewardsPage` writes the gear heading whatever the items, and hides the credit and XP fields by amount; only `SetNoReward`, another path, hides the heading. `SetNumItemChoices(0)` runs the movie's own code, which was not read; the frame shows it leaves the heading.

Ruffle logs four parts of the reward clip as not registered when the page draws: `inventory - cell`, `eridium counter`, `item card` and `scanlines`, the library `SharedWillowMission`'s imports from SharedWillowInventory and SharedWillowComponents. Probably the same nested-import limit as above; not confirmed. The Fire mission has nothing to show in the first three, so they are not why the row is empty; `scanlines` is a small gap in the look. A mission with an item reward will need the cell and the card, and that is a later task.

## What the real game does (2026-11-07)

Three sessions with `tools/real_game/mission_screen_capture.ps1` on the zuhu PC, level 8, saves blocked and compared identical afterwards. The turn-in was reached by direct `UpdateObjective` calls, so the objective path is not what these record. A pass of the host's suite says what the host did; these say what the game did, with the mods below loaded.

**SDK mods.** Every session had these loaded, and the session record lists them: Better UI Controls, Dialog Skipper (autoskip on), Apple's Borderlands Cheats, Commander, Loot Collector, RTSE, Sanity Saver and others. Better UI Controls changes this screen: it hooks `HandleInputKey` to send `Enter` on the use key's release and to turn the movement keys into arrows, and `HandleRewardInputKey` to take the reward on the use key's press. So in these sessions E accepted the offer: the game's handler returned false on it, and the accept came from the `Enter` the mod sent (read from the mod's source, and it fits the rows). What E does with no mod loaded stays UNVERIFIED until a stock run. The other mods were checked with a text search of their code only. Dialog Skipper blocks friendly NPCs' dialog events, so no timing around lines is read here.

**Confirmed, on keys the mod does not touch (one run each):**
- E from the world opens the screen and nothing is accepted. Enter's release accepts, and the screen closes inside that press.
- Enter's release turns in: status and experience change in that press, before the reward is taken, and the reward page shows.
- The page waits. The mouse and Escape do nothing there. Enter's release runs `AcceptReward` and `MissionRewardsReceived`.
- With another mission available, the page then goes to that mission's offer rather than closing. The host closes, because the slice has one mission.
- The world is not paused under the screen: game time runs and NPCs idle. `bPauseGameWhileActive` and `bCaptureInput` read false. The mouse moved nothing, though `IsLookInputIgnored` reads false; why is unread.
- F on the offer reaches the handler, which returns false, and the screen stays.

**Space, one press, cause UNVERIFIED.** Space accepted the offer. The handler returned false on its press and release, and `extChoiceConfirmed` ran 3 ms later, with no `HandleInputKey` around it and no `Enter`. Better UI Controls does not mention Space. The likely route is the movie's own focused button taking Space as a click, but that was not read, and the mouse's route may be the same. The host's page takes keys itself and does not do this.

**The look, open and not done by the host:** the camera moves in close on the mission giver with the panel on one side, tilted; the HUD, minimap and tracker are hidden, as well as the gun; the panel has a level and difficulty row; the turn-in's body has a turn-in text and the objective lines; the prompt line sits under the panel; the reward page has an animated figure.

## The stall on the first E (2026-11-06)

H14 showed one frame of about 16 s on the first E, and none on the second; the page itself drew 0.4 s after it opened. The stall is not the page: it is in the VM's first `useMarcus`, which runs on the old path too. Timed on the zuhu PC with the core CLI (Release): the first use costs about 26 s and a second use almost nothing; no package is loaded for it. A timing trace puts all of it in two object loads in the dialog code, which caches them afterwards (`DialogSystem::load`, `instantiateExport` with depth 4): Marcus's dialog group about 15.5 s, then a talk act of that group about 17 s (this run's clock). Not fixed here.

## Open

- A stock read of E and WASD, with Better UI Controls off (the maintainer's decision, for one run): until then the host's rule for those keys is its reading of the script. Enter and Escape are observed.
- The flat layout against the real screen.
- The mouse: the script takes the left button on the reward page and on a one-mission offer, and the movie has a close button. The host takes keys only, and the page shows no cursor.
- The movie's 3D turn, the post-process override and the focus camera the real screen uses; the difficulty bar and the player's XP and money (the movie's `SetLevelAndDifficulty`, `SetPlayerXP`, `SetPlayerTotalCredits` are not called yet); the screen's sounds, which wait for the audio lane.
- **Dialogue:** Marcus's lines and subtitles stay out of scope until audio has a decoder. What they need: decoded WEMs for the lines the slice names (`tools/audio_slice_chain.py` already resolves them, but the dismissal line is not in its manifest), the two-step dialog dispatch that is only reported today, and a subtitle source and a place to draw it.
