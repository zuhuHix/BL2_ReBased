// Drive the installed StatusMenu movie's Map frame the way StatusMenuMapGFxObject does (docs/verification/MAP_SCREEN.md):
// the map clip's own ActionScript loads the level's tactical map movie (setMapMovie), scales it (setMapScale), places icons
// from its landmark templates (addLandmark / updateLandmark) and toggles the legend and the fog blobs. Where the game's
// natives decide (centring, the player arrow, which icons, pan and zoom steps) the rules below are host choices, marked
// UNVERIFIED. The state is the HUD minimap's (OpenWillowMinimap::StateField) plus the map's name.
(function (root) {
  'use strict';
  // The tactical map movie: movie x follows world +Y and movie y world -X, 128 uu per movie unit, origin at the tactical
  // map volume's bounds centre (measured for the HUD minimap in the running game, docs/verification/HUD_MINIMAP.md).
  const UU_PER_MOVIE_UNIT = 128;
  // StatusMenuMapGFxObject (script and its installed class defaults): the map opens at the saved scale, else MinScale (135),
  // and setMapScale sets that number as the map movie's percent scale. AnalogScale zooms by a factor (clamped to MinScale ..
  // MaxScale 395); PanHorz / PanVert move the centre by PanIncrement (0.05) of the map's size, times
  // lerp(1, scale / MinScale, PanZoomFactor 0.25). Both also multiply by a frame-time term (GetDeltaTimeScalar, native), so
  // what one key press does is not read: one press here zooms by ZOOM_FACTOR and pans one PanIncrement (UNVERIFIED).
  const MIN_SCALE = 135, MAX_SCALE = 395, ZOOM_FACTOR = 1.2, PAN_INCREMENT = 0.05, PAN_ZOOM_FACTOR = 0.25;
  // Landmark template frames (the StatusMenu movie's landmark sprite) for the minimap state's icon kinds; the stock kinds
  // follow StatusMenuMapGFxObject.CompassIconFrames (shop, health, rstation = travel station, customization, transition).
  // How long the page waits for the StatusMenu movie, and then for the map movie, before it shows why the map is missing.
  const READY_TIMEOUT_MS = 15000, MAP_LOAD_TIMEOUT_MS = 10000;
  const LANDMARK_FRAMES = {shop:'shop', health:'health', fasttravel:'rstation', customize:'customization', leveltravel:'transition',
    eligible:'missionNew', turnin:'missionComplete', objective:'objective'};

  // A world point in the map movie's own units.
  function moviePoint(x, y, centre) {
    return [(y - centre[1]) / UU_PER_MOVIE_UNIT, -(x - centre[0]) / UU_PER_MOVIE_UNIT];
  }
  const clampScale = scale => Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale));
  // One pan press as a fraction of the map's size (PanHorz / PanVert without the frame-time term).
  const panFraction = scale => PAN_INCREMENT * (1 + (scale / MIN_SCALE - 1) * PAN_ZOOM_FACTOR);

  // Where the map movie goes and where each icon lands, in the map clip's coordinates. The view opens centred on the player
  // (`pan` [0, 0]) at `scale` percent, the player's point at `centre` (the crosshairs). UNVERIFIED: the natives' centring.
  function layout(state, view) {
    const m = state.minimap, s = view.scale / 100;
    const [px, py] = moviePoint(m.x, m.y, m.centre);
    const mapX = view.centre[0] - px * s + view.pan[0], mapY = view.centre[1] - py * s + view.pan[1];
    const at = (x, y) => { const [u, v] = moviePoint(x, y, m.centre); return [mapX + u * s, mapY + v * s]; };
    const [playerX, playerY] = at(m.x, m.y);
    // The arrow points along the view yaw with north (world +X) up: rotation = yaw (UNVERIFIED sign and offset).
    const landmarks = [{key:'player', template:'players', frame:'player', x:playerX, y:playerY, r:m.yaw}];
    const counts = {};
    for (const icon of m.icons || []) {
      const frame = LANDMARK_FRAMES[icon.kind];
      if (!frame) continue;
      const index = counts[icon.kind] = (counts[icon.kind] ?? -1) + 1;
      const [x, y] = at(icon.x, icon.y);
      landmarks.push({key:`${icon.kind}${index}`, template:'landmarks', frame, x, y, r:0});
    }
    return {mapX, mapY, scale:view.scale, landmarks};
  }

  // Pan limits: the map point under the crosshairs (the player's point minus pan / scale) stays inside the map movie's own
  // rectangle `rect` (movie units), so the map cannot be pushed out of the mask at any zoom. A host rule (the game clamps
  // its DesiredCenter natively; UNVERIFIED).
  function clampPan(pan, player, rect, scalePercent) {
    if (!rect) return pan;
    const s = scalePercent / 100;
    const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
    return [clamp(pan[0], (player[0] - rect.xMax) * s, (player[0] - rect.xMin) * s),
      clamp(pan[1], (player[1] - rect.yMax) * s, (player[1] - rect.yMin) * s)];
  }

  // Status-menu tabs from the Map screen (StatusMenuExGFxMovie: One..Five -> TrySwitchToScreen, Comma / Period ->
  // GotoPrevTab / GotoNextTab). From Map, previous is Missions when there are missions (else Challenges) and next is
  // Inventory. Returns a route, '' for "stay", or null for a page the host does not have.
  const TAB_ROUTES = {1:'/__ow_tab_missions', 2:'', 3:'/__ow_tab_inventory', 4:'/__ow_tab_skills', 5:null};
  function tabRoute(key, hasMissions = true) {
    if (key in TAB_ROUTES) return TAB_ROUTES[key];
    if (key === ',') return hasMissions ? TAB_ROUTES[1] : TAB_ROUTES[5];
    if (key === '.') return TAB_ROUTES[3];
    return undefined;
  }

  root.owMapScreen = {moviePoint, clampScale, panFraction, clampPan, layout, tabRoute, READY_TIMEOUT_MS, MAP_LOAD_TIMEOUT_MS, LANDMARK_FRAMES, MIN_SCALE, MAX_SCALE, ZOOM_FACTOR};

  // ---- Page ----------------------------------------------------------------------------------------------------------
  if (!root.RufflePlayer || !root.document) return;
  const startupAt = performance.now();
  const player = root.RufflePlayer.newest().createPlayer();
  root.document.body.appendChild(player);
  player.ruffle().load({url:'UI_StatusMenu/harness.swf', base:'UI_StatusMenu/'});
  root.owPlayer = player;
  const ROOT = '_level1', MAP = ROOT + '.map';
  const call = (path, method, ...args) => player.ow(path, 'apply', method, args);
  const get = (path, member) => player.ow(path, 'get', member);
  const set = (path, member, value) => player.ow(path, 'set', member, value);

  let ready = false, mapLoaded = false, loadingMovie = '', state = null, mapRect = null, mapLoadTimer = 0;
  const view = {scale:MIN_SCALE, pan:[0, 0], centre:[0, 0]};
  const placed = new Map(); // landmark key -> clip name the movie gave it

  function close() { root.location.href = '/__ow_close_map'; }
  // The loading panel stays up until the map movie has loaded; on a failure it shows why (Escape and Tab still close).
  let lastMessage = '';
  function showMessage(text) {
    if (text === lastMessage) return; // the host pushes its state every second
    lastMessage = text;
    const loading = root.document.getElementById('loading');
    loading.hidden = false;
    loading.classList.remove('finished');
    loading.querySelector('small').textContent = `${text} Press Esc to return.`;
    console.log(`OpenWillow Map message: ${text}`);
  }
  function hideLoading() {
    const loading = root.document.getElementById('loading');
    loading.classList.add('finished');
    setTimeout(() => { loading.hidden = true; }, 350);
  }
  function go(route) { if (route) root.location.href = route; }
  root.addEventListener('keydown', event => {
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    const key = event.key.toLowerCase();
    // M does not close the stock map (read live, twice); its hint offers Escape.
    if (key === 'escape' || key === 'tab') { event.preventDefault(); close(); return; }
    if (key === 'm') { event.preventDefault(); return; }
    const route = tabRoute(event.key);
    if (route !== undefined) { event.preventDefault(); go(route); return; }
    if (key === 'i') { event.preventDefault(); go('/__ow_tab_inventory'); return; }
    if (key === 'k') { event.preventDefault(); go('/__ow_tab_skills'); return; }
    if (key === 'l') { event.preventDefault(); go('/__ow_tab_missions'); return; }
    if (!ready || !mapLoaded) return;
    if (key === 'pagedown' || key === '=' || key === '+') { event.preventDefault(); zoom(ZOOM_FACTOR); }
    else if (key === 'pageup' || key === '-' || key === '_') { event.preventDefault(); zoom(1 / ZOOM_FACTOR); }
    else if (key.startsWith('arrow')) { event.preventDefault(); pan(key.slice(5)); }
    else if (key === 'q') { event.preventDefault(); call(MAP, 'toggleLegend'); report(); }
  });
  root.addEventListener('wheel', event => { if (ready && mapLoaded) zoom(event.deltaY < 0 ? ZOOM_FACTOR : 1 / ZOOM_FACTOR); }, {passive:true});

  function zoom(factor) {
    const before = view.scale;
    view.scale = clampScale(view.scale * factor);
    // Keep the point under the crosshairs where it is.
    const k = view.scale / before;
    view.pan = limitPan([view.pan[0] * k, view.pan[1] * k]);
    render();
  }
  function limitPan(pan) {
    const m = state?.minimap;
    return m ? clampPan(pan, moviePoint(m.x, m.y, m.centre), mapRect, view.scale) : pan;
  }
  function pan(direction) {
    const step = {left:[1, 0], right:[-1, 0], up:[0, 1], down:[0, -1]}[direction];
    if (!step) return;
    const width = Number(get(`${MAP}.mapPlaceholder`, '_width')) || 0, height = Number(get(`${MAP}.mapPlaceholder`, '_height')) || 0;
    const f = panFraction(view.scale);
    view.pan = limitPan([view.pan[0] + step[0] * width * f, view.pan[1] + step[1] * height * f]);
    render();
  }

  // The map clip's ExternalInterface calls: its onLoad, and the map movie's load (the game reads the movie's rectangle and
  // the discovered areas there; this host has no fog state but "not uncovered", as the minimap).
  root.extStatusMenuMapOnLoad = () => {};
  root.extStatusMenuMapPostMapMovieLoad = () => {
    mapLoaded = true;
    clearTimeout(mapLoadTimer);
    call(MAP, 'setMapScale', 100);
    // The map movie's own rectangle in its units (the pan limits).
    const rect = call(`${MAP}.mapPlaceholder`, 'getRect', `${MAP}.mapPlaceholder`);
    mapRect = rect && [rect.xMin, rect.xMax, rect.yMin, rect.yMax].every(Number.isFinite) && rect.xMax > rect.xMin ? rect : null;
    call(MAP, 'setMapScale', view.scale);
    hideLoading();
    for (const [blob, uncovered] of Object.entries(state?.minimap?.blobs || {})) call(MAP, 'toggleFogOfWarBlob', blob, Boolean(uncovered));
    console.log(`OpenWillow Map movie loaded ${loadingMovie}`);
    render();
  };

  function render() {
    if (!ready || !state) return;
    if (state.name) set(`${MAP}.header.mapName`, 'text', state.name);
    const m = state.minimap;
    if (!m || !m.movie) { showMessage('No map for this area (the minimap manifest is missing: tools/prepare_minimap.py).'); return; }
    if (loadingMovie !== m.movie) {
      loadingMovie = m.movie; mapLoaded = false; mapRect = null;
      call(MAP, 'setMapMovie', m.movie);
      clearTimeout(mapLoadTimer);
      mapLoadTimer = setTimeout(() => { if (!mapLoaded) showMessage(`The map movie did not load (${m.movie}).`); }, MAP_LOAD_TIMEOUT_MS);
      return;
    }
    if (!mapLoaded) return;
    view.pan = limitPan(view.pan); // the player moves too
    const out = layout(state, view);
    call(MAP, 'setMapScale', out.scale);
    set(`${MAP}.mapPlaceholder`, '_x', out.mapX);
    set(`${MAP}.mapPlaceholder`, '_y', out.mapY);
    const offset = [Number(get(`${MAP}.landmarkContainer`, '_x')) || 0, Number(get(`${MAP}.landmarkContainer`, '_y')) || 0];
    const seen = new Set();
    for (const mark of out.landmarks) {
      seen.add(mark.key);
      const x = mark.x - offset[0], y = mark.y - offset[1];
      if (!placed.has(mark.key)) placed.set(mark.key, call(MAP, 'addLandmark', mark.template, x, y, mark.r, 100, mark.frame, '', false));
      else call(MAP, 'updateLandmark', placed.get(mark.key), x, y, mark.r, 100, mark.frame, '', false);
    }
    for (const [key, clip] of placed) if (!seen.has(key)) { call(MAP, 'removeLandmark', clip); placed.delete(key); }
    report();
  }

  // Test hook: one console line per change of what the page shows (the host's map capture reads it).
  let lastReport = '';
  function report() {
    const line = 'OWMAPPAGE ' + JSON.stringify({ready, mapLoaded, movie:loadingMovie, scale:view.scale, pan:view.pan.map(Math.round),
      landmarks:[...placed.keys()], legend:Boolean(get(`${MAP}.legend`, '_visible')), name:state?.name || ''});
    if (line !== lastReport) { console.log(line); lastReport = line; }
  }

  // Host entry point: {minimap, name} (OpenWillowMapScreen::StateJson).
  root.owMap = next => { state = next; render(); };

  // The hint bar's text is the movie's own "$<StringAliasMap:map_bar>" token (SetTooltipsText sets nothing on the Map
  // screen); tools/prepare_map_screen.py resolves it into the ignored map.json. The field keeps its own font and colour.
  let hintBar = '';
  fetch('map.json').then(r => (r.ok ? r.json() : null)).then(data => { hintBar = data?.strings?.bar || ''; setHintBar(); },
    () => console.log('OpenWillow Map: no map.json (python tools/prepare_map_screen.py); the hint bar keeps its token'));
  function setHintBar() {
    if (!ready && typeof player.ow !== 'function') return;
    const path = `${ROOT}.tooltips.tooltips`, html = String(get(path, 'htmlText') || '');
    const escaped = hintBar.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    // The field's embedded face holds only the token's own glyphs (no brackets, + or /): print with the font library's
    // $WillowBody, as the other pages' hint lines do, keeping the field's size and colour.
    if (hintBar && html.includes('StringAliasMap:map_bar'))
      set(path, 'htmlText', html.replace(/FACE="WillowBody"/, 'FACE="$WillowBody"').replace(/\$&lt;StringAliasMap:map_bar&gt;/, escaped));
  }

  function addHeaderHits() {
    const layer = root.document.getElementById('hit-layer');
    const hits = [['nav1', '/__ow_tab_missions', 'Missions tab'], ['nav3', '/__ow_tab_inventory', 'Inventory tab'],
      ['nav4', '/__ow_tab_skills', 'Skills tab'], ['pcCloseButton', '/__ow_close_map', 'Close map']];
    for (const [clip, route, label] of hits) {
      const bounds = call(`${ROOT}.header.${clip}`, 'getBounds', ROOT);
      if (!bounds || ![bounds.xMin, bounds.yMin, bounds.xMax, bounds.yMax].every(Number.isFinite)) continue;
      const button = root.document.createElement('button');
      button.className = 'movie-hit';
      button.setAttribute('aria-label', label);
      Object.assign(button.style, {left:`${bounds.xMin}px`, top:`${bounds.yMin}px`,
        width:`${bounds.xMax - bounds.xMin}px`, height:`${bounds.yMax - bounds.yMin}px`});
      button.addEventListener('click', () => { root.location.href = route; });
      layer.appendChild(button);
    }
  }

  const readyBy = performance.now() + READY_TIMEOUT_MS;
  const timer = setInterval(() => {
    if (performance.now() > readyBy) {
      clearInterval(timer);
      showMessage('The status menu movie did not load (UI_StatusMenu in local/ui/run).');
      return;
    }
    try {
      if (typeof player.ow !== 'function') return;
      const total = get(ROOT, '_totalframes');
      if (!total || get(ROOT, '_framesloaded') < total) return;
      if (get(`${MAP}.mapPlaceholder`, '_name') === undefined) { player.ow(ROOT, 'gotoAndStop', 'map'); return; }
      clearInterval(timer);
      // The movie's ring is the curved glass sheet in game (see skills.js); glass_sheet.js draws the stand-in.
      set(`${ROOT}.ring`, '_visible', false);
      setHintBar();
      root.owMenu3d?.apply(root.document.body, Number(get(MAP, '_yrotation')) || 0, Number(get(MAP, '_x')), Number(get(MAP, '_y')));
      view.centre = [Number(get(`${MAP}.crosshairs`, '_x')) || 0, Number(get(`${MAP}.crosshairs`, '_y')) || 0];
      addHeaderHits();
      ready = true;
      console.log(`OpenWillow Map page ready in ${Math.round(performance.now() - startupAt)}ms`);
      // No state or no map movie within the deadline: say so instead of an empty map.
      mapLoadTimer = setTimeout(() => {
        if (!mapLoaded) showMessage(state ? 'The map movie did not load.' : 'The host sent no map state.');
      }, MAP_LOAD_TIMEOUT_MS);
      render();
    } catch (error) {
      console.warn('Waiting for StatusMenu:', error);
    }
  }, 100);
})(typeof window !== 'undefined' ? window : globalThis);
