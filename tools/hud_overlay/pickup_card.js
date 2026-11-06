// The world pickup card: the item card BL2 shows while the player looks at a dropped item.
// In the game it is its own movie (ItemPickupGFxMovie, UI_Inworld.Weapon_Card; the shared "item card" clip at
// inventory.card1), rendered to a texture that a particle (fx_shared_items Part_ItemCard, material MatItemCard) draws at
// the item, so it floats in the world. The host has no such particle: this page draws the converted movie in its own
// Ruffle player over the HUD and places it from the item's screen point. Card size and placement are host fits
// (UNVERIFIED: the particle's size and offset are not read). The card is filled by item_card.js, the code the inventory
// page uses, from the host's "pickupCard" field (OpenWillowPickupCard.cpp): {id, x, y, level, item, compare} or null.
(function (root) {
  'use strict';
  const CARD = '_level1.inventory.card1';
  // Card height on screen as a fraction of the viewport height, and the gap between the item and the card's bottom
  // edge. UNVERIFIED host fits, to be measured in a real-game session.
  const CARD_SCREEN_HEIGHT = 0.3, CARD_GAP = 0.03;

  // Where the card goes for an item at viewport fraction (x, y), for a card `aspect` (width / height) wide: centred over
  // the item, its bottom CARD_GAP above it, kept inside the viewport. Rectangle {left, top, width, height} in the units
  // of viewWidth and viewHeight.
  function placement(x, y, viewWidth, viewHeight, aspect) {
    const height = CARD_SCREEN_HEIGHT * viewHeight, width = height * aspect;
    const left = Math.min(Math.max(x * viewWidth - width / 2, 0), Math.max(viewWidth - width, 0));
    const top = Math.min(Math.max(y * viewHeight - CARD_GAP * viewHeight - height, 0), Math.max(viewHeight - height, 0));
    return {left, top, width, height};
  }

  // What a new state asks of the card: 'hide', 'show' (same item, maybe moved) or 'fill' (another item or other data).
  function plan(previous, next) {
    if (!next || !next.item) return 'hide';
    if (!previous || !previous.item) return 'fill';
    const same = previous.id === next.id && JSON.stringify(previous.item) === JSON.stringify(next.item)
      && JSON.stringify(previous.compare || null) === JSON.stringify(next.compare || null) && previous.level === next.level;
    return same ? 'show' : 'fill';
  }

  // The card runs in its own same-origin iframe (pickup_card.html), so its Ruffle canvas is not the one hud_warp.js reads:
  // the HUD's warp patches canvas contexts in the HUD document and redraws the newest one in place of the HUD movie. In
  // the HUD page, apply() only forwards the state; in the frame, it drives the movie.
  const inFrame = /\/pickup_card\.html$/.test((root.location && root.location.pathname) || '');

  // HUD page side.
  let frame = null, frameLoaded = false, forwarded;
  function forward(card) {
    forwarded = card || null;
    if (!frame) {
      if (!forwarded) return; // the frame (and its movie) is made only once there is a card to show
      const doc = root.document;
      frame = doc.createElement('iframe');
      frame.src = 'pickup_card.html';
      frame.setAttribute('allowtransparency', 'true');
      frame.style.cssText = 'position:fixed;left:0;top:0;width:100%;height:100%;border:0;background:transparent;pointer-events:none';
      frame.addEventListener('load', () => { frameLoaded = true; forward(forwarded); });
      doc.body.appendChild(frame);
      return;
    }
    if (frameLoaded && frame.contentWindow && frame.contentWindow.owPickupCard) frame.contentWindow.owPickupCard.apply(forwarded);
  }

  // Frame side. The harness stage is 1280x720 and fills the frame (as the HUD's fills its page), so placement works in
  // those movie units and moves the loaded movie (_level1) itself; the card shows and hides through the movie's own
  // TurnOnPickupCard / TurnOffPickupCard, which play its "in" and "out" frames (the game's timing around them is native,
  // UNVERIFIED).
  const MOVIE_WIDTH = 1280, MOVIE_HEIGHT = 720;
  const READY_TIMEOUT_MS = 15000; // give up waiting for the card clip after this long (logged)
  let player = null, writer = null, api = null, ready = false, wanted = null, shown = null, noMovie = false;

  function start(doc) {
    if (player || noMovie || !root.RufflePlayer || !root.owItemCard) return;
    player = root.RufflePlayer.newest().createPlayer();
    player.style.cssText = 'position:fixed;left:0;top:0;width:100%;height:100%;display:block';
    doc.body.appendChild(player);
    api = {
      call:(path, method, ...args) => player.ow(path, 'apply', method, args),
      get:(path, member) => player.ow(path, 'get', member),
      set:(path, member, value) => player.ow(path, 'set', member, value)
    };
    // Without the movie there is no card, and it is not tried again. The rest of the game is not affected.
    const gone = () => { noMovie = true; ready = false; shown = null; player = null; console.log('OpenWillow pickup card off: no movie'); };
    root.owMovieGuard.load(player, {url:'UI_Inworld/harness.swf', base:'UI_Inworld/'}, 'pickup card', ['UI_Inworld/Weapon_Card.swf'], gone).then(found => {
      if (!found) { gone(); return; }
      const giveUpAt = Date.now() + READY_TIMEOUT_MS;
      const poll = () => {
        let name = null;
        try { name = typeof player.ow === 'function' ? api.get(CARD, '_name') : null; } catch (e) { name = null; }
        if (!name && Date.now() > giveUpAt) {
          console.log(`OpenWillow pickup card movie failed: no ${CARD} after ${READY_TIMEOUT_MS / 1000} s (is UI_Inworld/Weapon_Card.swf converted?)`);
          return;
        }
        if (!name) { setTimeout(poll, 100); return; }
        writer = root.owItemCard.createCardWriter(api);
        ready = true;
        console.log('OpenWillow pickup card movie ready');
        drive(wanted);
      };
      poll();
    });
  }

  function drive(card) {
    wanted = card || null;
    if (!player) start(root.document);
    if (!ready) return;
    const action = plan(shown, wanted);
    if (action === 'hide') {
      if (shown) {
        api.call('_level1', 'TurnOffPickupCard');
        console.log('OpenWillow pickup card hidden');
      }
      shown = null;
      return;
    }
    if (action === 'fill') {
      // The pickup card's frame style is not observed; 'default' is the card background's first frame (UNVERIFIED).
      writer.configureCard(CARD, wanted.item, wanted.compare || null, 'default', false, wanted.level);
      if (!shown) api.call('_level1', 'TurnOnPickupCard');
      console.log(`OpenWillow pickup card shown ${wanted.id} "${wanted.item.name}" compare=${wanted.compare ? wanted.compare.id : 'none'}`);
    }
    // The card background's bounds in the loaded movie's space (they follow the card's height; the card clip's own bounds
    // include hidden rows and glows, as inventory.js notes for its fits). Measured with _level1 unscaled.
    api.set('_level1', '_xscale', 100);
    api.set('_level1', '_yscale', 100);
    api.set('_level1', '_x', 0);
    api.set('_level1', '_y', 0);
    const bounds = api.call(`${CARD}.bkgd`, 'getBounds', '_level1');
    const cardWidth = bounds.xMax - bounds.xMin, cardHeight = bounds.yMax - bounds.yMin;
    if (cardWidth > 0 && cardHeight > 0) {
      const box = placement(Number(wanted.x), Number(wanted.y), MOVIE_WIDTH, MOVIE_HEIGHT, cardWidth / cardHeight);
      const scale = box.height / cardHeight;
      api.set('_level1', '_xscale', scale * 100);
      api.set('_level1', '_yscale', scale * 100);
      api.set('_level1', '_x', box.left - bounds.xMin * scale);
      api.set('_level1', '_y', box.top - bounds.yMin * scale);
    }
    shown = wanted;
  }

  // The HUD state's "pickupCard" field (index.html): forwarded to the frame, which draws it.
  function apply(card) {
    if (inFrame) drive(card);
    else forward(card);
  }

  root.owPickupCard = {apply, placement, plan, CARD};
})(typeof window !== 'undefined' ? window : globalThis);
