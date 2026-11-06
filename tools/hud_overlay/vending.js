// DOM and Ruffle glue for the vending page. The rules (validation, lists, selection, requests) are in vending_model.js.
// The host calls window.owVending(state) (shape in local/p2/vendors/spec.md, section B) and reads the page's
// `OWVEND {...}` console lines. The page never changes money itself; it waits for the next state.
const model = window.OwVendingModel;
const $ = id => document.getElementById(id);
const COLUMN_LISTS = {shop: 'list-shop', backpack: 'list-backpack', buyback: 'list-buyback'};
const KIND_LABEL = {weapon: 'Weapon', item: 'Item', ammo: 'Ammo', health: 'Health', shield: 'Shield',
  grenade_mod: 'Grenade mod', class_mod: 'Class mod'};

let state = null, lists = {shop: [], backpack: [], buyback: []};
let sel = {column: 'shop', index: 0};
let lastSerial = -1, localMessage = '', stateError = '', lastRequest = '', movieState = 'loading';

// Stock movie as frame art. Any failure leaves the plain HTML panel, which works on its own, and removes the player:
// Ruffle shows its own "Something went wrong" panel for a missing or broken movie, and its load() still resolves.
const MOVIE_URL = 'UI_VendingMachine/VendingMachineInterface.swf';
function dropMovie(reason) {
  movieState = reason;
  if (window.owPlayer) window.owPlayer.remove();
  window.owPlayer = null;
}
// Ruffle marks a failed movie by showing its #panic element (seen in the browser with a missing file).
function watchForPanic(player, checksLeft) {
  const panic = player.shadowRoot && player.shadowRoot.querySelector('#panic');
  if (panic && !panic.classList.contains('hidden')) { dropMovie('failed'); return; }
  if (checksLeft > 0) setTimeout(() => watchForPanic(player, checksLeft - 1), 500);
}
fetch(MOVIE_URL, {method: 'HEAD'}).then(response => {
  if (!response.ok) { movieState = 'missing'; return; } // no player at all: the HTML panel only
  const player = window.RufflePlayer.newest().createPlayer();
  $('stage').appendChild(player);
  window.owPlayer = player;
  player.ruffle().load({url: MOVIE_URL, base: 'UI_VendingMachine/'})
    .then(() => { movieState = 'loaded'; watchForPanic(player, 20); }, () => dropMovie('failed'));
}).catch(() => { movieState = 'missing'; });

function send(action, id) {
  const line = model.buildRequest(state ? state.serial : 0, action, id);
  lastRequest = line;
  console.log(line);
}

function act() {
  if (!state) return;
  const result = model.actionFor(sel, lists, state);
  localMessage = result.ok ? '' : result.message;
  if (result.ok) send(result.action, result.id);
  render();
}

function close() {
  send('close', null);
}

// Text for one row; the host's name and numbers go in through textContent only.
function rowElement(item, column, index) {
  const el = document.createElement('div');
  el.className = `row r${item.rarity}`;
  if (sel.column === column && sel.index === index) el.classList.add('selected');
  if (column === 'shop' && !item.affordable) el.classList.add('dim');
  el.setAttribute('role', 'option');
  const name = document.createElement('span');
  name.textContent = `${item.name}${item.level ? ` (L${item.level})` : ''}${item.equipped ? ' [equipped]' : ''}`;
  const price = document.createElement('span');
  price.className = 'price';
  price.textContent = model.formatPrice(item.price);
  el.append(name, price);
  el.addEventListener('click', () => { sel = {column, index}; localMessage = ''; render(); });
  el.addEventListener('dblclick', () => { sel = {column, index}; act(); });
  return el;
}

function renderList(column, rows, emptyText) {
  const box = $(COLUMN_LISTS[column]);
  box.replaceChildren();
  if (rows.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'empty';
    empty.textContent = emptyText;
    box.appendChild(empty);
    return;
  }
  rows.forEach((item, index) => box.appendChild(rowElement(item, column, index)));
}

function renderFeatured() {
  const box = $('featured-row');
  $('featured').hidden = !(state && state.featured);
  box.replaceChildren();
  if (!state || !state.featured) return;
  box.appendChild(rowElement(lists.shop[0], 'shop', 0));
}

function render() {
  if (!state) return;
  $('panel').hidden = !state.open;
  $('title').textContent = state.machine.title || 'Vending machine';
  $('money').textContent = model.formatPrice(state.money);
  $('eridium').textContent = String(state.eridium);
  $('used').textContent = `${state.used}/${state.capacity}`;
  sel = {column: sel.column, index: model.clampIndex(sel.index, model.lengths(lists)[sel.column])};
  renderFeatured();
  renderList('shop', lists.shop, state.machine.kind === 'blackmarket' ? 'Not stocked yet' : 'Nothing in stock');
  renderList('backpack', lists.backpack, 'Backpack is empty');
  renderList('buyback', lists.buyback, 'Nothing to buy back');
  const picked = lists[sel.column][sel.index];
  const note = picked ? model.poolNote(state, picked) : '';
  $('detail').textContent = picked ? `${picked.name}: ${KIND_LABEL[picked.kind] || picked.kind}, rarity ${picked.rarity}`
    + (note ? `. ${note}` : '') : '';
  $('message').textContent = stateError || model.refusalLabel(localMessage || state.message);
  tickCountdown();
}

// The seconds to restock come from the host's world time: the host pushes the state every second while the shop is
// open, so the page shows that number as it is (no local clock, which would drift from a paused or slowed world).
function tickCountdown() {
  if (!state) return;
  $('countdown').textContent = model.formatCountdown(state.resetSeconds);
}

window.owVending = raw => {
  const result = model.validateState(raw);
  if (!result.ok) {
    console.log('OWVENDERR ' + JSON.stringify({error: result.error}));
    stateError = 'Bad state from host: ' + result.error;
    if (state) render();
    return;
  }
  if (result.state.serial < lastSerial) return; // an older state arrived late
  // The host pushes a state every second, so only a new opening clears the page's own refusal; moving or acting does too.
  if (model.isNewOpening(state, result.state)) localMessage = '';
  sel = model.selectionAfterState(sel, state, result.state);
  stateError = '';
  state = result.state;
  lastSerial = state.serial;
  lists = model.buildLists(state);
  render();
};

window.owVendingReport = () => {
  console.log('OWVENDPAGE ' + JSON.stringify({serial: state ? state.serial : null, movie: movieState,
    selection: sel, lengths: model.lengths(lists), lastRequest}));
};

document.addEventListener('keydown', event => {
  const command = model.keyCommand(event.key);
  if (!command) return;
  event.preventDefault();
  if (command === 'act') act();
  else if (command === 'close') close();
  else {
    sel = model.moveSelection(sel, command, model.lengths(lists));
    localMessage = '';
    render();
  }
});

// Tells the host the page can take input (it keeps game input until this line arrives; OpenWillowVendorPage.cpp).
console.log('OWVENDREADY');
