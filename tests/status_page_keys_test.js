// M on the Map and Missions pages, with each page's real key handler and invented surroundings: what the handlers do with the key.
// The behaviour is from one press each in the real game (docs/verification/CONTROLS.md). Run: node tests/status_page_keys_test.js
'use strict';
const assert = require('assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const OVERLAY = path.join(__dirname, '..', 'tools', 'hud_overlay');
const element = () => ({classList: {add() {}, remove() {}, contains: () => false}, style: {}, hidden: false, textContent: '', appendChild() {}, querySelector: () => element(), setAttribute() {}, addEventListener() {}});

// Loads the page's scripts and returns its keydown handlers, and where a key sent the page.
function page(files) {
  const keys = [];
  const location = {href: '', search: ''};
  const player = {style: {}, ruffle: () => ({load: () => Promise.resolve()}), ow: () => undefined, remove() {}};
  const context = vm.createContext({
    console: {log() {}, warn() {}, error() {}},
    location,
    document: {body: element(), getElementById: () => element(), querySelector: () => element(), createElement: element},
    performance: {now: () => 0},
    setInterval: () => 1, clearInterval() {}, setTimeout: () => 1,
    fetch: () => new Promise(() => {}),
    URLSearchParams,
    addEventListener: (type, f) => { if (type === 'keydown') keys.push(f); },
    RufflePlayer: {newest: () => ({createPlayer: () => player})},
    owMissionLog: {}, owMenu3d: {}, owMovieGuard: {FILES: {}, load: () => new Promise(() => {})},
  });
  context.window = context;
  context.globalThis = context;
  for (const file of files) vm.runInContext(fs.readFileSync(path.join(OVERLAY, file), 'utf8'), context);
  return {location, press(key) {
    location.href = '';
    for (const f of keys) f({key, altKey: false, ctrlKey: false, metaKey: false, preventDefault() {}});
    return location.href;
  }};
}

const missions = page(['missions.js']);
assert.strictEqual(missions.press('m'), '/__ow_tab_map', 'M on the Missions page goes to the Map');
assert.strictEqual(missions.press('l'), '/__ow_close_missions', 'L still closes it');
console.log('PASS Missions: M goes to the Map, L closes');

// map.js holds its handler inside its page start; the source is the check, against the handler's first lines.
const map = fs.readFileSync(path.join(OVERLAY, 'map.js'), 'utf8');
const handler = map.slice(map.indexOf("addEventListener('keydown'"));
const closing = handler.split('\n').find(line => line.includes('close()'));
assert.ok(closing.includes("'escape'") && closing.includes("'tab'"), closing);
assert.ok(!closing.includes("'m'"), 'M is not a closing key on the Map');
assert.ok(handler.includes("key === 'l'"), 'L still goes to the Missions');
console.log('PASS Map: M is not a closing key');
console.log('status_page_keys_test: 2/2 passed');
