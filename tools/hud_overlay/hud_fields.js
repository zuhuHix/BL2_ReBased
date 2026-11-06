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
      // The turn-in label is the strings file's; without it (no file, or no value) the movie keeps its own text: null = no call.
      if (tracker.turnIn) rows.push(['TurnIn', strings && typeof strings.turnIn === 'string' ? strings.turnIn : null]);
      else for (const line of tracker.lines || [])
        rows.push([line.complete ? 'Complete' : line.optional ? 'Optional' : 'NotComplete', line.text || '']);
    }
    const calls = [];
    for (let k = 1; k <= TRACKER_MODULES; ++k) {
      const module = `${TRACKER}.Module${k}`;
      const row = rows[k - 1];
      if (!row) { calls.push([module, 'gotoAndStop', 'Off']); continue; }
      calls.push([module, 'gotoAndStop', 'NoMovement'], [module + '.inner', 'gotoAndStop', row[0]]);
      if (row[1] !== null) calls.push(textCall(module + '.inner.text', row[1]));
    }
    return calls;
  }

  // The use prompt: null, or {frame: a label of p1.tools, text: for its desc field} (OpenWillowUsePrompt.cpp). Which of these the game
  // drives, and how, is native and was not read live (UNVERIFIED). `desc` is both the field's instance name and its bound variable on the
  // clip. Read in the Ruffle bench (2026-11-06): a dot path to it does not give the field, and writing through that path only took the first
  // time, so a later prompt kept the first one's words. Writing and reading the variable on the clip tracks every change.
  const TOOLS = P1 + '.tools', USE_RETRY_MS = 1000;
  function useCalls(prompt) {
    if (!prompt || !prompt.frame) return [[TOOLS, 'gotoAndStop', 'none']];
    return [[TOOLS, 'gotoAndStop', prompt.frame], [TOOLS, 'set', 'desc', prompt.text || '']];
  }
  // The field is HTML: the variable reads back wrapped in its format tags.
  function plainDesc(value) {
    if (typeof value !== 'string') return null;
    return value.replace(/<[^>]*>/g, '').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&amp;/g, '&');
  }
  function newUseMemo() { return {line: null, failed: null, retryAt: 0, frames: {none: 1}}; }

  // Applies the use prompt once per change. A prompt counts as applied only when the clip reads back on that label's frame with exactly
  // the text asked for; the frame of a label is learned from the first good read (`none` is the clip's first frame), and a new label must
  // not be on another label's frame, which is where a jump that did nothing would leave it. A failure is logged once and tried again
  // about a second later, and stays inside this function, so it can never stop the rest of the HUD.
  function applyUse(call, prompt, memo, log, now) {
    const line = JSON.stringify(prompt);
    if (line === memo.line) return;
    if (line === memo.failed && now < memo.retryAt) return;
    const label = prompt && prompt.frame ? prompt.frame : 'none';
    const wanted = label === 'none' ? null : prompt.text || '';
    let frame, shown = '', problem = '';
    memo.line = null;   // the clip is about to change: until a good read-back it is known to show nothing, so going back to A re-applies A
    try {
      for (const c of useCalls(prompt)) call(c[0], c[1], c[2], c[3]);
      frame = call(TOOLS, 'get', '_currentframe');
      const known = memo.frames[label];
      if (!(Number(frame) > 0)) problem = 'no frame read back';
      else if (known !== undefined ? Number(frame) !== known : Object.values(memo.frames).includes(Number(frame))) problem = `the clip is on frame ${frame}, not ${label}`;
      else {
        // The jump landed, so this is the label's frame even if the text fails below: a later jump that does nothing cannot pass from here.
        memo.frames[label] = Number(frame);
        if (wanted !== null) {
          shown = plainDesc(call(TOOLS, 'get', 'desc'));
          if (shown !== wanted) problem = `the text reads ${JSON.stringify(shown)}`;
        }
      }
    } catch (error) { problem = String(error); }
    if (problem) {
      if (memo.failed !== line) log(`OWUSEPROMPTPAGE failed asked=${line}: ${problem}`);
      memo.failed = line;
      memo.retryAt = now + USE_RETRY_MS;
      return;
    }
    log(`OWUSEPROMPTPAGE asked=${line} frame=${frame} text=${shown}`);
    memo.line = line;
    memo.failed = null;
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

  // ---- Damage-direction arrows and the level-up banner (docs/verification/HUD_PARITY.md) ----
  // Read from the movie's own frame scripts: p1.dmg_u / dmg_r / dmg_d / dmg_l stop on frame 1 with their inner clip hidden;
  // from frame 2 they show it and animate by themselves (36 frames). The inner clip's labels name the absorber (`Health`, or
  // a shield type: `Original`, `Generic`, `Nova`, ...); the host has no shield damage, so every hit is `Health` (UNVERIFIED
  // which label the game picks). p1.levelup's `go` frame writes the banner text itself, starts the inner animation and
  // flashes the XP bar. `levelDisp.text` is left as it is: whatever the native puts there was not observed.
  // feedback: {hits, side: 'u'|'r'|'d'|'l', levelUps, level}; previous: the last one applied (or null).
  function feedbackCalls(feedback, previous) {
    const calls = [];
    if (!feedback) return calls;
    const before = previous || {hits: feedback.hits, levelUps: feedback.levelUps};   // the first state is the baseline
    if (Number(feedback.hits) > Number(before.hits || 0) && /^[urdl]$/.test(feedback.side || '')) {
      const arrow = `${P1}.dmg_${feedback.side}`;
      calls.push([arrow + '.inner', 'gotoAndStop', 'Health'], [arrow, 'gotoAndPlay', 2]);
    }
    // One banner per level-up count increase (the host counts one per level change, a multi-level jump included). The
    // banner is wound back to `stop` first: a `go` while it already sits on `go` would not run that frame's script again.
    if (Number(feedback.levelUps) > Number(before.levelUps || 0))
      calls.push([P1 + '.levelup', 'gotoAndStop', 'stop'], [P1 + '.levelup', 'gotoAndPlay', 'go']);
    return calls;
  }
  // banners: how many banners the page has played (index.html counts the `go` calls), compared with the host's level-ups.
  function feedbackLine(shown, get, banners) {
    const f = shown.feedback || {};
    const arrows = ['u', 'r', 'd', 'l'].map(side => `${side}=${get(`${P1}.dmg_${side}`, '_currentframe')}/${get(`${P1}.dmg_${side}.inner`, '_currentframe')}`);
    return `OWHUDFXPAGE hits=${f.hits || 0} side=${f.side || '-'} levelUps=${f.levelUps || 0} banners=${banners || 0} ${arrows.join(' ')}`
      + ` levelup=${get(P1 + '.levelup', '_currentframe')}/${get(P1 + '.levelup.inner', '_currentframe')} levelText=${esc(get(P1 + '.levelup.inner.levelup1', 'text'))}`;
  }

  const api = {WEAPON_FRAMES, textCall, weaponCalls, pageLine, readyLine, barFrame, downCalls, secondWindCalls, trackerCalls,
    downLine, trackerLine, TRACKER_MODULES, feedbackCalls, feedbackLine, useCalls, applyUse, newUseMemo};
  root.owHudFields = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
