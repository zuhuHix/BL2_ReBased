// M and L on the Missions page with its real key handler and invented surroundings, and on Skills by source (the Map page: map_screen_test.js).
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
  return {location, press(key) {
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
console.log('status_page_keys_test: 1/1 passed (the Map page: tests/map_screen_test.js; Skills: source check below)');

// skills.js needs its whole page to load; its handler's routes, by source.
const skills = fs.readFileSync(path.join(OVERLAY, 'skills.js'), 'utf8');
for (const [key, route] of [['m', '/__ow_tab_map'], ['l', '/__ow_tab_missions']]) {
  const at = skills.indexOf(`event.key.toLowerCase() === '${key}'`);
  assert.ok(at > 0 && skills.slice(at, at + 200).includes(route), `Skills: ${key} routes to ${route}`);
}
// K is Skills' own key: it stays, and it is not in the closing branch.
const closing = skills.slice(skills.indexOf("if (event.key === 'Escape'"), skills.indexOf('closeSkills();', skills.indexOf("if (event.key === 'Escape'")));
assert.ok(closing.includes("'Tab'") && !closing.includes("'k'"), 'Skills: K does not close');
