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
// The movie's feedback clip. Every request carries a number (`request`) and the host's state names the last one it
// answered (`handled`), so a result counts only for the request it answers. In movie mode one request waits at a time: a
// press while one waits is ignored, and a request with no answer is let go after PENDING_TIMEOUT_MS (host choice), with one
// log line; its late answer is then ignored, also logged.
// The numbers start at a random base on each page load, so a reloaded page never reuses a number the host may still echo.
// (Random rather than the time: the page keeps no clock of its own.) Up to 2^50 plus the count stays a safe integer.
let pending = null, pendingTimer = null, busyLogged = false, fieldsLogged = false;
let requestCount = window.owVendingRequestBase !== undefined ? window.owVendingRequestBase : Math.floor(Math.random() * 2 ** 40) * 1024;
const expired = new Set();
const PENDING_TIMEOUT_MS = window.owVendingPendingTimeoutMs || 3000;
function clearPending() {
  pending = null;
  busyLogged = false;
  if (pendingTimer !== null) clearTimeout(pendingTimer);
  pendingTimer = null;
}

// BL2's own shop movie (vending_movie.js) draws the page when both converted movies run. Without them, or when Ruffle fails
// later, the plain HTML panel below is the page (the missing-UI-movie rule; movie_guard.js removes a failed player). The
// keys, rules and requests are the same either way. One line says which page ran.
const movie = window.owVendingMovie;
function usePage(mode) {
  if (mode === movieState) return;
  movieState = mode;
  if (state) {
    // The movie groups rows by section and the HTML page does not: keep the selected row (by id), not its index.
    const next = mode === 'movie' ? movie.groupLists(model.buildLists(state)) : model.buildLists(state);
    sel = model.remapSelection(sel, lists, next);
    lists = next;
  }
  document.body.classList.toggle('movie', mode === 'movie');
  console.log(`OWVENDPAGEMODE ${mode === 'movie' ? 'movie' : 'html'}`);
  render();
}
window.owVendingMovieLost = () => usePage('html');
movie.start($('stage'), $('controls'), (picked, andAct) => {
  sel = picked;
  localMessage = '';
  if (andAct) act(); else render();
}).then(running => usePage(running ? 'movie' : 'html'), () => usePage('html'));

function send(action, id) {
  const request = ++requestCount;
  const line = model.buildRequest(state ? state.serial : 0, action, id, request);
  lastRequest = line;
  clearPending();
  if (action !== 'close') {
    const waiting = pending = {action, serial: state ? state.serial : 0, request};
    pendingTimer = setTimeout(() => {
      if (pending !== waiting) return;
      console.log(`OWVENDPENDING no answer to ${action} (request ${request}) after ${PENDING_TIMEOUT_MS} ms; the page takes presses again`);
      expired.add(request);
      clearPending();
    }, PENDING_TIMEOUT_MS);
  }
  console.log(line);
}

function act() {
  if (!state) return;
  if (movieState === 'movie' && pending) {
    if (!busyLogged) console.log(`OWVENDBUSY ${pending.action} waits for its answer; press ignored`);
    busyLogged = true;
    return;
  }
  const result = model.actionFor(sel, lists, state);
  localMessage = result.ok ? '' : result.message;
  if (result.ok) send(result.action, result.id);
  render();
  // A press the page refuses itself plays the refusal at once, each time.
  if (!result.ok && movieState === 'movie') movie.feedback(movie.feedbackFor(null, null, null, model.refusalLabel(result.message), ''));
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
  const message = stateError || model.refusalLabel(localMessage || state.message);
  $('message').textContent = message;
  tickCountdown();
  if (movieState === 'movie') movie.render({state, lists, sel});
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
  // The answer to the waiting request: only a state that names it (`handled`). A refusal plays bad (the `refusal` field, never
  // the stock notice in `message`); a buy or sale plays its result once the money moved. A host without the two fields gives
  // no host results in the clip.
  const handled = result.state.handled;
  let done = null;
  if (handled === undefined || result.state.refusal === undefined) {
    if (!fieldsLogged) console.log('OWVENDNOANSWER the host state has no handled/refusal fields; the clip plays no host results');
    fieldsLogged = true;
  } else if (pending && handled === pending.request) {
    done = result.state.refusal ? movie.feedbackFor(null, null, null, model.refusalLabel(result.state.refusal), '')
      : movie.feedbackFor(pending, state, result.state, '', '');
    clearPending();
  } else if (expired.has(handled)) {
    expired.delete(handled);
    console.log(`OWVENDLATE the answer to request ${handled} came after its timeout; ignored`);
  }
  if (model.isNewOpening(state, result.state)) clearPending();
  state = result.state;
  lastSerial = state.serial;
  lists = model.buildLists(state);
  if (movieState === 'movie') lists = movie.groupLists(lists); // one header per section, in the order on screen
  render();
  if (done && movieState === 'movie') movie.feedback(done);
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
    sel = model.moveSelection(sel, command, model.lengths(lists), movieState === 'movie' ? movie.TAB_ORDER : model.COLUMNS);
    localMessage = '';
    render();
  }
});

// Tells the host the page can take input (it keeps game input until this line arrives; OpenWillowVendorPage.cpp).
console.log('OWVENDREADY');
