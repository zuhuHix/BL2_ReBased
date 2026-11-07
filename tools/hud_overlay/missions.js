// Drive the installed StatusMenu movie's Missions frame (the mission log) the way StatusMenuExGFxMovie fills it: the list
// through the root's SetTextEntry / MoveHighlightBar / SetMoreBars / setActiveMission, the summary through the movie paths
// UpdateMissionDetails writes, and the level panel through SetPlayerLevel / SetPlayerXP (docs/verification/MENUS_PARITY.md).
// Rules shared with the Node tests are in mission_log.js. The movie, Ruffle and missionlog.json (tools/prepare_mission_log.py)
// are served from ignored local/ui/run. Inside UE the host calls owMissions(snapshot); without a host, ?demo=<state> shows
// host-made sample data (none / active / turnin / complete).
const startupAt = performance.now();
const log = window.owMissionLog;
const player = window.RufflePlayer.newest().createPlayer();
document.body.appendChild(player);
player.ruffle().load({url: 'UI_StatusMenu/harness.swf', base: 'UI_StatusMenu/'});
window.owPlayer = player;

const ROOT = '_level1', MISSIONS = ROOT + '.missions';
const call = (path, method, ...args) => player.ow(path, 'apply', method, args);
const get = (path, member) => player.ow(path, 'get', member);
const set = (path, member, value) => player.ow(path, 'set', member, value);
const query = new URLSearchParams(location.search);

let def = null, ready = false, snapshot = null;
let rows = [], selected = -1, listTop = 0, selectedId = null;
let canScroll = false;
// The hint line's font colour and size are the movie's own: the tooltips field's initial format, read once the movie is ready
// (readHintFormat), so no game value is copied into this file. Neutral values stand in only if the field cannot be read.
let hintColor = '#ffffff', hintSize = 12;
function readHintFormat() {
  const html = String(get(`${ROOT}.tooltips.tooltips`, 'htmlText') || '');
  const color = /COLOR="(#[0-9A-Fa-f]{6})"/.exec(html), size = /SIZE="(\d+)"/.exec(html);
  if (color) hintColor = color[1];
  if (size) hintSize = Number(size[1]);
}
let renderedKey = '';
window.owMissionsReady = false;

// Host-made sample snapshots for a page without a host (invented text, not game data). The XP curve is a placeholder.
const SAMPLE_CURVE = level => Math.round(60 * level ** 2.8);
function sample(state) {
  const objectives = [{name: 'go', text: 'Walk to the marked place', progress: state === 'active' ? 0 : 1, count: 1,
    done: state !== 'active', optional: false}, {name: 'shoot', text: 'Hit the target', progress: state === 'active' ? 2 : 5,
    count: 5, done: state !== 'active', optional: false}];
  const status = ['active', 'turnin', 'complete'].includes(state) ? state : undefined;
  const missions = status === undefined ? [] : [{id: 'sample.mission', name: 'Sample Mission', status, plot: false,
    summary: 'Check the page without the game.', description: 'A host-made sample mission.', turnIn: 'The sample giver',
    level: 8, xp: 120,
    tracked: status === 'active' || status === 'turnin', objectives}];
  const required = [8, 9].map(SAMPLE_CURVE);
  return {enabled: true, missions, player: {level: 8, xp: Math.round((required[0] + required[1]) / 2), maxLevel: 30, required}};   // an invented cap for the no-host sample
}

// The XP needed for `level`, from the two the host sends (start of this level, next level); other levels are not needed
// unless the XP is past the next level, which the host does not send.
function requiredFrom(player) {
  return level => level === player.level ? player.xpLevelStart ?? player.required?.[0] : player.xpNext ?? player.required?.[1];
}

function listedMissions() {
  return (snapshot?.missions || []).filter(mission => mission.status !== log.NOT_STARTED);
}

function setText(path, html) {
  set(path, 'html', true);
  set(path, 'htmlText', html);
}
// Several of the movie's text fields embed a font subset without '/', ':' or '-'; the movie's full imported $WillowBody alias has
// them (the same fix skills.js applies). A field the movie function wrote as plain text is re-set as HTML in that face.
const FACE = text => `<font face="$WillowBody">${text}</font>`;
function refaceText(path) {
  const text = get(path, 'text');
  if (typeof text === 'string') setText(path, FACE(log.escapeHtml(text)));
}

function renderList() {
  const entries = def.list.entries;
  const container = def.list.container;
  call(ROOT, 'ClearAllEntries', container);
  listTop = log.windowTop(rows.length, Math.max(0, selected), listTop, entries);
  for (let slot = 0; slot < entries && listTop + slot < rows.length; ++slot) {
    const row = rows[listTop + slot];
    // The row button's embedded font subset lacks ',' ':' '!' (the slice mission's name lost them in the engine capture): $WillowBody face.
    call(ROOT, 'SetTextEntry', container, slot + 1, FACE(log.rowText(row, listTop + slot === selected, def)), row.type);
  }
  if (selected >= 0) call(ROOT, 'MoveHighlightBar', container, selected - listTop + 1);
  call(ROOT, 'SetMoreBars', `${container}.${def.list.moreUp}`, listTop > 0, `${container}.${def.list.moreDown}`, listTop + entries < rows.length);
  // UpdateActiveMissionIcon: the tracked mission's slot (1-based, 0 = none).
  const tracked = rows.findIndex(row => row.kind === 'mission' && snapshot.missions.find(m => m.id === row.id)?.tracked);
  call(ROOT, 'setActiveMission', tracked >= listTop && tracked < listTop + entries ? tracked - listTop + 1 : 0);
}

function renderDetails() {
  const mission = rows[selected] && snapshot.missions.find(m => m.id === rows[selected].id);
  if (!mission) return;
  player.ow(`${MISSIONS}.missionSummary.header`, 'gotoAndStop', log.headerFrame(mission.status, mission.plot));
  set(`${MISSIONS}.missionSummary.header.description`, 'text', mission.name);
  player.ow(`${MISSIONS}.rewardPanel.OptionalRequired`, 'gotoAndStop', mission.plot ? 'required' : 'optional');
  setText(`${MISSIONS}.rewardPanel.OptionalRequired.textField`, log.escapeHtml(mission.plot ? def.strings.storyMission : def.strings.optionalMission));
  // SetRewardsTotalCredits shows the credits panel only for an amount above 0. The host sends "credits" only when it knows the
  // reward (it does not evaluate GetCurrencyReward yet), so an unknown reward hides the panel: UNVERIFIED for any mission.
  call(ROOT, 'SetRewardsTotalCredits', Number.isFinite(mission.credits) ? mission.credits : 0);
  call(ROOT, 'SetRewardsXP', def.strings.xpRewards.replace('%d', String(mission.xp || 0)));
  const playerLevel = snapshot.player?.level || 1;
  call(ROOT, 'SetLevelAndDifficulty', def.strings.missionLevel.replace('%d', String(mission.level || 0)),
    log.difficultyFrame(mission.level || 0, playerLevel, def.difficulty));
  refaceText(`${MISSIONS}.missionSummary.difficultybar.level`);
  // SetRewardCard: the host sends no reward item presentations (not evaluated), so the item reward clip stays hidden (UNVERIFIED).
  set(`${MISSIONS}.rewardPanel.reward_weap`, '_visible', false);
  setText(`${MISSIONS}.missionSummary.description`, FACE(log.descriptionHtml(mission, def)));
  set(`${MISSIONS}.missionSummary.description`, 'scroll', 1);
  canScroll = Number(get(`${MISSIONS}.missionSummary.description`, 'maxscroll')) > 1;
}

function renderGeneral() {
  const p = snapshot.player;
  if (!p) return;
  const bar = log.xpBar(p.level, p.xp, p.maxLevel, requiredFrom(p));
  call(ROOT, 'SetPlayerLevel', def.strings.levelString + bar.level);
  call(ROOT, 'SetPlayerXP', bar.text, bar.percent);
  refaceText(`${MISSIONS}.levelPanel.level`);
  refaceText(`${MISSIONS}.levelPanel.xp`);
  // UpdateGeneralDetails ends with SetScrollArrows: the summary's scroll arrows follow the description's scroll range.
  call(ROOT, 'SetScrollArrows');
  // The movie's SetPlayerTotalCredits writes to a clip that is not on the missions frame ('mission.credits'); called as the game does.
  if (Number.isFinite(p.credits)) call(ROOT, 'SetPlayerTotalCredits', p.credits);
}

function renderTooltips() {
  const mission = rows[selected] && snapshot.missions.find(m => m.id === rows[selected].id);
  const canTrack = !!mission && !mission.tracked && [log.ACTIVE, log.REQUIRED_DONE, log.TURN_IN].includes(mission.status);
  setText(`${ROOT}.tooltips.tooltips`, `<p align="center"><font face="$WillowBody" size="${hintSize}" color="${hintColor}">${
    log.escapeHtml(log.tooltipBar(def, {canTrack, canScroll}))}</font></p>`);
}

function render() {
  if (!ready || !snapshot) return;
  const key = JSON.stringify([snapshot, selectedId]);
  if (key === renderedKey) return;
  renderedKey = key;
  const missions = listedMissions();
  if (!missions.length) {
    // extSetupMissionLogScreen with no missions: GotoNoMissions. extSetupNoMissionsPage then writes
    // missions.title_questname.text, a field this movie does not have, so the frame's own static text stays.
    call(ROOT, 'gotoNoMissions');
    rows = []; selected = -1; selectedId = null;
    renderGeneral();
    setText(`${ROOT}.tooltips.tooltips`, `<p align="center"><font face="$WillowBody" size="${hintSize}" color="${hintColor}">${
      log.escapeHtml(def.strings.tooltipCancel)}</font></p>`);
    report();
    return;
  }
  call(ROOT, 'gotoRegularMissions');
  // gotoRegularMissions goes to a 'missionlog' label the missions clip does not have (its frames are 'default' and
  // 'missionlog_none'), so once the clip shows the no-missions frame it stays there; the game rebuilds the screen on each open.
  // The page returns the clip to its first frame and re-runs the list's init, as the root's missionlog frame script does.
  if (Number(get(MISSIONS, '_currentframe')) !== 1) {
    player.ow(MISSIONS, 'gotoAndStop', 1);
    call(`${MISSIONS}.selections`, 'init');
  }
  setText(`${MISSIONS}.selections.missionHeaderText`, log.listHeader(def));
  rows = log.listRows(missions, def);
  selected = rows.findIndex(row => row.id === selectedId);
  if (selected < 0) {
    // First open: the tracked mission when there is one, else the first mission (FindObjectIndex of the context object / native default).
    const tracked = rows.findIndex(row => row.kind === 'mission' && missions.find(m => m.id === row.id)?.tracked);
    selected = tracked >= 0 ? tracked : log.firstMission(rows);
  }
  selectedId = rows[selected]?.id ?? null;
  renderList();
  renderDetails();
  renderGeneral();
  renderTooltips();
  report();
}

function select(index) {
  if (index < 0 || index >= rows.length || rows[index].kind !== 'mission' || index === selected) return false;
  selected = index;
  selectedId = rows[index].id;
  renderedKey = '';
  render();
  return true;
}

// The movie's own list events (MissionListPanel: rollOver -> extLogListRollover(slot), click -> extLogListTrack(slot),
// more arrows -> extLogUp / extLogDown), slot being 0-based in the visible window.
window.extLogListRollover = slot => select(listTop + Number(slot));
window.extLogListTrack = slot => { select(listTop + Number(slot)); track(); };
window.extLogUp = () => select(log.moveSelection(rows, selected, -1));
window.extLogDown = () => select(log.moveSelection(rows, selected, +1));
window.extSetDescriptionScrollValue = () => {};
window.extSetCurrentScreen = () => {};

// HitEnterOnLog: ServerSetActiveMission on the highlighted mission; the host decides (OWMISSION {"track": id}).
function track() {
  const mission = rows[selected] && snapshot.missions.find(m => m.id === rows[selected].id);
  if (mission) console.log('OWMISSION ' + JSON.stringify({track: mission.id}));
}

function scrollDescription(step) {
  const path = `${MISSIONS}.missionSummary.description`;
  const max = Number(get(path, 'maxscroll')) || 1;
  set(path, 'scroll', Math.max(1, Math.min(max, (Number(get(path, 'scroll')) || 1) + step)));
}

function close() { location.href = '/__ow_close_missions'; }
window.addEventListener('keydown', event => {
  if (event.altKey || event.ctrlKey || event.metaKey) return;
  const key = event.key.toLowerCase();
  if (key === 'escape' || key === 'tab') { event.preventDefault(); close(); return; }
  if (key === 'l') { event.preventDefault(); return; }   // L stayed on the stock log (read live, one press); its hint offers Escape
  // M on the stock log went to the Map (read live, twice).
  if (key === 'm') { event.preventDefault(); location.href = '/__ow_tab_map'; return; }
  if (!ready || !snapshot) return;
  if (key === 'arrowup') { event.preventDefault(); window.extLogUp(); }
  else if (key === 'arrowdown') { event.preventDefault(); window.extLogDown(); }
  else if (key === 'enter') { event.preventDefault(); track(); }
  else if (key === 'pageup') { event.preventDefault(); scrollDescription(-3); }
  else if (key === 'pagedown') { event.preventDefault(); scrollDescription(3); }
  else if (key === 'i') { event.preventDefault(); location.href = '/__ow_tab_inventory'; }
  else if (key === 'k') { event.preventDefault(); location.href = '/__ow_tab_skills'; }
});

// Header tabs (nav1..nav5 = Missions, Map, Inventory, Skills, Challenges) and the close button, as HTML hit boxes over the movie.
function addHeaderHits() {
  const layer = document.getElementById('hit-layer');
  const hits = [['nav3', '/__ow_tab_inventory', 'Inventory tab'], ['nav4', '/__ow_tab_skills', 'Skills tab'],
    ['pcCloseButton', '/__ow_close_missions', 'Close missions']];
  for (const [clip, route, label] of hits) {
    const bounds = call(`${ROOT}.header.${clip}`, 'getBounds', ROOT);
    if (!bounds || ![bounds.xMin, bounds.yMin, bounds.xMax, bounds.yMax].every(Number.isFinite)) continue;
    const button = document.createElement('button');
    button.className = 'movie-hit';
    button.setAttribute('aria-label', label);
    Object.assign(button.style, {left: `${bounds.xMin}px`, top: `${bounds.yMin}px`,
      width: `${bounds.xMax - bounds.xMin}px`, height: `${bounds.yMax - bounds.yMin}px`});
    button.addEventListener('click', () => { location.href = route; });
    layer.appendChild(button);
  }
}

// Test hook: the page's state as one console line (the host's mission-log check reads it).
function report() {
  console.log('OWMISSIONPAGE ' + JSON.stringify({ready, rows: rows.map(row => row.kind === 'mission' ? row.id : `#${row.key}`),
    selected: selectedId, top: listTop, frame: get(MISSIONS, '_currentframe'),
    header: rows[selected] ? log.headerFrame(snapshot.missions.find(m => m.id === rows[selected].id).status,
      snapshot.missions.find(m => m.id === rows[selected].id).plot) : null}));
}
window.owMissionsReport = report;

// Host entry point: the mission-log snapshot (UOpenWillowQuest::MissionLogJson).
window.owMissions = state => {
  snapshot = state;
  render();
};

function finishLoading() {
  const loading = document.getElementById('loading');
  loading.classList.add('finished');
  setTimeout(() => { loading.hidden = true; }, 350);
}

async function boot() {
  const response = await fetch('missionlog.json');
  if (!response.ok) throw new Error('missing local missionlog.json (python tools/prepare_mission_log.py)');
  def = await response.json();
  const timer = setInterval(() => {
    try {
      if (typeof player.ow !== 'function') return;
      const total = get(ROOT, '_totalframes');
      if (!total || get(ROOT, '_framesloaded') < total) return;
      if (get(MISSIONS, '_totalframes') === undefined) { player.ow(ROOT, 'gotoAndStop', 'missionlog'); return; }
      clearInterval(timer);
      // The movie's ring is the curved glass sheet in game; over the transparent host viewport it is an opaque rectangle
      // (see skills.js), so glass_sheet.js draws the stand-in and the ring is hidden.
      set(`${ROOT}.ring`, '_visible', false);
      readHintFormat();
      window.owMenu3d.apply(document.body, Number(get(MISSIONS, '_yrotation')) || 0, Number(get(MISSIONS, '_x')), Number(get(MISSIONS, '_y')));
      addHeaderHits();
      ready = true;
      window.owMissionsReady = true;
      console.log(`OWINVTIME js_missions_ready pageMs=${Math.round(performance.now() - startupAt)}`);
      finishLoading();
      if (!snapshot && query.has('demo')) snapshot = sample(query.get('demo'));
      render();
    } catch (error) {
      console.warn('Waiting for StatusMenu:', error);
    }
  }, 100);
}

boot().catch(error => {
  document.querySelector('#loading small').textContent = 'Could not load the mission log. Press Esc to return.';
  console.error('OpenWillow Missions:', error);
});
