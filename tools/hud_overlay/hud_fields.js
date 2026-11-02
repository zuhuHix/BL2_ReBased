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

  // ---- Fight For Your Life, second wind, mission tracker (docs/verification/HUD_PARITY.md) ----
  // Clip names, labels and frame directions read from the movie in the Ruffle bench: p1.dying1 has the labels off / fight /
  // revive / duel; on `fight` its bleedout bar has 100 frames, frame 1 full and frame 100 empty (as the vitals bars), with
  // the title field `fight` and the hint field `suicide`. p1.secondwind plays its banner from `go` (about 1 s) with two
  // text layers. p1.MissionNotifications holds Module1..Module6 stacked 22 px apart; a module's inner clip shows a mission
  // name (`MissionName`), an objective (`NotComplete`, `Complete`, `Optional`) or `TurnIn`. Which of these the game drives,
  // and with what, is native and was not observed live (UNVERIFIED). Texts come from hud_strings.json
  // (tools/prepare_hud_strings.py: the game's own localization, kept under local/); without it they stay as the movie has them.
  const DYING = P1 + '.dying1', SECONDWIND = P1 + '.secondwind', TRACKER = P1 + '.MissionNotifications';
  const TRACKER_MODULES = 6;
  const barFrame = fill => 1 + Math.round((1 - Math.min(1, Math.max(0, Number(fill) || 0))) * 99);

  // down: null (up) or {phase: 'injured' | 'dead', bleed: the bleed-out left, 0..1}. Only `injured` shows the panel.
  function downCalls(down, strings) {
    if (!down || down.phase !== 'injured') return [[DYING, 'gotoAndStop', 'off']];
    const calls = [[DYING, 'gotoAndStop', 'fight'], [DYING + '.bleedout', 'gotoAndStop', barFrame(down.bleed)]];
    if (strings && strings.injured) calls.push(textCall(DYING + '.fight', strings.injured));
    if (strings && strings.injuredTip) calls.push(textCall(DYING + '.suicide', strings.injuredTip));
    return calls;
  }

  // secondWind: how many second winds the host has had; the banner plays once per increase.
  function secondWindCalls(count, previous, strings) {
    if (!(Number(count) > Number(previous || 0))) return [];
    const calls = [];
    if (strings && strings.secondWind)
      for (const layer of ['text1', 'text2']) calls.push(textCall(`${SECONDWIND}.inner.${layer}`, strings.secondWind));
    calls.push([SECONDWIND, 'gotoAndPlay', 'go']);
    return calls;
  }

  // tracker: null (no mission shown) or {mission, turnIn: bool, lines: [{text, complete, optional}]}. Module1 is the mission
  // name, then one module per objective line (or one TurnIn module when the mission is ready to turn in), the rest off.
  function trackerCalls(tracker, strings) {
    const rows = [];
    if (tracker && tracker.mission) {
      rows.push(['MissionName', tracker.mission]);
      if (tracker.turnIn) rows.push(['TurnIn', (strings && strings.turnIn) || '']);
      else for (const line of tracker.lines || [])
        rows.push([line.complete ? 'Complete' : line.optional ? 'Optional' : 'NotComplete', line.text || '']);
    }
    const calls = [];
    for (let k = 1; k <= TRACKER_MODULES; ++k) {
      const module = `${TRACKER}.Module${k}`;
      const row = rows[k - 1];
      if (!row) { calls.push([module, 'gotoAndStop', 'Off']); continue; }
      calls.push([module, 'gotoAndStop', 'NoMovement'], [module + '.inner', 'gotoAndStop', row[0]], textCall(module + '.inner.text', row[1]));
    }
    return calls;
  }

  // OWHUDDOWNPAGE / OWHUDTRACKPAGE: what the page applied and what the movie shows for it (tools/check_hud_down.py).
  const esc = t => String(t ?? '').replace(/\r/g, '').replace(/[^\x21-\x7e]/g, c => '\\u' + c.charCodeAt(0).toString(16).padStart(4, '0'));
  function downLine(shown, get) {
    const d = shown.down;
    return `OWHUDDOWNPAGE phase=${d ? d.phase : 'up'} bleed=${d ? Number(d.bleed).toFixed(2) : '1.00'}`
      + ` secondWind=${shown.secondWind || 0} dying=${get(DYING, '_currentframe')} bleedout=${d && d.phase === 'injured' ? get(DYING + '.bleedout', '_currentframe') : '-'}`
      + ` title=${d && d.phase === 'injured' ? esc(get(DYING + '.fight', 'text')) : '-'} windFrame=${get(SECONDWIND, '_currentframe')}`;
  }
  function trackerLine(shown, get) {
    const t = shown.tracker;
    const parts = [];
    for (let k = 1; k <= TRACKER_MODULES; ++k) {
      const module = `${TRACKER}.Module${k}`;
      const frame = get(module, '_currentframe');
      parts.push(`m${k}=${frame}` + (frame > 1 ? `/${get(module + '.inner', '_currentframe')}/${esc(get(module + '.inner.text', 'text'))}` : ''));
    }
    return `OWHUDTRACKPAGE mission=${esc(t && t.mission)} turnIn=${t && t.turnIn ? 1 : 0} lines=${t ? (t.lines || []).length : 0} ${parts.join(' ')}`;
  }

  const api = {WEAPON_FRAMES, textCall, weaponCalls, pageLine, readyLine, barFrame, downCalls, secondWindCalls, trackerCalls,
    downLine, trackerLine, TRACKER_MODULES};
  root.owHudFields = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
