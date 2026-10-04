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
claim that all imports or callbacks work. Native list-provider integration,
live rotating previews, original materials and general nested import support
remain incomplete. Sample ammo/currency values are hidden.

## Implementation

`inventory.html` runs the installed StatusMenu inventory frame. It receives
`owInventory` snapshots with stable item IDs, slots, level and evaluated
weapon stats. Equip requests contain ID and destination slot; UE resolves
the ID and validates before applying. The page does not optimistically equip.
The latest source adds host and page level-requirement checks. Initial demo
loadout setup still uses the existing direct slot population path.

The inventory cells show static-pose weapon renders from the corresponding
locally exported skeletal meshes. `tools/render_weapon_previews.py` reads the
UModel 1590 glTF exports and writes small transparent PNG previews under ignored
`local/ui/run/previews/`; rerun it after provisioning the ignored item exports.
The renders use mesh pose and flat neutral shading; original BL2 materials and
live rotation are not included. The card shows supplied damage, fire rate,
reload speed and magazine size.
It does not infer accuracy from spread or invent flavor text, value or
elemental DPS. Type icon is scoped to the current Infinity pistol recipes.
Weapon names remain accessible to screen readers but are not drawn over the
thumbnail cells. No drag/drop, header-tab switching, full gear system or
original list sorting.

The launcher adds
`-owflashinventory=http://127.0.0.1:<port>/inventory.html` by default;
`-NoInventoryMovie` selects the fallback host inventory widget. The automated combat capture
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
revision. The latest UE capture is
`host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor/OWCombat_7_Inventory.png`;
it shows the StatusMenu movie with mesh thumbnails over Sanctuary. This is a
visual runtime check, not a matched original-game comparison.

The prior independent critic score remains **5/10 overall** and predates these
thumbnails. Imported frames/fonts are recognizable; Maya/environment
composition, inventory details and original backpack geometry still differ.
Skills layout/state differences remain; no new gun parity claim.

UE 5.8 compiled the inventory adapter and moved-active-slot correction.
The subsequent launch exited before loading the map: error 4551 loading
`UnrealEditor-OpenWillow.dll`. Windows formats this as
"An Application Control policy has blocked this file." After adding the
host level guard, rebuilding also failed loading `OpenWillowModuleRules.dll`
with 0x800711C7, then reported a file-in-use error during recompilation.
**The latest host level-guard source is not compiled or runtime-verified.**
No security settings were changed. Native computer-use inspection was also
unavailable (missing native pipe after retry/reset).

## Follow-up UModel inventory-movie smoke — 2026-09-28

UModel / UE Viewer build 1590 from the official
`https://github.com/gildor2/UEViewer` repository (checkout
`a0bfb468d42be831b126632fd8a0ae6b3614f981`) remains the first external
candidate. The local executable was
`C:\Users\yorad\Tools\UEViewer\umodel.exe` (SHA-256
`13502E5A4D8F6B5F32252AFEBD6360F7302CCFACCF6B8DDA65BEFF0BE2D364A0`);
Windows PE metadata says `0.0.0.0`, so the build number comes from the
existing project benchmark record. A bounded attempt against the installed
game used `-path=<BL2 CookedPCConsole> -game=border -export -gltf
-nooverwrite -uncook -out=local/external/umodel/inventory-swf-smoke-20260928-object
Startup StatusMenu`. A PowerShell stopwatch around the process and output pipe
measured **0.19 s**. It scanned 920 files (15 skipped), found one object,
reported `Unknown class "SwfMovie"` and `no supported objects`, and exported
zero assets. One 16,108-byte log was written under ignored `local/`; there were
no duplicate exports. A dotted object selector did not match (0.15 s), while
`-list Startup` identified `SwfMovie StatusMenu` and
`SwfMovie SharedWillowInventory`. The established local
`tools/extract_swfmovie.py` and converted movie path remains necessary for UI
payloads. This does not change UModel's role for supported meshes and textures.

Next gate: restore normal authorized UE build/load capability, build the
latest source, then verify equip/reject/move-active-slot and skill spend /
close/reopen in UE; capture matching original-game states. Thumbnails and
Maya preview remain implementation work, not just missing test evidence.

## Host snapshot fields (2026-09-29)

AI-assisted. `UOpenWillowInventory::StateJson` (OpenWillowInventory.cpp) now
also sends these top-level fields to `window.owInventory`. Optional fields are
omitted when the host has no value; nothing here is a claim of parity with the
game's inventory.

```json
{"slotsUnlocked":4, "money":1234567, "eridium":42,
 "ammo":{"pistol":{"current":166,"max":200}, "smg":{...}, "ar":{...},
         "shotgun":{...}, "sniper":{...}, "launcher":{...},
         "grenade":{"current":2,"max":3}},
 "activeAmmoType":"pistol"}
```

Each weapon item also carries `ammoType` (same keys) when it resolves. `items`
lists every held weapon and gear item, equipped ones included; `backpackCount`
counts only items not in a slot (weapons or gear).

- `ammo.*.max`: **decoded, base value only.** `BaseMaxValue.BaseValueConstant`
  of `D_Resourcepools.AmmoPools.Ammo_<pool>_Pool` in Startup.upk, read with
  `ow-package --properties <index> --property-offset 4` (each read consumed the
  whole object, 0 trailing bytes): pistol (Repeater_Pistol) 200, SMG
  (Patrol_SMG) 360, assault rifle (Combat_Rifle) 280, shotgun 80, sniper 48,
  launcher (Rocket_Launcher) 12, grenade (Grenade_Protean) 3. Ammo SDUs
  (`UpgradeLevelAttribute`, `MaxValueUpgrade`, `TotalUpgradeCount` 6), class and
  skill modifiers are not applied, so a real character's maxima will be higher.
- `ammo.*.current`: the reserve the host holds, magazine not included. Start
  value is **UNVERIFIED** (full pool). Firing draws from the magazine and a
  reload (auto on empty, or **R**) moves rounds from the pool after the
  weapon's evaluated reload time; a shot costs `round(shot_cost)` rounds and 0
  for the Infinity. Magazine size is `round(magazine)`; no reload animation,
  no ammo pickups, no grenade throwing, no per-weapon ammo-cost rules beyond
  the evaluated card. All UNVERIFIED against the game.
- Weapon to pool: recipe display type, else the balance prefix
  (`GD_Weap_Pistol`, `GD_Weap_SMG`, `GD_Weap_AssaultRifle`, `GD_Weap_Shotgun`,
  `GD_Weap_SniperRifle`, `GD_Weap_Launchers`). Local recipes confirm the
  `GD_Weap_Pistol`, `GD_Weap_Shotgun` and `GD_Weap_AssaultRifle` prefixes and
  the "Shotgun" and "Assault Rifle" display types; the "Pistol", SMG, sniper and
  launcher names and prefixes are UNVERIFIED guesses.
- `slotsUnlocked` (2..4): the game data has an `SDU Weapon Equip Slot` usable
  item, so locked slots exist. How many slots a new character starts with and
  how each SDU counts are **not decoded**. The host default is 4 for the test
  build; `-owslots=<2..4>` overrides it. `Equip` refuses a locked slot
  (host-side, independent of the page); it refuses to lock a slot that holds a
  weapon.
- `money`, `eridium`: present only when set with `-owmoney=` / `-owerid=`
  (the capture run sets made-up 1234567 and 42). There is no economy, so
  nothing changes them. No starting balance is verified.
- `activeAmmoType`: pool of the held weapon; omitted when unarmed or unresolved.
- Capacity: `LoadRecipes` now respects `backpackCapacity` (it used to bypass
  it). The 12-slot base is still UNVERIFIED.

Test setup (only with `-owcombatshots`): level 36 unless `-owlevel` is given,
so the level-30 recipes and the level-36 gear manifest item equip; slots take
one weapon per ammo type first; demo purse and reserves are set (made up); the
run sends the page Down, E, F key events and writes `OWCombat_7b_InventoryCompare`
and `OWCombat_7c_InventoryInspect`, then fires slot 2 from a 2-round magazine
to exercise reload. This also changes the level in the Skills capture.

Automated (this pass): UE 5.8 build of `OpenWillowEditor` succeeded;
`-owinventoryselftest` (synthetic add, duplicate ID, equip, locked-slot
refusal, unequip, favorite, trash, take/re-add by stable ID, slot index shift,
magazine and reload arithmetic) logged "passed"; `ctest` 6/6 passed and
`verify_packages.py` matched all nine packages. Runtime capture:
`OWCombat_7_Inventory.png` shows four populated slots, the ammo panel and
currency bar from these fields; `-owslots=2` shows padlocks on slots 3 and 4;
the log showed a reload of 11 shotgun rounds moving the pool from 37 to 26.
Not verified: the drop, favorite, trash and equip messages through the page
(no live click test in this pass; only the data-layer round trip), any of the
UNVERIFIED items above, and visual parity (compare card layout overlaps the
equipped list in `7b`, the shield cell has no preview, `type` is empty for the
Infinity recipes).

## Tab, header tabs, varied demo, critic review — 2026-09-29

AI-assisted; details and limits in DECISIONS.md (2026-09-29, "Inventory on
Tab..."). Independent critic (stills plus code reading, four local reference
images only, no web search): **5.5/10 overall**, below the 8/10 target. Per
area: frame 7, equipped/gear 6.5, backpack 6, card 6.5, ammo/currency 7.5,
Maya and backdrop 3.5, compare/inspect 3.5. Ranked gaps: backdrop (the real
game shows a dimmed world behind curved glass), Maya framing and outfit, overall
UI scale/position (real menu sits further right and is about 10% larger),
compare cards too small, inspect box has no real counterpart, backpack not fused
to Equipped, three of four gear cells empty (no real card data for them). Critic
code review found no functional bug; it noted that close keys did nothing until
the page was ready (fixed afterwards: Tab/Escape/I now close before ready) and a
possible one-frame HUD flicker on tab switch (not measured).

After the critic run Maya was moved back and down (DistanceCm 280, ScreenX 0.86,
HeadTopScreenY 0.17) so more of her torso is visible; that framing change was
not re-scored. The Skills capture `OWCombat_8_Skills.png` caught only the
loading state, so Skills correctness after a tab switch is covered by the
in-engine test's page-open check, not by a visual comparison.

## World-only backdrop pass — 2026-09-30

AI-assisted. Existing UModel exports reused; no extraction changes. The
generated local post-process material preserves visible custom-stencil-247
Maya/outline pixels and grades the world after tonemapping. This follows the
host engine's documented [post-process material/custom stencil path](https://dev.epicgames.com/documentation/unreal-engine/post-process-materials-in-unreal-engine).
Gain, saturation and vignette are actor-owned material instance parameters;
closing the menu destroys the post-process. Missing material disables world
grading with a warning. Custom depth/stencil is enabled in DefaultEngine.ini.

Automated checks:

```text
UE 5.8 OpenWillowEditor Win64 Development: Result: Succeeded
Menu-look Python commandlet: Success - 0 error(s), 2 warning(s)
CTest: 100% tests passed, 0 tests failed out of 6
Package verification (decoded bytes, counts and export fields match):
Core: 234397 bytes, 1621 exports
Engine: 5878264 bytes, 33166 exports
GameFramework: 61714 bytes, 258 exports
GearboxFramework: 1224040 bytes, 7098 exports
WillowGame: 13054200 bytes, 56443 exports
GFxUI: 136680 bytes, 841 exports
IpDrv: 230751 bytes, 1364 exports
OnlineSubsystemSteamworks: 265760 bytes, 1709 exports
AkAudio: 39503 bytes, 176 exports
OWINVTEST SUMMARY result=PASS steps=26 passed=26 failed=0 not_run=0 keys=slate
```

The material commandlet warnings concern legacy reference gathering on outline
replacement and the initial lookup of a not-yet-created backdrop asset.
Fresh rendered engine captures completed without a backdrop shader compile
error. Visual checks: suit panels read yellow; 300 cm framing clipped the right
elbow, so 345 cm was restored and captured; head, arm and outline fit. Key/rim
intensities reduced to 4/5 lux after the first capture exposed overlighting.
The after-close gameplay capture restores world colour and camera framing.
Before/after stills are ignored local files under `local/inventory/`.
In-engine functional run: `tools/test_inventory_actions.ps1 -Port 8786`, log
`local/inventory-actions/run-20260930-000050.log`; all 26 steps passed, covering
equip/unequip, favorite/trash, drop/pickup, full backpack, locked slots, gear
level gates, Skills/Inventory routes and close/reopen. Physical keyboard and
mouse use remain separate from these injected Slate inputs.

This is self-review, not an independent critic re-score; the earlier **5.5/10**
remains historical. Face brightness, outfit differences, world blur and curved
glass remain visible gaps against `local/ui/ref/inventory_maya_real.jpg`.
Arbitrary stencil occlusion and temporal edge stability are UNVERIFIED.

## 2026-09-30 stock-target refinement and rotating Inspect

AI-assisted; target confirmed as stock inventory and default Maya. No new
external extractor or dependency. UModel 1590 payloads and the existing Ruffle
runtime remain local. The page owns adaptation; UE validates inventory requests
and renders the Maya/weapon display copies.

Automated extraction/observation: `prepare_inventory_gear.py` consumed two
existing UI Trace SDK JSONL files (`uitrace_20260926_220818` / `_221202`),
87 completed gear observations, 11 unique cards, 76 duplicates, 0.287 s:
4 shields / 4 class mods / 3 relics / 0 grenade mods. Output is ignored
`local/inventory/observed_gear.json`. Stat icons and original Flash flavour
markup survive; visual identity and package balance remain unresolved. Unknown
types/incomplete transactions are excluded, not classified as working assets.

```text
UE 5.8 OpenWillowEditor: Result: Succeeded (10.51 s, lighting refinement)
CTest: 100% tests passed, 0 tests failed out of 6 (8.55 s)
Package verification: all nine decoded byte/count/export comparisons match
Core 234397 / 1621; Engine 5878264 / 33166
GameFramework 61714 / 258; GearboxFramework 1224040 / 7098
WillowGame 13054200 / 56443; GFxUI 136680 / 841
IpDrv 230751 / 1364; OnlineSubsystemSteamworks 265760 / 1709
AkAudio 39503 / 176
weapon_stats_test.py: 11 passed; inventory_gear_test.py: 4 passed
node --check inventory.js; git diff --check: passed
OWINVTEST SUMMARY result=PASS steps=32 passed=32 failed=0 not_run=0 keys=slate
```

Runtime log: `local/inventory-actions/run-20260930-010138.log`; command:
`./tools/test_inventory_actions.ps1 -Port 8791 -Extra @('-owinventoryshots','-NoLiveCoding')`.
New checks cover DOM weapon drag to slot/backpack, rejection of weapon-to-shield,
native Inspect frame receipt, an 84-degree DOM pointer orbit (second frame),
and Escape closing Inspect while keeping inventory open. The first Inspect run
used the wrong case for the helper key, was stopped, corrected, then rerun.
Previous equip/drop/lock/level-gate/tab/close checks all still pass.

Visual self-review: settled engine open capture shows currency zero, yellow
suit, full elbow and world behind translucent glass. Camera-pitch compensation
keeps the display upright relative to the menu camera. Inspect before/after
captures show actual mesh rotation; dedicated lights/manual exposure corrected
the initially dark mesh. Screenshots remain in ignored
`host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor/OWInventory_*.png`.
Browser clicking a native favorite hit target produced the expected stable-ID
request. This is not physical in-engine mouse validation or an independent
critic re-score.

Still below 1:1: affine panels lack the original perspective, compare placement
and stock focus/swap/sort behaviour remain approximate, Inspect occupies a
temporary host box, gear meshes are unresolved, and character/weapon shaders
are heuristic. Continuous readback latency and arbitrary stencil occlusion
are unverified. Historical critic score **5.5/10** is unchanged.

### Equipped transfer and stock comparison follow-up

Runtime: `local/inventory-actions/run-20260930-011746.log`, **35/35 PASS**;
same command/Slate route as above. Added equipped Enter opening transfer,
selection preserving the source and full-size cards, Escape cancelling without
closing. First-stat fields are asserted visible after the native tween.
Widths measured 268.1 / 289.6 in root bounds. The settled
`OWInventory_transfer_selection_preserves_full_size_cards.png` shows both
numeric stat/delta columns and large cards over the panels. HTML category
arrows are now hidden when covered; that final CSS-only change was checked
through the existing occlusion path, without another engine build.

Recorded card positions/scales and panel tweens replace the 55% host layout.
The source/candidate roles follow the observed left-origin transfer. Visual
comparison uses the third-party stock/controller capture at
https://www.thatgamesux.com/borderlands-2-can-there-be-too-much-loot
(ignored `local/ui/ref/inventory_compare_real.jpg`), not a matched Maya session.
Browser Enter/Down/E verified persistent source and the correct equip request;
runtime host equip validation remains covered by the full suite.

UE build succeeded; node syntax and diff checks passed; CTest **6/6**, 8.47 s;
all nine package checks match the byte/export counts listed above. 1:1 panel
projection, backpack-origin focus flow, original Inspect layout, unresolved
gear meshes and material parity remain unverified. No independent critic run.

### Selected weapon on Maya's display copy

Existing UModel 1590 `Rifle_Siren.Idle_Inventory` converted/imported locally:
381 frames / 30 fps. Import success, 0 errors / 1 warning. The first relative
input path failed and was corrected to absolute. A separate bounded
`Pistol_Siren` export request (0.274 s) found no object; no substitute package
identity was invented. Existing package listing and body reference identify
`R_Weapon_Bone`; selected host recipe meshes attach there with an ink hull.
Gear selection restores the base inventory idle and clears the gun.

UE build succeeded, 13.85 s. Runtime **35/35 PASS**, log
`local/inventory-actions/run-20260930-012557.log`. Preview logs confirm named
pistol/shotgun/SMG meshes and armed idle; gear clears both. Fresh open/compare
captures show the attachment following the animated hand. Material paint is
still incomplete (SMG rendered mostly neutral); sharing the Rifle idle across
weapon types is not verified stock hold-definition behaviour. CTest **6/6**,
8.54 s; all nine package checks match; JS syntax/diff checks pass. Browser
reopen callback resets Inspect and reissues the current selection. That final
cached-state/empty-selection JS change did not require another engine build.

All animation, mesh, export logs and screenshots remain ignored local/UE
outputs. No independent critic and no claim of 1:1 parity.

### Continuous backpack window (2026-09-30)

Seven full rows plus a masked eighth-row preview replace discrete pagination.
Native cells are grouped under AS2 scrollRect; the partial HTML hit target uses
the same visible boundary. Wheel and chevrons move one row, PageUp/PageDown seven;
selection reveals itself with the minimum window movement. Browser measurement
shows ~9.5 px of the partial hit target. In-engine 37/37 PASS, log
`local/inventory-actions/run-20260930-014550.log`; new checks verify one-row
advance retaining six IDs in order and large upward delta clamping to the top.
UE build 7.75 s; CTest 6/6 (20.39 s), all nine package checks match.
Original-game wheel acceleration and physical bottom-edge clicks remain
unverified. This does not establish complete inventory parity.

### Default head preview palette and crop anchors (2026-09-30)

External export: UModel 1590, `-game=border -export -png`, package
`CD_Siren_Skin_Default_SF`, object `Mati_Default_Head`, type
`MaterialInstanceConstant`: 0.773 s, exit 0, eight files / 6,508,229 bytes,
zero failures/duplicates in the fresh local output. Four PNGs, two material
summaries and two property dumps; cooked Master_Player graph remains unavailable.
The property dump matches the earlier export SHA256. All output stays local.

The look script's optional OPENWILLOW_MENU_HEAD_PROPS input creates a separate
inventory head instance. Existing factor-two gain is compensated on the face;
hair uses the source default shadow colour instead of violet midtone. A fresh
engine capture shows dark-blue hair and less pale face. This is visual tuning
of an approximate shader, not a verified stock shading reconstruction.
Gameplay mesh materials are not replaced. Import succeeded (zero errors/two
warnings: initial missing generated instance and reference-gathering warning).

Crop/header regression fixed by capturing header anchors before Ruffle applies
scrollRect. Browser header controls now measure ~169-193 px, above the first
row, instead of ~52-76 px beside the tabs. Engine open/reopen checks assert the
relative placement. Final validation results are recorded below.

Final checks: UE build succeeded (7.95 s); in-engine **37/37 PASS**, log
`local/inventory-actions/run-20260930-015423.log`, including header placement
on initial open, reopen and return from Skills. Fresh open capture confirms
header at backpack and corrected head material; preview palette load logged.
CTest **6/6** (15.60 s); all nine decoded byte/count/export comparisons match.
Python/JS syntax and diff checks pass. No independent critic or matched stock
Maya capture; panel perspective, shader ramps, focus/sort and Inspect parity
remain incomplete.

### Stock sort key and transfer guards (2026-09-30)

Existing SDK trace `_221202`, seq 16508-16512, resolves forward/backward sort
aliases to Page Up/Page Down. Transfer tooltip disables Drop and Sort. The page
now follows those bindings/guards and presents a contextual Sort hint. The
underlying host sort modes are still unverified against the full stock cycle.
No new extraction tool/output; observations remain ignored local data.

Repeated render verification exposed accumulated crop offsets, beyond the
previous header fix. Fixed zero-origin scrollRect with panel-local row placement
replaces the changing mask origin. Twelve browser renders retained matching
native/HTML positions (x 803.75, y 194.95/194.9375). Runtime comparison and sort
checks now assert native hit clips match their HTML targets within two pixels.
Three new action checks cover transfer Drop/Sort refusal and directional sort
with stable selection. The first run's direction was reversed; the trace alias
return supplied the correction before final validation.

Final verification: UE build succeeded (8.06 s). Runtime **40/40 PASS**, log
`local/inventory-actions/run-20260930-020452.log`, including fixed-direction
PageUp/PageDown, transfer guards and native/HTML alignment after sort/comparison.
Settled engine comparison capture shows rows in their frames and disabled grey
Drop/Sort hints. CTest **6/6** (22.16 s); all nine package checks match. JS syntax
and diff checks pass. No independent critic or full stock focus/sort claim.

### Backpack-origin transfer (2026-09-30)

Original-game trace `_220818` seq 23550 / 24230 and right-origin TweenCards
observations establish a missing flow: backpack selection starts transfer to
equipped slots before confirmation. E/Enter now pins the source, permits
compatible destination selection, compares its occupant, and confirms through
host validation. Escape cancels while retaining source/menu/equipment.
Empty destinations use pending Equip with no comparison item. Gear cannot move
to weapon slots. Transfer snapshots retain a source regardless of which panel
it started in. Native card tween direction is preserved; both panel focus
arguments match the observed comparison calls. No new exports/dependencies.

The first runtime passed 43/43. Follow-up adds empty-destination checks and
captures both right-origin states. Exact stock initial analogue, keyboard grid
traversal, hover/click timing, sort cycle, 3D projection and Inspect/material
parity remain incomplete; this is not a 1:1 completion claim.

Final verification: UE build succeeded (7.84 s); runtime **45/45 PASS**, log
`local/inventory-actions/run-20260930-021213.log`. New captures
`OWInventory_backpack_transfer_changes_destination.png` and
`OWInventory_backpack_transfer_empty_destination.png` show yellow source card
on the right, green destination comparison on the left, and a single source
card/Equip hint for an empty target. Both retain the source weapon preview.
CTest **6/6** (21.64 s); all nine decoded byte/count/export comparisons match;
JS syntax and diff checks pass. Screenshots/logs/assets remain ignored outputs.
Maya's animated sway can move her behind the backpack; exact pose/framing over
the whole idle and independent critic remain unverified. 1:1 is incomplete.

### Full-loop preview framing (2026-09-30)

No new exports or imports. Forward kinematics of the existing converted armed
idle (381 frames / 30 fps) finds a stationary root; head lateral range is
-10.16..11.69 cm, so the observed sway is authored. Default ScreenX .79 -> .86
fits the default-Maya stock reference's right-side composition without removing
that motion. Head anchor bounds are checked throughout a live 13-second loop.

UE build 11.49 s; runtime **46/46 PASS**, log
`local/inventory-actions/run-20260930-021751.log`. 2,110 projected Head-bone
samples: normalized x **.806.. .906**, y **.319.. .360**. Fresh open and
empty-target captures show face clear of backpack at sampled poses. CTest
**6/6** (22.01 s); all nine package checks match; diff check passes.
This is a head-anchor framing check, not proof of every silhouette/weapon
pixel, every viewport aspect ratio or original-game full-cycle pose parity.
No independent critic; the full menu goal remains incomplete.

### Scaleform projection support probe (2026-09-30)

Developer-only `tools/hud_overlay/probe_scaleform_3d.js`: execute its source in
the ready inventory browser, then await `owProbeScaleform3D()`. The page never
loads it automatically. Synthetic green square; four cases, 405.5 ms including
four 100 ms waits; gfxExtensions=true. Flat/yrotation45/depth300 all 100x100;
ordinary rotation45 141.4x141.4. The 3D properties round-trip but have no geometry
effect in Ruffle 0.7.0-nightly.2026.9.26. Cleanup removes the probe clip.
This supplies an observed renderer limitation, not stock transform values.
Autodesk documents these as Scaleform-specific AS2 extensions:
https://help.autodesk.com/cloudhelp/ENU/Scaleform-Help/scaleform_help/3di.html.
Neither local SDK trace has a nonzero 3D SetDisplayInfo call. Exact projection
therefore remains unverified and unimplemented. No new tool/export dependency.
CTest 6/6 (8.64 s), all nine package checks match, JS syntax/diff checks pass;
no engine rerun because runtime unchanged. HTTP probe loading failed; direct
execution of its repository source through the preview bridge succeeded.

### Getter output observation fix (2026-09-30)

Tracer 0.2.1 adds return object identity and out.D for GetDisplayInfo. The old
tracer omits the getter's completed output; zero-filled input D is not evidence
of a flat original transform. No additional native query or property write.
Three synthetic callback contract checks pass; real post-hook output timing
remains unverified until a fresh original-game capture. Existing local installed
tracer updated with prior own-script backup under local/ui/tool-backups; source/
installed SHA256 match. trace_dir.txt preserved. No live game/capture this pass.
CTest 6/6 (8.53 s), all nine package checks match, Python syntax/diff pass.
Menu runtime unchanged; projection and complete menu parity remain unresolved.

### Selected SMG paint pass (2026-09-30)

Existing UModel 1590 payloads supply two MICs and four textures for one selected
Maliwan epic SMG. New preparation/import scripts apply an explicitly approximate
palette material to its existing mesh; no exports or mesh reimport. Its one
glTF primitive contains UV1. Commandlet reports zero errors/warnings, script
execution 1.00 s. Fresh engine capture
`OWInventory_backpack_transfer_empty_destination.png` shows blue/pale metal
paint rather than neutral grey. Source graph/packed normal/emissive/pattern and
lighting parity remain unverified; no claim about other weapon materials.

Runtime log `local/inventory-actions/run-20260930-023458.log`: 46/46 PASS;
2049 head samples, normalized x .806.. .906, y .319.. .360 over 13 seconds.
CTest 6/6 (17.39 s); nine package checks match; Python syntax checks pass.
Only self-review, no independent critic. Native computer-use pipe unavailable;
fresh original-game getter observation and physical input checks remain open.

Batch follow-up: 14 supported recipes / 6 unsupported pattern inputs / 8
Infinity variants excluded to retain previous paint. 56 local texture imports
reference 24 unique source PNGs; all 14 source primitives have UV1. Commandlet
reports zero errors/warnings (11.67 s script execution). Runtime
`run-20260930-023958.log` passes 46/46, with 1802 full-loop head samples.
The early comparison screenshot includes Preparing Shaders (2), so it does
not verify final batch appearance. CTest 6/6 (20.95 s); nine package checks
match; syntax/diff and 14 complete input validations pass. Null pattern
negative check rejected. No general shader parity or all-weapon claim.

### Maya hold-reference audit (2026-09-30)

`audit_maya_menu_pose.py` validates reflected AnimSetList object typing and local
AnimSet reference identity through the existing package CLI. Nine holds / zero
trailing property bytes / eight unsupported WeaponActions. Five weapon classes
reference Rifle_Siren; launcher references RocketLauncher_Siren, unarmed
Unarmed_Siren. This establishes set references, not menu action selection/IK.
UModel 1590 launcher export: .086 s, 9 MD5 clips / 357936 bytes, no duplicates
or failures, no Idle_Inventory. UModel hold-definition dump is unsupported
(Unknown class, .114 s); exit 0 does not mean success. Payloads stay ignored.
CTest 6/6 (8.57 s), nine package checks match, syntax/diff pass. Native pipe
recheck still explicitly unavailable. No runtime change or new engine capture.

### Independent panel-plane rendering benchmark (2026-09-30)

Developer-only `probe_panel_projection.js`, existing Ruffle/local movie. A
temporary second player draws native equipment-panel art under a synthetic
CSS rotateY(20deg)/1200 px perspective. Its HTML target shares the plane:
center elementFromPoint passes; snapshot shows art/green border transformed
together. Two completed runs 3018.1/2990.5 ms. Last 60 RAF intervals mean
5.97 ms, max 8 ms (browser cadence, not UE throughput). Cleanup leaves one
main player and ready inventory. No output assets. First awaited observation
timed out; later instrumented run reported premature frame readiness, fixed
by waiting for _framesloaded == _totalframes and using _level1-relative bounds.
No restart was based solely on timeout; cleanup/terminal state was observed.

This tests feasibility only, not stock angles, whole-menu depth ordering,
drag/tween synchronization, memory cost or engine integration. Not loaded by
inventory.html; production path unchanged. CTest 6/6 (8.69 s), nine package
checks match, JS syntax/diff pass. No engine rerun or independent critic.

### Inventory navigation and empty-cell selection (2026-09-30)

AI-assisted interaction pass. Arrow keys and gamepad D-pad share one navigation
table (see "Original-game keyboard observation" below for what it is based on).
Wheel/chevrons still scroll; PageUp/PageDown still sort. This first version chose
neighbours from native cell centers; the observation below replaced that with the
observed table.

Empty weapon selection clears the previous card/preview ID. Equipment selection
survives unrelated snapshots and follows host changes to the selected slot.
Confirmed backpack equips update navigation to their destination. HTML pressed
state identifies equipment by slot index (including empty cells) and follows
backpack keys. Transfer sources stay pinned; Escape restores the source.
Inspect suppresses directional inventory navigation. No parser, dependency or
engine changes; no new extraction backend.

Automated checks:

```text
node tests/inventory_navigation_test.js
14/14 navigation checks passed (synthetic, stock traversal unverified)
node --check tools/hud_overlay/inventory.js
git diff --check
ctest --test-dir build -C Release --output-on-failure
100% tests passed, 0 tests failed out of 6
Total Test time (real) = 19.25 sec
python tools/verify_packages.py --reader build/Release/ow-package.exe
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

Node runs the actual adapter with synthetic snapshots, cell centers and minimal
DOM, without Ruffle, Slate or host validation. The new worktree lacks the prior
ignored seed, recipes and reference captures. Seven UI movies were recovered
with the existing reader/converter/library workaround and the same Ruffle web
version 0.7.0-nightly.2026.9.26 from npm. Final recovery: 17.823 s, 29 runtime
files including Ruffle, under ignored local/ui/run; report and recovery script
also stay local. Dynamic -nopack images retain transparent placeholders.
Initial exact-name texture lookup failed; local recovery resolved case-insensitive
package texture paths and empty export names using explicit image filenames,
then reconverted. This does not prove general identity resolution.

The in-engine action test's `select_backpack_weapon` step used ArrowDown from the
equipped panel to reach the backpack. Under the new navigation that stays in
equipment, so the step now presses ArrowRight eight times (extra presses in
Backpack are no-ops). **Edited but not compiled or run**: this worktree has no
imported Sanctuary/Maya content, scene data or UE binaries, so the previous
46/46 result predates this pass and the suite needs a rerun on a seeded checkout.

T3 preview navigation (including environment-port navigation) reports connection
refused while the local server responds HTTP 200. No live browser, UE runtime,
physical-input or matched original-game check completed for this pass. No
independent critic. Full 1:1 menu parity remains incomplete.

### Original-game keyboard observation (2026-09-30)

AI-assisted. The community mod SDK (mod manager v3.8, unrealsdk v3.2.0, pyunrealsdk
v1.10.0; the DLL SHA-256 values match THIRD_PARTY.md) was installed in the player's
Borderlands 2 folder, with `tools/sdk_trace/openwillow_uitrace` (v0.2.1, plus an opt-in
`autostart.txt` switch that turned out not to auto-enable on a first run; the mod was
enabled from the in-game mod menu). Keys were sent to the running original game with
scan-code input while the player's own character sat in Sanctuary; nothing was
equipped, dropped or sorted permanently (sort was cycled and restored, transfers were
cancelled with Escape). Screenshots, the action log with UTC timestamps and the 2,919
function trace (`uitrace_20260930_164540.jsonl`, ~4 MB) stay under ignored `local/`.
Observed, PC keyboard, no key held:

- Weapon slots 1-4 are a vertical chain; Up at slot 1 and Down at the last slot do not wrap.
- Down from slot 4 enters the shield (top-left of a 2x2 gear grid: shield / class mod over
  grenade mod / relic). Up from the shield returns to slot 4. Right shield->class mod,
  grenade mod->relic; Down shield->grenade mod, class mod->relic; Up grenade mod->shield;
  Left class mod->shield and relic->grenade mod (later Left from grenade mod did nothing).
- Right from a weapon slot or from the relic enters Backpack, on the remembered row.
  Left from Backpack returns to the last selected equipped cell (slot 4 and relic seen),
  not to the cell the selected item would occupy.
- Backpack Up/Down are linear across category headers and continue through trailing
  `[EMPTY]` cells to the last cell; neither end wraps. Empty cells show no item card.
- PageDown cycles the backpack header ALL -> TYPES -> BRANDS -> ITEMS -> VALUE -> ALL;
  PageUp runs it backwards. Groups are the movie's own sub-headers (weapon types,
  manufacturers, item classes). The host's DEFAULT/NAME/RARITY/LEVEL/DAMAGE list is
  therefore **not** stock and has not been replaced yet.
- Enter on an equipped weapon opens a compare view: equipped card left, candidate card
  right, backpack header `(COMPARE)`, non-weapon cells outlined red; Up/Down walk
  candidates; Escape cancels to the same slot. Enter on a backpack weapon opens the
  same view with Up/Down choosing the destination slot; Left/Right changed nothing.
- F opens a full-screen Inspect with a large rotating gun and the card at top-left
  (this port still uses a smaller host box); Escape returns to the same selection.

Not observed and left as marked host guesses in `inventory.js` (`GEAR_NEIGHBOURS`):
Up from the class mod and relic, Left from the shield, Right/Left in compare view, mouse
hover/click, gamepad input, and behaviour with fewer than four unlocked weapon slots.
Item names and stats in the screenshots belong to the player's save and must not be
copied into the repository.

Port changes from this: `navigateInventory` now follows the table above (no wrap, backpack
Left uses `lastEquippedIndex`, backpack-origin swap clamps on Up/Down and ignores
Left/Right). `tests/inventory_navigation_test.js` was rewritten to those observations:

```text
node tests/inventory_navigation_test.js
17/17 navigation checks passed (traversal from original-game observation; UNVERIFIED cells noted in inventory.js)
```

Still open for 1:1: the stock sort list and grouping, trailing empty backpack cells, the
full-screen Inspect, red-outlined ineligible cells in compare view, and everything mouse
and controller. The in-engine action suite has not been rerun since this change.

Worktree seed (this session): `tools/seed_inventory_demo.py` rolls 18 demo recipes,
`tools/seed_inventory_assets.ps1` exports and imports Maya's Idle_Inventory (271 frames),
the armed Rifle_Siren idle (381 frames), the menu look and the weapon meshes;
`python tools/render_weapon_previews.py` and `tools/prepare_skill_tree.py` regenerate
previews and the skill tree. Not restored: the original gear manifest and
`observed_gear.json` (built from traces that no longer exist) and the Infinity proxy
material, so gear steps of the action suite will not pass until they are rebuilt.

### Original-game sort and compare observation, and the action-suite rerun (2026-09-30)

AI-assisted; same capture session as the keyboard observation above (screenshots and the
2,919-function trace stay under ignored `local/`; item names and stats in them belong to
the player's save and are not copied here).

**Sort cycle (observed).** PageDown sends `extOnChangeSort(Delta=+1)`, PageUp `-1`, so PageDown
runs ALL -> TYPES -> BRANDS -> ITEMS -> VALUE -> ALL. The port has it reversed (PageUp forward).
Every sort step selected the **first cell** and showed the list from the top.

- Sub-headers are slim labels above the first item of each group, drawn inside the scrolling
  list; one is about 0.37 of a cell pitch tall. The first sits directly under
  "BACKPACK (TAG)". The tag is the sort mode; the port's host-invented category filter
  (`[`/`]`, chevrons) has no stock counterpart.
- ALL: everything grouped by class. WEAPONS, then RELICS, then CLASS MODS were seen. Where
  SHIELDS and GRENADE MODS sit was not in the observed pack: UNVERIFIED.
- TYPES: weapons only, grouped by weapon type ("ASSAULT RIFLES" before "SUB-MACHINE GUNS";
  other labels and alphabetical order are guesses).
- BRANDS: everything grouped by manufacturer, alphabetical (five brands seen; the Bandit
  header read "BANDIT MADE" while the card logo reads "BANDIT").
- ITEMS: non-weapons only, RELICS then CLASS MODS (so class order is not alphabetical).
- VALUE: one headerless list, dearest first.
- Empty cells follow the last item in every mode. How many a filtered mode shows is unknown.

**Compare view (observed, both origins).** Backpack tag reads "(COMPARE)". The list is the
weapons only under one "WEAPONS" sub-header, then empty cells. The four **gear cells of the
Equipped panel are outlined red** (the movie's cell symbols have a `bad` frame; frame labels
`normal, locked, bad, lockedbad, added` read from the converted library). Equipped-origin:
green comparison frame on the equipped card. Gear compare (what turns red, what header)
was not observed.

**Inspect (observed).** Full-screen dark backdrop, the item card at top-left, a large gun
filling the middle; Escape returns to the same selection.

**Trace.** `SetSortLabel` / `ApplySortConfiguration` carry `SortFilterCategorizeData
{SortType, FilterType, CategoryType, SortTitleLookupKey}`: the default is `(0,0,0,"")`,
compare `(2,1,1,"Compare")`, leaving compare `(2,0,1,"all")`. What the numbers mean is UNVERIFIED.
**The sort ordering itself is native code** (`extOnChangeSort`, `ApplySortConfiguration` are
`FUNC_Native`), so it cannot be read from bytecode; see
[SCRIPT_BYTECODE_DISASM.md](SCRIPT_BYTECODE_DISASM.md) for what can.

**Action suite rerun (this worktree, seeded, slate keys, 2 slots).** `tools/test_inventory_actions.ps1`
reported **30/46 passed, 16 failed**. The regression from the navigation change was one
assumption: after `unequip` the page now keeps the emptied equipment cell selected (the
step `backpack_transfer_empty_destination` pressed `e` expecting the backpack pistol), and
steps 21-30 cascaded from it. The step now presses ArrowRight first (edited, **not yet
rerun**: it needs an editor rebuild). Steps 37, 42, 43 fail for a different, known reason:
the gear manifest is not restored in this worktree ("no shield in the local gear manifest").
Steps 13/14 (`pageup_sorts_preserving_selection`, `pagedown_reverses_sort`) pass today but
encode the wrong stock behaviour (sort does not preserve selection; PageDown is forward) and
must change with the sort work.

**State of the sort-list work.** A partial, **non-running** implementation of the stock list
model is on branch `t3code/wip-inventory-sort-list` (one WIP commit: modes, grouping,
entries with sub-headers and empty cells, entry-based scrolling). Unfinished: the render loop,
empty-cell selection/hit boxes, removing the category filter, the page report in
`OpenWillowMayaHUD.cpp` (`cat`, `backpackHeaderAnchored`, `backpackRowsAligned` assume the old
rows), the in-engine steps, and the node tests. Remaining order of work: (2) sort list,
(3) selectable empty cells, (4) full-screen Inspect with an auto-rotating gun, (5) red `bad`
cell state on the gear cells during compare; then weapon models/textures and Phaselock.

### First interpreter connection (2026-10-01)

AI-assisted. Ordinary item-only backpack Up/Down now executes the installed
`InventoryListPanelGFxObject.MoveDelta` through the linked C++ VM. The page sends
an ordered request to the HUD, the host supplies provider length/entry kind,
and the returned index updates actual page selection. Equipment navigation,
transfers, equip actions, sorting and rendering still use the host adapter.
Category headers and empty entries are not supplied to this first provider.
No script listing or game bytes were transcribed into project source.

The two new Slate-key engine checks pass: `vm_backpack_down` moves index 1 to 2,
`vm_backpack_up` returns 2 to 1. Each executes 55 expressions; page reports
two completed calls, 110 expressions, zero VM errors. This verifies the
key -> page -> UE5 -> installed script -> page selection route, not full
original-game menu parity. Synthetic navigation checks: 22/22; CTest: 8/8;
all nine package differential comparisons match. Full details and enum
validation are in [the VM record](SCRIPT_VM_PROTOTYPE.md).

The successful engine launch used `-ddc=InstalledNoZenLocalFallback -d3d11`.
An initial launch stalled waiting for Zen, and the next passed cache startup
but stalled loading Sanctuary before gameplay. Both test-owned processes were
stopped before retry; no authored editor state or renderer/cache project
configuration changed. The log is under ignored
`local/inventory-actions/run-20261001-004912.log`.

Full action runner: **41/48 passed, seven failed, zero not run; overall FAIL**.
Steps 29-32 (favorite/trash/drop) act on the selected item after the drag tests
while expecting the earlier transfer item; step 34 cascades from that mismatch.
No VM movement calls occurred after the successful step 14. Steps 39 and 45
fail because the shield recipe/manifest is missing. These failures have not
been independently compared against a VM-disabled baseline, so they are not
claimed to be proven pre-existing regressions. Raw passes also include weak
downstream checks: step 44 reports a shield equip with an empty ID, and the
pickup checks depend on the earlier incorrect DropId. Do not treat those passes
as evidence of gear/pickup parity. Correcting these test/data issues remains open.
The bounded VM acceptance is the two explicit expression-count/selection checks.

### Suite correction 2026-10-01

AI-assisted. Test and fixture work only: no change to `tools/hud_overlay/inventory.js`, `src/`,
`CMakeLists.txt` or the host runtime. Files: `OpenWillowInventoryActionTest.cpp/.h`,
`tools/test_inventory_actions.ps1`. The 2026-10-01 record above reported 41/48 with raw passes
that proved little (a shield equip with an empty id, pickup checks against a stale `DropId`);
this pass fixes the test, not the product.

**Gear data.** `prepare_inventory_gear.py` ran on the one trace still present,
`local/ui/traces/uitrace_20260930_164540.jsonl` (the 2026-09-30 keyboard-observation capture):
36 completed observations, 8 unique cards, 28 duplicates, 0.188 s, 1 shield / 4 class mods /
2 relics / 1 grenade mod, written to ignored `local/inventory/observed_gear.json`. (The earlier
note that the source traces no longer exist applied to the 09-26 pair, not this file.) No new
original-game capture was needed or made. I did **not** promote it to `gear_manifest.json`:
the cards come from a player's save (levels 35-50, not the 36 the old steps assumed), and the
suite should not depend on them. Instead the test adds one obviously fake shield
(`test_shield_synthetic_1`, "TEST SHIELD (SYNTHETIC)", level 36, one fake stat) to the host
inventory before the page opens, and selects it by walking to it in the unfiltered backpack
(the host-only `[ ]` category filter is no longer used by the suite). The weapons are still the
seeded local recipe demo set, chosen by what the page reports, never by a hard-coded id.

**Statuses.** Every row is `PASS`, `FAIL`, `NOT_RUN` or `KNOWN_DIVERGENCE`
(`OWINVTEST step=N action=... status=... ok=... detail=...`). Each step now has its own
precondition, evaluated against a fresh page report before any input is sent; a precondition that
does not hold is `NOT_RUN` (nothing sent, nothing claimed). The summary reads
`result=PASS | PASS_WITH_KNOWN_DIVERGENCE | FAIL` plus
`passed= failed= not_run= known_divergence=`; the runner exits 0 / 3 / 1 and 2 for no summary,
and refuses to start while an Unreal editor is running. `KNOWN_DIVERGENCE` is never a pass.
Steps 15/16 (PageUp/PageDown) assert what the host does today and report it as
`KNOWN_DIVERGENCE`: stock PageDown is forward (ALL, TYPES, BRANDS, ITEMS, VALUE) and each step
selects the first cell, whereas the host's PageUp is forward over its own modes and keeps the
selection. The sort list itself is the separate unfinished feature on
`t3code/wip-inventory-sort-list`.

Other corrections: marking and drop steps first walk the page selection to the working weapon
(arrow keys, one burst at a time) and assert it, then start from a checked host state
(favorite/trash flags, no pickup already nearby); `DropId` is set only after that check, so a
failed drop cannot leak into the pickup steps. Pickup steps need a recorded drop, a pickup actor
in range and the dropped item out of the inventory. The shield steps assert level, selection and
worn-state before and after; empty ids fail instead of matching anything. The refused-drag and
level-gate-in-page steps are negative checks whose key/drag delivery cannot be observed; they
require a preceding positive control (the earlier drag steps pass first) and say so in the row.
Transfer, scroll, inspect and slot steps gained exact selection/state preconditions
(candidate moved by ArrowDown; Up returns to the working weapon; key `2` must change the target).

**Evidence.** Launch: `-ddc=InstalledNoZenLocalFallback -d3d11` (now the runner's default
`-Extra`; no project config changed); UE 5.8 `OpenWillowEditor Win64 Development` build
succeeded (14.5 s, last build). The suite ran three times in total:

```text
run-20261001-091241.log  PASS=43 FAIL=1 NOT_RUN=3 KNOWN_DIVERGENCE=2   result=FAIL (49 steps)
run-20261001-091727.log  PASS=47 FAIL=0 NOT_RUN=0 KNOWN_DIVERGENCE=2   result=PASS_WITH_KNOWN_DIVERGENCE
run-20261001-092122.log  PASS=47 FAIL=0 NOT_RUN=0 KNOWN_DIVERGENCE=2   result=PASS_WITH_KNOWN_DIVERGENCE
node tests/inventory_navigation_test.js: 22/22 (synthetic)   python tests/inventory_gear_test.py: 4 OK
ctest --test-dir build -C Release: 8/8 passed (19.08 s)
verify_packages.py: all nine decoded byte/count/export comparisons match
```

The first run is the useful negative evidence: the shield walk timed out at 40 s (one key per
3 s report round trip), that step failed and the three steps depending on the selected shield
reported `NOT_RUN` with the reason, rather than passing on an empty id. Walking now sends
bursts of up to eight ordered presses and waits for the expected row. Both later runs are
identical. Run-to-run stability is two identical runs, not a statistical claim. Non-passes in the
final runs: steps 15 and 16 only, both `KNOWN_DIVERGENCE` as above.

Not reproduced: the old selected-item mismatch (the page selected a different weapon after the
drag steps). In all three runs the page already selected the working weapon there, so the
explicit select step was a no-op at that point; why it differed earlier is **UNVERIFIED**
(hover selection under the in-game cursor is a hypothesis, not tested). The new step makes the
suite independent of it either way, but the walk was only exercised for real by the shield step.

Still unverified: a **VM-disabled baseline was not run** (no switch to turn the inventory VM off
exists in the files this pass could touch), so the VM steps are not compared with the host
adapter; stock backpack traversal and sort remain as in the observation sections above; keys
are Slate events, not physical input; weak negative checks noted above; the host `[ ]`
category filter and the host sort modes have no in-suite coverage now except steps 15/16.
Item names in local logs come from the seeded recipes and are not copied here.

## Inventory open time — 2026-10-02

The maintainer reported a slow inventory open. Measured before changing anything. This PC, `-game
-windowed` 1280x720, `-d3d11`, slice recipes (`-Items local/items/slice`), imported page path.
No real-game open time exists yet (the real-game capture is blocked), so **nothing here is a
parity claim**.

**Instrumentation.** `OWINVTIME <event> t=<ms since the open request>` lines in the UE log:
host events from `AOpenWillowMayaHUD` (open request, per-part game-thread cost, first frame
after the open, first state push, first menu preview) and page events from `inventory.js`
(`js_open`, `js_resize`, `js_first_render`, `js_painted`, plus page boot marks `js_page_start`,
`js_ruffle_api`, `js_frames_loaded`, `js_movie_ready`). The host passes the page its open time
(Unix ms); `js_painted` is two animation frames after the page is open, ready and rendered.
`tools/test_inventory_actions.ps1 -OpenBench N [-OpenBenchDelay S] [-Items dir]
[-NoInventoryMovie]` runs `-owinvopenbench` (open, wait for `js_painted`, hold 2 s, close, 3 s,
repeat, quit) instead of the action suite and prints a table per open. `-owinvnopreload` turns
the preloads below off for A/B runs.

**Before** (medians over the launches listed; ms from the open request to `js_painted`):

| case | launches | painted | game-thread part |
|---|---|---|---|
| first open, page already booted (open 20 s after start) | 4 | 443 (358-488) | 164-238 ms sync in the open frame: inventory VM init 119-166, Maya display spawn 45-115 (sync `LoadObject`); open frame 207-290 ms |
| repeat open (2nd-5th) | 22 opens | ~60 (48-87) | ~1.3 ms |
| open pressed as the world appears (page still booting) | 3 | 6030 (5471-7365) | page boot dominates |
| first weapon preview after open | 13 | 32 typical, outliers 95 / 303 / 1036 | sync weapon mesh load |

Page boot, `js_page_start` to `js_movie_ready`, 10 launches: median ~5.0 s (4.5-6.3; 9.2 s
on the first launch after a build). Inside it: Ruffle API ready ~1.2 s, the SWF library chain
(StatusMenu imports SharedWillowInventory and SharedWillowComponents; the browser fetched
Components three times and Inventory twice, ~60 ms per fetch, 0.3-1 s apart) until ~3.7-5.3 s,
then the 300 ms layout settle and opening tween. In `-game` runs from this worktree the engine
init before frame 1 (asset registry gather without a cache, ~9 s) also delays when the page
script starts; that is not part of an in-game key press.

**Causes found.** (1) One-off game-thread work in the frame of the first open: the inventory VM
reads WillowGame/Core packages (~120 ms) and the Maya display loads its meshes synchronously
(~50-115 ms). (2) The first preview of each weapon loads its mesh synchronously (usually ~30 ms,
up to 1 s once). (3) An open during the first ~5 s of play waits for Ruffle to boot the movie.

**Changed.**
- `OpenWillowMayaHUD.cpp`: the VM is created and the display's meshes, materials and animations
  are loaded at HUD BeginPlay (level start) and held, instead of in the open frame; the starting
  inventory's weapon meshes are loaded once on the first HUD frame. Startup cost measured:
  assets 23-118 ms (615 ms once, cold), VM 116-149 ms, weapon meshes 51-188 ms for 4 recipes
  (it grows with the starting backpack).
- `OpenWillowInventoryMayaDisplay.*`: `PreloadAssets` lists the same assets `BeginPlay` loads.
- `inventory.js`: timing marks only, plus the hover fix below.

**After** (same setup):

| case | launches | painted | game-thread part |
|---|---|---|---|
| first open, page already booted | 3 | 234 (213-252) | 1.3-1.8 ms; open frame 45-47 ms |
| repeat open | 8 opens | ~62 (55-75) | ~1.3 ms |
| open pressed as the world appears | 5 | 5507 (5349-6030) | page boot, unchanged |
| first weapon preview | 9 | 0.5-1.2 ms | |

**Tried and dropped (no measured gain):** a 60 Hz browser frame rate for the status pages
(page boot median ~4.8 s vs ~5.0 s, within run-to-run spread), and HTTP caching of the
content-hashed Ruffle wasm/core files (`js_ruffle_api` 1.15-1.25 s either way; the wasm fetch was
already ~130 ms). Both reverted.

**Host widget fallback** (no `-owflashinventory`, what `tools/run_quest.ps1` hand play gets):
2.6-4.9 ms per open and a 8-16 ms frame over 9 launches at levels 1 and 8, with and without
the preloads. One earlier launch showed a 1058 ms first fallback open; it did not reproduce and
its cause is **not identified**.

**Hover selection under a resting cursor (fixed).** With the desktop cursor parked over an
equipped cell inside the game window, the page received ~500 identical `pointermove` events
(UE keeps re-sending the last mouse position) and each one re-selected that cell, so the action
suite lost its selection after step 7 (6 PASS / 1 FAIL / 42 NOT_RUN). This confirms the
hypothesis noted as untested in the previous section. Hover now selects only when the pointer
position actually changes; the first event over a cell just records the position.

**Quest save.** `UOpenWillowQuest::Save()` runs from `Pump()` every tick and rewrote the save
file every time. It now writes only when the text changed. Measured in a passing first run:
9327 calls, 8 writes, 0.77 ms per write, so the old behaviour cost about 7 s of file writes over
that run (estimate: calls x per-write time; the old per-call cost was not timed directly). Not
on the inventory path: the quest launchers do not pass `-owflashinventory`.

**Suites.** `tools/test_inventory_actions.ps1`: 45 PASS / 0 FAIL / 2 NOT_RUN (short local
backpack) / 2 KNOWN_DIVERGENCE (sort steps 15-16), identical in runs at 01:29 and 08:23 local
time. `tools/test_quest.ps1`: 73/73 first run and 10/10 resume at 01:23 with this code; two
later first runs (08:24, 08:26) ended silently after check 67, ~58 s in, with no error line and
no crash dump. A build of HEAD without these edits, to compare, could not link at that point:
another lane's uncommitted `src/` changes no longer matched the built `ow-core.lib`. That
failure is **unexplained**. Follow-up (08:38-08:45): after a clean rebuild of `ow-core` and the UE
module with both lanes' code in place, the quest suite passed 73/73 and 10/10 and exited cleanly
(no crash report), mover 16/16, inventory 45/0/2/2. Two runs at 08:29-08:30 on the earlier build
had passed every check but crashed during shutdown, and the module's debug symbols had been
overwritten by the failed HEAD link, so those stacks could not be read. A mixed build state is the
likely cause; it is not proven.

**UNVERIFIED / not done.** The original game's open time; the page boot (~5 s) is unchanged,
and cutting it needs fewer or deduplicated library loads in the converted movie or Ruffle work,
not tried here. Opens of a page that is not ready show the page's own "Loading inventory" panel.
Items picked up later still load their mesh on first preview. All numbers are from one PC.

**Original game, 2026-10-02** ([REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md)). Burst screen captures of the
running game (1280x720 windowed, about 20 ms per frame, key press to frame): first open in the process ~156 ms to the
first page frame, 272 ms to 90 % of the opening animation; repeat opens 126-150 ms and 236-258 ms (3 opens). The
capture's own latency is included, so these are upper bounds. The host's numbers above are measured differently
(log events to `js_painted`), so the two are side by side, not a parity result.

## Skills preload, real-game comparison and the stock sort list: 2026-10-04

AI-assisted (Claude). Lane D of the 2026-10-04 orchestration. Frames, traces and lists named below stay under ignored
`local/` (`local/realgame/d_session/`, `local/orch/D/`); item names and numbers in them belong to the player's save and are
not copied here.

### Skills page: why it was still loading, and the fix

`OWCombat_8_Skills.png` of the baseline run (`local/orch/baseline`) showed "Loading Maya's skill tree..." because the page
was created when K was pressed and needed about **3.4 s** (log: `OpenWillow Skills page loaded` 23:08:37.5, three
`tree initialized` lines at 23:08:40.7-40.9) to load the StatusMenu movie, its shared imports and the icon movies, while the
scripted capture looked 2.7 s after the key. The inventory page never had this problem because it is loaded hidden at level
start. The host now does the same for the skills page (`CachedSkillsBrowser`/`CachedSkillsRoot` in
`OpenWillowMayaHUD.cpp`; `skills.js` gains `owSkillsOpened` and `OWINVTIME js_skills_*` lines).
Measured (same machine, `-game -windowed` 1280x720): preload populated 5.2 s after page start (both pages booting at once;
the inventory page's own boot grew from 4.4 s to 6.1 s while they share the CPU); **from the key to a composited,
populated page 49 ms** (`js_skills_open` populated=true, `js_skills_painted` sinceOpen=49) and **one second after the Skills
header tab, populated** (`OWCombat_D6_SkillsOneSecondAfterTab.png`). The real game shows the page about 0.28 s after K
and settled by 0.65-0.85 s (burst frames, `local/realgame/d_session/skills_burst`, 50 ms steps, quarter scale). A press in
the first ~6 s of play still waits for the preload.

### Real-game session (Maya L8 in Sanctuary; saves backed up, driver removed, game stopped by us)

Method: `tools/real_game/realgame.ps1` (lock, `Backup-Saves`, `Install-Driver`, `block_saves()`), scan-code keys, window
captures, plus a throw-away GFx bridge-call hook (`local/orch/D/rg/gfx_trace.py`) around single key presses. Saves: two
files differed after the session (`Save000A.sav.bak`, `WillowEngine.ini`, both written by game start and character
select); both were restored from the backup and the whole `SaveData`/`Config` folders compared equal. **The install runs
community mods** (`BetterUIControls`, `PythonPartNotifier`, `ItemLights` ...): the "Accessory:/Barrel:/Grip:" part lines on
cards come from `PythonPartNotifier`, and `BetterUIControls` equips a shield/class mod/grenade mod/relic at once on E, so
**gear compare was not observed**. Everything below was seen with those mods present.

Observed (screen captures; **confirmed in game** only for what the frames show, not for the native rules behind them):

- **Compare frames.** The card of the item being moved has the green frame, the card it is compared with the yellow one,
  in both origins (equipped origin: equipped card left and green, candidate right and yellow; backpack origin: equipped card
  left and yellow, backpack card right and green). The host had both inverted.
- **Compare numbers.** Each row shows the value and an arrow only (green up better, red down worse, none when equal);
  no difference figure (every golden card's `aux` field is empty too). The damage arrow of a 21x7 gun against 25x2 was
  "better", so it compares damage times projectile count. Reload compares lower-is-better. The host printed "+14"-style
  deltas; it no longer does.
- **Backpack panel arrows.** With the cursor on a backpack item (no compare view) the card already carries arrows against the
  equipped item of the slot last used.
- **Card text.** Values use the label cyan, not white; a projectile count prints as a smaller gold "x7" after the damage;
  an elemental weapon adds two rows (damage per second, chance) with the labels already in the golden cards.
- **Compare view.** The four gear cells are outlined red (`bad` frame), the backpack header reads "(COMPARE)", the list is
  weapons only under one WEAPONS header.
- **Backpack focus layout.** With the cursor in the Backpack the Backpack panel is enlarged and centred (cell pitch about 65 px
  against 46 px in the equipped view), the Equipped panel recedes behind the card and a "BACKPACK 13/12" plate appears at the
  bottom right. The movie does it from `SetActivePanelByName("Backpack"|"Equipped")` (bridge trace); the host does not
  reproduce it yet (Ruffle ignores the Z tween).
- **Stock sort (one backpack of 13 items; ids not recorded).** PageDown cycles ALL, TYPES, BRANDS, ITEMS, VALUE; every change
  selects the first item. TYPES: ASSAULT RIFLES, PISTOLS, SHOTGUNS, SUB-MACHINE GUNS, SNIPER RIFLES (category key order);
  BRANDS: header per manufacturer in alphabetical order ("BANDIT MADE" for Bandit), an item without a manufacturer last;
  ITEMS: PERSONAL, SHIELDS; ALL: WEAPONS, PERSONAL, SHIELDS; VALUE: no headers, dearest first. Items of equal rarity keep no
  recognisable order (the game's quick sort is unstable); the first weapon in ALL and TYPES was the only uncommon one.
  This agrees with `NATIVE_INVENTORY_SORT.md` on every point it can be tested on; mission weapons, manufacturer grades,
  launchers and level >= 51 ties were not in the pack.
- **Inspect (F).** A full-screen opaque dark backdrop with a faint vignette, the card at the top left, the item large in the
  middle (static: no auto-rotation in 2 s) and the hints "[Mouse-1] Rotate  [Mouse-2] Pan (grey)  [Mouse-Wheel-Up/Down] Zoom
  [P] Screenshot  [Escape] Close".

### Host changes (`tools/hud_overlay/inventory.js`, `OpenWillowInventory.*`, `OpenWillowMayaHUD.*`)

- Stock sort list replaces the DEFAULT/NAME/RARITY/LEVEL/DAMAGE modes and the `[ ]` category filter: comparators and
  headers of `NATIVE_INVENTORY_SORT.md` (ties: pickup order, a host choice), PageDown +1 / PageUp -1 with wrap, first item
  selected after each change, only while the cursor is in the Backpack, headers drawn with the movie's header clip inside the
  scrolling list, trailing empty cells (one per free slot), list scrolled by entry. A transfer shows the Compare list.
  Not done: selecting the empty cells.
- Weapon cards: projectile count, status rows, value colour, arrow-only compare with the real frame roles, the red `bad`
  gear cells in a weapon transfer (the same rule for a gear transfer is **UNVERIFIED**).
- Full-screen Inspect (host layout: the native 3D frame, 512x320 with its own dark background, is shown with a `lighten`
  blend; a larger transparent frame needs a change to `OpenWillowInventoryPreviewActor`, which is another lane's file).
- Suite: steps 15/16 are now `pagedown_selects_first_item_of_types` and `pageup_returns_to_all_first_item`; four synthetic
  filler weapons make the wheel steps runnable; `cat` and the category-control anchoring are gone from the page report.

### Backpack focus, plate and Inspect refinements (same day)

- **Backpack focus layout** is in the page: with the cursor in the Backpack (and no transfer) the Backpack panel is drawn at
  scale 0.92 x 1.0 at (520, 75), the Equipped panel shrinks to 0.52 and sits behind the card, the purse gives way to the
  "BACKPACK used/capacity" plate (movie clip `storageCount`, text field `capacity`; the movie's digit font has no "/", so the text
  is set again with the imported font at size 24, matched by eye). The constants are measured on the real capture, not read from
  the movie, which does this with a Z tween Ruffle ignores. Real capture: panel 520-775 x 75-640.
- **Inspect** clips the movie to the card and the hint line so the opaque backdrop is not covered by the movie's glow layers; the
  3D frame is requested at 1024x640 (render target resized from the HUD) and the black background is removed by a border flood
  fill with a tight tolerance (the frame's clear colour is exactly black, `OWINSPECTKEY` log line). Parts of a gun that render as
  pure black and touch the border can still be removed; a clean fix needs an alpha capture in the preview actor (Lane C's file).
- **Capture sequence.** `-owinvshots` (HUD) drives the page through open, backpack, the five sorts, both compares, Inspect and the
  Skills tab; it starts when the inventory page has logged `js_movie_ready` and the skills page `js_skills_populated`, because the
  first capture attempt photographed the "Loading inventory..." screen (the pages load after the engine's first long frames).
- **Open:** the empty backpack cells cannot be selected; a Q "toggle overview" view on the Skills page; a Phaselock eye sigil on the
  HUD movie (Lane A's observation; needs the HUD clip name and a host flag); white flavour lines on cards.

### Round 10 (2026-10-04, after the critic's round-9 report)

AI-assisted. Page-only changes in `inventory.js`, `inventory.html`, `skills.js`: sub-header rows centred on their text field; compare
view with a narrowed Equipped panel (slots filled) and the Backpack panel moved right with the right-edge fade off; the movie's
highlight symbol as a full-width selection band; cards rescaled to measured widths (`CARD_FIT`, `FRAME_INSET`, a fit, UNVERIFIED);
Inspect clipped to the card frame and hint strip, movie `sway`/`scanlines` hidden; Skills `Q` overview (`OVERVIEW`, a fit; the installed
`Gfx_SkillTree` defaults are OverviewOffset.X 235, OverviewGlobalOffset.X -50, OverviewScale 85). Suite after the changes: 49 PASS /
0 FAIL / 0 NOT_RUN / 0 KNOWN_DIVERGENCE. Not reproduced: perspective tilt and curved glass (the movie's 3D transforms, ignored by
Ruffle).

### Round 11 (2026-10-04)

AI-assisted, page-only (`inventory.js`, `skills.js`). Cause found for the backpack rows sitting about 23 px right of the panel centre: the list's
`scrollRect` had a negative x origin (added in round 10 for the selection band), and Ruffle shifts content right by that amount instead of revealing
content to the left of the origin. The origin is zero again; the group starts further left and its contents are drawn further right. Backpack focus
panel/rows (frame 520-775, rows 173 px), compare panels, equipped-origin highlight, Skills placement (the Phaselock card is fitted by its background clip because
the movie resets the card clip's scale and position), Inspect level strip and the overview footer/dimming follow measured frames (fits, UNVERIFIED). Suite
after the changes: 49 PASS / 0 FAIL / 0 NOT_RUN / 0 KNOWN_DIVERGENCE. Perspective tilt and glass sheen stay open.
