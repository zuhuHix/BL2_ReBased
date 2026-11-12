## 2026-09-29: Inventory on Tab, header tabs, varied demo weapons, shield preview

AI-assisted. The maintainer asked for the whole inventory menu to work, opened
with **Tab**, and said local decoding of anything the menu needs is fine for
speed. That widens what is decoded (more weapon meshes and a shield mesh from
the installed game through the existing UModel path) but not where it goes: all
game-derived output (recipes, glTF, textures, preview PNGs, the gear manifest)
stays under ignored `local/`; no game file, decompiled source or third-party
code was added to the repository, and no GPL tool was used.

- **Tab.** `AOpenWillowPlayerController::ToggleMenu` (the map list's Tab) now
  hands Tab to `AOpenWillowWalker::ToggleInventory` while Maya is the pawn; the
  map list stays on other pawns and via `OWMapList`. The page closes on Tab,
  Escape or I; category cycling moved to `[` / `]` (Tab used to cycle it).
- **Header tabs.** The five StatusMenu header tabs (movie clips nav1..nav5) have
  hit boxes on both pages. Inventory and Skills switch through the intercepted
  routes `/__ow_tab_skills` and `/__ow_tab_inventory` (host: `PendingTabSwitch`
  in `AOpenWillowMayaHUD`); K / I do the same from the keyboard. Missions, Map
  and Challenges have no host data, so they are disabled with an "unavailable"
  label instead of being faked.
- **Backpack panel.** The converted movie leaves it full size and overlapping the
  INVENTORY title. The page now scales it to 0.74 and places it at
  (682, 134) in the 1280x720 stage; row positions are computed in stage pixels
  and converted to panel units. Numbers are read from the reference captures
  (host choices, not movie values).
- **Type icons.** Probed in the bench: the movie draws frames `Sniper` and
  `Rocket`; `Sniper Rifle` / `Rocket Launcher` draw nothing (the launcher card
  used to show a pistol).
- **Demo weapons.** 18 new rolled recipes (pistol, SMG, assault rifle, shotgun,
  sniper, launcher; seven manufacturers; common to legendary) rolled with
  `tools/weapon_recipe.py` / `weapon_stats.py`, filtered with
  `filter_gestalt_gltf.py`, previews from `render_weapon_previews.py`, meshes
  imported by `import_weapon_items.py` (28 items). UNVERIFIED: rarity is the
  maximum over parts; 11 recipes drop fragments supplied only by `*_None` parts
  because those decode with meaningless mesh names; preview paint is the same
  approximation as before (colours are not faithful); rolled part mixes can be
  cross-manufacturer. In-hand meshes still use a neutral grey material except
  the Infinity.
- **Backpack size.** The demo library exceeds the 12-slot base, so the host sets
  the class maximum (39) unless `-owbackpack=<12..39>`; an optional local
  `load_order.txt` in the recipe folder orders the first items so the list is
  not eight identical guns in a row.
- **Shield preview.** "The Bee" now has a rendered preview from the shared
  Hyperion shield gestalt (four Hyperion fragments, section totals agree); the
  fragment choice for this specific roll and the paint are UNVERIFIED. Relic,
  grenade mod and class mod previews exist locally but no such items are in the
  manifest, so no cell shows them.
- **Maya display.** Her two lights now carry distinct forward-shading
  priorities, which removes the editor's "Multiple directional lights" on-screen
  warning; the leftover bone-name debug logging was removed.

Automated: UE 5.8 build succeeded; in-engine `tools/test_inventory_actions.ps1`
26/26 (adds Tab open/close through the controller path, K / I tab switching, `]`
category cycling); `ctest` 6/6; `verify_packages.py` all nine packages match;
`tests/weapon_stats_test.py`, `skill_stats_test.py`, `skill_info_test.js` pass.
A local browser check (`tab_check.py`, synthetic snapshot) covers Tab, K, `]`,
the disabled tabs and the Skills-tab click. Visual: fresh 1280x720 engine
captures `OWCombat_7*`; the independent critic's score is recorded in
`docs/verification/INVENTORY_MOVIE_PROTOTYPE.md`. Not verified: a physical
keyboard Tab press (the test injects the key through the player controller),
drag and drop, the Missions / Map / Challenges tabs, the in-hand materials, and
any claim of parity with the original inventory.

