// OpenWillow: field rules for BL2's HUD movie that are not bar frames. Our own code; loaded by index.html and by
// tests/hud_fields_test.js.
//
// Text: Ruffle hands non-ASCII strings from JavaScript to the movie as their UTF-8 bytes (the no-break spaces the
// game puts in "2 / 78" showed as an A-circumflex and a space), so such text goes in through htmlText as numeric
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

  const api = {WEAPON_FRAMES, textCall, weaponCalls};
  root.owHudFields = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
