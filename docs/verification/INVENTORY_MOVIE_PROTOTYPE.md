# Inventory movie prototype — 2026-09-28

AI-assisted implementation and independent critic review. **Overall 5/10;
the requested 8/10 real-game comparison gate is not met.** This is an opt-in
prototype, not inventory, HUD, skills or weapon parity.

## External tools and ownership

The established UModel build 1590 export path remains the gun payload source;
no new weapon export or gun-material verification happened in this pass.
UI uses the already approved local converter and Ruffle web build
`0.7.0-nightly.2026.9.26` (source https://github.com/ruffle-rs/ruffle,
MIT OR Apache-2.0; provenance in THIRD_PARTY.md). No new runtime dependency.
The project owns stable recipe IDs, equipment state, validation, movie
callbacks and verification. All movies, recipes, screenshots and traces stay
under ignored `local/` or UE `Saved/`.

## Bounded library benchmark

The converted SharedWillowInventory library stopped preloading at its nested
imports in Ruffle. Inventory lacked its backpack, small gear cells and ammo
panel. `tools/prepare_inventory_movie.py` preserves tag bytes and moves the
three ImportAssets(2) tags immediately before End in this single-frame
library. It validates file/tag lengths and refuses multi-frame files. This
is a measured Ruffle workaround, not a general SWF linking solution.

Command (input is the untouched, previously converted local library):

```text
python tools/prepare_inventory_movie.py --input local/ui/bench/SharedWillowInventory.before-defer.swf --output local/ui/run/SharedWillowInventory/SharedWillowInventory.swf
```

Repeat benchmark: 0.104 seconds, 1 input, 1 output, 346697 bytes, 3 import
tags moved, no duplicate output files. Repeated output SHA-256 matches the
served library: `060ded58146592d9c779dd8b0340df84a765c85125330c414c5e7b8955b328f5`.
Backpack, four weapon cells, four gear cells and ammo panel became visible.
Ruffle still reports character-ID collisions and AVM1/font warnings; no
claim that all imports or callbacks work. Runtime thumbnail textures,
native list-provider integration, preview rendering and general nested
import support remain incomplete. Sample ammo/currency values are hidden.

## Implementation

`inventory.html` runs the installed StatusMenu inventory frame. It receives
`owInventory` snapshots with stable item IDs, slots, level and evaluated
weapon stats. Equip requests contain ID and destination slot; UE resolves
the ID and validates before applying. The page does not optimistically equip.
The latest source adds host and page level-requirement checks. Initial demo
loadout setup still uses the existing direct slot population path.

The card shows supplied damage, fire rate, reload speed and magazine size.
It does not infer accuracy from spread or invent flavor text, value or
elemental DPS. Type icon is scoped to the current Infinity pistol recipes.
Weapon names temporarily occupy thumbnail cells. No Maya menu preview,
drag/drop, header-tab switching, full gear system or original list sorting.

The launcher exposes `-InventoryMovie`; it adds
`-owflashinventory=http://127.0.0.1:<port>/inventory.html`. Without it the
existing inventory widget remains available. The automated combat capture
allows an additional 15 seconds for the imported inventory to initialize.

Skills changes: keyboard focus updates the selected skill, and the page
does not request a spend on a locked, maxed or zero-point skill. UE remains
the authoritative validator.

## Automated verification

```text
ctest --test-dir build -C Release --output-on-failure
100% tests passed, 0 tests failed out of 6
Total Test time (real) = 9.07 sec
```

`ctest` is not on PATH here; the executable recorded in CMakeCache.txt was
used. `python tools/verify_packages.py --reader build/Release/ow-package.exe`:

```text
Core: 234397 bytes, 1621 exports; decoded bytes, counts and export fields match
Engine: 5878264 bytes, 33166 exports; decoded bytes, counts and export fields match
GameFramework: 61714 bytes, 258 exports; decoded bytes, counts and export fields match
GearboxFramework: 1224040 bytes, 7098 exports; decoded bytes, counts and export fields match
WillowGame: 13054200 bytes, 56443 exports; decoded bytes, counts and export fields match
GFxUI: 136680 bytes, 841 exports; decoded bytes, counts and export fields match
IpDrv: 230751 bytes, 1364 exports; decoded bytes, counts and export fields match
OnlineSubsystemSteamworks: 265760 bytes, 1709 exports; decoded bytes, counts and export fields match
AkAudio: 39503 bytes, 176 exports; decoded bytes, counts and export fields match
```

Synthetic checks: library workaround 3/3, converter 8/8, weapon stats 3/3,
skill stats 4/4, skill info 9 cases. The local golden text check reports
55 exact matches plus 8 exact before a class-mod block, 0 missed, among 63
recorded texts; this does not verify every skill or every state.

Local browser check `local/ui/bench/menu_interaction_check.py`, Edge at
1280x720: correct stable-ID equip request, snapshot refresh, replay after
reload, page level guard, keyboard skill selection, locked/zero-point spend
suppression and a valid spend request pass with no page exceptions. These
use simulated host responses, not a live UE equipment round trip.

## Visual comparison and runtime limits

Original-game references and source URLs are recorded in
`local/ui/ref/sources.md`. Inventory reference: `inventory_weapon_real.jpg`;
focused Maya reference: `skills_maya_focused.jpg`, plus the maintainer's
`_crop_real.png`. Browser captures: `local/ui/bench/inventory_imported.png`,
`inventory_after_snapshot.png`, `skills_keyboard_verified.png`.
The latter is captured after asynchronous icon loading settles and shows
rank counters and the selected skill. Older UE captures do not prove this
revision. These references are not a matched current live-game session.

Independent critic: **5/10 overall**. Imported frames/fonts are recognizable;
missing weapon thumbnails, Maya/environment composition, inventory details
and original backpack geometry prevent 8/10. Skills layout/state differences
remain; no new host screenshot was obtained. No new gun parity claim.

UE 5.8 compiled the inventory adapter and moved-active-slot correction.
The subsequent launch exited before loading the map: error 4551 loading
`UnrealEditor-OpenWillow.dll`. Windows formats this as
"An Application Control policy has blocked this file." After adding the
host level guard, rebuilding also failed loading `OpenWillowModuleRules.dll`
with 0x800711C7, then reported a file-in-use error during recompilation.
**The latest host level-guard source is not compiled or runtime-verified.**
No security settings were changed. Native computer-use inspection was also
unavailable (missing native pipe after retry/reset).

Next gate: restore normal authorized UE build/load capability, build the
latest source, then verify equip/reject/move-active-slot and skill spend /
close/reopen in UE; capture matching original-game states. Thumbnails and
Maya preview remain implementation work, not just missing test evidence.
