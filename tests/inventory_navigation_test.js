// Synthetic adapter regression checks. No game assets or renderer are needed.
// The expected traversal comes from keyboard observations of the original game
// (2026-09-30, see docs/verification/INVENTORY_MOVIE_PROTOTYPE.md); the synthetic
// state only stands in for item data.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const nodes = new Map(), logs = [];
let focused = null;
function node(key) {
  if (!nodes.has(key)) nodes.set(key, {style:{}, dataset:{}, attrs:{},
    addEventListener(){}, prepend(){}, focus(){focused=key;},
    setAttribute(name,value){this.attrs[name]=value;}});
  return nodes.get(key);
}
const context = vm.createContext({console:{log:value=>logs.push(value), error(){}},
  performance:{now:()=>0}, innerWidth:1280, innerHeight:720,
  requestAnimationFrame(){}, setInterval(){}, setTimeout(){}, queueMicrotask(){},
  navigator:{}, location:{}, document:{getElementById:node,
    querySelector:node, querySelectorAll:()=>[]},
  window:{addEventListener(){}, RufflePlayer:{newest:()=>({createPlayer:()=>({
    ow(){}, ruffle:()=>({load:()=>new Promise(()=>{})})})})}}});
vm.runInContext(fs.readFileSync(path.join(__dirname,'../tools/hud_overlay/inventory.js'),'utf8'),context);
const run = code => vm.runInContext(code,context);
run(`
  ready = true;
  render = () => {};
  drawCard = () => {};
  announce = () => {};
  function key(key) { handleKey({key, preventDefault(){}}); }
  function reset(open=4, gear=true) {
    selectedId=null; targetSlot=0; targetGearSlot=null; firstRow=0;
    navigationPanel='equipped'; lastBackpackId=null; lastEquippedIndex=0;
    transferSourceId=null; transferFromEquipped=false; compareId=null;
    categoryIndex=sortIndex=0; inspectMode=false; state=null; lastState='';
    const weapons=['w0','w1','w2','w3'].map((id,i)=>i<open?id:null);
    window.owInventory({level:10,activeSlot:0,slotsUnlocked:open,
      slots:weapons,
      gearSlots:gear?{shield:'shield',grenadeMod:'gm',classMod:'cm',relic:'relic'}:{},
      items:[...weapons.filter(Boolean).map(id=>({id,name:id,level:1})),
        ...(gear?[{id:'shield',itemType:'shield'},{id:'gm',itemType:'grenade_mod'},
          {id:'cm',itemType:'class_mod'},{id:'relic',itemType:'relic'}]:[]).map(item=>({name:item.id,level:1,...item})),
        ...Array.from({length:12},(_,i)=>({id:'b'+i,name:'b'+i,level:1})),
        {id:'spare',name:'spare',itemType:'shield',level:1}]});
  }
`);
let passed = 0;
function check(name, test) { test(); passed++; console.log('PASS '+name); }
const press = (...keys) => keys.forEach(value => run(`key(${JSON.stringify(value)})`));
const selected = () => run('selectedId');
check('Weapon column walks slot 1-4 and stops at the top (no wrap)', () => {
  run('reset()'); press('ArrowUp'); assert.equal(selected(), 'w0');
  press('ArrowDown', 'ArrowDown', 'ArrowDown'); assert.equal(selected(), 'w3');
});
check('Down from the last weapon enters the shield; Up returns to it', () => {
  press('ArrowDown'); assert.equal(selected(), 'shield');
  press('ArrowUp'); assert.equal(selected(), 'w3');
});
check('Gear is a 2x2 grid: shield/class mod over grenade mod/relic, no wrap', () => {
  run('reset(); selectEquipmentSlot(4)');
  press('ArrowRight'); assert.equal(selected(), 'cm');
  press('ArrowDown'); assert.equal(selected(), 'relic');
  press('ArrowLeft'); assert.equal(selected(), 'gm');
  press('ArrowUp'); assert.equal(selected(), 'shield');
  press('ArrowLeft'); assert.equal(selected(), 'shield');
  press('ArrowDown', 'ArrowDown'); assert.equal(selected(), 'gm');
  press('ArrowLeft'); assert.equal(selected(), 'gm');
});
check('Right from a weapon slot or a right-hand gear cell enters the backpack', () => {
  run('reset()'); press('ArrowRight'); assert.equal(selected(), 'b0');
  run('reset(); selectEquipmentSlot(7)'); press('ArrowRight'); assert.equal(selected(), 'b0');
  run('reset(); selectEquipmentSlot(6)'); press('ArrowRight'); assert.equal(selected(), 'b0');
  press('ArrowRight'); assert.equal(selected(), 'b0');
});
check('Left in the backpack returns to the last equipped cell', () => {
  run('reset(); selectEquipmentSlot(7)'); press('ArrowRight', 'ArrowDown', 'ArrowDown', 'ArrowLeft');
  assert.equal(selected(), 'relic');
  run('reset(); selectEquipmentSlot(2)'); press('ArrowRight', 'ArrowLeft');
  assert.equal(selected(), 'w2');
});
check('Backpack keeps its row when re-entered and stops at the top', () => {
  run('reset()'); press('ArrowRight', 'ArrowUp'); assert.equal(selected(), 'b0');
  press('ArrowDown', 'ArrowDown', 'ArrowDown'); assert.equal(selected(), 'b3');
  press('ArrowLeft', 'ArrowRight'); assert.equal(selected(), 'b3');
});
check('Backpack scrolls minimally', () => {
  run('reset()'); press('ArrowRight');
  run("for(let i=0;i<8;i++) key('ArrowDown')");
  assert.equal(selected(), 'b8'); assert.equal(run('firstRow'), 2);
});
check('Locked weapon slots are skipped', () => {
  run('reset(2)'); press('ArrowDown'); assert.equal(selected(), 'w1');
  press('ArrowDown'); assert.equal(selected(), 'shield');
  press('ArrowUp'); assert.equal(selected(), 'w1');
  run('selectEquipmentSlot(2)'); assert.equal(selected(), 'w1');
});
check('Empty equipment clears stale item and disables item actions', () => {
  run('reset(); selectEquipmentSlot(3); window.owInventory({...state,slots:["w0","w1","w2",null]})');
  assert.equal(selected(), null);
  const before = logs.length; run("key('q'); key('e')");
  assert.equal(logs.slice(before).filter(value => value.startsWith('OWITEM ')).length, 0);
  assert.equal(focused, '#controls [data-kind="slot"][data-slot="3"]');
});
check('Empty-slot focus survives an unrelated host snapshot', () => {
  run('window.owInventory({...state,money:23})');
  assert.equal(selected(), null); assert.equal(run('selectedSlotIndex()'), 3);
});
check('Host updates refresh the occupant of the selected slot', () => {
  run('reset(); selectEquipmentSlot(3); window.owInventory({...state,slots:["w0","w1",null,"w2"]})');
  assert.equal(selected(), 'w2'); assert.equal(run('targetSlot'), 3);
  run('window.owInventory({...state,slots:["w0","w1",null,null]})');
  assert.equal(selected(), null); assert.equal(run('navigationPanel'), 'equipped');
});
check('Pointer selection updates the destination and the remembered cell', () => {
  run("reset(); select('w2')"); assert.equal(run('targetSlot'), 2);
  run("select('relic')"); assert.equal(run('lastEquippedIndex'), 7);
  press('ArrowRight', 'ArrowLeft'); assert.equal(selected(), 'relic');
});
check('Backpack-origin swap: Up/Down choose the destination, Left/Right do nothing, no wrap', () => {
  run("reset(); select('b0'); key('e')");
  assert.equal(run('transferSourceId'), 'b0');
  press('ArrowDown'); assert.equal(run('targetSlot'), 1);
  press('ArrowLeft', 'ArrowRight'); assert.equal(run('targetSlot'), 1);
  press('ArrowDown', 'ArrowDown', 'ArrowDown'); assert.equal(run('targetSlot'), 3);
  press('ArrowUp', 'ArrowUp', 'ArrowUp', 'ArrowUp'); assert.equal(run('targetSlot'), 0);
  assert.equal(selected(), 'b0');
  press('Escape'); assert.equal(selected(), 'b0'); assert.equal(run('navigationPanel'), 'backpack');
});
check('Equipped-origin swap walks compatible candidates with Up/Down only', () => {
  run("reset(); select('shield'); key('e')");
  assert.equal(run('transferSourceId'), 'shield'); assert.equal(selected(), 'spare');
  press('ArrowLeft', 'ArrowRight', 'ArrowDown'); assert.equal(selected(), 'spare');
  press('Escape'); assert.equal(selected(), 'shield');
  assert.equal(run('navigationPanel'), 'equipped');
});
check('Inspect blocks navigation; PageUp retains the sort binding', () => {
  run('reset(); inspectMode=true'); press('ArrowDown'); assert.equal(selected(), 'w0');
  run('inspectMode=false'); press('PageUp'); assert.equal(run('sortIndex'), 1);
});
check('Gamepad D-pad shares keyboard navigation', () => {
  run('reset(); gamepadActions[13]()'); assert.equal(selected(), 'w1');
  run('gamepadActions[15]()'); assert.equal(selected(), 'b0');
  run('gamepadActions[14]()'); assert.equal(selected(), 'w1');
});
check('Confirmed host equip moves navigation to the occupied destination', () => {
  run("reset(); select('b0'); window.owInventory({...state,slots:['w0','b0','w2','w3']})");
  assert.equal(selected(), 'b0'); assert.equal(run('targetSlot'), 1);
  assert.equal(run('navigationPanel'), 'equipped');
  press('ArrowUp'); assert.equal(selected(), 'w0');
});
console.log(`${passed}/${passed} navigation checks passed (traversal from original-game observation; UNVERIFIED cells noted in inventory.js)`);
