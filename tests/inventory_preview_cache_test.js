// Synthetic checks of the inventory page's thumbnail cache (attachPreview / prunePreviewCache / loadPreview in
// tools/hud_overlay/inventory.js). No game assets, movie or renderer: a minimal fake DOM stands in for the browser and
// the test fires the image events itself. Item ids and preview names are invented.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');

// A fake element that records its listeners, so the test can fire load/error and count what is still attached.
function element(tag) {
  const listeners = new Map();
  return {
    tag, style:{setProperty(){}}, dataset:{}, attrs:{}, children:[], parent:null, complete:false, naturalWidth:0, src:'',
    classList:{add(){}, toggle(){}},
    addEventListener(type, fn, options) { if (!listeners.has(type)) listeners.set(type, []); listeners.get(type).push({fn, once:Boolean(options?.once)}); },
    removeEventListener(type, fn) { listeners.set(type, (listeners.get(type) || []).filter(entry => entry.fn !== fn)); },
    fire(type) {
      for (const entry of [...(listeners.get(type) || [])]) {
        if (entry.once) this.removeEventListener(type, entry.fn);
        entry.fn();
      }
    },
    listenerCount() { return [...listeners.values()].reduce((sum, list) => sum + list.length, 0); },
    appendChild(child) { child.parent?.removeChild(child); child.parent = this; this.children.push(child); return child; },
    removeChild(child) { this.children = this.children.filter(node => node !== child); child.parent = null; },
    remove() { this.parent?.removeChild(this); },
    setAttribute(name, value) { this.attrs[name] = value; }, prepend() {}, focus() {}
  };
}
const byId = new Map();
const getById = key => { if (!byId.has(key)) byId.set(key, element('div')); return byId.get(key); };
const context = vm.createContext({console:{log(){}, error(){}}, performance:{now:()=>0}, innerWidth:1280, innerHeight:720,
  requestAnimationFrame(){}, setInterval(){}, setTimeout(){}, queueMicrotask(fn){ fn(); }, navigator:{}, location:{},
  document:{getElementById:getById, querySelector:getById, querySelectorAll:()=>[], addEventListener(){}, visibilityState:'visible',
    createElement:element, createElementNS:(ns, tag) => element(tag)},
  window:{addEventListener(){}, RufflePlayer:{newest:()=>({createPlayer:()=>({ow(){}, ruffle:()=>({load:()=>new Promise(()=>{})})})})}}});
vm.runInContext(fs.readFileSync(path.join(__dirname, '../tools/hud_overlay/menu3d.js'), 'utf8'), context);
vm.runInContext(fs.readFileSync(path.join(__dirname, '../tools/hud_overlay/inventory.js'), 'utf8'), context);
const run = code => vm.runInContext(code, context);

// Give the page a snapshot with two invented weapons (the cache reads item ids and preview URLs from it).
run(`state = {slots:[null,null,null,null], gearSlots:{}, items:[{id:'gun_a', name:'a'}, {id:'gun_b', name:'b'}]};`);
const attach = (kind, id) => run(`(() => { const button = document.createElement('button');
  const image = attachPreview(button, ${JSON.stringify(kind)}, itemById(${JSON.stringify(id)})); return {button, image}; })()`);
const loaded = image => { image.complete = true; image.naturalWidth = 64; image.fire('load'); };
let passed = 0;
function check(name, test) { test(); passed++; console.log('PASS ' + name); }

check('A loaded thumbnail is reused for the same kind, item and URLs, and keeps no listeners', () => {
  const first = attach('backpack', 'gun_a');
  assert.equal(first.image.src, '/previews/gun_a.png');
  loaded(first.image);
  assert.equal(first.image.listenerCount(), 0); // load and error handlers (and the onFailure closure) are gone
  const second = attach('backpack', 'gun_a');
  assert.equal(second.image, first.image);
  assert.equal(second.image.parent, second.button); // moved into the rebuilt button
  assert.equal(first.button.children.length, 0);
});

check('A different item, kind or preview URL gets a new image', () => {
  const cached = attach('backpack', 'gun_a').image;
  assert.notEqual(attach('backpack', 'gun_b').image, cached);
  assert.notEqual(attach('slot', 'gun_a').image, cached);
  run(`itemById('gun_a').assetId = 'gun_a_variant';`);
  const renamed = attach('backpack', 'gun_a').image;
  assert.notEqual(renamed, cached);
  assert.equal(renamed.src, '/previews/gun_a_variant.png');
  run(`delete itemById('gun_a').assetId;`);
});

check('An image that never loads is not cached and drops its listeners after the last URL fails', () => {
  const {button, image} = attach('backpack', 'gun_b');
  image.fire('error'); // the only URL for gun_b fails: the placeholder replaces the image
  assert.equal(image.listenerCount(), 1); // only the once-only cache hook from attachPreview is left; it never fires
  assert.equal(button.children.length, 1);
  assert.notEqual(button.children[0], image);
  assert.notEqual(attach('backpack', 'gun_b').image, image);
});

check('Items that left the snapshot, or whose preview URLs changed, are pruned', () => {
  run('loadedPreviews.clear(); previewStatus.clear();');
  loaded(attach('backpack', 'gun_a').image);
  loaded(attach('backpack', 'gun_b').image);
  loaded(attach('slot', 'gun_b').image);
  assert.equal(run('loadedPreviews.size'), 3);
  run(`state.items = state.items.filter(item => item.id !== 'gun_a'); prunePreviewCache();`);
  assert.deepEqual(JSON.parse(run('JSON.stringify([...loadedPreviews.keys()].sort())')), ['backpack|gun_b|/previews/gun_b.png', 'slot|gun_b|/previews/gun_b.png']);
  run(`itemById('gun_b').assetId = 'gun_b_new'; prunePreviewCache();`);
  assert.equal(run('loadedPreviews.size'), 0);
});

console.log(`${passed}/${passed} preview cache checks passed (synthetic fake DOM)`);
