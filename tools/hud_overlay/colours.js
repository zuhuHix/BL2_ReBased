// The game's UI colours for the overlay pages, kept out of the repository (DECISIONS.md 2026-11-08).
// serve.py answers /game_colours.js with `window.OW_GAME_COLOURS = {...}` from the ignored
// local/ui/colours/game_colours.json (tools/game_colours.py writes it), or with null when the file is missing.
// Without it every key falls back to a neutral grey below (not the game's) and the page logs that once.
// Load order on a page: game_colours.js, then this file, then the page's scripts. Works in Node for the tests:
// set globalThis.OW_GAME_COLOURS before calling. Keep the keys in step with NEUTRAL in tools/game_colours.py.
(function (root) {
  'use strict';
  const NEUTRAL = {
    rarity_1: '#FFFFFF', rarity_2: '#E0E0E0', rarity_3: '#C0C0C0', rarity_4: '#A0A0A0', rarity_5: '#808080',
    card_text: '#D0D0D0', card_projectile_count: '#B0B0B0', card_red_text: '#909090', hint_disabled: '#707070',
    list_header_text: '#F0F0F0', inventory_band: '#909090', inventory_tile_fill: '#A0A0A0', inventory_tile_frame: '#B0B0B0',
    skills_text: '#C8C8C8', skill_current: '#B0B0B0', skill_maxed: '#E0E0E0', skill_locked: '#808080',
    skill_class_mod: '#F0F0F0', skill_tag: '#D8D8D8'
  };
  const HEX = /^#[0-9a-f]{6}$/i;
  let warned = false;
  function table() {
    const local = root.OW_GAME_COLOURS;
    if (local && typeof local === 'object') return local;
    if (!warned) {
      warned = true;
      const log = root.console && (root.console.warn || root.console.log);
      if (log) log.call(root.console, 'OpenWillow game colours missing (local/ui/colours/game_colours.json); using neutral greys');
    }
    return {};
  }
  // '#rrggbb' (lower case, as the Flash markup uses) for a key.
  function hex(key) {
    if (!(key in NEUTRAL)) throw new Error(`unknown colour key ${key}`);
    const value = table()[key];
    return (typeof value === 'string' && HEX.test(value) ? value : NEUTRAL[key]).toLowerCase();
  }
  // The same colour as a number, for Flash tints (0xRRGGBB).
  function tint(key) { return Number.parseInt(hex(key).slice(1), 16); }
  // Every key as a CSS custom property (--ow-<key-with-dashes>) on the page, for its style sheet.
  function applyCss(doc) {
    if (!doc || !doc.documentElement || !doc.documentElement.style) return;
    for (const key of Object.keys(NEUTRAL)) doc.documentElement.style.setProperty(`--ow-${key.replace(/_/g, '-')}`, hex(key));
  }
  const api = {hex, tint, applyCss, keys: Object.keys(NEUTRAL), neutral: key => NEUTRAL[key].toLowerCase(),
    loaded: () => Boolean(root.OW_GAME_COLOURS && typeof root.OW_GAME_COLOURS === 'object')};
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else {
    root.owColours = api;
    if (root.document) applyCss(root.document);
  }
})(typeof window !== 'undefined' ? window : globalThis);
