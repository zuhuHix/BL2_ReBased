## 2026-09-26: Maya's Skills tab populated through the StatusMenu movie

The real-game UI trace under ignored `local/ui/traces` showed the host's
`SetupSkillTree` opening `skills`, then calling movie methods to set the class
portrait, points and branch names. The movie called `extInitTree` for its three
branches. A browser probe of the converted StatusMenu verified that these
methods, `SetCellVisible`, `SetInfo`, and `loadMovie` on each icon container
render Maya's data from `tools/prepare_skill_tree.py` without copying menu
script logic into the project.

`tools/hud_overlay/skills.js` waits for all three movie callbacks, then fills
Maya's 30 skill cells, action-skill art, portrait, labels and descriptions.
Its transparent HTML hit targets follow the movie clips' `getBounds` after the
opening tween so hover and branch arrows work in Ruffle. The UE host opens the
page with **K**, sends its current zero skill points and restores game input on
close. The standalone `?points=N` value is only for visual checks.

Automated extraction check: the local JSON has three branches and 30 skills;
all 32 distinct referenced movies have SWFs and converter reports, with zero
missing external textures and zero dropped tags in those reports. Visual check
in a local browser at 1280x720: all 30 icons render; hovering Mind's Eye
updates the info panel; the right arrow centres Cataclysm. The movie still
logs Ruffle character-ID collisions and AVM1 stack underflows. A UE 5.8
Sanctuary game-window capture at 1280x720 shows the populated menu with Maya's
portrait, action-skill description, three branches and icons. The first
capture at 3 seconds after opening was blank because the imported movie had
not initialized; the menu logged ready about 4 seconds after opening and
rendered in a later capture. The automated UE run also observed the page's
close route restoring the game view. Manual pointer and keyboard interaction
in the UE window, skill spending, earned points and skill effects remain
UNVERIFIED.

