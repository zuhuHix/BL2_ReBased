// Marcus's mission screen: BL2's own movie in Ruffle, filled from the host's state. The host calls window.owMissionScreenState(state) with
// {page: 'offer' | 'turnin' | 'reward' | 'closed', log: <its mission log snapshot>} and window.owMissionScreenKey(key) for the scroll keys; it owns every other
// key. Without the movie, or if it fails, a plain panel says the same things, so the player can still see the offer and what the keys do.
// ?demo=offer|turnin|reward shows invented sample data without a host. The rules are in mission_screen_model.js.
const model = window.owMissionScreen;
const $ = id => document.getElementById(id);
const MOVIE = {url: 'UI_Mission/harness.swf', base: 'UI_Mission/'};

let strings = model.FALLBACK, player = null, movieLive = false, state = null, drawnKey = '', drawTimer = 0;
const frameOf = {offer: 'intro', turnin: 'intro', reward: 'reward'};

function call(path, op, name, value) {
  return player.ow(path, op, name, value);
}
function exists(path) {
  try { return call(path, 'get', '_name') !== undefined; } catch (error) { return false; }
}
function applyStep([op, path, name, value]) {
  try {
    if (op === 'set') call(path, 'set', name, value);
    else if (op === 'html') { call(path, 'set', 'html', true); call(path, 'set', name, value); }
    else if (op === 'goto') call(path, name, value);
    else if (op === 'call') call(path, 'apply', name, value);
  } catch (error) { console.log(`OpenWillow mission screen write failed: ${path} ${name} (${error})`); }
}

function showPanel(page, mission) {
  const panel = $('panel');
  panel.hidden = !mission;
  if (!mission) return;
  const lines = model.panelText(page, mission, strings);
  panel.replaceChildren(...lines.map((text, i) => {
    const line = document.createElement(i === 1 ? 'h2' : 'p');
    line.textContent = text;
    return line;
  }));
}

// Draws the page once the movie has settled: it plays into its frame and re-creates the clip's fields on the way, so what is written earlier is lost.
function drawMovie(page, mission, attempts = 300, lastFrame = -1, sameFor = 0) {
  clearTimeout(drawTimer);
  const clip = page === 'reward' ? '_level1.reward' : '_level1.missions.header';
  const frame = exists(clip) ? Number(call('_level1', 'get', '_currentframe')) : -1;
  const still = frame === lastFrame ? sameFor + 1 : 0;
  if (frame < 0 || still < model.SETTLED_POLLS) {
    if (attempts <= 0) {
      console.log(`OWMISSIONPAGE ${JSON.stringify({page, drawn: false, why: 'the clip never appeared'})}`);
      showPanel(page, mission);
      return;
    }
    drawTimer = setTimeout(() => drawMovie(page, mission, attempts - 1, frame, still), 50);
    return;
  }
  const plan = page === 'reward' ? model.rewardPlan(mission, strings) : model.listPlan(page, mission, strings);
  plan.forEach(applyStep);
  console.log(`OWMISSIONPAGE ${JSON.stringify({page, drawn: true, movie: true, frame, mission: mission.name})}`);
}

function draw() {
  const page = state?.page, mission = model.missionOf(state);
  const open = page === 'offer' || page === 'turnin' || page === 'reward';
  const key = open && mission ? `${page}|${mission.id}|${movieLive}` : '';
  if (key === drawnKey) return;
  drawnKey = key;
  document.body.classList.toggle('open', Boolean(key));
  clearTimeout(drawTimer);
  if (!key) { $('panel').hidden = true; return; }
  if (!movieLive) {
    showPanel(page, mission);
    console.log(`OWMISSIONPAGE ${JSON.stringify({page, drawn: true, movie: false, mission: mission.name})}`);
    return;
  }
  $('panel').hidden = true;
  try { call('_level1', 'gotoAndPlay', frameOf[page]); } catch (error) { console.log(`OpenWillow mission screen goto failed (${error})`); }
  drawMovie(page, mission);
}

window.owMissionScreenState = next => { state = next; draw(); };
window.owMissionScreenKey = key => {
  if (!movieLive) return;
  const field = '_level1.missions.description';
  try {
    if (key === 'PageUp') call(field, 'set', 'scroll', Number(call(field, 'get', 'scroll')) - 1);
    if (key === 'PageDown') call(field, 'set', 'scroll', Number(call(field, 'get', 'scroll')) + 1);
  } catch (error) { console.log(`OpenWillow mission screen scroll failed (${error})`); }
};

function ready() {
  console.log('OWMISSIONREADY');
  const demo = new URLSearchParams(location.search).get('demo');
  if (demo) {
    window.owMissionScreenState({page: demo, log: {missions: [{id: 'sample', name: 'Sample Mission', plot: false, level: 8, xp: 120,
      description: 'A host-made sample mission, to check the page without the game.', turnInDescription: 'The sample is done.'}]}});
  }
}

// Ready once the movie is on a frame and has stayed there: a goto sent while it is still loading is lost.
function waitForMovie(triesLeft, lastFrame = -1, sameFor = 0) {
  const frame = exists('_level1') ? Number(call('_level1', 'get', '_currentframe')) : -1;
  const still = frame > 0 && frame === lastFrame ? sameFor + 1 : 0;
  if (still >= model.SETTLED_POLLS) { movieLive = true; ready(); return; }
  if (triesLeft <= 0) { console.log('OpenWillow mission screen movie did not start; using the plain panel'); ready(); return; }
  setTimeout(() => waitForMovie(triesLeft - 1, frame, still), 100);
}

const loadStrings = fetch('missionscreen.json').then(r => (r.ok ? r.json() : null)).then(data => { if (data?.strings) strings = {...model.FALLBACK, ...data.strings}; }, () => {});
loadStrings.then(() => {
  player = window.RufflePlayer.newest().createPlayer();
  $('stage').appendChild(player);
  window.owPlayer = player;
  window.owMovieGuard.load(player, MOVIE, 'mission screen', window.owMovieGuard.FILES.missionScreen, () => { movieLive = false; drawnKey = ''; draw(); })
    .then(loading => { if (loading) waitForMovie(300); else ready(); });
});
