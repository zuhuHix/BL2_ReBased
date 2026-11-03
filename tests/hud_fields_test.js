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

// The ready line echoes the host's per-run id from the URL, wherever it sits in the query, and never invents one.
const ID = '0123456789ABCDEF0123456789ABCDEF';
assert.strictEqual(fields.readyLine(`?owid=${ID}`, 'C:\\checkout\\tools\\hud_overlay'),
  `OpenWillow HUD movie ready id=${ID} pages=C:\\checkout\\tools\\hud_overlay`);
assert.strictEqual(fields.readyLine(`?hud3d=affine&owid=${ID}`, null), `OpenWillow HUD movie ready id=${ID} pages=unknown`);
assert.strictEqual(fields.readyLine('?hud3d=affine', 'x'), 'OpenWillow HUD movie ready id=- pages=x');
assert.strictEqual(fields.readyLine('', undefined), 'OpenWillow HUD movie ready id=- pages=unknown');
// Only the id's own characters: a crafted value cannot smuggle a second " id=" field in.
assert.strictEqual(fields.readyLine(`?owid=${ID}%20id=other`, 'x'), `OpenWillow HUD movie ready id=${ID} pages=x`);

// ---- down state, second wind, tracker (invented texts and values) ----
const D = '_level1.p1.dying1';
const S = {injured: 'DOWN TITLE', injuredTip: 'DOWN HINT', secondWind: 'WIND', turnIn: 'HAND IN'};
// The bleed-out bar maps like the vitals bars: full = frame 1, empty = frame 100, clamped.
assert.deepStrictEqual([1, 0.5, 0, -1, 2].map(fields.barFrame), [1, 51, 100, 100, 1]);
// Up and dead hide the panel; injured shows `fight` with the bar and both texts.
assert.deepStrictEqual(fields.downCalls(null, S), [[D, 'gotoAndStop', 'off']]);
assert.deepStrictEqual(fields.downCalls({phase: 'dead', bleed: 0}, S), [[D, 'gotoAndStop', 'off']]);
assert.deepStrictEqual(fields.downCalls({phase: 'injured', bleed: 0.25}, S), [[D, 'gotoAndStop', 'fight'],
  [D + '.bleedout', 'gotoAndStop', 75], [D + '.fight', 'set', 'text', 'DOWN TITLE'], [D + '.suicide', 'set', 'text', 'DOWN HINT']]);
// Without the strings file the movie's own texts are left alone.
assert.deepStrictEqual(fields.downCalls({phase: 'injured', bleed: 1}, {}), [[D, 'gotoAndStop', 'fight'], [D + '.bleedout', 'gotoAndStop', 1]]);
// The banner plays once per increase of the count, never for the same or a lower count.
const W = '_level1.p1.secondwind';
assert.deepStrictEqual(fields.secondWindCalls(1, 0, S), [[W + '.inner.text1', 'set', 'text', 'WIND'],
  [W + '.inner.text2', 'set', 'text', 'WIND'], [W, 'gotoAndPlay', 'go']]);
assert.deepStrictEqual(fields.secondWindCalls(1, 1, S), []);
assert.deepStrictEqual(fields.secondWindCalls(0, undefined, S), []);
assert.deepStrictEqual(fields.secondWindCalls(2, 1, {}), [[W, 'gotoAndPlay', 'go']]);
// Tracker: the mission name, one module per objective line (state from complete / optional), the rest off.
const M = k => `_level1.p1.MissionNotifications.Module${k}`;
const rows = calls => calls.filter(c => c[2] !== 'NoMovement' && c[1] !== 'set').map(c => [c[0], c[2]]);
const t1 = fields.trackerCalls({mission: 'Mission X', lines: [{text: 'Step one', complete: true}, {text: 'Step two', optional: true}, {text: 'Step three'}]}, S);
assert.deepStrictEqual(rows(t1), [[M(1) + '.inner', 'MissionName'], [M(2) + '.inner', 'Complete'], [M(3) + '.inner', 'Optional'],
  [M(4) + '.inner', 'NotComplete'], [M(5), 'Off'], [M(6), 'Off']]);
assert.deepStrictEqual(t1.filter(c => c[1] === 'set').map(c => c[3]), ['Mission X', 'Step one', 'Step two', 'Step three']);
// Ready to turn in: the name and one TurnIn module with the turn-in label; no mission: every module off.
const t2 = fields.trackerCalls({mission: 'Mission X', turnIn: true, lines: [{text: 'Step one', complete: true}]}, S);
assert.deepStrictEqual(rows(t2).slice(0, 3), [[M(1) + '.inner', 'MissionName'], [M(2) + '.inner', 'TurnIn'], [M(3), 'Off']]);
assert.deepStrictEqual(t2.filter(c => c[1] === 'set').map(c => c[3]), ['Mission X', 'HAND IN']);
assert.deepStrictEqual(fields.trackerCalls(null, S), [1, 2, 3, 4, 5, 6].map(k => [M(k), 'gotoAndStop', 'Off']));
// No strings file, or one without the turn-in value: the TurnIn module is shown but its text is left as the movie has it
// (no text call at all, never an empty string); the host's own texts (mission name) are still written.
for (const missing of [{}, null, {injured: 'DOWN TITLE'}]) {
  const t3 = fields.trackerCalls({mission: 'Mission X', turnIn: true, lines: []}, missing);
  assert.deepStrictEqual(rows(t3).slice(0, 2), [[M(1) + '.inner', 'MissionName'], [M(2) + '.inner', 'TurnIn']]);
  assert.deepStrictEqual(t3.filter(c => c[1] === 'set').map(c => [c[0], c[3]]), [[M(1) + '.inner.text', 'Mission X']]);
}
// More objective lines than modules: the extra lines are dropped, never written past Module6.
const many = fields.trackerCalls({mission: 'M', lines: Array.from({length: 9}, (_, i) => ({text: `L${i}`}))}, S);
assert.ok(many.every(c => !c[0].includes('Module7')));
// Page lines: the down line reads the movie's frames and title through the getter; the tracker line lists each module.
const fake = {[D]: {_currentframe: 2}, [D + '.bleedout']: {_currentframe: 75}, [D + '.fight']: {text: 'DOWN TITLE'}, [W]: {_currentframe: 1}};
const get = (t, k) => (fake[t] || {})[k] ?? 1;
assert.strictEqual(fields.downLine({down: {phase: 'injured', bleed: 0.25}, secondWind: 0}, get),
  'OWHUDDOWNPAGE phase=injured bleed=0.25 secondWind=0 dying=2 bleedout=75 title=DOWN\\u0020TITLE windFrame=1');
// Up: the bar and title are not read (the getter's dying frame is whatever the movie shows).
assert.strictEqual(fields.downLine({}, get), 'OWHUDDOWNPAGE phase=up bleed=1.00 secondWind=0 dying=2 bleedout=- title=- windFrame=1');
assert.strictEqual(fields.trackerLine({tracker: null}, get), 'OWHUDTRACKPAGE mission= turnIn=0 lines=0 m1=1 m2=1 m3=1 m4=1 m5=1 m6=1');

// ---- hit arrows and the level-up banner (invented counts) ----
const P = '_level1.p1';
// The first state seen is the baseline: nothing plays for hits or level-ups that happened before the page loaded.
assert.deepStrictEqual(fields.feedbackCalls({hits: 3, side: 'r', levelUps: 1, level: 9}, null), []);
// A new hit plays that side's arrow on its Health frame; the other sides are left alone.
assert.deepStrictEqual(fields.feedbackCalls({hits: 4, side: 'l', levelUps: 1}, {hits: 3, levelUps: 1}),
  [[P + '.dmg_l.inner', 'gotoAndStop', 'Health'], [P + '.dmg_l', 'gotoAndPlay', 2]]);
// No new hit, or a hit with no side: no arrow.
assert.deepStrictEqual(fields.feedbackCalls({hits: 4, side: 'u', levelUps: 1}, {hits: 4, levelUps: 1}), []);
assert.deepStrictEqual(fields.feedbackCalls({hits: 5, side: '-', levelUps: 1}, {hits: 4, levelUps: 1}), []);
// A level-up plays the banner from `go` (its text is the movie's own), together with a hit in the same state.
assert.deepStrictEqual(fields.feedbackCalls({hits: 5, side: 'd', levelUps: 2}, {hits: 4, levelUps: 1}),
  [[P + '.dmg_d.inner', 'gotoAndStop', 'Health'], [P + '.dmg_d', 'gotoAndPlay', 2],
   [P + '.levelup', 'gotoAndStop', 'stop'], [P + '.levelup', 'gotoAndPlay', 'go']]);
// One banner per increase of the host's count: the host counts a multi-level jump as one level-up, so 8 -> 10 in one
// update arrives as levelUps 1 -> 2 and plays one banner (the rule, UNVERIFIED); nothing replays without an increase.
const jump = fields.feedbackCalls({hits: 0, side: 'u', levelUps: 2, level: 10}, {hits: 0, levelUps: 1, level: 8});
assert.strictEqual(jump.filter(c => c[2] === 'go').length, 1);
assert.deepStrictEqual(fields.feedbackCalls({hits: 0, side: 'u', levelUps: 2, level: 10}, {hits: 0, levelUps: 2, level: 10}), []);
const fxFake = {[P + '.dmg_r']: {_currentframe: 5}, [P + '.dmg_r.inner']: {_currentframe: 2}, [P + '.levelup']: {_currentframe: 2},
  [P + '.levelup.inner']: {_currentframe: 30}, [P + '.levelup.inner.levelup1']: {text: 'BANNER'}};
const fxGet = (t, k) => (fxFake[t] || {})[k] ?? 1;
assert.strictEqual(fields.feedbackLine({feedback: {hits: 1, side: 'r', levelUps: 1}}, fxGet, 1),
  'OWHUDFXPAGE hits=1 side=r levelUps=1 banners=1 u=1/1 r=5/2 d=1/1 l=1/1 levelup=2/30 levelText=BANNER');

console.log('hud_fields_test: all checks passed');
