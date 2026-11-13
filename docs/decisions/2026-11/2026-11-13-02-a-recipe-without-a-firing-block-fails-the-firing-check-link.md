## 2026-11-13: A recipe without a firing block fails the firing check; link 8 names the gun's firing

AI-assisted (Claude Code). Two leftovers of the 2026-11-12 sweep (a tool or check keeps or refuses, it never guesses in
silence) concerned how a gun fires.

**What the host does with such a recipe.** `UOpenWillowInventory::ReadRecipe` parses the `firing` object only when it is
there and has schema 1 (`FOpenWillowFiringData::Parse`). Otherwise the item keeps no loaded block, and the walker fires the
plain instant trace for every shot, with the card's pellets and spread. The firing test already logged
`OWFIRING warning id=<gun> no firing block: fired as a trace`; normal play and a refused block said nothing.

**The firing checker.** `tools/check_weapon_firing.py` graded a recipe without a block as a trace gun, so the
`weapon_firing` suite could pass a gun whose firing was never read. Now such a recipe, or one whose block the host would
refuse (not an object, schema not 1), is a FAIL on its own line that names the recipe file and
`tools/weapon_firing_coverage.py --game <BL2 folder> --only-firing`, and nothing else is graded for that gun. A trace gun
says so in its block (`fire_type` `EWWFT_HitScan`). The host's own "no firing block" warning also fails the gun. No switch
for old recipes was added: `tools/test_weapon_firing.ps1` already regenerates missing blocks before a run and stops if one
is still missing, so no documented use needs one.

**The host.** `ReadRecipe` now logs `OpenWillow <id> firing: block not read (not a schema 1 object): the host fires a plain
trace` when a recipe has a block it refuses. No behaviour change.

**Link 8 (`equip_gun`).** After the equip and draw, the link adds the held gun's firing kind to its detail
(`firing=Trace`, `firing=Bullet` or `firing=Rocket`) and FAILs with `firing=none` when no block was loaded.
`tools/check_slice_gate.py` overrules a PASS without a loaded kind, so a log from before this change no longer passes link 8.
The run does not fire the looted gun after equipping it (the mission's shots use the lent pistol), so its shots are not
compared with its block; that needs a new step in the run and was left out.

Verified: synthetic tests only (`tests/check_weapon_firing_test.py` 99, `tests/check_slice_gate_test.py` 70). The checkers
now check that the host did what the recipe says, nothing about the real game: the firing blocks stay `UNVERIFIED` in game.
The C++ is not yet seen in an in-engine run. In the data this worktree was seeded with, 28 of the 51 recipes in the firing
test's folders have no block, all of them in `local/items`; one of them is on the suite's default gun list, and the runner
regenerates it before a run.
