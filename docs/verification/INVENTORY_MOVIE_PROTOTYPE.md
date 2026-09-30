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
