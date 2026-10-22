// OpenWillow: field rules for BL2's HUD movie that are not bar frames. Our own code; loaded by index.html and by
// tests/hud_fields_test.js.
//
// Text: Ruffle hands non-ASCII strings from JavaScript to the movie as their UTF-8 bytes (the no-break spaces the
// game puts around "/" showed as an A-circumflex and a space), so such text goes in through htmlText as numeric
// entities, which keeps the field's own format (seen in the bench).
// Weapon frames: what the game shows per held weapon type, read live on 2026-10-22 (docs/verification/HUD_PARITY.md):
// the crosshair style (p1.crosshairs.outer), its idle spread (p1.crosshairs.outer.weapon; the game widens it while
// firing, not driven here) and the ammo-type icon (p1.bullets.bullet_icon). Only the two types observed; the others
// keep the movie's own frames (UNVERIFIED).
(function (root) {
  'use strict';
  const P1 = '_level1.p1';
  const WEAPON_FRAMES = {pistol: {outer: 3, spread: 9, ammoIcon: 3}, shotgun: {outer: 7, spread: 17, ammoIcon: 5}};

  // The harness call ([target, op, a, b]) that puts `text` into the text field `field`.
  function textCall(field, text) {
    text = String(text);
    if (/^[\x00-\x7f]*$/.test(text)) return [field, 'set', 'text', text];
    const html = [...text].map(c => c.codePointAt(0) > 127 || '<>&'.includes(c) ? `&#${c.codePointAt(0)};` : c).join('');
    return [field, 'set', 'htmlText', html];
  }

  // The harness calls for the held weapon's ammo type key (pistol, smg, ar, shotgun, sniper, launcher); [] when the
  // type's frames were not observed.
  function weaponCalls(type) {
    const f = WEAPON_FRAMES[type];
    if (!f) return [];
    return [[P1 + '.crosshairs.outer', 'gotoAndStop', f.outer], [P1 + '.crosshairs.outer.weapon', 'gotoAndStop', f.spread],
      [P1 + '.bullets.bullet_icon', 'gotoAndStop', f.ammoIcon]];
  }

  // The OWHUDPAGE line: the state the page has applied, written as the host's OWHUDSTATE line writes it, and what the
  // movie shows for it, read back from its clips (f* bar frames, t* texts, the crosshair and ammo-icon frames).
  // tools/check_hud_shots.py pairs it with the host's lines. `get(target, prop)` is the harness getter; characters
  // outside printable ASCII are written as \uXXXX, as the host writes the no-break spaces.
  function pageLine(shown, get) {
    const fix = v => (v === null || v === undefined ? 'null' : Number(v).toFixed(3));
    const esc = t => String(t ?? '').replace(/\r/g, '')
      .replace(/[^\x21-\x7e]/g, c => '\\u' + c.charCodeAt(0).toString(16).padStart(4, '0'));
    const frame = clip => get(`${P1}.${clip}`, '_currentframe');
    const text = field => esc(get(`${P1}.${field}`, 'text'));
    return `OWHUDPAGE health=${fix(shown.health)} shield=${fix(shown.shield)} ammo=${fix(shown.ammo)}`
      + ` grenades=${fix(shown.grenades)} weaponOut=${shown.weaponOut ? 1 : 0} weaponType=${shown.weaponType || '-'}`
      + ` healthText=${esc(shown.healthText)} shieldText=${esc(shown.shieldText)} ammoText=${esc(shown.ammoText)}`
      + ` grenadeText=${esc(shown.grenadeText)} level=${esc(shown.levelText)}`
      + ` fHealth=${frame('health')} fShield=${frame('shield')} fBullets=${frame('bullets')} fGrenades=${frame('grenades')}`
      + ` tHealth=${text('health.health_disp')} tShield=${text('shield.shield_disp')} tAmmo=${text('bullets.bullet_c')}`
      + ` tGrenade=${text('grenades.grenadesDisp')} tLevel=${text('levelClassMod.level')}`
      + ` outer=${frame('crosshairs.outer')} spread=${frame('crosshairs.outer.weapon')} icon=${frame('bullets.bullet_icon')}`;
  }

  // The ready line: echoes the host's per-run id from the page URL (`?owid=`; the host shows the movie only when it
  // matches) and names the pages directory the server serves (serve.py's X-OpenWillow-Pages header; for diagnosis).
  function readyLine(search, pages) {
    const id = (/[?&]owid=([0-9A-Za-z]+)/.exec(search || '') || [])[1];
    return `OpenWillow HUD movie ready id=${id || '-'} pages=${pages || 'unknown'}`;
  }

  const api = {WEAPON_FRAMES, textCall, weaponCalls, pageLine, readyLine};
  root.owHudFields = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
