// BL2's own vending movie for the vending page: UI_VendingMachine/VendingMachineInterface (the item of the day, the credits,
// the ammo panel and the health bars) with UI_TwoPanelInterface/TwoPanelInterface (the header, the list panel and the item
// card), both converted by tools/prepare_vending_movie.py into ignored local/ui/run. The page's rules (vending_model.js)
// and keys (vending.js) are unchanged; this file only draws the page's state into the movie, as inventory.js does for the
// StatusMenu. In the game the lists are filled natively; here the page attaches the movie's own cells and headers.
// Layout, tab mapping and labels are host choices and UNVERIFIED against the game (docs/verification/VENDORS.md).
(function (root) {
  'use strict';

  // ---- Pure helpers (node tests: tests/vending_movie_test.js) ----------------------------------------------------------
  // The header frame and the background of the list panel and the item of the day, per machine kind. The shop's script
  // passes a number per shop type, and each panel's script wrapper turns it into one of these frame labels before calling
  // the movie (both read with script_disasm.py): Marcus red, the Ammo Dump green, Dr. Zed grey, the Black Market purple.
  const HEADER_FRAME = {weapons:'weapons', items:'ammo', health:'medicine', blackmarket:'eridium'};
  const PANEL_BACKGROUND = HEADER_FRAME;
  // The page's three lists as the header's three tabs. The game's tab icons and their order are not read: host choice.
  const TAB_SIDE = {backpack:'left', shop:'middle', buyback:'right'};
  const TAB_ORDER = ['backpack', 'shop', 'buyback'];
  const TAB_TITLE = {shop:'PURCHASE', backpack:'SELL', buyback:'BUY BACK'};
  const ACTION_WORD = {shop:'Purchase', backpack:'Sell', buyback:'Buy back'};
  // Section headers in the list, in our own wording. Guns group by their card type.
  const KIND_SECTION = {ammo:'Ammo', item:'Items', health:'Health', shield:'Shields', grenade_mod:'Grenade Mods',
    class_mod:'Class Mods'};
  const GUN_SECTION = {pistol:'Pistols', smg:'SMGs', shotgun:'Shotguns', 'assault rifle':'Assault Rifles', ar:'Assault Rifles',
    'sniper rifle':'Sniper Rifles', sniper:'Sniper Rifles', 'rocket launcher':'Launchers', launcher:'Launchers', rocket:'Launchers'};
  // The card's item type for gear rows (item_card.js picks the type icon from it); ammo and health rows get none.
  const GEAR_TYPES = new Set(['shield', 'grenade_mod', 'class_mod']);
  // Ammo panel fields per host pool key ("pistol" ammo lives in the clip named "repeater"; as inventory.js).
  const AMMO_CLIPS = {pistol:'ammo_repeater', smg:'ammo_smg', ar:'ammo_ar', shotgun:'ammo_shotgun', sniper:'ammo_sniper',
    launcher:'ammo_rocket', grenade:'ammo_grenade'};
  const MAX_DISPLAY_CREDITS = 99999999;
  // How long the movie may take to load before the page gives up on it and runs the HTML page (host choice): a load that
  // never settles would otherwise leave an empty player up for good.
  const LOAD_TIMEOUT_MS = 8000;

  // The cell picture. The game's cells show a 3D thumbnail it renders at run time, which this host does not have. The page
  // puts the item card's own type icon in the cell instead (a frame of `item card - weapon type`, read in the bench), tinted
  // with the rarity colour. Ammo and class mods have no icon frame and stay empty. A stand-in, not the game's picture.
  const GUN_ICON = {pistol:'Pistol', smg:'SMG', shotgun:'Shotgun', 'assault rifle':'ar', ar:'ar', 'sniper rifle':'Sniper',
    sniper:'Sniper', 'rocket launcher':'Rocket', launcher:'Rocket', rocket:'Rocket'};
  const KIND_ICON = {shield:'Shield', grenade_mod:'Grenade', health:'health'};
  // Ammo shows the icon of its gun type, as the real Ammo Dump's card does for its ammo (one frame, 2026-11-07 capture).
  const AMMO_ICON = {pistol:'Pistol', smg:'SMG', ar:'ar', shotgun:'Shotgun', sniper:'Sniper', launcher:'Rocket', grenade:'Grenade'};
  const AMMO_TYPE = {pistol:'pistol', smg:'smg', ar:'assault rifle', shotgun:'shotgun', sniper:'sniper rifle', launcher:'rocket launcher'};
  function cellIcon(row) {
    if (!row) return null;
    if (row.kind === 'weapon') return GUN_ICON[String(row.card && row.card.type || '').trim().toLowerCase()] || null;
    if (row.kind === 'ammo') return AMMO_ICON[row.ammoType] || null;
    return KIND_ICON[row.kind] || null;
  }

  function sectionLabel(row) {
    if (row.kind === 'weapon') return GUN_SECTION[String(row.card && row.card.type || '').trim().toLowerCase()] || 'Weapons';
    return KIND_SECTION[row.kind] || 'Items';
  }

  // The lists as the movie shows them: rows of one section together, sections in the order they first appear, rows in
  // their own order inside a section (a stable sort), so each section has one header as in the real list. The item of the
  // day stays first. The page selects and acts on these lists, so the selection moves in the order on screen.
  function groupLists(lists) {
    const grouped = {};
    for (const [column, rows] of Object.entries(lists)) {
      const order = new Map();
      // The item of the day has its own panel, so it does not decide where its section goes in the list.
      rows.filter(row => !(row.featured && column === 'shop')).concat(rows)
        .forEach(row => { const label = sectionLabel(row); if (!order.has(label)) order.set(label, order.size); });
      const rank = row => (row.featured && column === 'shop' ? -1 : order.get(sectionLabel(row)));
      grouped[column] = rows.map((row, i) => ({row, i})).sort((a, b) => (rank(a.row) - rank(b.row)) || (a.i - b.i)).map(x => x.row);
    }
    return grouped;
  }

  // The list a tab shows. The shop tab leaves the item of the day out: the movie shows it in its own panel.
  // Returns [{header} | {row, index}], index being the row's place in the page's list (the selection's index).
  function listEntries(rows, column) {
    const entries = [];
    let last = null;
    rows.forEach((row, index) => {
      if (column === 'shop' && row.featured) return;
      const label = sectionLabel(row);
      if (label !== last) entries.push({header:label});
      last = label;
      entries.push({row, index});
    });
    return entries;
  }

  // The first entry to draw so that the selected row and its section header show: keeps `top` when the row is already
  // inside the `fits` visible entries, otherwise scrolls the least.
  function scrollTop(entries, selectedIndex, top, fits) {
    const at = entries.findIndex(entry => entry.row && entry.index === selectedIndex);
    const maxTop = Math.max(0, entries.length - fits);
    let next = Math.min(Math.max(0, top), maxTop);
    if (at < 0) return next;
    const wanted = at > 0 && entries[at - 1].header ? at - 1 : at;
    if (wanted < next) next = wanted;
    if (at >= next + fits) next = at - fits + 1;
    return Math.min(Math.max(0, next), maxTop);
  }

  // What the item card shows for a row: the host card (guns) under the row's name and level, the price as the card's value.
  function cardItem(row) {
    if (!row) return null;
    const base = row.card ? {...row.card} : {};
    const ammoType = row.kind === 'ammo' && AMMO_TYPE[row.ammoType] ? {type:AMMO_TYPE[row.ammoType]} : {};
    return {...base, ...ammoType, name:row.name, rarity:row.rarity, level:row.level || undefined, levelKnown:Boolean(row.level),
      value:row.priceValid ? row.price : null, itemType:GEAR_TYPES.has(row.kind) ? row.kind : base.itemType};
  }

  // What an empty tab shows. The stock shows the list panel's own SOLD OUT widget (the shop's script turns it on for that
  // panel); the backpack and buy-back show one empty cell, which prints (EMPTY) as the item of the day's does. Null when
  // the tab has rows.
  function emptyView(column, entries) {
    if (entries.some(entry => entry.row)) return null;
    return column === 'shop' ? 'soldout' : 'cell';
  }

  function tooltip(column, featuredSelected) {
    const word = featuredSelected ? 'Purchase' : ACTION_WORD[column];
    return `[Enter] ${word}    [Left]/[Right] Tabs    [Escape] Cancel`;
  }

  // What the shop's feedback clip shows after a state push, or null. The clip has a frame for a purchase, a sale and a
  // refusal (its frame labels); each prints the label it is given. `pending` is the page's last
  // request {action, serial}; it counts as done when the money moved its way within the same opening (a restock alone moves
  // the serial, not the money). A message shows as a refusal whenever it is new.
  function feedbackFor(pending, previous, next, message, shownMessage) {
    if (message && message !== shownMessage) return {frame:'bad', label:message.toUpperCase()};
    if (!pending || !previous || !next || previous.opening !== next.opening || previous.machine.id !== next.machine.id) return null;
    if (next.serial <= pending.serial) return null;
    // The two result frames print the label they are given (an empty one shows nothing, seen in the bench); the words are ours.
    if ((pending.action === 'buy' || pending.action === 'buyback') && next.money < previous.money) return {frame:'purchased', label:'PURCHASED'};
    if (pending.action === 'sell' && next.money > previous.money) return {frame:'sold', label:'SOLD'};
    return null;
  }

  const api = {HEADER_FRAME, PANEL_BACKGROUND, TAB_SIDE, TAB_ORDER, TAB_TITLE, AMMO_CLIPS, sectionLabel, listEntries, scrollTop, cardItem,
    tooltip, cellIcon, groupLists, emptyView, LOAD_TIMEOUT_MS, feedbackFor};
  if (typeof module === 'object' && module.exports) { module.exports = api; return; }

  // ---- The movie ------------------------------------------------------------------------------------------------------
  const VM = '_level1.vendingMachine', TPI = '_level1.owTPI', MAIN = TPI + '.main', PANEL = MAIN + '.storagePanel';
  const ROWS = PANEL + '.owRows';
  // The type icons come from SharedWillowComponents, loaded into a clip of its own above the panels: a clip attaches only
  // symbols of its own movie's library, and the list panel's library does not hold the icon.
  const ICONS = '_level1.owIcons';
  const ICON_FILL = 0.7; // share of the cell height the icon fills (host choice)
  const STAGE_W = 1280, STAGE_H = 720;
  // Row geometry inside the list panel (panel units). The movie's own list clip gives the area; the cell is scaled to its
  // width. The header pitch is a host choice measured by eye in the bench.
  const HEADER_PITCH = 24, ROW_GAP = 2;
  // The card in the real shop frames sits left of the list, nearly beside it (its right edge about 15 px into the list, at
  // 1280x720); the movie's own position, without the game's 3D tilt, put it about 120 px over the list. Host fit by eye
  // from the Dr. Zed frame, UNVERIFIED.
  const CARD_SHIFT_X = -100;
  const POLL_MS = 100, POLL_TRIES = 150;

  let player = null, ready = false, lost = false, view = null, top = 0, serial = 0, drawn = [];
  let iconsReady = false, icons = [], guard = null;
  let writer = null, onSelect = null, controls = null;
  const call = (path, method, ...args) => player.ow(path, 'apply', method, args);
  const get = (path, member) => player.ow(path, 'get', member);
  const set = (path, member, value) => player.ow(path, 'set', member, value);
  const exists = path => { try { return get(path, '_name') !== undefined && get(path, '_name') !== null; } catch (e) { return false; } };
  const wait = ms => new Promise(resolve => root.setTimeout(resolve, ms));
  async function until(test) {
    for (let i = 0; i < POLL_TRIES; i++) { if (test()) return true; await wait(POLL_MS); }
    return false;
  }

  // Resolves true when both movies run; false when the page must keep its HTML panel (the player is then gone).
  // Gives the movie up for good: the guard's load is cancelled (a late answer does nothing, its watch is disconnected) and
  // the player removed. Returns false for start() to return.
  function giveUp() {
    lost = true;
    ready = false;
    if (guard && guard.cancel) guard.cancel();
    if (player) player.remove();
    return false;
  }

  // Resolves true when the movie runs, false when the page must use HTML. Any exception while setting the movie up (a
  // movie call that throws, say) is a failed start too: the movie is given up (guard cancelled, player removed) before the
  // false, so no player or panic watch is left behind the HTML page. `options.loadTimeoutMs` is for tests.
  async function start(stage, controlLayer, select, options = {}) {
    try {
      return await setUp(stage, controlLayer, select, options);
    } catch (error) {
      root.console.log(`OpenWillow movie setup failed: Vending (${error && error.message || error}); the HTML page runs`);
      return giveUp();
    }
  }

  async function setUp(stage, controlLayer, select, options) {
    controls = controlLayer;
    onSelect = select;
    player = root.RufflePlayer.newest().createPlayer();
    stage.appendChild(player);
    const timeout = options.loadTimeoutMs || LOAD_TIMEOUT_MS;
    guard = root.owMovieGuard.load(player, {url:'UI_VendingMachine/harness.swf', base:'UI_VendingMachine/'},
      'Vending', root.owMovieGuard.FILES.vending, () => { lost = true; ready = false; if (root.owVendingMovieLost) root.owVendingMovieLost(); });
    let timer = null;
    const timedOut = new Promise(resolve => { timer = root.setTimeout(() => resolve('timeout'), timeout); });
    const loading = await Promise.race([guard, timedOut]);
    root.clearTimeout(timer);
    if (loading === 'timeout') {
      giveUp();
      root.console.log(`OpenWillow movie timed out: Vending (UI_VendingMachine/harness.swf) after ${timeout} ms; the HTML page runs`);
      return false;
    }
    if (!loading) return false;
    if (!await until(() => exists(VM + '.iotd'))) return giveUp();
    // The shop movie places the two-panel interface it imports, but Ruffle does not resolve that library's own imports, so the
    // placed copy has no panels or cards. Loaded as a movie of its own it resolves them (seen in the bench): hide the placed
    // copy and load the library into a clip at the stage origin, where its panels sit as in the shop's layout.
    set('_level1.twoPanelInterface', '_visible', false);
    call('_level1', 'createEmptyMovieClip', 'owTPI', 5000);
    call(TPI, 'loadMovie', '../UI_TwoPanelInterface/TwoPanelInterface.swf');
    if (!await until(() => exists(PANEL + '.list') && exists(MAIN + '.mainCard.titleLine'))) return giveUp();
    call('_level1', 'createEmptyMovieClip', 'owIcons', 6000);
    call(ICONS, 'loadMovie', '../SharedWillowComponents/SharedWillowComponents.swf');
    iconsReady = await until(() => {
      call(ICONS, 'attachMovie', 'item card - weapon type', 'owProbe', 1);
      const loaded = exists(ICONS + '.owProbe.tintClip');
      if (loaded) call(ICONS + '.owProbe', 'removeMovieClip');
      return loaded;
    });
    writer = root.owItemCard.createCardWriter({call, get, set});
    ready = !lost;
    if (ready) draw();
    return ready;
  }

  function hideStockParts() {
    // The library's custom-background holder draws a placeholder square; the shop uses no custom background here.
    set(TPI + '.dynamicBackgroundClip', '_visible', false);
    set(MAIN + '.compareCard', '_visible', false);
    set(MAIN + '.playerPanel', '_visible', false);
    set(PANEL + '.list', '_visible', false);
    set(PANEL + '.pcSortButton', '_visible', false);
  }

  function drawHeader(state, column) {
    call(MAIN, 'SetHeader', HEADER_FRAME[state.machine.kind] || 'weapons', TAB_TITLE[column]);
    call(MAIN, 'SetNavHeaderIconCount', 3);
    call(MAIN, 'SetActiveTab', TAB_SIDE[column]);
    call(PANEL, 'SetBackground', PANEL_BACKGROUND[state.machine.kind] || 'weapons');
    call(PANEL, 'SetSortLabel', '');
  }

  function drawStatus(state) {
    const blackMarket = state.machine.kind === 'blackmarket';
    set(VM + '.credits', '_visible', !blackMarket);
    set(VM + '.eridium', '_visible', blackMarket);
    call(VM + '.credits', 'SetLanguageExt', 'INT');
    call(VM + '.credits', 'SetValue', Math.min(state.money, MAX_DISPLAY_CREDITS));
    if (blackMarket) call(VM + '.eridium', 'SetValue', state.eridium);
    // The ammo panel shows at the ammo machine, the health bars at the health machine (the shop's script does the same per
    // shop type).
    const ammo = state.machine.kind === 'items' && state.ammo;
    set(VM + '.ammo', '_visible', Boolean(ammo));
    for (const [key, clip] of Object.entries(AMMO_CLIPS)) {
      const pool = ammo ? state.ammo[key] : null;
      for (const [suffix, value] of [['', pool && pool.current], ['_max', pool && pool.max]]) {
        set(`${VM}.ammo.${clip}${suffix}`, '_visible', Boolean(pool));
        if (pool) set(`${VM}.ammo.${clip}${suffix}`, 'text', String(value));
      }
    }
    const health = state.machine.kind === 'health' && state.health;
    set(VM + '.healthBar', '_visible', Boolean(health));
    if (health) call(VM + '.healthBar', 'SetHealth', Math.floor(state.health.current), Math.floor(state.health.max));
    // The shield bar beside it, from the host's shield; hidden without a shield (no capacity) or without the field.
    const shield = state.machine.kind === 'health' && state.shield && state.shield.max > 0 ? state.shield : null;
    set(VM + '.shieldBar', '_visible', Boolean(shield));
    if (shield) call(VM + '.shieldBar', 'SetShield', Math.floor(shield.current), Math.floor(shield.max));
    call(MAIN, 'SetStorageInfoCardTitle', 'BACKPACK');
    call(MAIN, 'SetStorageInfoCardData', `${state.used}/${state.capacity}`);
  }

  function drawFeatured(state, lists, sel) {
    const featured = lists.shop[0] && lists.shop[0].featured ? lists.shop[0] : null;
    const iotd = VM + '.iotd';
    call(iotd, 'SetBackground', PANEL_BACKGROUND[state.machine.kind] || 'weapons');
    call(iotd, 'SetTimeRemaining', root.OwVendingModel.formatCountdown(state.resetSeconds));
    call(iotd, 'SetCellStatus', !featured, false);
    call(iotd + '.cell', 'SetEmptyCell', !featured);
    call(iotd + '.cell', 'SetSoldOut', false);
    if (featured) call(iotd + '.cell', 'SetRarityColor', root.owItemCard.itemColor(featured));
    const selected = Boolean(featured) && sel.column === 'shop' && sel.index === 0;
    call(iotd, 'SetSelected', selected);
    if (featured) drawIcon(iotd + '.cell.hitTestClip', featured);
    return selected ? featured : null;
  }

  // Cells and headers are attached under fresh names each time: a clip removed and attached again under the same name in
  // one frame keeps the old one's text (inventory.js found this).
  function clearRows() {
    for (const name of drawn) call(`${ROWS}.${name}`, 'removeMovieClip');
    drawn = [];
    for (const name of icons) call(`${ICONS}.${name}`, 'removeMovieClip');
    icons = [];
  }

  // A type icon centred on a cell (`cellPath`'s bounds in _level1 space), or nothing for a row without one.
  function drawIcon(cellPath, row) {
    const frame = cellIcon(row);
    if (!iconsReady || !frame) return;
    const box = call(cellPath, 'getBounds', '_level1');
    if (!box || !(box.xMax > box.xMin)) return;
    const name = `owIcon${++serial}`, path = `${ICONS}.${name}`;
    call(ICONS, 'attachMovie', 'item card - weapon type', name, 100 + serial % 10000);
    icons.push(name);
    call(path, 'SetFrameAndTint', frame, root.owItemCard.itemColor(row));
    const own = call(path, 'getBounds', path);
    if (!own || !(own.xMax > own.xMin)) return;
    const scale = Math.min((box.xMax - box.xMin) * 0.8 / (own.xMax - own.xMin), (box.yMax - box.yMin) * ICON_FILL / (own.yMax - own.yMin));
    set(path, '_xscale', scale * 100);
    set(path, '_yscale', scale * 100);
    set(path, '_x', (box.xMin + box.xMax) / 2 - (own.xMin + own.xMax) / 2 * scale);
    set(path, '_y', (box.yMin + box.yMax) / 2 - (own.yMin + own.yMax) / 2 * scale);
  }

  function attach(symbol) {
    const name = `ow${++serial}`;
    call(ROWS, 'attachMovie', symbol, name, 100 + serial % 10000);
    drawn.push(name);
    return `${ROWS}.${name}`;
  }

  function drawList(lists, sel, featuredSelected) {
    if (!exists(ROWS)) call(PANEL, 'createEmptyMovieClip', 'owRows', 2000);
    clearRows();
    if (controls) controls.replaceChildren();
    const area = {x:Number(get(PANEL + '.list', '_x')), y:Number(get(PANEL + '.list', '_y')),
      w:Number(get(PANEL + '.list', '_width')), h:Number(get(PANEL + '.list', '_height'))};
    set(ROWS, '_x', area.x);
    set(ROWS, '_y', area.y);
    const entries = listEntries(lists[sel.column] || [], sel.column);
    const empty = emptyView(sel.column, entries);
    call(PANEL, 'SetEmptyPanelWidget', empty === 'soldout', empty === 'soldout');
    // Measure one cell for the scale and pitch.
    const probe = attach('inventory - cell');
    const cell = call(probe, 'getBounds', probe);
    call(probe, 'removeMovieClip');
    drawn.pop();
    if (!cell || !(cell.xMax > cell.xMin)) return;
    const scale = area.w / (cell.xMax - cell.xMin), pitch = (cell.yMax - cell.yMin) * scale + ROW_GAP;
    const fits = Math.max(1, Math.floor((area.h + ROW_GAP) / pitch));
    top = scrollTop(entries, featuredSelected ? -1 : sel.index, top, fits);
    if (empty === 'cell') {
      const path = attach('inventory - cell');
      set(path, '_xscale', scale * 100);
      set(path, '_yscale', scale * 100);
      set(path, '_x', -cell.xMin * scale);
      set(path, '_y', -cell.yMin * scale);
      call(path, 'SetSoldOut', false);
      call(path, 'SetEmptyCell', true);
      call(path, 'SetSelected', false);
    }
    let y = 0, k = top;
    for (; k < entries.length && y + (entries[k].header ? HEADER_PITCH : pitch) <= area.h + ROW_GAP; k++) {
      const entry = entries[k];
      if (entry.header) {
        const header = attach('inventory - storage panel - header');
        set(header + '.textField', 'text', entry.header);
        const b = call(header, 'getBounds', header);
        if (b) { set(header, '_x', (area.w - (b.xMax - b.xMin)) / 2 - b.xMin); set(header, '_y', y - b.yMin); }
        y += HEADER_PITCH;
        continue;
      }
      const path = attach('inventory - cell');
      set(path, '_xscale', scale * 100);
      set(path, '_yscale', scale * 100);
      set(path, '_x', -cell.xMin * scale);
      set(path, '_y', y - cell.yMin * scale);
      call(path, 'SetSoldOut', false);
      call(path, 'SetEmptyCell', false);
      call(path, 'SetRarityColor', root.owItemCard.itemColor(entry.row));
      call(path, 'SetSelected', !featuredSelected && entry.index === sel.index);
      hitBox(path, entry, sel.column);
      drawIcon(`${path}.hitTestClip`, entry.row);
      y += pitch;
    }
    set(PANEL + '.moreUp', '_visible', top > 0);
    set(PANEL + '.moreDown', '_visible', k < entries.length);
  }

  // A transparent button over a cell for the mouse; stage pixels to percentages of the player box (as inventory.js does).
  function hitBox(path, entry, column) {
    if (!controls || !onSelect) return;
    const b = call(path, 'getBounds', '_level0');
    if (!b || !(b.xMax > b.xMin)) return;
    const button = root.document.createElement('button');
    button.type = 'button';
    button.className = 'movie-cell';
    button.setAttribute('aria-label', entry.row.name);
    Object.assign(button.style, {left:`${b.xMin / STAGE_W * 100}%`, top:`${b.yMin / STAGE_H * 100}%`,
      width:`${(b.xMax - b.xMin) / STAGE_W * 100}%`, height:`${(b.yMax - b.yMin) / STAGE_H * 100}%`});
    button.addEventListener('click', () => onSelect({column, index:entry.index}, false));
    button.addEventListener('dblclick', () => onSelect({column, index:entry.index}, true));
    controls.appendChild(button);
  }

  let cardHomeX = null;
  function drawCard(row) {
    const card = MAIN + '.mainCard';
    if (cardHomeX === null) cardHomeX = Number(get(card, '_x'));
    set(card, '_x', cardHomeX + CARD_SHIFT_X);
    writer.configureCard(card, cardItem(row), null, 'highlight', false, Infinity);
    // Rows without a gun type (health, and grenade ammo) have no card icon; item_card.js would draw the pistol one.
    const typed = row && (row.kind === 'weapon' || GEAR_TYPES.has(row.kind) || (row.kind === 'ammo' && AMMO_TYPE[row.ammoType]));
    if (row) set(card + '.typeIcon', '_visible', Boolean(typed));
  }

  function draw() {
    if (!ready || !view) return;
    const {state, lists, sel} = view;
    hideStockParts();
    drawHeader(state, sel.column);
    drawStatus(state);
    const featuredSelected = Boolean(lists.shop[0] && lists.shop[0].featured) && sel.column === 'shop' && sel.index === 0;
    drawList(lists, sel, featuredSelected);
    const featured = drawFeatured(state, lists, sel);
    drawCard(featured || (lists[sel.column] || [])[sel.index] || null);
    call(MAIN, 'SetTooltipText', tooltip(sel.column, Boolean(featured)));
  }

  // Plays the feedback clip (Feedback.Trigger(frame, label) in the shop movie).
  function feedback(shown) {
    if (ready && shown) call(VM + '.feedback', 'Trigger', shown.frame, shown.label);
  }

  function render(next) {
    if (next.state && view && next.state.opening !== view.state.opening) top = 0;
    view = next;
    draw();
  }

  root.owVendingMovie = {...api, start, render, feedback, isReady:() => ready};
})(typeof window !== 'undefined' ? window : globalThis);
