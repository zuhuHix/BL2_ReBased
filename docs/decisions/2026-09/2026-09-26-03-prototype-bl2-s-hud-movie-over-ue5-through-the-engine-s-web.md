## 2026-09-26: Prototype: BL2's HUD movie over UE5 through the engine's web browser

The maintainer chose to prototype the quick path before any native Ruffle
embedding: UE's built-in CEF browser (the `WebBrowser` engine module, shipped
with UE 5.8; no new repository dependency) shows a transparent local page that
runs Ruffle with the converted HUD, and the C++ HUD pushes Maya's state into it.
This is a stopgap to test the real movie with live game data, not the final
integration.

- `-owflashhud=<url>` on the Maya HUD (`AOpenWillowMayaHUD`) adds a
  transparent, hit-test-invisible `SWebBrowser` over the viewport. The canvas
  bars, crosshair and weapon panel are then skipped; damage numbers are still
  drawn by the host. The module must be loaded explicitly
  (`IWebBrowserModule::Get()`); `SWebBrowserView` creates no window otherwise,
  which surfaced as an immediate load error.
- `tools/hud_overlay/index.html` (our code) runs Ruffle with
  `wmode: 'transparent'`, waits until the HUD has loaded all frames before
  jumping to `16_9` (jumping earlier left `p1` missing), hides `bossModule`,
  and exposes `owHud(state)`. `tools/hud_overlay/serve.py` serves it with the
  converted movies from `local/ui/run`; `tools/run_ue_flash_hud.ps1` starts
  both.
- Clip mapping, observed in the Ruffle bench by setting frames and reading
  them back, not read from the game's scripts: `p1.health`, `p1.shield`,
  `p1.grenades` and `p1.bullets` have 100 bar frames with frame 1 full;
  `shield`/`grenades` frame 101 is `none`, `bullets` 102/103 are
  `weaponSwitch`/`noWeapon`; `p1.xpbar` frame N is N% full; `p1.character`
  frame `siren` shows the action-skill icon. Whether the game uses exactly
  these frames for a given value is UNVERIFIED.
- Only real host state is sent: Maya takes no damage and has no XP, grenade
  or magazine tracking yet, so vitals are full with empty number fields,
  grenades are hidden, and the ammo text is the recipe's magazine size.

Checked: a `-game` run with `-owcombattest -owcombatshots` logged "page
loaded" and "HUD movie ready" from inside UE, and the captures show the movie's
bars, action-skill icon, XP bar, ammo panel, minimap and crosshair over
Sanctuary, under the inventory screen. Not checked: frame cost of the
browser, input focus in a long play session, behaviour at other window aspect
ratios, and anything against the real game's HUD beyond layout by eye.

