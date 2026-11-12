## 2026-11-08: The game's UI colours move out of the code into an ignored local file (colours lane)

AI-assisted (Claude). Maintainer approved 2026-11-08. Exact colours taken from the game (rarity tiers, card and
menu text tints, the inventory band and tile, the skill info box, the skill-tag colour, the red flavour line)
are game-derived data, so the running code no longer carries them. They live in the ignored
`local/ui/colours/game_colours.json`. `tools/game_colours.py` writes it: the five rarity tiers are read from the
install (`GD_Globals.General.Globals` RarityLevelColors in Startup.upk, first row per rating, with `--reader`
and `--package`); the other keys are measurements or trace observations and come from a private `--seed` notes
file. The file travels between machines with `tools/private_sync.ps1`, which now mirrors `local/ui/colours` by
default. Pages get it from `serve.py` as `/game_colours.js`; `tools/hud_overlay/colours.js` hands keys to
`item_card.js`, `inventory.js`, `skills.js`, `skill_info.js` and CSS variables (`vending.html`,
`inventory.html`). Python reads it through `game_colours.colour()` (`prepare_skill_tree.py`'s skill tag,
`weapon_stats.py`'s red-line test). A seed value wins over the package.

Finding, for the maintainer: the install's RarityLevelColors differ from the old hard-coded fallback tiers for
uncommon, rare and very rare (common and legendary match). The fallback was only used for items without a
host `rarityColor` and for the vending page's row edges; items generated from the packages already used the
install's table. This machine's file was seeded from the old values so nothing changes on screen; dropping
those three keys from the seed makes the install's values win. Which is right is UNVERIFIED.

Without the file every key falls back to a neutral grey (or white) that is plainly not the game's, and the
page logs `OpenWillow game colours missing` once (Python: one stderr line). `weapon_stats.py` then recognises
no flavour line as red, so regenerated cards lose their red line until the file is there. Tests use made-up
colours only. Old commits still hold the earlier values; the history is not rewritten (maintainer's call).
Plain-white defaults outside the item card (the inventory page's own text helpers, weapon_stats' white lines), page chrome of the host-made stand-in pages (loading screens, vending
page, glass sheet) and host choices recorded as such stay in the code. Verified: the JS and Python tests
listed in the PR. Not verified here: the in-engine inventory and skills suites with and without the file
(handed to the batch runner); UNVERIFIED that the pages look unchanged with the file present.

`README.md.bak`, a stray tracked backup, is deleted (maintainer said yes); it held GitHub diagram styling only.

Review r1 (Luna, 2026-11-08): the five rarity fallbacks are now one mid grey (white was the stock Common);
a colours file that is valid JSON but not `{"colours": {...}}` counts as unreadable and warns;
`prepare_skill_tree.py --output` and `game_colours.py --output` must resolve (links followed) inside this
checkout's `local/`. A junctioned subfolder of `local/` that points elsewhere is therefore refused too.
`private_sync.ps1` resolves junctions and symbolic links on the store path before refusing a store inside the repository.

Review r2 (Luna, 2026-11-08): `private_sync.ps1` no longer compares path strings. It writes a uniquely named marker
file in the repository root, refuses a store when the store or any existing ancestor holds that marker (so link
chains of any length, UNC shares of the checkout and other spellings are caught), and always deletes the marker.
The item card's plain text (flavour lines other than the red one, and its text helpers' default) reads the new
`card_plain_text` key; without the file it is a light grey.


