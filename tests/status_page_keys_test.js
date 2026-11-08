// M, L and each page's own key on the Missions and Skills pages, with their real key handlers and invented surroundings (the Map page: map_screen_test.js).
// The behaviour is from the real-game reads and the stock script (docs/verification/CONTROLS.md, with sample counts). Run: node tests/status_page_keys_test.js
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
  return {context, location, press(key) {
    location.href = '';
    for (const f of keys) f({key, altKey: false, ctrlKey: false, metaKey: false, preventDefault() {}});
    return location.href;
  }};
}

const missions = page(['missions.js']);
assert.strictEqual(missions.press('m'), '/__ow_tab_map', 'M on the Missions page goes to the Map');
assert.strictEqual(missions.press('l'), '', 'L stays on it');
assert.strictEqual(missions.press('Escape'), '/__ow_close_missions');
assert.strictEqual(missions.press('Tab'), '/__ow_close_missions');
console.log('PASS Missions: M goes to the Map, L stays, Escape and Tab close');

const skills = page(['colours.js', 'skill_nav.js', 'skills.js']);
assert.strictEqual(skills.press('m'), '/__ow_tab_map', 'M on Skills goes to the Map');
assert.strictEqual(skills.press('l'), '/__ow_tab_missions', 'L on Skills goes to the log');
assert.strictEqual(skills.press('i'), '/__ow_tab_inventory', 'I on Skills goes to the Inventory');
// K is Skills' own key: it stays. skills.js closes through the host's hook when there is one, so both ways of closing are watched.
assert.strictEqual(skills.press('k'), '', 'K on Skills neither closes nor navigates');
assert.strictEqual(skills.press('K'), '', 'nor with Shift or Caps Lock');
assert.strictEqual(skills.press('Escape'), '/__ow_close_skills');
assert.strictEqual(skills.press('Tab'), '/__ow_close_skills');
let hostCloses = 0;
skills.context.owCloseSkills = () => { hostCloses++; };
assert.strictEqual(skills.press('k'), '');
assert.strictEqual(hostCloses, 0, 'K does not close through the host hook either');
skills.press('Escape');
assert.strictEqual(hostCloses, 1, 'the control: Escape does');
console.log('PASS Skills: M, L and I route, K stays, Escape and Tab close');
console.log('status_page_keys_test: 2/2 passed (the Map page: tests/map_screen_test.js)');
