// Synthetic checks for tools/hud_overlay/hud_fields.js: text routing (ASCII as text, anything else as htmlText
// entities) and the per-weapon-type frames read from the live game. Run: node tests/hud_fields_test.js
'use strict';
const assert = require('assert');
const path = require('path');
const fields = require(path.join(__dirname, '..', 'tools', 'hud_overlay', 'hud_fields.js'));

const F = '_level1.p1.bullets.bullet_c';
const NBSP = String.fromCharCode(160);

// ASCII stays plain text, including an empty string (clears the field).
assert.deepStrictEqual(fields.textCall(F, '12 / 340'), [F, 'set', 'text', '12 / 340']);
assert.deepStrictEqual(fields.textCall(F, ''), [F, 'set', 'text', '']);
assert.deepStrictEqual(fields.textCall(F, 250), [F, 'set', 'text', '250']);
// The game's no-break spaces go in as entities; markup characters are escaped once the html path is taken.
assert.deepStrictEqual(fields.textCall(F, `12${NBSP}/${NBSP}340`), [F, 'set', 'htmlText', '12&#160;/&#160;340']);
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

// The OWHUDPAGE line: the applied state as the host's OWHUDSTATE writes it, then the movie's read-backs, with
// non-ASCII escaped and the field's trailing \r dropped. A fake getter stands in for the harness.
const movie = {
  '_level1.p1.health': 1, '_level1.p1.shield': 101, '_level1.p1.bullets': 15, '_level1.p1.grenades': 1,
  '_level1.p1.health.health_disp': '250\r', '_level1.p1.shield.shield_disp': '', '_level1.p1.bullets.bullet_c': `12${NBSP}/${NBSP}340`,
  '_level1.p1.grenades.grenadesDisp': `2${NBSP}/${NBSP}4`, '_level1.p1.levelClassMod.level': '07',
  '_level1.p1.crosshairs.outer': 3, '_level1.p1.crosshairs.outer.weapon': 9, '_level1.p1.bullets.bullet_icon': 3,
};
const line = fields.pageLine({health: 1, shield: null, ammo: 0.857, grenades: 0.5, weaponOut: true, weaponType: 'pistol',
  healthText: '250', shieldText: '', ammoText: `12${NBSP}/${NBSP}340`, grenadeText: `2${NBSP}/${NBSP}4`, levelText: '07'},
  (target, prop) => movie[target]);
assert.strictEqual(line, 'OWHUDPAGE health=1.000 shield=null ammo=0.857 grenades=0.500 weaponOut=1 weaponType=pistol'
  + ' healthText=250 shieldText= ammoText=12\\u00a0/\\u00a0340 grenadeText=2\\u00a0/\\u00a04 level=07'
  + ' fHealth=1 fShield=101 fBullets=15 fGrenades=1 tHealth=250 tShield= tAmmo=12\\u00a0/\\u00a0340'
  + ' tGrenade=2\\u00a0/\\u00a04 tLevel=07 outer=3 spread=9 icon=3');
// Unarmed with no type and missing fields: '-' for the type, 'null' for fills never sent, empty texts.
assert.ok(fields.pageLine({weaponOut: false}, () => undefined).startsWith(
  'OWHUDPAGE health=null shield=null ammo=null grenades=null weaponOut=0 weaponType=- healthText= shieldText='));

console.log('hud_fields_test: all checks passed');
