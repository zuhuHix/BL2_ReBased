// Run the installed StatusMenu movie and adapt its input to the host-owned
// inventory. The movie draws the frame art; HTML only supplies hit targets
// where Ruffle cannot provide the original Scaleform data-provider callbacks.
const startupAt = performance.now();
window.owInventoryStartedAt = startupAt;
window.owInventoryMovieReady = false;
window.owInventoryReady = false;
const player = window.RufflePlayer.newest().createPlayer();
document.getElementById('presentation').prepend(player);
window.owPlayer = player;

const ROOT = '_level1', INV = ROOT + '.inventory';
const call = (path, method, ...args) => player.ow(path, 'apply', method, args);
const get = (path, member) => player.ow(path, 'get', member);
const set = (path, member, value) => player.ow(path, 'set', member, value);
const colors = [0xffffff, 0xffffff, 0x39ff14, 0x3c8dff, 0xb43cff, 0xffb400];
// Fully visible backpack rows. The real game shows about seven rows (the reference captures show seven
// and the top of an eighth); the converted cell is drawn at ROW_SCALE so seven fit above the panel's
// scroll chevron. Both numbers are host choices measured in the bench, not movie values.
const VISIBLE_ROWS = 7, ROW_SCALE = 0.94, ROW_PITCH = 61;
const RENDERED_ROWS = VISIBLE_ROWS + 1, PEEK_HEIGHT = 16;
// Reference composition, measured from the local stock inventory stills in
// 1280x720 space. The runtime does not project the movie's GFx panel depth;
// the host places/sizes Backpack beside Equipped. These are presentation
// choices, not decoded movie values or proof of original 3D projection parity.
const COMPOSITION_SCALE = 1.09;
const PANEL_SCALE = 0.62, PANEL_SCALE_Y = 0.70, PANEL_LEFT = 754, PANEL_TOP = 135;
// This list is a host-side convenience. The original movie's full sort cycle
// was not exercised in the local game trace, so these modes are not parity claims.
const sortModes = [
  {key:'default', label:'DEFAULT'},
  {key:'name', label:'NAME'},
  {key:'rarity', label:'RARITY'},
  {key:'level', label:'LEVEL'},
  {key:'damage', label:'DAMAGE'}
];
// Recorded stock card positions/scales. The stock comparison capture confirms
// that full-size cards intentionally overlay the upper equipment panels.
// All positions are movie-local; Ruffle still lacks the original 3D projection.
const compareTween = [0.2,-355,-150,500,-30,-145,500,-30,-145,500,-355,-150,500,75,81,81,75];
const comparePanelTweens = [
  ['Equipped',0.2,53,-25,0,-176,-25,-10000,-30,-25,-2650],
  ['Backpack',0.2,64,-45,-300,385,-70,-4950,390,-43,-2650]
];
// Backpack filter (the original's "(ALL)" tag). Labels are the host's wording, not movie strings:
// the converted movie only carries a placeholder header ("Assault Rifles").
const gearSlots = [
  {key:'shield', itemType:'shield', label:'Shield'},
  {key:'grenadeMod', itemType:'grenade_mod', label:'Grenade Mod'},
  {key:'classMod', itemType:'class_mod', label:'Class Mod'},
  {key:'relic', itemType:'relic', label:'Relic'}
];
const categories = [
  {key:'all', label:'ALL', match:null},
  {key:'weapons', label:'WEAPONS', match:item => !gearSlotForItem(item)},
  ...gearSlots.map(slot => ({key:slot.key, label:`${slot.label.toUpperCase()}S`, match:item => item.itemType === slot.itemType}))
];
// `rounding` is the stat's presentation rounding (docs/verification/NATIVE_WEAPON_RULES.md section 2,
// read from native code and data; tools/weapon_stats.py present() is the reference): damage up, magazine
// down, the rest half up to one decimal. The golden cards print accuracy with one decimal and no '%'.
const weaponCardStats = [
  {key:'damage', label:'Damage', decimals:0, rounding:'ceil', higherIsBetter:true, icon:'weaponDamage'},
  // Optional host field (the presentation remap of the evaluated spread).
  {key:'accuracy', label:'Accuracy', decimals:1, rounding:'half', higherIsBetter:true, icon:'weaponAccuracy'},
  {key:'fireRate', label:'Fire Rate', decimals:1, rounding:'half', higherIsBetter:true, icon:'weaponFireRate'},
  {key:'reloadTime', label:'Reload Speed', decimals:1, rounding:'half', higherIsBetter:false, icon:'weaponsReloadSpeed'},
  {key:'magazine', label:'Magazine Size', decimals:0, rounding:'floor', higherIsBetter:true, icon:'weaponClipSize'}
];
// A stat as the card prints it, on the stored single-precision value (as present() in tools/weapon_stats.py):
// sizes under 1e-8 print as 0; 'ceil' and 'floor' to an integer; 'half' half up to `decimals`.
function cardRound(value, rounding, decimals) {
  let stored = Math.fround(value);
  if (Math.abs(stored) < 1e-8) stored = 0;
  if (rounding === 'ceil') return Math.ceil(stored);
  if (rounding === 'floor') return Math.floor(stored);
  // Half up with every step in single precision, as the game's rounding does (weapon_stats.half_up).
  const scale = Math.fround(10 ** decimals);
  return Math.floor(Math.fround(Math.fround(stored * scale) + 0.5)) / scale;
}
const statIcons = new Map([
  ['capacity','shieldCapacity'], ['rechargerate','shieldRechargeRate'],
  ['rechargedelay','shieldRechargeDelay'], ['ampdamage','weaponDamage'],
  ['ampshotdrain','impactshield']
]);
const weaponTypeIcons = new Map([
  ['pistol','Pistol'], ['smg','SMG'], ['shotgun','Shotgun'],
  ['assault rifle','ar'], ['ar','ar'], ['sniper rifle','Sniper'],
  ['sniper','Sniper'], ['rocket launcher','Rocket'], ['rocket','Rocket'], ['launcher','Rocket']
]);
// Frame names probed in the bench (setting the card's type icon frame and looking at it): Pistol, SMG,
// Shotgun, ar, Sniper and Rocket draw an icon; 'Sniper Rifle' and 'Rocket Launcher' draw nothing.
// The movie's ammo panel (INV.ammo) holds one "<clip>" (current) and "<clip>_max"
// text field per ammo type plus a `highlight` clip whose frame labels are the
// `frame` values below (frame labels read from the converted SharedWillowInventory
// "ammo box" sprite; "pistol" ammo lives in the clip named "repeater"). Host
// snapshot keys are the `key` values.
const ammoTypes = [
  {key:'pistol', clip:'ammo_repeater', frame:'pistol'}, {key:'smg', clip:'ammo_smg', frame:'smg'},
  {key:'ar', clip:'ammo_ar', frame:'ar'}, {key:'shotgun', clip:'ammo_shotgun', frame:'shotgun'},
  {key:'sniper', clip:'ammo_sniper', frame:'sniper'}, {key:'launcher', clip:'ammo_rocket', frame:'rocket'},
  {key:'grenade', clip:'ammo_grenade', frame:'grenade'}
];
const weaponAmmoKeys = new Map([
  ['pistol','pistol'], ['smg','smg'], ['shotgun','shotgun'], ['assault rifle','ar'], ['ar','ar'],
  ['sniper rifle','sniper'], ['sniper','sniper'], ['rocket launcher','launcher'], ['rocket','launcher'],
  ['launcher','launcher']
]);
// The credits clip has eight digit clips (digit1..digit8); the original clamps what
// it displays (GetCappedDisplayCurrencyAmount in the game trace; the exact cap is UNVERIFIED).
const MAX_DISPLAY_CREDITS = 99999999;
// Card-local pixels of the flavour block the movie's own layout already leaves room for
// (found empirically in the bench, so UNVERIFIED against the original).
const FUN_STATS_OVERLAP = 6;
const READY_SETTLE_MS = 300, LAYOUT_SETTLE_MS = 250, LAYOUT_POLL_MS = 100;
let ready = false, state = null, selectedId = null, targetSlot = 0, targetGearSlot = null, firstRow = 0;
let renderedLayout = '', pendingLayout = '', pendingSince = 0, renderedCard = '';
let sortIndex = 0, categoryIndex = 0, compareId = null, compareLayoutActive = false, lastState = '';
let transferSourceId = null;
let navigationPanel = 'equipped', lastBackpackId = null, lastEquippedIndex = 0;
let transferFromEquipped = false;
let transferCategoryBefore = 0;
let compareStartedFromLeft = false;
let lastMenuPreviewId = null;
// Open-time instrumentation. The host calls owOpenTiming(<Unix ms of the open request>) on every
// open; "OWINVTIME js_<event> sinceOpen=<ms>" lines reach the UE log through the console bridge.
// js_painted is logged two animation frames after the page is ready, rendered and open, i.e. once
// the browser has composited a frame of the open inventory.
let openTiming = null;
function timeLog(event, extra = '') {
  const since = openTiming ? Date.now() - openTiming.hostEpoch : -1;
  console.log(`OWINVTIME js_${event} sinceOpen=${since} pageMs=${Math.round(performance.now() - startupAt)} epoch=${Date.now()} ready=${ready} ${extra}`);
}
function schedulePaintedLog() {
  if (!openTiming || openTiming.paintScheduled || !ready || !state || !firstStateRendered) return;
  openTiming.paintScheduled = true;
  const timing = openTiming;
  requestAnimationFrame(() => requestAnimationFrame(() => {
    if (timing === openTiming) timeLog('painted', `size=${innerWidth}x${innerHeight}`);
  }));
}
window.owOpenTiming = hostEpoch => {
  openTiming = {hostEpoch, paintScheduled:false};
  timeLog('open', `size=${innerWidth}x${innerHeight} visibility=${document.visibilityState} hasState=${!!state}`);
  schedulePaintedLog();
};
document.addEventListener('visibilitychange', () => timeLog('visibility', document.visibilityState));
timeLog('page_start');
window.owRefreshMenuPreview = () => {
  lastMenuPreviewId = null;
  inspectMode = false;
  inspectPointer = null;
  if (transferSourceId) finishTransfer(true);
  if (ready && state) drawCard();
};
let headerPending = false, headerSerial = 0, headerName = '';
let inspectMode = false;
let inspectItemId = null, inspectYaw = 0, inspectPitch = 0, inspectImage = '', inspectFrameCount = 0;
let inspectResolved = false;
let inspectPointer = null, inspectLastRequest = 0;
let firstStateRendered = false;
const escapeHtml = text => String(text).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
function itemColor(item) {
  const raw = item?.rarityColor;
  if (typeof raw === 'string' && /^#[\da-f]{6}$/i.test(raw)) return Number.parseInt(raw.slice(1), 16);
  if (Number.isInteger(item?.rarity) && item.rarity >= 0 && item.rarity < colors.length)
    return item.rarityKnown === false ? colors[0] : colors[item.rarity];
  return colors[0];
}
const color = itemColor;
const itemById = id => state?.items.find(item => item.id === id);
// url -> 'ok' | 'missing', so a rebuilt overlay does not re-request (or flash a blank cell for)
// a preview the server has already said it does not have.
const previewStatus = new Map();
function previewUrls(item) {
  // A present-but-empty assetId means the host has no verified mesh preview.
  if (Object.prototype.hasOwnProperty.call(item || {}, 'assetId') && !item.assetId) return [];
  return [...new Set([item?.assetId, item?.id].filter(Boolean))]
    .map(asset => `/previews/${encodeURIComponent(asset)}.png`);
}
// Card type-icon frames were probed in the bench: 'Shield', 'Grenade' and 'Artifact' (the relic)
// draw an icon; no frame named like 'Class Mod' does, so class mods show none (frame name
// unknown, UNVERIFIED).
function movieTypeIcon(item) {
  if (item?.itemType === 'shield') return 'Shield';
  if (item?.itemType === 'grenade_mod') return 'Grenade';
  if (item?.itemType === 'class_mod') return 'Class Mod';
  if (item?.itemType === 'relic') return 'Artifact';
  const type = String(item?.type || 'pistol').trim().toLowerCase();
  return weaponTypeIcons.get(type) || 'Pistol';
}
const gearSlotForItem = item => gearSlots.find(slot => slot.itemType === item?.itemType)?.key || null;
const itemCategoryLabel = item => gearSlotForItem(item)
  ? gearSlots.find(slot => slot.key === gearSlotForItem(item)).label.toUpperCase() + 'S' : 'WEAPONS';

// UNVERIFIED PLACEHOLDER ART: the game's real gear icons are 3D renders that this host does
// not produce yet, and a weapon whose mesh was not exported has no thumbnail either. Both get a
// generic silhouette per type, tinted with the item's rarity colour, instead of a blank cell.
// These are original shapes, not game art.
const gearIconShapes = {
  shield:{box:'0 0 64 48', svg:'<path d="M32 4 54 12v14c0 12-10 18-22 20C20 44 10 38 10 26V12z"/><path class="line" d="M32 9 19 15v11c0 8 5 13 13 15M32 9v32"/>'},
  grenade_mod:{box:'0 0 64 48', svg:'<circle cx="32" cy="29" r="15"/><path d="M28 14h8v5h-8z"/><path class="line" d="M36 10c6-2 10 0 12 4M22 24c2-4 6-6 10-6"/>'},
  class_mod:{box:'0 0 64 48', svg:'<rect x="15" y="10" width="34" height="28" rx="4"/><path class="line" d="M15 18H8m7 8H8m7 8H8m41-16h7m-7 8h7m-7 8h7M24 20h16v12H24z"/>'},
  relic:{box:'0 0 64 48', svg:'<path d="M32 3 52 19 32 45 12 19z"/><path class="line" d="M12 19h40M32 3l-8 16 8 26 8-26z"/>'}
};
const weaponIconShapes = {
  pistol:'<path d="M12 10h56v12H50l-4 5-3 11H32l4-16H12z"/><path class="line" d="M68 13H80M22 14h34M43 22h7"/>',
  smg:'<path d="M14 11h52v11H52l-3 4 2 12H41l-3-16H14z"/><path d="M66 12h10v8H66zM76 13h8v6h-8z"/><path class="line" d="M20 14h40M8 16h6"/>',
  ar:'<path d="M12 12h56l6 4 16 2v9H72l-5 4H60v9h-9l-3-9H12z"/><path d="M40 6h16v6H40z"/><path class="line" d="M12 15h48M4 15h8M62 18h10"/>',
  shotgun:'<path d="M6 14h54l8 3 18 2v8H70l-8 4H60l-8 3H6z"/><path class="line" d="M6 17h50M18 26h30"/>',
  sniper:'<path d="M4 15h60l8 2 20 3v7H70l-6 3H52v6H42l-2-6H4z"/><path d="M30 6h26v7H30z"/><path class="line" d="M4 18h56M33 9h20"/>',
  launcher:'<path d="M10 9h50v16H10z"/><path d="M60 12h14v10H60zM46 25h10l2 12H44z"/><path class="line" d="M10 14h50M10 20h50M74 16h10"/>'
};
function placeholderShape(item) {
  const gear = gearIconShapes[item?.itemType];
  if (gear) return gear;
  const key = ammoKeyForItem(item) || 'pistol';
  return {box:'0 0 96 44', svg:weaponIconShapes[key] || weaponIconShapes.pistol};
}
function gearPlaceholder(item) {
  const tint = '#' + itemColor(item).toString(16).padStart(6, '0');
  const shape = placeholderShape(item);
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('viewBox', shape.box);
  svg.setAttribute('class', 'gear-icon');
  svg.setAttribute('aria-hidden', 'true');
  svg.style.setProperty('--tint', tint);
  svg.innerHTML = shape.svg;
  return svg;
}
const equippedIdFor = item => {
  const gearSlot = gearSlotForItem(item);
  return gearSlot ? state?.gearSlots?.[gearSlot] || null : state?.slots?.[targetSlot] || null;
};
const equippedItems = () => [
  ...(state?.slots || []),
  ...gearSlots.map(slot => state?.gearSlots?.[slot.key] || null)
];
const equippedIds = () => new Set(equippedItems().filter(Boolean));
const selectedSlotIndex = () => {
  const item = itemById(selectedId);
  const gearSlot = gearSlotForItem(item);
  return gearSlot ? 4 + gearSlots.findIndex(slot => slot.key === gearSlot)
    : targetGearSlot ? 4 + gearSlots.findIndex(slot => slot.key === targetGearSlot) : targetSlot;
};

// Optional host field: how many of the four weapon slots are open (a new Maya has 2).
// Absent or invalid means all four, so older hosts keep working.
function slotsUnlocked() {
  const raw = state?.slotsUnlocked;
  if (raw === undefined || raw === null || raw === '' || !Number.isFinite(Number(raw))) return 4;
  return Math.max(1, Math.min(4, Math.floor(Number(raw))));
}
const slotLocked = index => index >= slotsUnlocked() && index < 4;
function firstOpenSlot(preferred) {
  const open = slotsUnlocked();
  return Number.isInteger(preferred) && preferred >= 0 && preferred < open ? preferred : 0;
}
// Optional host fields money/eridium/ammo: only finite non-negative numbers are shown.
function displayCount(value) {
  if (typeof value !== 'number' && typeof value !== 'string') return null;
  if (value === '' || !Number.isFinite(Number(value))) return null;
  return Math.max(0, Math.floor(Number(value)));
}
function ammoEntry(key) {
  const entry = state?.ammo?.[key];
  const current = displayCount(entry?.current), max = displayCount(entry?.max);
  return current === null || max === null ? null : {current, max};
}
const hasAmmoField = () => Boolean(state?.ammo) && typeof state.ammo === 'object';
function ammoKeyForItem(item) {
  if (!item || gearSlotForItem(item)) return null;
  return weaponAmmoKeys.get(String(item.type || 'pistol').trim().toLowerCase()) || null;
}

window.owInventory = snapshot => {
  if (!snapshot || !Array.isArray(snapshot.items) || !Array.isArray(snapshot.slots)
      || snapshot.slots.length !== 4) return;
  const firstSnapshot = state === null;
  const serial = JSON.stringify(snapshot);
  if (serial === lastState) return;
  lastState = serial;
  state = snapshot;
  if (transferSourceId && !itemById(transferSourceId)) finishTransfer();
  if (firstSnapshot && Number.isInteger(state.activeSlot) && state.activeSlot >= 0 && state.activeSlot < 4)
    targetSlot = state.activeSlot;
  if (!Number.isInteger(targetSlot) || targetSlot < 0 || targetSlot >= slotsUnlocked())
    targetSlot = firstOpenSlot(state.activeSlot);
  if (!transferSourceId && navigationPanel === 'backpack' && equippedIds().has(selectedId)) {
    navigationPanel = 'equipped';
    targetGearSlot = gearSlotForItem(itemById(selectedId));
    if (!targetGearSlot) targetSlot = state.slots.indexOf(selectedId);
  }
  if (!firstSnapshot && !transferSourceId && navigationPanel === 'equipped') {
    const id = targetGearSlot ? state.gearSlots?.[targetGearSlot] : state.slots[targetSlot];
    selectedId = itemById(id)?.id || null;
  }
  if (!itemById(selectedId)) {
    const heldId = state.slots[targetSlot];
    // An intentionally selected empty equipment cell must stay empty when
    // unrelated ammo/currency snapshots arrive.
    if (firstSnapshot || selectedId !== null || navigationPanel !== 'equipped') {
      selectedId = transferSourceId ? backpackItems()[0]?.id || null : itemById(heldId)?.id || backpackItems()[0]?.id || null;
      navigationPanel = equippedIds().has(selectedId) || !selectedId ? 'equipped' : 'backpack';
    }
  }
  if (!itemById(compareId) || compareId === selectedId) compareId = null;
  if (ready) render();
};

function text(path, value, size = 15, tint = 0xffffff, align = '') {
  const body = `<font face="$WillowBody" size="${size}" color="#${tint.toString(16).padStart(6,'0')}">${escapeHtml(value)}</font>`;
  set(path, 'htmlText', align ? `<p align="${align}">${body}</p>` : body);
}

function backpackItems() {
  const equipped = equippedIds();
  const category = categories[categoryIndex];
  const items = state?.items.filter(item => !equipped.has(item.id) && (!category.match || category.match(item))) || [];
  const mode = sortModes[sortIndex];
  if (mode.key === 'default') return items;
  const compareText = (left, right) => String(left ?? '').localeCompare(String(right ?? ''), undefined, {numeric:true, sensitivity:'base'});
  return items.map((item, order) => ({item, order})).sort((a, b) => {
    let order = 0;
    if (mode.key === 'name') order = compareText(a.item.name, b.item.name);
    else if (mode.key === 'rarity') order = Number(b.item.rarity || 0) - Number(a.item.rarity || 0);
    else if (mode.key === 'level') order = Number(b.item.level || 0) - Number(a.item.level || 0);
    else if (mode.key === 'damage') order = Number(b.item.damage || 0) - Number(a.item.damage || 0);
    return order || a.order - b.order;
  }).map(entry => entry.item);
}

function requestAction(action, fields = {}) {
  console.log('OWITEM ' + JSON.stringify({action, ...fields}));
}

// Native HTML drag events carry only the stable inventory ID. Every drop is
// checked against the latest snapshot, then goes through the host's existing
// equip/unequip validation; the page never moves an item optimistically.
let draggedItemId = null;
const dragLayer = document.getElementById('controls');
dragLayer.addEventListener('dragstart', event => {
  const source = event.target.closest('[data-item-id]');
  const item = source && itemById(source.dataset.itemId);
  if (!item || !event.dataTransfer) { event.preventDefault(); return; }
  draggedItemId = item.id;
  event.dataTransfer.setData('text/plain', item.id);
  event.dataTransfer.effectAllowed = 'move';
});
dragLayer.addEventListener('dragend', () => { draggedItemId = null; });
function inventoryDropRequest(id, target) {
  const item = itemById(id);
  if (!item || !target) return null;
  const kind = target.dataset.kind;
  if (kind === 'backpack' || kind === 'backpack-zone') {
    const slot = (state.slots || []).indexOf(id);
    if (slot >= 0) return {action:'unequip', slot};
    const gearSlot = gearSlotForItem(item);
    return gearSlot && state.gearSlots?.[gearSlot] === id ? {action:'unequip', gearSlot} : null;
  }
  if (kind !== 'slot') return null;
  const slot = Number(target.dataset.slot);
  if (!Number.isInteger(slot) || slot < 0 || slot >= 8 || slotLocked(slot)) return null;
  const levelAllowed = item.levelKnown === false || item.level === undefined || Number(item.level) <= Number(state.level);
  if (!levelAllowed) return null;
  const gearSlot = gearSlotForItem(item);
  if (slot < 4)
    return !gearSlot && state.slots[slot] !== id ? {action:'equip', id, slot} : null;
  const destination = gearSlots[slot-4]?.key;
  return gearSlot === destination && state.gearSlots?.[destination] !== id
    ? {action:'equip', id, gearSlot:destination} : null;
}
dragLayer.addEventListener('dragover', event => {
  if (!inventoryDropRequest(draggedItemId, event.target.closest('[data-kind]'))) return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
});
dragLayer.addEventListener('drop', event => {
  const id = event.dataTransfer?.getData('text/plain');
  const request = inventoryDropRequest(id, event.target.closest('[data-kind]'));
  draggedItemId = null;
  if (!request) return;
  event.preventDefault();
  const {action, ...fields} = request;
  requestAction(action, fields);
});

function select(id) {
  if (!itemById(id) || selectedId === id) return;
  if (transferSourceId && !transferFromEquipped) {
    const slot = state.slots.indexOf(id);
    if (!gearSlotForItem(itemById(transferSourceId)) && slot >= 0) setTargetSlot(slot);
    return;
  }
  if (transferSourceId && equippedIds().has(id)) return;
  if (selectedId !== id) compareId = transferSourceId;
  selectedId = id;
  navigationPanel = equippedIds().has(id) ? 'equipped' : 'backpack';
  if (navigationPanel === 'backpack') lastBackpackId = id;
  else if (!gearSlotForItem(itemById(id))) lastEquippedIndex = targetSlot = state.slots.indexOf(id);
  else lastEquippedIndex = 4 + gearSlots.findIndex(slot => slot.key === gearSlotForItem(itemById(id)));
  targetGearSlot = gearSlotForItem(itemById(id));
  syncSelection();
  drawCard();
}

function equip() {
  const item = itemById(selectedId);
  if (item && equippedIds().has(item.id) && !transferSourceId) { beginEquippedTransfer(); return; }
  const levelAllowed = item?.levelKnown === false || item?.level === undefined
    || Number(item.level) <= Number(state.level);
  if (!item || !levelAllowed) return;
  if (!transferSourceId) { beginBackpackTransfer(); return; }
  const gearSlot = gearSlotForItem(item);
  if (gearSlot) {
    if (state.gearSlots?.[gearSlot] !== item.id) { requestAction('equip', {id:item.id, gearSlot}); finishTransfer(); }
  } else if (slotLocked(targetSlot)) {
    return;
  } else if (state.slots[targetSlot] !== item.id) {
    requestAction('equip', {id:item.id, slot:targetSlot});
    finishTransfer();
  }
}

function unequip() {
  const item = itemById(selectedId);
  if (targetGearSlot) {
    if (state?.gearSlots?.[targetGearSlot]) requestAction('unequip', {gearSlot:targetGearSlot});
    return;
  }
  const gearSlot = gearSlotForItem(item);
  if (gearSlot && state?.gearSlots?.[gearSlot]) requestAction('unequip', {gearSlot});
  else if (state?.slots[targetSlot]) requestAction('unequip', {slot:targetSlot});
}

function dropSelected() {
  if (transferSourceId) return;
  if (itemById(selectedId)) requestAction('drop', {id:selectedId});
}

function toggleMark(action) {
  if (itemById(selectedId)) requestAction(action, {id:selectedId});
}

function toggleCompare() {
  if (transferSourceId) { equip(); return; }
  if (!transferSourceId && equippedIds().has(selectedId)) { beginEquippedTransfer(); return; }
  const equippedId = equippedIdFor(itemById(selectedId));
  if (!equippedId || equippedId === selectedId || !itemById(selectedId)) {
    compareId = null;
  } else {
    compareId = compareId === equippedId ? null : equippedId;
  }
  drawCard();
}

function beginBackpackTransfer() {
  const source = itemById(selectedId);
  if (!source || equippedIds().has(source.id)) return;
  transferSourceId = source.id;
  transferFromEquipped = false;
  navigationPanel = 'equipped';
  transferCategoryBefore = categoryIndex;
  targetGearSlot = gearSlotForItem(source);
  compareId = equippedIdFor(source);
  render();
}

function beginEquippedTransfer() {
  const source = itemById(selectedId);
  if (!source || !equippedIds().has(source.id)) return;
  const gearSlot = gearSlotForItem(source);
  const candidates = (state.items || []).filter(item => !equippedIds().has(item.id)
    && gearSlotForItem(item) === gearSlot);
  if (!candidates.length) { announce('No compatible backpack items'); return; }
  if (!gearSlot) targetSlot = state.slots.indexOf(source.id);
  targetGearSlot = gearSlot;
  transferSourceId = source.id;
  transferFromEquipped = true;
  transferCategoryBefore = categoryIndex;
  compareId = source.id;
  categoryIndex = categories.findIndex(category => category.key === (gearSlot || 'weapons'));
  selectedId = candidates[0].id;
  navigationPanel = 'backpack';
  lastBackpackId = selectedId;
  firstRow = scrollForSelected(backpackItems());
  render();
}
function finishTransfer(cancel = false) {
  if (!transferSourceId) return;
  if (cancel && itemById(transferSourceId)) selectedId = transferSourceId;
  transferSourceId = compareId = null;
  transferFromEquipped = false;
  navigationPanel = equippedIds().has(selectedId) || !selectedId ? 'equipped' : 'backpack';
  categoryIndex = transferCategoryBefore;
  firstRow = scrollForSelected(backpackItems());
  if (ready) render();
}

function closeInventory() { location.href = '/__ow_close_inventory'; }

// The StatusMenu header's five tabs, left to right (nav1..nav5 in the converted movie). Only the two
// pages this host has (inventory, skills) are reachable; Missions, Map and Challenges have no host
// data yet, so their hit boxes are disabled rather than pretending. The route is intercepted by the UE
// browser (OnBeforeNavigation) like the close route.
const headerTabs = [
  {nav:'nav1', label:'Missions'}, {nav:'nav2', label:'Map'},
  {nav:'nav3', label:'Inventory', current:true},
  {nav:'nav4', label:'Skills', route:'/__ow_tab_skills'},
  {nav:'nav5', label:'Challenges'}
];
function switchTab(tab) {
  if (tab.route) location.href = tab.route;
}
function addHeaderTabs() {
  for (const tab of headerTabs) {
    const bounds = readBounds(`${ROOT}.header.${tab.nav}`);
    if (!bounds) continue;
    const usable = Boolean(tab.route);
    const label = tab.current ? `${tab.label} (current tab)` : usable ? `${tab.label} tab` : `${tab.label} tab, not available yet`;
    const button = overlayButton(bounds, label, () => switchTab(tab), 'tab');
    if (!usable) button.classList.add(tab.current ? 'tab-current' : 'tab-unavailable');
    if (!usable && !tab.current) button.disabled = true;
    if (usable) {
      button.addEventListener('pointerenter', () => player.ow(`${ROOT}.header.${tab.nav}`, 'gotoAndStop', 'over'));
      button.addEventListener('pointerleave', () => player.ow(`${ROOT}.header.${tab.nav}`, 'gotoAndStop', 'up'));
    }
  }
}

// areaPath lets a caller lay the box over a child clip instead of the whole clip: the
// equipped cells' own bounds include animated glow/number art that jitters by ~10px.
// The host keeps sending pointer events while the cursor rests (UE re-sends the last mouse position;
// 499 identical events were logged over one equipped cell in a single open), so hover-select only on
// a real change of position. The first event over a cell only records where the cursor is.
let lastPointerX = null, lastPointerY = null;
function pointerMoved(event) {
  const moved = lastPointerX !== null && (event.clientX !== lastPointerX || event.clientY !== lastPointerY);
  lastPointerX = event.clientX; lastPointerY = event.clientY;
  return moved;
}
function hit(path, label, click, hover, tint, item = null, kind = 'item', areaPath = path) {
  const bounds = call(areaPath, 'getBounds', ROOT);
  if (!bounds || ![bounds.xMin,bounds.yMin,bounds.xMax,bounds.yMax].every(Number.isFinite)) return null;
  const button = document.createElement('button');
  button.className = 'cell';
  button.type = 'button';
  button.setAttribute('aria-label', item ? itemAriaLabel(item) : label);
  button.dataset.kind = kind;
  if (item) {
    button.draggable = true;
    button.dataset.itemId = item.id;
    button.classList.add('weapon-preview');
    button.classList.add(`weapon-${kind}`);
    if (gearSlotForItem(item)) button.classList.add('gear-item');
    button.setAttribute('aria-pressed', String(item.id === selectedId));
    const preview = document.createElement('img');
    preview.alt = '';
    preview.draggable = false;
    preview.decoding = 'async';
    // No rendered preview (gear, or a weapon without an exported mesh): show the placeholder.
    button.appendChild(preview);
    loadPreview(preview, item, () => { preview.remove(); button.appendChild(gearPlaceholder(item)); });
  }
  // The SWF stage is 1280 × 720; Ruffle stretches it to the player box.
  Object.assign(button.style, {
    left:`${bounds.xMin / 12.8}%`, top:`${bounds.yMin / 7.2}%`,
    width:`${(bounds.xMax-bounds.xMin) / 12.8}%`,
    height:`${(bounds.yMax-bounds.yMin) / 7.2}%`
  });
  if (tint) button.style.color = '#' + tint.toString(16).padStart(6,'0');
  if (click) button.addEventListener('click', click);
  // A rebuilt overlay beneath a stationary cursor must not behave like a new
  // selection. Pointer movement and deliberate keyboard focus still select.
  if (hover) { button.addEventListener('pointermove', event => { if (pointerMoved(event)) hover(event); }); button.addEventListener('focus', hover); }
  document.getElementById('controls').appendChild(button);
  return button;
}

// A plain hit box over stage-space (movie pixel) bounds that no movie clip provides.
function overlayButton(bounds, label, click, kind) {
  const button = document.createElement('button');
  button.className = 'cell';
  button.type = 'button';
  button.setAttribute('aria-label', label);
  button.title = label;
  button.dataset.kind = kind;
  Object.assign(button.style, {
    left:`${bounds.xMin / 12.8}%`, top:`${bounds.yMin / 7.2}%`,
    width:`${(bounds.xMax-bounds.xMin) / 12.8}%`, height:`${(bounds.yMax-bounds.yMin) / 7.2}%`
  });
  button.addEventListener('click', click);
  document.getElementById('controls').appendChild(button);
  return button;
}

function itemAriaLabel(item) {
  const flags = [item.favorite && 'favorite', item.trash && 'marked trash'].filter(Boolean);
  return [item.name, `level ${item.level}`, ...flags].join(', ');
}

function loadPreview(image, item, onFailure) {
  const urls = previewUrls(item).filter(url => previewStatus.get(url) !== 'missing');
  let next = 0;
  const tryNext = () => {
    if (next < urls.length) image.src = urls[next++];
    else onFailure();
  };
  image.addEventListener('load', () => previewStatus.set(urls[next - 1], 'ok'));
  image.addEventListener('error', () => {
    if (next > 0) previewStatus.set(urls[next - 1], 'missing');
    tryNext();
  });
  if (urls.length) tryNext(); else queueMicrotask(onFailure);
}

function addMarkControls(cell, item, owner) {
  if (!item || !owner) return;
  set(cell, 'bTrashFavoritesEnabled', true);
  const buttons = [];
  for (const kind of ['favorite', 'trash']) {
    const button = hit(`${cell}.${kind}`, `${kind === 'favorite' ? 'Favorite' : 'Trash'} ${item.name}`,
      () => { select(item.id); requestAction(kind, {id:item.id}); }, null, null, null, `mark-${kind}`);
    if (!button) continue;
    button.dataset.markOwner = item.id;
    button.style.pointerEvents = item[kind] ? 'auto' : 'none';
    buttons.push([button, kind]);
  }
  const enter = () => {
    call(cell, 'onRollOver');
    buttons.forEach(([button]) => { button.style.pointerEvents = 'auto'; });
  };
  const leave = event => {
    if (event.relatedTarget === owner || event.relatedTarget?.dataset.markOwner === item.id) return;
    call(cell, 'onRollOut');
    buttons.forEach(([button, kind]) => { button.style.pointerEvents = item[kind] ? 'auto' : 'none'; });
  };
  for (const button of [owner, ...buttons.map(([button]) => button)]) {
    button.addEventListener('pointerenter', enter);
    button.addEventListener('pointerleave', leave);
  }
}

function normalizeStatKey(value) { return String(value || '').toLowerCase().replace(/[^a-z0-9]/g, ''); }

function cardStats(item) {
  if (Array.isArray(item?.stats)) return item.stats.slice(0,5).map(stat => {
    const key = normalizeStatKey(stat.key || stat.label);
    return {
      key,
      label:String(stat.label || 'Stat'),
      value:stat.value,
      decimals:Number.isInteger(stat.decimals) ? stat.decimals : statDecimals(stat.value),
      higherIsBetter:typeof stat.higherIsBetter === 'boolean' ? stat.higherIsBetter : undefined,
      icon:stat.icon || statIcons.get(key)
    };
  });
  // Weapon values arrive unrounded; the printed number (and so the compare delta) is the rounded one.
  return weaponCardStats.filter(stat => item?.[stat.key] !== undefined && item?.[stat.key] !== null)
    .map(stat => {
      const raw = item[stat.key];
      const value = typeof raw === 'number' && Number.isFinite(raw) ? cardRound(raw, stat.rounding, stat.decimals) : raw;
      return {...stat, value};
    });
}

// Gear stat values arrive as display strings ("+88%", "6 m"); their number is compared as-is.
const statNumber = value => typeof value === 'number' ? value
  : typeof value === 'string' && /^\s*[-+]?\d/.test(value) ? Number.parseFloat(value) : NaN;
function statDecimals(value) {
  if (typeof value !== 'string') return 1;
  const match = /^\s*[-+]?\d*\.(\d+)/.exec(value);
  return match ? match[1].length : 0;
}

function statValueText(stat) {
  if (typeof stat.value === 'string') return stat.value;
  const value = Number(stat.value);
  if (stat.value !== '' && stat.value !== null && Number.isFinite(value)) return value.toFixed(stat.decimals) + (stat.suffix || '');
  return String(stat.value ?? '');
}

function statComparison(stat, otherStats) {
  const other = otherStats.find(candidate => candidate.key === stat.key);
  if (!other) return null;
  const value = statNumber(stat.value), otherValue = statNumber(other.value);
  if (!Number.isFinite(value) || !Number.isFinite(otherValue)) return null;
  const delta = value - otherValue;
  if (Math.abs(delta) < 1e-9) return {arrow:'blank', delta:''};
  const deltaText = `${delta > 0 ? '+' : ''}${delta.toFixed(stat.decimals)}`;
  if (typeof stat.higherIsBetter !== 'boolean') return {arrow:'blank', delta:deltaText};
  const better = stat.higherIsBetter ? delta > 0 : delta < 0;
  return {arrow:better ? 'up' : 'down', delta:deltaText};
}

function setCompareLayout(active, fromLeft = false) {
  if (compareLayoutActive === active && compareStartedFromLeft === fromLeft) return;
  compareLayoutActive = active;
  compareStartedFromLeft = fromLeft;
  call(INV, 'TweenPanel', 'Equipped', true, active);
  call(INV, 'TweenPanel', 'Backpack', true, active);
  call(INV, 'TweenCards', active, fromLeft);
}

const statMainX = new Map();
const statVisibility = new Map();
// Frame style: the movie's card background has three frames, named by SetBackgroundStyle (probed in
// the bench: default, 'highlight' = yellow, 'compare' = green). The game trace calls
// SetBackgroundStyle('highlight') on both cards while the menu opens, and both reference captures
// show a yellow frame around the selected card whatever its rarity, so the frame is a state, not a
// rarity colour: the main card is 'highlight'. That the game uses 'compare' for the second card is
// only inferred from the frame name (UNVERIFIED). Rarity colours the name, manufacturer and type icon.
function configureCard(card, item, compareItem = null, style = 'highlight') {
  call(card, 'SetVisible_', Boolean(item));
  if (!item) return;
  call(card, 'SetBackgroundStyle', style);
  const element = String(item.element || 'None');
  call(card, 'SetTitle', String(item.manufacturer || '').toLowerCase(), escapeHtml(item.name), color(item),
    movieTypeIcon(item), element === 'None' ? 'none' : element.toLowerCase(),
    equippedIds().has(item.id));
  call(card, 'TurnOffAllTopStats');
  const stats = cardStats(item), otherStats = cardStats(compareItem || {});
  for (let index=0; index<5; index++) {
    for (const field of ['mainField', 'auxField', 'arrow']) statVisibility.set(`${card}.stat${index+1}.${field}`, false);
    set(`${card}.stat${index+1}.arrow`, '_visible', false);
    set(`${card}.stat${index+1}.auxField`, '_visible', false);
  }
  stats.forEach((stat, index) => {
    const formatted = statValueText(stat);
    const compare = compareItem ? statComparison(stat, otherStats) : null;
    const arrow = compare?.arrow || 'blank';
    const delta = compare?.delta || '';
    call(card, 'SetTopStat', index, stat.label, formatted, arrow, delta, stat.icon || 'none');
    const row = `${card}.stat${index+1}`;
    text(`${row}.labelField`, stat.label, 14, 0xa4e8f3);
    text(`${row}.mainField`, formatted, 14);
    statVisibility.set(`${row}.mainField`, true);
    statVisibility.set(`${row}.auxField`, Boolean(delta));
    statVisibility.set(`${row}.arrow`, compare?.arrow === 'up' || compare?.arrow === 'down');
    if (compare?.arrow === 'up' || compare?.arrow === 'down') set(`${row}.arrow`, '_visible', true);
    if (delta) set(`${row}.auxField`, '_visible', true);
    // The value and the delta are both right-aligned to the same edge, so with a delta showing
    // the value moves left by the delta's width (its natural x is remembered per field).
    if (!statMainX.has(`${row}.mainField`)) statMainX.set(`${row}.mainField`, Number(get(`${row}.mainField`, '_x')));
    const deltaWidth = delta ? Number(get(`${row}.auxField`, 'textWidth')) : 0;
    set(`${row}.mainField`, '_x', statMainX.get(`${row}.mainField`) - (deltaWidth > 0 ? deltaWidth + 6 : 0));
  });
  // Legacy plain-text payloads are '; '-joined. The weapon contract puts red
  // text first; observed gear formatting below overrides this fallback.
  const funLines = typeof item.funStats === 'string' ? item.funStats.split(/;\s*/).filter(Boolean) : [];
  const redFirst = !gearSlotForItem(item);
  const funStats = funLines.map((line, index) =>
    `• <font color="${redFirst && index === 0 ? '#dc4646' : '#ffffff'}">${escapeHtml(line)}</font>\n`).join('');
  // Preserve observed gear colours/emphasis in the movie's TextField. This payload
  // is passed only to Flash; it must never be inserted into browser HTML.
  call(card, 'SetFunStats', typeof item.funStatsMarkup === 'string' && item.funStatsMarkup ? item.funStatsMarkup : funStats);
  const levelKnown = item.levelKnown !== false && Number.isFinite(Number(item.level));
  call(card, 'SetLevelRequirement', levelKnown, levelKnown && Number(item.level) <= Number(state.level), false,
    levelKnown ? `LEVEL REQUIREMENT: ${item.level}` : '');
  // Show the sale value whenever the host sends a number (valueKnown only says how sure it is).
  const valueKnown = item.value !== null && item.value !== '' && item.value !== undefined && Number.isFinite(Number(item.value));
  call(card, 'SetValue', valueKnown ? Number(item.value) : 0);
  call(card, 'SetEridiumValue', 0);
  set(card + '.valueClip', '_visible', valueKnown);
  set(card + '.eridiumCounter', '_visible', false);
  call(card, 'SetHeight');
  fitFunStats(card, funLines.length > 0);
}

// Ruffle has no TextField.numLines, which ItemCard.CalculateHeight needs to size the flavour
// block, so the card comes out short and the manufacturer/type row lands on the flavour text.
// Grow the card by the text's real height and push that row (and the value plate) down.
function fitFunStats(card, hasFunStats) {
  if (!hasFunStats) return;
  const textHeight = Number(get(`${card}.funstats`, 'textHeight'));
  const height = Number(get(card, 'dynamicHeight'));
  if (!(textHeight > 0) || !Number.isFinite(height)) return;
  const extra = textHeight - FUN_STATS_OVERLAP;
  for (const row of ['manufacturer', 'typeIcon', 'elementalIcon'])
    set(`${card}.${row}`, '_y', Number(get(`${card}.${row}`, '_y')) + extra);
  call(card, 'SetDirectHeight', height + extra);
}

function drawCard() {
  const item = itemById(selectedId);
  const previewId = item?.id || '';
  if (lastMenuPreviewId !== previewId) {
    lastMenuPreviewId = previewId;
    console.log('OWMENUPREVIEW ' + JSON.stringify({id:previewId}));
  }
  const compare = compareId && compareId !== selectedId ? itemById(compareId) : null;
  const fromLeft = Boolean(transferSourceId && transferFromEquipped && compare);
  configureCard(INV + '.mainCard', fromLeft ? compare : item, fromLeft ? item : compare);
  configureCard(INV + '.compareCard', fromLeft ? item : compare, fromLeft ? compare : item, 'compare');
  setCompareLayout(Boolean(compare) || Boolean(transferSourceId), fromLeft);
  const gearSlot = gearSlotForItem(item);
  const compareSlotLabel = gearSlot
    ? gearSlots.find(slot => slot.key === gearSlot)?.label || 'gear slot'
    : `slot ${targetSlot+1}`;
  // Match the observed stock tooltip line. Extra host keys remain available.
  const hints = [[transferSourceId ? (compare ? '[E] Swap' : '[E] Equip') : '[E] Select/Compare', !!item],
    ['[Q] Drop', !!item && !transferSourceId]];
  if (transferSourceId || (item && !equippedIds().has(item.id)))
    hints.push(['[Page Up]/[Page Down] Sort', !transferSourceId]);
  hints.push([transferSourceId ? '[Escape] Cancel' : '[Escape] Close', true], ['[F] Inspect', !!item]);
  const markup = hints.map(([label, enabled]) =>
    `<font color="${enabled ? '#a4e8f3' : '#666666'}">${escapeHtml(label)}</font>`).join('   ');
  set(ROOT + '.tooltips.tooltips', 'htmlText',
    `<p align="center"><font face="$WillowBody" size="15">${markup}</font></p>`);
  applyAmmoHighlight(item);
  // While the inspect panel is open it stands in for the selected card.
  if (inspectMode && item) call(INV + '.mainCard', 'SetVisible_', false);
  if (compare) announce(`Comparing ${item.name} with ${compareSlotLabel}: ${compare.name}`);
  updateInspect();
  applyCardOcclusion();
}

function syncSelection() {
  if (!ready || !state) return;
  const equipped = INV + '.equippedPanel';
  const selectedCell = selectedSlotIndex();
  for (let i=0; i<8; i++) {
    call(`${equipped}.cell${i+1}`, 'SetSelected', i === selectedCell);
    const cellButton = document.querySelector(`#controls [data-kind="slot"][data-slot="${i}"]`);
    if (cellButton) cellButton.setAttribute('aria-pressed', String(i === selectedCell));
  }
  const rows = backpackItems();
  for (let row=0; row<RENDERED_ROWS; row++) {
    const item = rows[firstRow+row];
    if (item) call(`${INV}.storagePanel.owRows.owRow${row}`, 'SetSelected', item.id === selectedId);
  }
  for (const button of document.querySelectorAll('#controls [data-kind="backpack"]'))
    button.setAttribute('aria-pressed', String(button.dataset.itemId === selectedId));
}

function scrollForSelected(rows) {
  const index = rows.findIndex(item => item.id === selectedId);
  const maximum = Math.max(0, rows.length - VISIBLE_ROWS);
  const start = Math.max(0, Math.min(maximum, firstRow));
  if (index < 0) return start;
  if (index < start) return index;
  return index >= start + VISIBLE_ROWS ? Math.min(maximum, index - VISIBLE_ROWS + 1) : start;
}

function scrollBackpack(delta) {
  const rows = backpackItems();
  const next = Math.max(0, Math.min(Math.max(0, rows.length-VISIBLE_ROWS), firstRow + delta));
  if (next === firstRow) return;
  firstRow = next;
  render();
}

function changeSort(direction = 1) {
  if (transferSourceId) return;
  sortIndex = (sortIndex + direction + sortModes.length) % sortModes.length;
  const rows = backpackItems();
  firstRow = scrollForSelected(rows);
  render();
  announce(`Backpack sorted by ${sortModes[sortIndex].label.toLowerCase()}`);
}

function announce(message) {
  const status = document.getElementById('live-status');
  if (status) status.textContent = message;
}

function updateInspect() {
  const inspect = document.getElementById('inspect-preview');
  const item = itemById(selectedId);
  if (!inspect) return;
  if (!inspectMode || !item) { inspect.hidden = true; return; }
  if (inspectItemId !== item.id) {
    inspectItemId = item.id;
    inspectYaw = inspectPitch = 0;
    inspectImage = '';
    inspectResolved = false;
    requestInspectFrame();
  }
  const bounds = readBounds(INV + '.mainCard.bkgd');
  if (!bounds) { inspect.hidden = true; return; }
  Object.assign(inspect.style, {
    left:`${bounds.xMin / 12.8}%`, top:`${bounds.yMin / 7.2}%`,
    width:`${(bounds.xMax-bounds.xMin) / 12.8}%`, height:`${(bounds.yMax-bounds.yMin) / 7.2}%`
  });
  inspect.replaceChildren();
  const title = document.createElement('strong');
  title.textContent = item.name;
  title.style.color = '#' + color(item).toString(16).padStart(6, '0');
  inspect.appendChild(title);
  const picture = document.createElement('div');
  picture.className = 'inspect-picture';
  const image = document.createElement('img');
  image.alt = `${item.name} preview`;
  image.draggable = false;
  picture.appendChild(image);
  if (inspectImage) image.src = inspectImage;
  else { image.remove(); picture.textContent = inspectResolved || gearSlotForItem(item) ? '3D model unavailable' : 'Loading 3D model…'; }
  inspect.appendChild(picture);
  const hint = document.createElement('span');
  hint.textContent = 'Drag to rotate   [F] Close';
  inspect.appendChild(hint);
  inspect.hidden = false;
}

function toggleInspect() {
  inspectMode = !inspectMode;
  if (inspectMode) inspectItemId = null;
  // Leaving inspect brings the hidden card back through the normal draw path.
  drawCard();
}

function requestInspectFrame() {
  if (!inspectMode || !inspectItemId) return;
  inspectLastRequest = performance.now();
  console.log('OWINSPECT ' + JSON.stringify({id:inspectItemId, yaw:inspectYaw, pitch:inspectPitch}));
}
window.owInspectFrame = reply => {
  if (!inspectMode || reply?.id !== inspectItemId) return;
  inspectImage = typeof reply.image === 'string' && reply.image.startsWith('data:image/png;base64,') ? reply.image : '';
  inspectFrameCount++;
  inspectResolved = true;
  updateInspect();
  if (!inspectImage) document.querySelector('#inspect-preview .inspect-picture').textContent = '3D model unavailable';
};
const inspectSurface = document.getElementById('inspect-preview');
inspectSurface.addEventListener('pointerdown', event => {
  if (event.button !== 0) return;
  event.preventDefault();
  inspectPointer = {id:event.pointerId, x:event.clientX, y:event.clientY};
  if (event.isTrusted) inspectSurface.setPointerCapture(event.pointerId);
});
inspectSurface.addEventListener('pointermove', event => {
  if (!inspectPointer || inspectPointer.id !== event.pointerId) return;
  inspectYaw = (inspectYaw + (event.clientX-inspectPointer.x)*.7) % 360;
  inspectPitch = Math.max(-80, Math.min(80, inspectPitch + (event.clientY-inspectPointer.y)*.7));
  inspectPointer.x = event.clientX; inspectPointer.y = event.clientY;
  if (performance.now()-inspectLastRequest >= 100) requestInspectFrame();
});
const endInspectDrag = event => {
  if (!inspectPointer || inspectPointer.id !== event.pointerId) return;
  inspectPointer = null;
  requestInspectFrame();
};
inspectSurface.addEventListener('pointerup', endInspectDrag);
inspectSurface.addEventListener('pointercancel', endInspectDrag);

// Hide the thumbnails and hit boxes of any cell a visible item card overlaps, so previews
// never bleed over card text. Full-size stock comparisons intentionally cover
// parts of both panels, including host category arrows and mark hit targets.
function applyCardOcclusion() {
  const stage = document.getElementById('stage').getBoundingClientRect();
  const scale = stage.width / 1280;
  const covers = [];
  if (itemById(selectedId) && !inspectMode) covers.push(readBounds(INV + '.mainCard.bkgd'));
  if (compareId && itemById(compareId)) covers.push(readBounds(INV + '.compareCard.bkgd'));
  const rects = covers.filter(Boolean).map(bounds => ({
    left:stage.left + bounds.xMin * scale, right:stage.left + bounds.xMax * scale,
    top:stage.top + bounds.yMin * scale, bottom:stage.top + bounds.yMax * scale
  }));
  for (const button of document.querySelectorAll('#controls .cell')) {
    const box = button.getBoundingClientRect();
    // A sliver of overlap (card glow art) does not count; a fifth of the cell does.
    const covered = rects.some(rect => {
      const width = Math.min(box.right, rect.right) - Math.max(box.left, rect.left);
      const height = Math.min(box.bottom, rect.bottom) - Math.max(box.top, rect.top);
      return width > 0 && height > 0 && width * height >= 0.2 * box.width * box.height;
    });
    button.classList.toggle('covered', covered);
  }
}

function focusItem(id) {
  if (!id) return;
  const button = [...document.querySelectorAll('#controls [data-item-id]')].find(node => node.dataset.itemId === id);
  button?.focus({preventScroll:true});
}

// Item-only backpack movement is executed by the installed UnrealScript VM.
// Requests are ordered; replies from an obsolete selection/list are discarded.
const inventoryVm = {enabled:false, calls:0, errors:0, steps:0, discarded:0, pending:null, queue:[], serial:0, failed:false};
window.owInventoryVm = inventoryVm;
window.owConfigureInventoryVm = enabled => { inventoryVm.enabled = Boolean(enabled) && !inventoryVm.failed; };
window.owCancelInventoryVm = () => { inventoryVm.pending = null; inventoryVm.queue.length = 0; };
function inventoryVmFailure(reason) {
  inventoryVm.pending = null; inventoryVm.errors++; inventoryVm.failed = true;
  inventoryVm.enabled = false; inventoryVm.queue.length = 0;
  console.error('OpenWillow inventory VM failed: ' + reason);
  announce('Inventory script error: ' + reason);
}
function requestInventoryMove(delta) {
  if (inventoryVm.queue.length >= 32) return;
  inventoryVm.queue.push(delta);
  dispatchInventoryMove();
}
function dispatchInventoryMove() {
  if (inventoryVm.pending || !inventoryVm.queue.length) return;
  if (!inventoryVm.enabled || navigationPanel !== 'backpack' || transferSourceId || inspectMode) {
    inventoryVm.queue.length = 0; return;
  }
  const rows = backpackItems();
  if (!rows.length) { inventoryVm.queue.length = 0; return; }
  const start = rows.findIndex(item => item.id === selectedId);
  if (start < 0) { inventoryVm.queue.length = 0; return; }
  const delta = inventoryVm.queue.shift(), serial = ++inventoryVm.serial;
  inventoryVm.pending = {serial, selectedId, ids:rows.map(item => item.id)};
  console.log('OWINVMOVE ' + JSON.stringify({serial, delta, start, count:rows.length}));
  setTimeout(() => {
    if (inventoryVm.pending?.serial === serial) inventoryVmFailure('host response timed out');
  }, 6000);
}
window.owInventoryVmResult = result => {
  const pending = inventoryVm.pending;
  if (!pending || result.serial !== pending.serial) return;
  inventoryVm.pending = null;
  if (result.error || !Number.isInteger(result.index) || result.index < 0 || result.index >= pending.ids.length
      || !Number.isInteger(result.steps) || result.steps <= 0) {
    inventoryVmFailure(result.error || 'invalid response');
    return;
  }
  const rows = backpackItems();
  if (navigationPanel !== 'backpack' || transferSourceId || inspectMode || selectedId !== pending.selectedId
      || rows.length !== pending.ids.length || rows.some((item, index) => item.id !== pending.ids[index])) {
    inventoryVm.discarded++; inventoryVm.queue.length = 0; return;
  }
  inventoryVm.calls++; inventoryVm.steps += result.steps;
  applyBackpackSelection(rows, result.index);
  dispatchInventoryMove();
};

function applyBackpackSelection(rows, index) {
  selectedId = rows[index].id;
  navigationPanel = 'backpack';
  lastBackpackId = selectedId;
  targetGearSlot = gearSlotForItem(rows[index]);
  compareId = transferSourceId;
  const nextFirstRow = scrollForSelected(rows);
  if (nextFirstRow !== firstRow) { firstRow = nextFirstRow; render(); focusItem(selectedId); }
  else { syncSelection(); drawCard(); focusItem(selectedId); }
  announce(itemAriaLabel(itemById(selectedId)));
}

function moveSelection(delta) {
  if (transferSourceId && !transferFromEquipped) {
    if (!targetGearSlot) cycleTargetSlot(delta);
    return;
  }
  const rows = backpackItems();
  if (!rows.length) return;
  if (inventoryVm.failed && navigationPanel === 'backpack' && !transferSourceId) return;
  if (inventoryVm.enabled && navigationPanel === 'backpack' && !transferSourceId) {
    requestInventoryMove(delta); return;
  }
  let index = rows.findIndex(item => item.id === selectedId);
  index = index < 0 ? 0 : Math.max(0, Math.min(rows.length-1, index + delta));
  applyBackpackSelection(rows, index);
}

// Host spatial navigation uses the movie's actual cell centers rather than
// assuming export order is screen order. Exact stock traversal is UNVERIFIED.
function selectEquipmentSlot(index) {
  if (!Number.isInteger(index) || index < 0 || index >= 8 || slotLocked(index)) return;
  if (transferSourceId) {
    if (!transferFromEquipped && !targetGearSlot && index < 4) setTargetSlot(index);
    return;
  }
  navigationPanel = 'equipped';
  lastEquippedIndex = index;
  targetGearSlot = index >= 4 ? gearSlots[index-4].key : null;
  if (index < 4) targetSlot = index;
  selectedId = itemById(equippedItems()[index])?.id || null;
  compareId = null;
  syncSelection();
  drawCard();
  document.querySelector(`#controls [data-kind="slot"][data-slot="${index}"]`)?.focus({preventScroll:true});
  announce(selectedId ? itemAriaLabel(itemById(selectedId))
    : `${index < 4 ? `Weapon slot ${index+1}` : gearSlots[index-4].label}, empty`);
}

// Equipment cell indices: 0-3 weapon slots, then gearSlots order (4 shield, 5 grenade mod,
// 6 class mod, 7 relic). The stock screen puts the gear in a 2x2 grid: shield / class mod
// on top, grenade mod / relic below. Observed in the original game (2026-09-30, keyboard
// arrows, no wrapping anywhere): Down from the last weapon enters the shield, Up from the
// shield returns to it, Right from a weapon slot or from a right-hand gear cell enters the
// backpack, Left from a left-hand cell does nothing. Up from class mod / relic was not
// observed; the entries marked UNVERIFIED are host guesses.
const GEAR_NEIGHBOURS = {
  4: {up:'weapons', down:5, right:6},
  5: {up:4, right:7},
  6: {up:'weapons', down:7, left:4, right:'backpack'},  // up UNVERIFIED
  7: {up:6, left:5, right:'backpack'}                   // up UNVERIFIED
};

function equipmentNeighbour(index, direction) {
  const lastWeapon = () => { for (let i = 3; i >= 0; i--) if (!slotLocked(i)) return i; return 0; };
  let next;
  if (index < 4) {
    if (direction === 'up') for (let i = index-1; i >= 0 && next === undefined; i--) { if (!slotLocked(i)) next = i; }
    else if (direction === 'down') {
      for (let i = index+1; i < 4 && next === undefined; i++) { if (!slotLocked(i)) next = i; }
      if (next === undefined) next = 4;
    } else if (direction === 'right') next = 'backpack';
  } else {
    next = GEAR_NEIGHBOURS[index]?.[direction];
    if (next === 'weapons') next = lastWeapon();
  }
  return next;
}

function enterBackpack() {
  const rows = backpackItems();
  const id = rows.find(item => item.id === lastBackpackId)?.id || rows[firstRow]?.id;
  if (id) { select(id); firstRow = scrollForSelected(rows); render(); focusItem(id); }
}

function navigateInventory(direction) {
  if (inspectMode) return;
  const horizontal = direction === 'left' || direction === 'right';
  if (transferSourceId) {
    // Observed: an equipped-origin swap walks the compatible backpack candidates with Up/Down;
    // a backpack-origin swap picks the destination weapon slot with Up/Down. In both, Left and
    // Right did nothing and the ends did not wrap.
    if (horizontal) return;
    if (transferFromEquipped) moveSelection(direction === 'up' ? -1 : 1);
    else if (!targetGearSlot) {
      for (let slot = targetSlot + (direction === 'up' ? -1 : 1); slot >= 0 && slot < 4; slot += direction === 'up' ? -1 : 1)
        if (!slotLocked(slot)) { setTargetSlot(slot); break; }
    }
    return;
  }
  if (navigationPanel === 'backpack') {
    if (!horizontal) moveSelection(direction === 'up' ? -1 : 1);
    // Observed: Left returns to the equipped cell last selected there, not to the cell the
    // selected backpack item would occupy.
    else if (direction === 'left') selectEquipmentSlot(lastEquippedIndex);
    return;
  }
  const next = equipmentNeighbour(selectedSlotIndex(), direction);
  if (next === 'backpack') enterBackpack();
  else if (Number.isInteger(next)) selectEquipmentSlot(next);
}

function setTargetSlot(slot) {
  if (transferSourceId && !transferFromEquipped && targetGearSlot) return;
  if (slot < 0 || slot > 3 || slotLocked(slot)) return;
  if (slot !== targetSlot) compareId = null;
  targetGearSlot = null;
  targetSlot = slot;
  if (transferSourceId && !transferFromEquipped) {
    compareId = state.slots[slot] || null;
    render();
    return;
  }
  const held = state.slots[targetSlot];
  const selectedIsBackpack = itemById(selectedId) && !equippedIds().has(selectedId);
  if (!selectedIsBackpack) {
    selectedId = itemById(held)?.id || null;
    navigationPanel = 'equipped';
    compareId = null;
  }
  render();
}

function cycleTargetSlot(direction) {
  const open = slotsUnlocked();
  setTargetSlot((targetSlot + direction + open) % open);
}

// Show the PC key numbers (1-4) beside the weapon cells. The movie's own PC path
// (traced in the game: StatusMenuEquippedPanelGFxObject.SetUpEquippedSlotIcons) puts each
// icons.dpad* clip on its "pc" frame and writes the slot number into its equipNumber field.
// Cell order is Up, Down, Left, Right = slots 1-4 (SetCellInfo's argument order).
function applySlotNumbers() {
  ['Up','Down','Left','Right'].forEach((direction, index) => {
    const icon = `${INV}.equippedPanel.icons.dpad${direction}`;
    player.ow(icon, 'gotoAndStop', 'pc');
    set(`${icon}.equipNumber`, 'text', String(index + 1));
  });
}

// Fill the movie's ammo panel and currency bar from optional host fields. Anything the
// host did not send is hidden rather than left showing the converted movie's 9999/999
// placeholders.
function applyStatus() {
  const hasAmmo = hasAmmoField();
  for (const type of ammoTypes) {
    const entry = hasAmmo ? ammoEntry(type.key) : null;
    for (const [suffix, value] of [['', entry?.current], ['_max', entry?.max]]) {
      const field = `${INV}.ammo.${type.clip}${suffix}`;
      set(field, '_visible', Boolean(entry));
      if (entry) set(field, 'text', String(value));
    }
  }
  const money = displayCount(state.money), eridium = displayCount(state.eridium);
  if (money !== null) {
    call(`${INV}.currencyPanel.credits`, 'SetLanguageExt', 'INT');
    call(`${INV}.currencyPanel.credits`, 'SetValue', Math.min(money, MAX_DISPLAY_CREDITS));
  }
  if (eridium !== null) set(`${INV}.currencyPanel.eridiumCounter.digits.eridiumText`, 'text', String(eridium));
}

// The converted movie re-asserts some visibilities while its opening tween runs, so
// these are re-applied on every layout poll and not only inside render().
function applyMovieVisibility() {
  if (!state) return;
  // The stock tween temporarily hides values; its native completion callback
  // is absent here. Restore the fields populated from the current snapshot.
  for (const [path, visible] of statVisibility)
    if (get(path, '_visible') !== visible) set(path, '_visible', visible);
  const hasMoney = displayCount(state.money) !== null, hasEridium = displayCount(state.eridium) !== null;
  set(INV+'.ammo', '_visible', hasAmmoField());
  set(INV+'.currencyPanel', '_visible', hasMoney || hasEridium);
  set(INV+'.currencyPanel.credits', '_visible', hasMoney);
  set(INV+'.currencyPanel.eridiumCounter', '_visible', hasEridium);
  // Both panels are always reachable here, so only the original's "go to the
  // backpack" chevron is kept (as in the reference); the left one stays hidden.
  set(INV+'.arrowLeft', '_visible', false);
  set(INV+'.arrowRight', '_visible', true);
  // The two dual-wield link brackets are Gunzerker-only art; Maya has no slot pairs.
  set(INV+'.equippedPanel.gunzerker1', '_visible', false);
  set(INV+'.equippedPanel.gunzerker2', '_visible', false);
}

// The ammo panel highlight follows the selected item, as the original's
// AmmoPanelGFxObject.SetHighlight does (weapon type frame, "none" otherwise).
function applyAmmoHighlight(item) {
  const key = ammoKeyForItem(item);
  const frame = ammoTypes.find(type => type.key === key)?.frame || 'none';
  player.ow(INV+'.ammo.highlight', 'gotoAndStop', frame);
}

// Backpack header: "BACKPACK (<filter>)" from the movie, plus a sub-label ("<category> (<count>)")
// drawn with the movie's own header clip above the first row and category chevrons beside it.
// The header clip is attached hidden and only shown once it has real bounds and text: a clip
// left at its default spot would show the movie's placeholder "Assault Rifles" (seen while the
// panels were mid-tween), so a failed attempt removes it and is retried by watchLayout().
//
// Every refresh attaches the clip under a fresh name and depth. Removing a clip and re-attaching
// one with the same name in the same frame leaves that name pointing at the doomed clip, so the
// text and position went to it and the survivor kept the movie's default "Assault Rifles" at the
// panel origin (the stray label seen right of the Backpack in the engine captures).
function refreshBackpackHeader(rowBounds = null, columnBounds = null) {
  const panel = INV + '.storagePanel';
  if (headerName) call(`${panel}.${headerName}`, 'removeMovieClip');
  headerName = `owHeader${++headerSerial}`;
  const path = `${panel}.${headerName}`;
  const rows = backpackItems();
  const category = categories[categoryIndex], mode = sortModes[sortIndex];
  // Original label: "BACKPACK" with a small "(ALL)" filter tag; SetSortLabel accepts the html.
  // The host-side sort mode takes the tag's place only for the unfiltered list.
  const tag = category.key === 'all' && mode.key !== 'default' ? mode.label : category.label;
  call(panel, 'SetSortLabel', `BACKPACK <font size="16">(${escapeHtml(tag)})</font>`);
  document.querySelectorAll('#controls [data-kind="category"]').forEach(node => node.remove());
  const panelBounds = readBounds(panel + '.bkgd'), firstRowBounds = rowBounds || readBounds(`${panel}.owRows.owRow0`);
  headerPending = !panelBounds || !firstRowBounds;
  if (headerPending) return;
  // Only a single category can be named honestly for an unfiltered mixed list.
  const names = [...new Set(rows.map(itemCategoryLabel))];
  const name = category.match ? category.label : names.length === 1 ? names[0] : 'ALL ITEMS';
  call(panel, 'attachMovie', 'inventory - storage panel - header', headerName, 2100 + headerSerial);
  set(path, '_visible', false);
  const bounds = readBounds(path);
  if (!bounds || bounds.xMax <= bounds.xMin) { call(path, 'removeMovieClip'); headerPending = true; return; }
  set(path+'.textField', 'htmlText',
    `<p align="center"><font face="$WillowBody" size="14" color="#e2edf1">${escapeHtml(name)}</font></p>`);
  const width = bounds.xMax - bounds.xMin, height = bounds.yMax - bounds.yMin;
  const desiredX = (firstRowBounds.xMin + firstRowBounds.xMax - width) / 2;
  const desiredY = firstRowBounds.yMin - height + 4;
  set(path, '_x', Number(get(path, '_x') || 0) + (desiredX - bounds.xMin) / (PANEL_SCALE * COMPOSITION_SCALE));
  set(path, '_y', Number(get(path, '_y') || 0) + (desiredY - bounds.yMin) / (PANEL_SCALE_Y * COMPOSITION_SCALE));
  set(path, '_visible', true);
  // Chevrons sit in the gutter between the visible cell column (the clip's own bounds include
  // glow art out to the panel frame) and the panel frame, level with the sub-label.
  const column = columnBounds || readBounds(`${panel}.owRows.owRow0.hitTestClip`) || firstRowBounds;
  const shown = readBounds(path) || bounds;
  const cy = (shown.yMin + shown.yMax) / 2, size = 24;
  [[-1, '‹', 'Previous category', column.xMin - size - 3],
   [1, '›', 'Next category', column.xMax + 3]].forEach(([delta, glyph, label, left]) => {
    const button = overlayButton({xMin:left, xMax:left + size, yMin:cy - size / 2, yMax:cy + size / 2},
      `${label} (${category.label})`, () => changeCategory(delta), 'category');
    button.classList.add('chevron');
    button.textContent = glyph;
  });
}

function changeCategory(delta) {
  categoryIndex = (categoryIndex + delta + categories.length) % categories.length;
  firstRow = 0;
  const rows = backpackItems();
  const selected = itemById(selectedId);
  // Keep an equipped selection; a backpack selection that the filter hides moves to the first row.
  if (selected && !equippedIds().has(selected.id) && !rows.some(item => item.id === selected.id)) {
    selectedId = rows[0]?.id || itemById(state.slots[targetSlot])?.id || null;
    targetGearSlot = gearSlotForItem(itemById(selectedId));
    compareId = null;
  }
  firstRow = scrollForSelected(rows);
  render();
  announce(`Backpack category ${categories[categoryIndex].label.toLowerCase()}, ${rows.length} items`);
}

// Scale and place the Backpack panel (see PANEL_SCALE). Idempotent: nothing is written once the clip
// is within half a pixel of the target, so the layout watcher does not chase its own change.
function placeStoragePanel() {
  const panel = INV + '.storagePanel';
  if (Math.abs(Number(get(panel, '_xscale')) - PANEL_SCALE * 100) > 0.5) {
    set(panel, '_xscale', PANEL_SCALE * 100);
    set(panel, '_yscale', PANEL_SCALE_Y * 100);
  }
  const bounds = readBounds(panel + '.bkgd');
  if (!bounds) return;
  const dx = PANEL_LEFT - bounds.xMin, dy = PANEL_TOP - bounds.yMin;
  if (Math.abs(dx) > 0.5) set(panel, '_x', Number(get(panel, '_x')) + dx / COMPOSITION_SCALE);
  if (Math.abs(dy) > 0.5) set(panel, '_y', Number(get(panel, '_y')) + dy / COMPOSITION_SCALE);
}

function render() {
  if (!ready || !state) return;
  const layer = document.getElementById('controls');
  // A rebuilt overlay would drop keyboard focus; remember it and restore it below.
  const focused = layer.contains(document.activeElement) ? document.activeElement : null;
  const focusedKey = focused ? {kind:focused.dataset.kind, id:focused.dataset.itemId, label:focused.getAttribute('aria-label')} : null;
  layer.replaceChildren();
  applyMovieVisibility();
  placeStoragePanel();
  applyStatus();
  applySlotNumbers();
  const equipped = INV + '.equippedPanel';
  const slotIds = equippedItems();
  const selectedCell = selectedSlotIndex();
  // Locked weapon slots are drawn empty whatever the snapshot says.
  const shownIds = slotIds.map((id, index) => slotLocked(index) ? null : id);
  call(equipped, 'SetCellInfo', ...shownIds.map(id => itemById(id) ? color(itemById(id)) : 0));
  for (let i=0; i<8; i++) {
    const cell = `${equipped}.cell${i+1}`, item = itemById(shownIds[i]), locked = slotLocked(i);
    call(cell, 'SetSoldOut', false);
    call(cell, 'SetEmptyCell', !item);
    // SetCellState switches the cell background frame (normal/locked/bad/...); the
    // locked frame carries the padlock art, so the "(EMPTY)" label is hidden there.
    if (i < 4) {
      call(cell, 'SetCellState', locked ? 'locked' : 'normal');
      set(cell+'.emptyLabel', '_visible', !locked && !item);
    }
    call(cell, 'SetTrashFavoriteMark', item?.trash ? 1 : item?.favorite ? 2 : 0);
    call(cell, 'SetSelected', !locked && i === selectedCell);
    const slot = i < 4 ? null : gearSlots[i-4];
    const label = slot ? slot.label : `Weapon slot ${i+1}`;
    if (locked) {
      const lockedButton = hit(cell, `${label}, locked`, null, null, null, null, 'slot-locked', `${cell}.hitTestClip`);
      if (lockedButton) lockedButton.disabled = true;
      continue;
    }
    const slotButton = hit(cell, item ? '' : `${label}, empty`, () => {
      if (transferSourceId) {
        if (!transferFromEquipped && i < 4) setTargetSlot(i);
        return;
      }
      selectEquipmentSlot(i);
      render();
    }, item ? () => select(item.id) : null, color(item), item, 'slot', `${cell}.hitTestClip`);
    if (slotButton) {
      slotButton.dataset.slot = String(i);
      slotButton.setAttribute('aria-pressed', String(i === selectedCell));
    }
    addMarkControls(cell, item, slotButton);
  }

  const rows = backpackItems();
  firstRow = Math.max(0, Math.min(firstRow, Math.max(0, rows.length-VISIBLE_ROWS)));
  const panel = INV + '.storagePanel';
  const panelBounds = call(panel+'.bkgd','getBounds',ROOT);
  const rowGroup = panel + '.owRows';
  if (!frameReady(rowGroup)) call(panel, 'createEmptyMovieClip', 'owRows', 2000);
  const localPanelBounds = call(panel+'.bkgd', 'getBounds', panel);
  let rowWidth = 0;
  if (panelBounds) {
    const zone = overlayButton(panelBounds, 'Return equipped item to backpack', () => {}, 'backpack-zone');
    zone.tabIndex = -1;
    zone.removeAttribute('title');
    zone.setAttribute('aria-hidden', 'true');
  }
  for (let row=0; row<RENDERED_ROWS; row++) {
    const path = `${panel}.owRows.owRow${row}`;
    call(path, 'removeMovieClip');
    const item = rows[firstRow+row];
    call(rowGroup, 'attachMovie', 'inventory - cell', `owRow${row}`, 2000+row);
    const localCell = call(path, 'getBounds', path);
    set(path, '_xscale', ROW_SCALE * 100);
    set(path, '_yscale', ROW_SCALE * 100);
    if (localPanelBounds && localCell) {
      rowWidth = (localCell.xMax-localCell.xMin) * ROW_SCALE;
      // Keep the mask origin at zero for every render. Ruffle defers changes
      // to a nonzero scrollRect origin; measuring during that transition made
      // repeated sort/filter/transfer renders accumulate a position offset.
      set(rowGroup, '_x', (localPanelBounds.xMin+localPanelBounds.xMax-rowWidth)/2);
      set(rowGroup, '_y', localPanelBounds.yMin+75);
      set(path, '_x', -localCell.xMin * ROW_SCALE);
      set(path, '_y', -localCell.yMin * ROW_SCALE + row*ROW_PITCH);
    }
    call(path, 'SetSoldOut', false);
    call(path, 'SetEmptyCell', !item);
    call(path, 'SetRarityColor', item ? color(item) : 0);
    call(path, 'SetTrashFavoriteMark', item?.trash ? 1 : item?.favorite ? 2 : 0);
    call(path, 'SetSelected', Boolean(item && item.id === selectedId));
    if (!item) continue;
    const rowButton = hit(path, '', () => {
      select(item.id);
      if (row >= VISIBLE_ROWS) { firstRow = scrollForSelected(rows); render(); }
    }, row < VISIBLE_ROWS ? () => select(item.id) : null, color(item), item, 'backpack', `${path}.hitTestClip`);
    if (row >= VISIBLE_ROWS && rowButton) rowButton.dataset.partial = 'true';
    if (row < VISIBLE_ROWS) addMarkControls(path, item, rowButton);
    const button = [...layer.querySelectorAll('[data-item-id]')].find(node => node.dataset.itemId === item.id);
    if (button) button.addEventListener('dblclick', event => { event.preventDefault(); select(item.id); equip(); });
  }
  const headerRowBounds = readBounds(rowGroup+'.owRow0');
  const headerColumnBounds = readBounds(rowGroup+'.owRow0.hitTestClip');
  if (rowWidth) {
    const height = VISIBLE_ROWS * ROW_PITCH + PEEK_HEIGHT;
    const top = headerRowBounds;
    set(rowGroup, 'scrollRect', {x:0, y:0, width:rowWidth, height});
    const bottom = top?.yMin + height * PANEL_SCALE_Y * COMPOSITION_SCALE;
    const stage = document.getElementById('stage').getBoundingClientRect();
    for (const button of layer.querySelectorAll('[data-partial=true]')) {
      const bounds = button.getBoundingClientRect();
      const limit = stage.top + bottom * stage.width / 1280;
      const cropped = Math.max(0, Math.min(100, (bounds.bottom-limit) / bounds.height * 100));
      button.style.clipPath = `inset(0 0 ${cropped}% 0)`;
    }
  }

  const count = Number.isFinite(state.backpackCount) ? state.backpackCount : rows.length;
  const capacity = Number.isFinite(state.backpackCapacity) && state.backpackCapacity > 0
    ? state.backpackCapacity : null;
  // The original feeds "used/capacity" to a hidden clip (INV.storageCount) and draws no
  // visible count, so the count stays out of the header (tooltip and screen readers only).
  call(INV, 'SetStorageInfoCardData', capacity ? `${count}/${capacity}` : String(count));
  refreshBackpackHeader(headerRowBounds, headerColumnBounds);
  hit(ROOT+'.header.pcCloseButton', 'Close inventory', closeInventory, null, null, null, 'close');
  addHeaderTabs();
  const sortButton = hit(panel+'.pcSortButton', `Sort backpack by ${sortModes[sortIndex].label.toLowerCase()}`, () => changeSort(), null, null, null, 'sort');
  if (sortButton && capacity) sortButton.title = `Backpack ${count}/${capacity}`;
  const morePrevious = firstRow > 0, moreNext = firstRow+VISIBLE_ROWS < rows.length;
  // The scroll chevrons are only drawn when there is somewhere to scroll.
  set(panel+'.moreUp', '_visible', morePrevious);
  set(panel+'.moreDown', '_visible', moreNext);
  if (morePrevious) hit(panel+'.moreUp', 'Scroll backpack up', () => scrollBackpack(-1), null, null, null, 'previous');
  if (moreNext) hit(panel+'.moreDown', 'Scroll backpack down', () => scrollBackpack(1), null, null, null, 'next');
  drawCard();
  document.getElementById('loading').hidden = true;
  document.getElementById('live-status').textContent = `${count}${capacity ? ` of ${capacity}` : ''} items in backpack. Sorted by ${sortModes[sortIndex].label.toLowerCase()}.`;
  renderedLayout = movieLayoutSignature();
  renderedCard = cardSignature();
  pendingLayout = '';
  if (focusedKey) restoreFocus(layer, focusedKey);
  if (!firstStateRendered) {
    firstStateRendered = true;
    window.owInventoryReady = true;
    window.owInventoryReadyAt = performance.now();
    console.log(`OpenWillow Inventory first state rendered in ${Math.round(window.owInventoryReadyAt-startupAt)}ms`);
    timeLog('first_render');
  }
  schedulePaintedLog();
  console.log(`OpenWillow Inventory state: ${state.items.length} items, ${rows.length} backpack`);
}

function restoreFocus(layer, key) {
  if (key.id && key.id !== selectedId) return;
  const nodes = [...layer.querySelectorAll('.cell')];
  const match = key.id ? nodes.find(node => node.dataset.itemId === key.id && node.dataset.kind === key.kind)
    : nodes.find(node => node.dataset.kind === key.kind && node.getAttribute('aria-label') === key.label);
  match?.focus({preventScroll:true});
}

function handleKey(event) {
  if (event.altKey || event.ctrlKey || event.metaKey) return;
  const key = event.key.toLowerCase();
  // The menu grabs the mouse and keyboard the moment it opens, so leaving must work even while the
  // movie is still loading (before ready/state).
  if (!ready || !state) {
    if (key === 'escape' || key === 'i' || key === 'tab') { event.preventDefault(); closeInventory(); }
    return;
  }
  if (key === 'escape' && inspectMode) { event.preventDefault(); toggleInspect(); }
  else if (key === 'escape' && transferSourceId) { event.preventDefault(); finishTransfer(true); syncSelection(); }
  else if (key === 'escape' || key === 'i' || key === 'tab') { event.preventDefault(); closeInventory(); }
  else if (key === 'k') { event.preventDefault(); switchTab(headerTabs[3]); }
  else if (/^[1-4]$/.test(event.key)) { event.preventDefault(); setTargetSlot(Number(event.key)-1); }
  else if (key === 'enter') { event.preventDefault(); equip(); }
  else if (key === 'e') { event.preventDefault(); equip(); }
  else if (key === 'c') { event.preventDefault(); toggleCompare(); }
  else if (key === 'q') { event.preventDefault(); dropSelected(); }
  else if (key === 't') { event.preventDefault(); toggleMark('trash'); }
  else if (key === 'f') { event.preventDefault(); toggleInspect(); }
  else if (key === ']') { event.preventDefault(); changeCategory(1); }
  else if (key === '[') { event.preventDefault(); changeCategory(-1); }
  else if (key === 'v') { event.preventDefault(); toggleMark('favorite'); }
  else if (key === 'delete' || key === 'backspace') { event.preventDefault(); unequip(); }
  else if (key === 'u') { event.preventDefault(); unequip(); }
  else if (key === 's') { event.preventDefault(); changeSort(); }
  else if (key.startsWith('arrow')) { event.preventDefault(); navigateInventory(key.slice(5)); }
  else if (key === 'pageup') { event.preventDefault(); changeSort(1); }
  else if (key === 'pagedown') { event.preventDefault(); changeSort(-1); }
}
window.addEventListener('keydown', handleKey);
let wheelRows = 0;
dragLayer.addEventListener('wheel', event => {
  if (!ready || !state || inspectMode || !event.target.closest('[data-kind="backpack"], [data-kind="backpack-zone"], [data-kind="previous"], [data-kind="next"], [data-kind="category"], [data-kind="sort"]')) return;
  event.preventDefault();
  const amount = event.deltaY * (event.deltaMode === 1 ? 1 : event.deltaMode === 2 ? VISIBLE_ROWS : .01);
  if (Math.sign(amount) !== Math.sign(wheelRows)) wheelRows = 0;
  wheelRows += amount;
  const steps = Math.trunc(wheelRows);
  if (steps) { wheelRows -= steps; scrollBackpack(steps); }
}, {passive:false});
// Letterbox the movie and its hit targets together. Panel composition lives
// in the movie, so the curved-glass art still reaches the viewport edges.
function layoutStage() {
  const scale = Math.min(innerWidth / 1280, innerHeight / 720);
  const width = 1280 * scale, height = 720 * scale;
  Object.assign(document.getElementById('presentation').style, {
    left:`${(innerWidth - width) / 2}px`, top:`${(innerHeight - height) / 2}px`,
    width:`${width}px`, height:`${height}px`
  });
}
layoutStage();
window.addEventListener('resize', () => {
  const started = performance.now();
  layoutStage(); render();
  timeLog('resize', `size=${innerWidth}x${innerHeight} renderMs=${Math.round(performance.now() - started)}`);
});

// CEF exposes the standard gamepad API on supported browsers. Actions are
// edge-triggered so holding a button does not enqueue repeated host requests.
const previousButtons = new Map();
const gamepadActions = {
  0: equip, 1: closeInventory, 2: dropSelected, 3: () => toggleMark('favorite'),
  4: () => cycleTargetSlot(-1), 5: () => cycleTargetSlot(1),
  6: toggleCompare,
  10: () => toggleMark('trash'),
  12: () => navigateInventory('up'), 13: () => navigateInventory('down'),
  14: () => navigateInventory('left'), 15: () => navigateInventory('right')
};
setInterval(() => {
  if (!ready || !navigator.getGamepads) return;
  for (const pad of navigator.getGamepads()) {
    if (!pad) continue;
    let previous = previousButtons.get(pad.index);
    if (!previous) { previous = []; previousButtons.set(pad.index, previous); }
    pad.buttons.forEach((button, index) => {
      const down = Boolean(button?.pressed);
      if (down && !previous[index] && gamepadActions[index]) {
        console.log(`OpenWillow Inventory gamepad button ${index} on pad ${pad.index} (${pad.id})`);
        gamepadActions[index]();
      }
      previous[index] = down;
    });
  }
}, 80);

let enteredInventory = false, movieConfigured = false, stableSince = 0, lastBounds = '';
player.ruffle().load({url:'UI_StatusMenu/harness.swf',base:'UI_StatusMenu/'}).then(() => {
  console.log(`OpenWillow Inventory StatusMenu loaded in ${Math.round(performance.now()-startupAt)}ms`);
  timeLog('statusmenu_loaded');
}).catch(showError);

function readBounds(path) {
  const bounds = call(path, 'getBounds', ROOT);
  if (!bounds || ![bounds.xMin,bounds.yMin,bounds.xMax,bounds.yMax].every(Number.isFinite)) return null;
  return bounds;
}

function frameReady(path) {
  const total = Number(get(path, '_totalframes'));
  return Number.isFinite(total) && total > 0;
}

function configureCompareTween() {
  call(INV, 'SetComparingTweenInfo', ...compareTween);
  for (const tween of comparePanelTweens)
    call(INV, 'SetFocusUnfocusedCompareTweenPositions', ...tween);
}

function prepareMovie() {
  if (!frameReady(INV+'.storagePanel') || !frameReady(INV+'.equippedPanel.cell1')
      || !frameReady(INV+'.mainCard') || !frameReady(INV+'.compareCard')) return false;
  const panel = INV+'.storagePanel';
  // The movie's curved ring is opaque in this runtime. Preserve its own art
  // while exposing the live world behind it, as in the local reference.
  set(ROOT+'.ring', '_alpha', 32);
  set(ROOT+'.scanlines', '_alpha', 9);
  configureCompareTween();
  call(INV, 'ConfigureForPlayer', 0);
  // This is the BL2 trace's Maya portrait path; the movie asset remains local.
  call(INV, 'SetPortrait', '/ package/UI_CharacterPortraits/Siren');
  call(panel, 'SetBackground', 'inventory');
  call(panel, 'SetSortLabel', 'BACKPACK');
  call(INV, 'SetLeftRightArrowVisibility', false, false);
  call(INV, 'TweenCards', false, false);
  call(INV+'.compareCard', 'SetVisible_', false);
  // The converted movie's ammo counters and currency bar hold placeholder 9999/999
  // values; they stay hidden until a snapshot supplies real numbers (applyStatus).
  set(INV+'.ammo', '_visible', false);
  set(INV+'.currencyPanel', '_visible', false);
  call(INV+'.mainCard', 'SetVisible_', false);
  movieConfigured = true;
  console.log(`OpenWillow Inventory movie structure ready in ${Math.round(performance.now()-startupAt)}ms`);
  timeLog('movie_structure');
  return true;
}

// Bounds (movie pixels, rounded) of the clips the hit boxes and thumbnails are laid over.
// '' means the movie has not laid them out yet. The card is left out of the panel
// signature because its height changes with every selected item.
function boundsSignature(paths) {
  const bounds = paths.map(readBounds);
  if (bounds.some(value => !value || value.xMax <= value.xMin || value.yMax <= value.yMin)) return '';
  return JSON.stringify(bounds.map(value => [value.xMin,value.yMin,value.xMax,value.yMax].map(n => Math.round(n*10)/10)));
}
const movieLayoutSignature = () => boundsSignature([INV+'.storagePanel.bkgd', INV+'.equippedPanel.cell1.hitTestClip',
  INV+'.equippedPanel.cell8.hitTestClip']);
const cardSignature = () => boundsSignature([INV+'.mainCard.bkgd', INV+'.compareCard.bkgd']);

// After the first render the movie can still move: its opening tween, the compare-card
// tween and any later re-layout shift the panels, and getBounds taken mid-tween leaves
// the hit boxes and thumbnails behind. Poll the layout, and once it has held still for
// LAYOUT_SETTLE_MS re-render against the new positions.
function watchLayout() {
  if (!ready || !state) return;
  try {
    applyMovieVisibility();
    const signature = movieLayoutSignature();
    if (signature && signature !== renderedLayout) {
      if (signature !== pendingLayout) { pendingLayout = signature; pendingSince = performance.now(); }
      else if (performance.now() - pendingSince >= LAYOUT_SETTLE_MS) render();
    } else {
      pendingLayout = '';
    }
    const card = cardSignature();
    if (card && card !== renderedCard) { renderedCard = card; updateInspect(); applyCardOcclusion(); }
    if (headerPending) refreshBackpackHeader();
  } catch (error) {
    console.error('OpenWillow Inventory layout watch:', error);
  }
}
setInterval(watchLayout, LAYOUT_POLL_MS);

const bootMarks = new Set();
function bootMark(name) { if (!bootMarks.has(name)) { bootMarks.add(name); timeLog(name); } }
function resourceTimes() {
  return performance.getEntriesByType('resource').filter(entry => entry.duration >= 50)
    .map(entry => `${entry.name.split('/').pop()}:${Math.round(entry.startTime)}+${Math.round(entry.duration)}`).join(',');
}
let prepareAttempts = 0;
function pollReady() {
  if (typeof player.ow !== 'function') { requestAnimationFrame(pollReady); return; }
  bootMark('ruffle_api');
  const total = Number(get(ROOT, '_totalframes'));
  if (!total || Number(get(ROOT, '_framesloaded')) < total) { requestAnimationFrame(pollReady); return; }
  if (!enteredInventory) {
    timeLog('frames_loaded', `total=${total} resources=${resourceTimes()}`);
    enteredInventory = true;
    player.ow(ROOT, 'gotoAndStop', 'inventory');
    requestAnimationFrame(pollReady);
    return;
  }
  if (!movieConfigured) {
    ++prepareAttempts;
    if (!prepareMovie()) { requestAnimationFrame(pollReady); return; }
    timeLog('prepare_attempts', `n=${prepareAttempts} resources=${resourceTimes()}`);
    requestAnimationFrame(pollReady);
    return;
  }
  const signature = movieLayoutSignature() + cardSignature();
  const now = performance.now();
  if (!signature || signature !== lastBounds) stableSince = now;
  lastBounds = signature;
  // Follow the actual opening tween: draw once the layout has held still for READY_SETTLE_MS
  // of wall-clock time (two identical animation frames are not enough, the tween can stall
  // between frames). watchLayout() corrects anything that still moves afterwards.
  if (signature && now - stableSince >= READY_SETTLE_MS) {
    // Apply after the opening tween settles, before deriving HTML hit bounds.
    for (const path of [INV, ROOT+'.header', ROOT+'.tooltips']) {
      set(path, '_x', Number(get(path, '_x')) * COMPOSITION_SCALE + 100);
      set(path, '_y', Number(get(path, '_y')) * COMPOSITION_SCALE);
      set(path, '_xscale', Number(get(path, '_xscale')) * COMPOSITION_SCALE);
      set(path, '_yscale', Number(get(path, '_yscale')) * COMPOSITION_SCALE);
    }
    ready = true;
    window.owInventoryMovieReady = true;
    window.owInventoryMovieReadyAt = performance.now();
    render();
    console.log(`OpenWillow Inventory movie ready in ${Math.round(window.owInventoryMovieReadyAt-startupAt)}ms`);
    timeLog('movie_ready');
    return;
  }
  requestAnimationFrame(pollReady);
}
requestAnimationFrame(pollReady);

function showError(error) {
  document.getElementById('loading').textContent = 'Inventory unavailable. Press Esc to close.';
  console.error('OpenWillow Inventory:', error);
}
// The movie library has been observed to take 15-65 s to initialize on a busy machine.
setTimeout(() => { if (!ready) showError('movie library did not initialize'); }, 120000);
