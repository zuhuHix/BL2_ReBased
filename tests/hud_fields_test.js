// Synthetic checks for tools/hud_overlay/hud_fields.js: text routing (ASCII as text, anything else as htmlText
// entities) and the per-weapon-type frames read from the live game. Run: node tests/hud_fields_test.js
'use strict';
const assert = require('assert');
const path = require('path');
const fields = require(path.join(__dirname, '..', 'tools', 'hud_overlay', 'hud_fields.js'));

const F = '_level1.p1.bullets.bullet_c';
const NBSP = String.fromCharCode(160);

// ASCII stays plain text, including an empty string (clears the field).
assert.deepStrictEqual(fields.textCall(F, '2 / 78'), [F, 'set', 'text', '2 / 78']);
assert.deepStrictEqual(fields.textCall(F, ''), [F, 'set', 'text', '']);
assert.deepStrictEqual(fields.textCall(F, 429), [F, 'set', 'text', '429']);
// The game's no-break spaces go in as entities; markup characters are escaped once the html path is taken.
assert.deepStrictEqual(fields.textCall(F, `2${NBSP}/${NBSP}78`), [F, 'set', 'htmlText', '2&#160;/&#160;78']);
assert.deepStrictEqual(fields.textCall(F, `<b>${NBSP}&`), [F, 'set', 'htmlText', '&#60;b&#62;&#160;&#38;']);
// A character outside the BMP is one entity, not two surrogates.
assert.deepStrictEqual(fields.textCall(F, 'x\u{1F600}'), [F, 'set', 'htmlText', 'x&#128512;']);

// Observed types: crosshair style, idle spread and ammo icon.
assert.deepStrictEqual(fields.weaponCalls('pistol'), [
  ['_level1.p1.crosshairs.outer', 'gotoAndStop', 3],
  ['_level1.p1.crosshairs.outer.weapon', 'gotoAndStop', 9],
  ['_level1.p1.bullets.bullet_icon', 'gotoAndStop', 3]]);
assert.deepStrictEqual(fields.weaponCalls('shotgun').map(c => c[2]), [7, 17, 5]);
// Types not observed leave the movie alone (no guessed frames).
for (const type of ['smg', 'ar', 'sniper', 'launcher', '', undefined, 'grenade']) assert.deepStrictEqual(fields.weaponCalls(type), []);

console.log('hud_fields_test: all checks passed');
