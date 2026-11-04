// Pure rules for the vending page (vending.html / vending.js): state checks, list building, sorting, price text,
// affordability, selection movement and the request the page sends back to the host. No DOM here, so node tests
// (tests/vending_test.js) can cover it. Browser: window.OwVendingModel. Node: module.exports.
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.OwVendingModel = api;
})(typeof window !== 'undefined' ? window : globalThis, function () {
  // Machine kinds follow the host's classification (tools/prepare_vendors.py): weapons, items (ammo and grenades),
  // health, and the black market (listed, not stocked yet).
  const MACHINE_KINDS = ['weapons', 'items', 'health', 'blackmarket'];
  // Row kinds the host sends (OpenWillowVendor.cpp): guns, the item rows (ammo, health), shields and grenade mods, and gear
  // the page lists as plain items or class mods.
  const ITEM_KINDS = ['weapon', 'item', 'ammo', 'health', 'shield', 'grenade_mod', 'class_mod'];
  const ACTIONS = ['buy', 'sell', 'buyback', 'close'];
  const COLUMNS = ['shop', 'backpack', 'buyback'];
  // Host choice (UNVERIFIED): the buy-back list keeps at most this many sold items.
  const BUYBACK_LIMIT = 10;

  const isInt = (value, min = 0) => Number.isInteger(value) && value >= min;
  const isText = value => typeof value === 'string' && value.length > 0;
  const isObject = value => value !== null && typeof value === 'object' && !Array.isArray(value);
  const isNumber = value => typeof value === 'number' && Number.isFinite(value) && value >= 0;
  // Ammo and health rows do not go into the backpack, so a full backpack does not stop them.
  const entersBackpack = kind => kind !== 'ammo' && kind !== 'health';

  // "$1,234": the price text the page shows.
  function formatPrice(amount) {
    return '$' + String(Math.max(0, Math.trunc(amount))).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  }

  // "mm:ss" for the restock countdown; negative values read as zero.
  function formatCountdown(seconds) {
    const total = Math.max(0, Math.floor(seconds));
    return String(Math.floor(total / 60)).padStart(2, '0') + ':' + String(total % 60).padStart(2, '0');
  }

  // Returns an error string for one item, or null. `priceField` is 'sell' for backpack items, 'price' otherwise.
  function itemError(item, where, priceField) {
    if (!isObject(item)) return `${where} is not an object`;
    if (!isText(item.id)) return `${where}.id must be a non-empty string`;
    if (!isText(item.name)) return `${where}.name must be a non-empty string`;
    if (!ITEM_KINDS.includes(item.kind)) return `${where}.kind must be one of ${ITEM_KINDS.join(', ')}`;
    if (!isInt(item.rarity, 1) || item.rarity > 5) return `${where}.rarity must be an integer 1-5`;
    if (!isInt(item[priceField])) return `${where}.${priceField} must be a non-negative integer`;
    if (item.level !== undefined && !isInt(item.level, 1)) return `${where}.level must be an integer of at least 1`;
    if (item.card !== undefined && !isObject(item.card)) return `${where}.card must be an object when present`;
    return null;
  }

  // Returns {ok:true, state} or {ok:false, error}. Rejects anything the page cannot show or act on safely.
  function validateState(state) {
    const fail = error => ({ok: false, error});
    if (!isObject(state)) return fail('state is not an object');
    if (!isInt(state.serial)) return fail('serial must be a non-negative integer');
    if (typeof state.open !== 'boolean') return fail('open must be a boolean');
    if (!isObject(state.machine) || !isText(state.machine.id)) return fail('machine.id must be a non-empty string');
    if (!MACHINE_KINDS.includes(state.machine.kind)) return fail('machine.kind is not a known kind');
    if (typeof state.machine.title !== 'string') return fail('machine.title must be a string');
    for (const key of ['money', 'eridium', 'capacity', 'used', 'resetSeconds']) {
      if (!isInt(state[key])) return fail(`${key} must be a non-negative integer`);
    }
    if (state.used > state.capacity) return fail('used is larger than capacity');
    if (typeof state.message !== 'string') return fail('message must be a string');
    // The ammo pools and the health (host numbers; health is a float on the host, so it is only checked as a number).
    if (state.ammo !== undefined) {
      if (!isObject(state.ammo)) return fail('ammo must be an object');
      for (const key of Object.keys(state.ammo)) {
        const pool = state.ammo[key];
        if (!isObject(pool) || !isInt(pool.current) || !isInt(pool.max)) return fail(`ammo.${key} needs current and max`);
      }
    }
    if (state.health !== undefined && (!isObject(state.health) || !isNumber(state.health.current) || !isNumber(state.health.max))) {
      return fail('health needs current and max');
    }
    if (!Array.isArray(state.shop)) return fail('shop must be an array');
    if (!Array.isArray(state.backpack)) return fail('backpack must be an array');
    if (!Array.isArray(state.buyback)) return fail('buyback must be an array');
    if (state.featured !== null && state.featured !== undefined) {
      const error = itemError(state.featured, 'featured', 'price');
      if (error) return fail(error);
      if (typeof state.featured.affordable !== 'boolean') return fail('featured.affordable must be a boolean');
    }
    for (let i = 0; i < state.shop.length; i++) {
      const error = itemError(state.shop[i], `shop[${i}]`, 'price');
      if (error) return fail(error);
      if (typeof state.shop[i].affordable !== 'boolean') return fail(`shop[${i}].affordable must be a boolean`);
    }
    for (let i = 0; i < state.buyback.length; i++) {
      const error = itemError(state.buyback[i], `buyback[${i}]`, 'price');
      if (error) return fail(error);
      if (typeof state.buyback[i].affordable !== 'boolean') return fail(`buyback[${i}].affordable must be a boolean`);
    }
    for (let i = 0; i < state.backpack.length; i++) {
      const error = itemError(state.backpack[i], `backpack[${i}]`, 'sell');
      if (error) return fail(error);
      if (typeof state.backpack[i].equipped !== 'boolean') return fail(`backpack[${i}].equipped must be a boolean`);
    }
    return {ok: true, state};
  }

  // Rarity high to low, then name; Array.prototype.sort is stable, so equal rows keep the host's order.
  function sortRows(rows) {
    return rows.slice().sort((a, b) => (b.rarity - a.rarity) || (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  }

  function row(item, source, featured) {
    return {source, id: item.id, name: item.name, kind: item.kind, rarity: item.rarity, level: item.level || 0,
      price: item[source === 'backpack' ? 'sell' : 'price'], priceValid: item.priceValid !== false,
      affordable: item.affordable === true && item.priceValid !== false,
      equipped: item.equipped === true, sellable: item.sellable !== false, featured, card: item.card || null,
      ammoType: item.ammoType || null};
  }

  // The line under the detail for what the row does to the player: the ammo pool it fills, or the health it heals.
  // Empty for other rows.
  function poolNote(state, item) {
    if (item.kind === 'ammo' && state.ammo && state.ammo[item.ammoType]) {
      const pool = state.ammo[item.ammoType];
      return `${pool.current}/${pool.max} rounds in the pool`;
    }
    if (item.kind === 'health' && state.health) return `Health ${Math.floor(state.health.current)}/${Math.floor(state.health.max)}`;
    return '';
  }

  // The three lists the page shows. Shop: the item of the day first, then the stock in the host's order (an item
  // that is also the featured one is listed once). Backpack and buy-back are sorted by rarity, then name.
  function buildLists(state) {
    const shop = [];
    if (state.featured) shop.push(row(state.featured, 'shop', true));
    for (const item of state.shop) {
      if (!state.featured || item.id !== state.featured.id) shop.push(row(item, 'shop', false));
    }
    return {
      shop,
      backpack: sortRows(state.backpack.map(item => row(item, 'backpack', false))),
      buyback: sortRows(state.buyback.slice(0, BUYBACK_LIMIT).map(item => row(item, 'buyback', false)))
    };
  }

  // Selection lengths for movement and clamping: {shop, backpack, buyback}.
  function lengths(lists) {
    return {shop: lists.shop.length, backpack: lists.backpack.length, buyback: lists.buyback.length};
  }

  // Moves {column, index}. Up/Down wrap inside the list; Tab cycles the columns; Left/Right step one column and stop
  // at the ends. The index is clamped to the new list.
  function moveSelection(sel, key, counts) {
    const column = COLUMNS.indexOf(sel.column);
    const length = counts[sel.column];
    if (key === 'up' || key === 'down') {
      if (length === 0) return {column: sel.column, index: 0};
      const step = key === 'down' ? 1 : -1;
      return {column: sel.column, index: (sel.index + step + length) % length};
    }
    let next = column;
    if (key === 'tab') next = (column + 1) % COLUMNS.length;
    else if (key === 'left') next = Math.max(0, column - 1);
    else if (key === 'right') next = Math.min(COLUMNS.length - 1, column + 1);
    else return sel;
    const name = COLUMNS[next];
    return {column: name, index: clampIndex(sel.index, counts[name])};
  }

  function clampIndex(index, length) {
    if (length === 0) return 0;
    return Math.min(Math.max(0, index), length - 1);
  }

  // What Enter or E does with the selected row. Returns {ok:true, action, id} to send, or {ok:false, message}
  // for the page to show. Unaffordable buys, equipped sales and unsellable (mission) items are never sent.
  function actionFor(sel, lists, state) {
    const list = lists[sel.column] || [];
    const item = list[sel.index];
    if (!item) return {ok: false, message: 'Nothing selected'};
    if (sel.column === 'backpack') {
      if (item.equipped) return {ok: false, message: 'Equipped items cannot be sold'};
      // The host marks items it will not buy (a weapon the mission lends); the page does not ask.
      if (!item.sellable) return {ok: false, message: 'Mission items cannot be sold'};
      return {ok: true, action: 'sell', id: item.id};
    }
    if (state.used >= state.capacity && entersBackpack(item.kind)) return {ok: false, message: 'Backpack full'};
    if (!item.priceValid) return {ok: false, message: 'Not for sale'};
    if (!item.affordable) return {ok: false, message: 'Insufficient funds'};
    return {ok: true, action: sel.column === 'buyback' ? 'buyback' : 'buy', id: item.id};
  }

  // The line the page logs for the host: 'OWVEND {"serial":n,"action":"buy","id":"..."}'. Close has id null.
  function buildRequest(serial, action, id) {
    if (!ACTIONS.includes(action)) throw new RangeError(`unknown action ${action}`);
    if (action !== 'close' && !isText(id)) throw new RangeError(`${action} needs an item id`);
    return 'OWVEND ' + JSON.stringify({serial, action, id: action === 'close' ? null : id});
  }

  // Keyboard names to commands; anything else is ignored by the page.
  const KEY_COMMANDS = {
    ArrowUp: 'up', w: 'up', W: 'up', ArrowDown: 'down', s: 'down', S: 'down',
    ArrowLeft: 'left', ArrowRight: 'right', Tab: 'tab', Enter: 'act', e: 'act', E: 'act', Escape: 'close'
  };
  function keyCommand(key) {
    return Object.prototype.hasOwnProperty.call(KEY_COMMANDS, key) ? KEY_COMMANDS[key] : null;
  }

  return {MACHINE_KINDS, ITEM_KINDS, ACTIONS, COLUMNS, BUYBACK_LIMIT, formatPrice, formatCountdown, validateState,
    sortRows, buildLists, lengths, moveSelection, clampIndex, actionFor, buildRequest, keyCommand, poolNote, entersBackpack};
});
