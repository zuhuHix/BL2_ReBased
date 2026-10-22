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
    querySelector:node, querySelectorAll:()=>[], addEventListener(){}, visibilityState:'visible'},
  window:{addEventListener(){}, RufflePlayer:{newest:()=>({createPlayer:()=>({
    ow(){}, ruffle:()=>({load:()=>new Promise(()=>{})})})})}}});
vm.runInContext(fs.readFileSync(path.join(__dirname,'../tools/hud_overlay/menu3d.js'),'utf8'),context);
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
  assert.equal(selected(), 'b8'); assert.equal(run('firstRow'), 3); // entry index: the WEAPONS header is entry 0
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
check('Inspect blocks navigation', () => {
  run('reset(); inspectMode=true'); press('ArrowDown'); assert.equal(selected(), 'w0');
  run('inspectMode=false');
});
// Stock sort list (docs/verification/NATIVE_INVENTORY_SORT.md; header texts and orders seen in the original game).
const sortItems = [
  {id:'p1',name:'p1',type:'Pistol',manufacturer:'Jakobs',rarity:1,value:30,level:5},
  {id:'p2',name:'p2',type:'Pistol',manufacturer:'Hyperion',rarity:2,value:80,level:5},
  {id:'a1',name:'a1',type:'Assault Rifle',manufacturer:'Bandit',rarity:1,value:20,level:5},
  {id:'s1',name:'s1',type:'Shotgun',manufacturer:'Jakobs',rarity:1,value:50,level:5},
  {id:'m1',name:'m1',type:'Sub-Machine Gun',manufacturer:'Hyperion',rarity:1,value:60,level:5},
  {id:'sh1',name:'sh1',itemType:'shield',manufacturer:'Tediore',rarity:1,value:37,level:3},
  {id:'rl',name:'rl',itemType:'relic',rarity:1,value:10,level:3},
  {id:'cm',name:'cm',itemType:'class_mod',rarity:1,value:15,level:3}
];
const listFor = mode => {
  run(`reset(); window.owInventory({level:10,activeSlot:0,slotsUnlocked:4,slots:[null,null,null,null],gearSlots:{},backpackCapacity:12,items:${JSON.stringify(sortItems)}}); sortIndex=${mode}; selectedId=null; navigationPanel='backpack'`);
  return JSON.parse(run("JSON.stringify(backpackEntries().map(e => e.header !== undefined ? '#'+e.header : e.empty ? '_' : e.item.id))"));
};
check('Stock sort list: ALL, TYPES, BRANDS, ITEMS and VALUE orders and sub-headers', () => {
  const empties = n => Array(n).fill('_');
  assert.deepEqual(listFor(0), ['#WEAPONS','p2','m1','s1','p1','a1','#RELICS','rl','#CLASS MODS','cm','#SHIELDS','sh1',...empties(4)]);
  assert.deepEqual(listFor(1), ['#ASSAULT RIFLES','a1','#PISTOLS','p2','p1','#SHOTGUNS','s1','#SUB-MACHINE GUNS','m1',...empties(7)]);
  assert.deepEqual(listFor(2), ['#BANDIT MADE','a1','#HYPERION','p2','m1','#JAKOBS','p1','s1','#TEDIORE','sh1','rl','cm',...empties(4)]);
  assert.deepEqual(listFor(3), ['#RELICS','rl','#CLASS MODS','cm','#SHIELDS','sh1',...empties(9)]);
  assert.deepEqual(listFor(4), ['p2','m1','s1','sh1','p1','a1','cm','rl',...empties(4)]);
});
check('PageDown cycles ALL > TYPES > BRANDS > ITEMS > VALUE, PageUp reverses; each step selects the first item', () => {
  listFor(0);
  run("navigationPanel='equipped'"); press('PageUp'); assert.equal(run('sortIndex'), 0); // only the backpack sorts
  run("selectedId='p1'; navigationPanel='backpack'");
  const modes = [], first = [];
  for (let i = 0; i < 5; i++) {
    press('PageDown'); modes.push(run('sortIndex'));
    first.push(run('(backpackEntries().find(entry => entry.item || entry.empty)?.item?.id || backpackEntries().find(entry => entry.empty)?.id)'));
  }
  assert.deepEqual(modes, [1,2,3,4,0]);
  assert.deepEqual(first, ['a1','a1','rl','p2','p2']);
  assert.equal(selected(), 'p2');
  press('PageUp'); assert.equal(run('sortIndex'), 4); assert.equal(selected(), 'p2');
  press('PageUp'); assert.equal(run('sortIndex'), 3); assert.equal(selected(), 'rl');
  run("sortIndex=0; selectedId='w0'; navigationPanel='equipped'"); // equipped selection: PageDown does not sort
  press('PageDown'); assert.equal(run('sortIndex'), 0);
});
check('ALL equal-rarity cross-type weapons use the captured descending type-key tie order (UNVERIFIED)', () => {
  const items = [
    {id:'weapon_ar',type:'Assault Rifle',rarity:2,level:5},
    {id:'weapon_rocket',type:'Rocket Launcher',rarity:2,level:5},
    {id:'weapon_smg',type:'Sub-Machine Gun',rarity:2,level:5},
    {id:'weapon_sniper',type:'Sniper Rifle',rarity:2,level:5}
  ];
  run(`reset(); window.owInventory({level:10,activeSlot:0,slotsUnlocked:4,slots:[null,null,null,null],gearSlots:{},backpackCapacity:8,items:${JSON.stringify(items)}}); sortIndex=0`);
  assert.deepEqual(JSON.parse(run("JSON.stringify(backpackItems().map(item=>item.id))")),
    ['weapon_sniper','weapon_smg','weapon_rocket','weapon_ar']);
});
check('Sort with no ITEMS selects the first empty backpack cell and keeps backpack focus', () => {
  run(`reset(2,false); window.owInventory({level:10,activeSlot:0,slotsUnlocked:2,slots:['w0','w1',null,null],gearSlots:{},backpackCapacity:39,
    items:[{id:'w0',name:'w0',type:'Pistol',level:1},{id:'w1',name:'w1',type:'Pistol',level:1}]});
    sortIndex=2; navigationPanel='backpack'; selectedId='w0'; targetSlot=1; changeSort(1)`);
  assert.equal(run('sortIndex'), 3);
  assert.equal(selected(), 'empty:0');
  assert.equal(run('lastBackpackId'), 'empty:0');
  assert.equal(run('navigationPanel'), 'backpack');
  assert.equal(run('equippedSelectionIndex()'), -1);
});
check('Backpack weapon compare uses the active weapon, not the last destination slot', () => {
  run("reset(); setTargetSlot(1); select('b0'); toggleCompare()");
  assert.equal(run('targetSlot'), 1); // still the separate destination choice
  assert.equal(run('compareId'), 'w0'); // activeSlot 0, even though slot 2 was visited
  run("compareId=null; navigationPanel='backpack'; beginBackpackTransfer()");
  assert.equal(run('compareId'), 'w0');
  assert.equal(run('targetSlot'), 0); // transfer starts at its compare partner
  run('finishTransfer(true)');
});
check('Equipped-origin weapon compare keeps the selected weapon slot and a backpack candidate (D4)', () => {
  run("reset(); selectEquipmentSlot(0); beginEquippedTransfer()");
  assert.equal(run('transferSourceId'), 'w0');
  assert.equal(run('compareId'), 'w0');
  assert.equal(run('targetSlot'), 0);
  assert.equal(selected(), 'b0');
  assert.equal(run('transferFromEquipped'), true);
  assert.equal(run('equippedSelectionIndex()'), 0);
  run('finishTransfer(true)');
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
check('VM movement waits for its result and preserves queued key order', () => {
  run('reset(); inventoryVm.failed=false; window.owConfigureInventoryVm(true)');
  press('ArrowRight', 'ArrowDown', 'ArrowDown');
  assert.equal(selected(), 'b0');
  assert.equal(run('inventoryVm.queue.length'), 1);
  run('window.owInventoryVmResult({serial:inventoryVm.pending.serial,index:1,steps:55,error:""})');
  assert.equal(selected(), 'b1');
  assert.equal(run('inventoryVm.pending.selectedId'), 'b1');
  run('window.owInventoryVmResult({serial:inventoryVm.pending.serial,index:2,steps:55,error:""})');
  assert.equal(selected(), 'b2'); assert.equal(run('inventoryVm.pending'), null);
});
check('VM replies do not overwrite a changed selection or a reordered list', () => {
  press('ArrowDown');
  run("select('b5'); window.owInventoryVmResult({serial:inventoryVm.pending.serial,index:3,steps:55,error:''})");
  assert.equal(selected(), 'b5');
  press('ArrowDown');
  run("sortIndex=1; state.items.find(item=>item.id==='b0').name='zzzz'; window.owInventoryVmResult({serial:inventoryVm.pending.serial,index:6,steps:55,error:''})");
  assert.equal(selected(), 'b5'); assert.equal(run('inventoryVm.pending'), null);
});
check('VM failure blocks movement and cannot be silently re-enabled by a snapshot', () => {
  run('reset(); window.owConfigureInventoryVm(true)');
  press('ArrowRight', 'ArrowDown');
  run('window.owInventoryVmResult({serial:inventoryVm.pending.serial,index:-1,steps:0,error:"UNIMPLEMENTED test"}); window.owConfigureInventoryVm(true)');
  press('ArrowDown'); assert.equal(selected(), 'b0');
  assert.equal(run('inventoryVm.enabled'), false);
  run('inventoryVm.failed=false; window.owConfigureInventoryVm(false)');
});
check('Closing cancels VM work and obsolete serials cannot change selection', () => {
  run('reset(); window.owConfigureInventoryVm(true)');
  press('ArrowRight', 'ArrowDown');
  run('const canceledSerial=inventoryVm.pending.serial; window.owInventoryVmResult({serial:canceledSerial+1,index:1,steps:55,error:""})');
  assert.equal(selected(), 'b0'); assert.notEqual(run('inventoryVm.pending'), null);
  run('window.owCancelInventoryVm(); window.owInventoryVmResult({serial:canceledSerial,index:1,steps:55,error:""})');
  assert.equal(selected(), 'b0'); assert.equal(run('inventoryVm.pending'), null);
  run('window.owConfigureInventoryVm(false)');
});
check('Only the current VM request can time out', () => {
  run('reset(); const vmTimers=[]; setTimeout=callback=>vmTimers.push(callback); window.owConfigureInventoryVm(true)');
  press('ArrowRight', 'ArrowDown');
  run('window.owCancelInventoryVm(); vmTimers[0]()');
  assert.equal(run('inventoryVm.failed'), false);
  press('ArrowDown');
  run('vmTimers[1]()');
  assert.equal(run('inventoryVm.failed'), true); assert.equal(run('inventoryVm.pending'), null);
  assert.equal(selected(), 'b0');
});
check('Weapon card numbers round like tools/weapon_stats.py (shared synthetic cases)', () => {
  const pageKey = {damage:'damage', magazine:'magazine', fire_rate:'fireRate', reload_time:'reloadTime', accuracy:'accuracy'};
  const {cases} = JSON.parse(fs.readFileSync(path.join(__dirname, 'card_rounding_cases.json'), 'utf8'));
  for (const {field, value, text} of cases) {
    const shown = run(`statValueText(cardStats({${pageKey[field]}:${JSON.stringify(value)}})[0])`);
    assert.equal(shown, text, `${field} ${value}`);
  }
  // The compare shows an arrow computed on the printed numbers and no difference figure (real captures).
  const arrow = (item, other, index = 0) => run(`statComparison(cardStats(${JSON.stringify(item)})[${index}], cardStats(${JSON.stringify(other)})).arrow`);
  assert.equal(arrow({damage:753.2}, {damage:648.9}), 'up');
  assert.equal(arrow({damage:99.2}, {damage:100}), 'blank'); // both print 100: no arrow
  assert.equal(arrow({reloadTime:2.0}, {reloadTime:2.2}, 0), 'up');   // lower reload is better
  assert.equal(arrow({reloadTime:2.2}, {reloadTime:2.0}, 0), 'down');
  assert.equal(run(`'delta' in statComparison(cardStats({damage:5})[0], cardStats({damage:4}))`), false);
  // A 21x7 gun beats 25x2 on total damage although each pellet does less.
  assert.equal(arrow({damage:20.5, projectiles:7}, {damage:24.2, projectiles:2}), 'up');
});
check('A multi-projectile weapon prints its count after the damage; elemental guns add two status rows', () => {
  const texts = item => JSON.parse(run(`JSON.stringify(cardStats(${JSON.stringify(item)}).map(stat => [stat.label, statValueText(stat)]))`));
  assert.deepEqual(texts({damage:20.648, projectiles:10, accuracy:28}), [['Damage','21x10'], ['Accuracy','28.0']]);
  assert.deepEqual(texts({damage:20.648, projectiles:1}), [['Damage','21']]);
  const fire = texts({damage:49.128, element:'Fire', statusDps:25.52106431570439, statusChance:15.600000768000005});
  assert.deepEqual(fire, [['Damage','50'], ['Burn Damage / sec.','25.5'], ['Ignite Chance','15.6%']]);
  // Labels the real cards printed for the other two elements; slag and no element add nothing.
  assert.deepEqual(texts({damage:5, element:'Shock', statusDps:60, statusChance:15.6}).slice(1),
    [['Shock Damage / sec.','60.0'], ['Electrocute Chance','15.6%']]);
  assert.deepEqual(texts({damage:5, element:'Corrosive', statusDps:375.5, statusChance:15.6}).slice(1),
    [['Corrode Damage / sec.','375.5'], ['Corrode Chance','15.6%']]);
  assert.equal(texts({damage:5, element:'Slag', statusDps:1, statusChance:1}).length, 1);
  assert.equal(texts({damage:5, element:'None'}).length, 1);
  assert.equal(texts({damage:5, element:'Fire'}).length, 1); // no status numbers from the host: no rows
  // A stored 33.349998 is a single-precision tie that prints 33.4 on the real card (golden launcher).
  assert.equal(run(`statValueText(cardStats({damage:5, element:'Fire', statusDps:1, statusChance:33.349998})[2])`), '33.4%');
});
check('Compare view outlines the cells that cannot take the compared item', () => {
  run('reset()');
  assert.equal(run('[0,1,2,3,4,5,6,7].some(compareRefusesCell)'), false);
  run("selectedId='b0'; beginBackpackTransfer()");
  assert.deepEqual(JSON.parse(run('JSON.stringify([0,1,2,3,4,5,6,7].map(compareRefusesCell))')), [false,false,false,false,true,true,true,true]);
  // UNVERIFIED rule for a gear item: everything but its own cell.
  run("finishTransfer(true); selectedId='spare'; beginBackpackTransfer()");
  assert.deepEqual(JSON.parse(run('JSON.stringify([0,1,2,3,4,5,6,7].map(compareRefusesCell))')), [true,true,true,true,false,true,true,true]);
  run('finishTransfer(true)');
  assert.equal(run('[0,1,2,3,4,5,6,7].some(compareRefusesCell)'), false);
});
check('Down past the last backpack item selects the empty cells; item actions ignore them', () => {
  // 2026-10-21 capture: the selection walks onto the trailing "(EMPTY)" cells and no card is shown.
  run('reset(); inventoryVm.enabled = false; inventoryVm.failed = false; inventoryVm.pending = null; inventoryVm.queue.length = 0; window.owInventory({...state, backpackCapacity:15})'); // 13 backpack items, 2 empty cells
  press('ArrowRight'); run("for(let i=0;i<12;i++) key('ArrowDown')");
  assert.equal(selected(), 'spare');
  press('ArrowDown'); assert.equal(selected(), 'empty:0');
  press('ArrowDown', 'ArrowDown'); assert.equal(selected(), 'empty:1'); // stops at the last cell
  const before = logs.length;
  press('e', 'q', 'v', 'f');
  assert.equal(logs.slice(before).filter(line => /^OWITEM/.test(line)).length, 0);
  assert.equal(run('inspectMode'), false); assert.equal(run('transferSourceId'), null);
  // A new snapshot keeps the empty selection; Left/Right come back to it.
  run('window.owInventory({...state, money:5})'); assert.equal(selected(), 'empty:1');
  press('ArrowLeft', 'ArrowRight'); assert.equal(selected(), 'empty:1');
  run('selectEquipmentSlot(1)'); press('ArrowRight');
  assert.equal(selected(), 'empty:1'); // D20 M6: Right restores the remembered empty cell.
  press('ArrowUp', 'ArrowUp'); assert.equal(selected(), 'spare');
});
check('An empty selection scrolls into the last visible row (D20 D2g, placement UNVERIFIED)', () => {
  run(`reset(2,false); window.owInventory({level:10,activeSlot:0,slotsUnlocked:2,slots:['w0','w1',null,null],gearSlots:{},backpackCapacity:39,
    items:[{id:'w0',name:'equipped0',type:'Pistol',level:1},{id:'w1',name:'equipped1',type:'Pistol',level:1},
      {id:'sniper',type:'Sniper Rifle',rarity:2,level:1},{id:'smg',type:'Sub-Machine Gun',rarity:2,level:1},
      {id:'launcher',type:'Rocket Launcher',rarity:2,level:1},{id:'ar',type:'Assault Rifle',rarity:2,level:1}]});
    navigationPanel='equipped'; selectEquipmentSlot(0); navigateInventory('right'); for(let i=0;i<17;i++) navigateInventory('down')`);
  assert.equal(selected(), 'empty:13');
  const index = run('backpackEntries().findIndex(entry=>entry.id===selectedId)');
  const relative = index - run('firstRow');
  assert.equal(relative, 7); // eighth cell row, with the next cell clipped
  assert.ok((relative + 1) * run('ROW_PITCH') <= run('viewHeight()+listPeekHeight()+1'));
  assert.ok((relative + 1) * run('ROW_PITCH') + run('ROW_PITCH') > run('viewHeight()+listPeekHeight()'));
});
check('The VM is not asked to move onto or between empty cells', () => {
  run("reset(); window.owInventory({...state, backpackCapacity:15}); inventoryVm.enabled = true; inventoryVm.failed = false; select('spare'); navigationPanel = 'backpack'");
  const before = logs.length;
  press('ArrowDown'); assert.equal(selected(), 'empty:0');
  press('ArrowUp'); assert.equal(selected(), 'spare');
  assert.equal(logs.slice(before).filter(line => /^OWINVMOVE/.test(line)).length, 0);
  press('ArrowUp'); // an item-to-item move still goes to the VM
  assert.equal(logs.slice(before).filter(line => /^OWINVMOVE/.test(line)).length, 1);
  run('inventoryVm.enabled = false; inventoryVm.pending = null; inventoryVm.queue.length = 0');
});
check('Unequip keys do nothing on an empty backpack cell (review of PR #46)', () => {
  // Weapon slot 1 was the last equipped cell; U/Delete/Backspace must not unequip it from an empty cell.
  run('reset(); inventoryVm.enabled = false; inventoryVm.failed = false; inventoryVm.pending = null; inventoryVm.queue.length = 0; window.owInventory({...state, backpackCapacity:15})');
  press('ArrowRight'); run("for(let i=0;i<13;i++) key('ArrowDown')");
  assert.equal(selected(), 'empty:0'); assert.equal(run('targetSlot'), 0);
  const before = logs.length;
  press('u', 'Delete', 'Backspace');
  assert.equal(logs.slice(before).filter(line => /^OWITEM/.test(line)).length, 0);
});
check('Queued VM moves that reach the last item continue onto the empty cells in order (review of PR #46)', () => {
  run("reset(); window.owInventory({...state, backpackCapacity:15}); inventoryVm.failed = false; inventoryVm.pending = null; inventoryVm.queue.length = 0; inventoryVm.enabled = true; select('b11'); navigationPanel = 'backpack'");
  const moves = () => logs.filter(line => /^OWINVMOVE/.test(line)).length;
  const before = moves();
  press('ArrowDown', 'ArrowDown'); // the first goes to the VM; the second waits for its reply
  assert.equal(moves() - before, 1); assert.equal(run('inventoryVm.queue.length'), 1); assert.equal(selected(), 'b11');
  run('window.owInventoryVmResult({serial:inventoryVm.pending.serial,index:12,steps:55,error:""})'); // delayed reply: now on 'spare'
  // The queued Down now starts on the last item, so it is the host's and lands on the first empty cell; nothing more goes to the VM.
  assert.equal(selected(), 'empty:0'); assert.equal(moves() - before, 1);
  assert.equal(run('inventoryVm.queue.length'), 0); assert.equal(run('inventoryVm.pending'), null);
  // From the empty cell, Up is the host's too and returns to the last item without a VM request.
  press('ArrowUp'); assert.equal(selected(), 'spare'); assert.equal(moves() - before, 1);
  run('inventoryVm.enabled = false');
});
check('The menu turn maps measured screen points back to movie points (menu3d.js)', () => {
  // No turn: identity. An 11-degree turn about (640, 360): the centre is fixed; a point left of it is drawn further left and
  // taller, so the movie point for a screen point at x 300 lies right of 300, and projecting it forward gives 300 again.
  assert.deepEqual(JSON.parse(run('JSON.stringify(window.owMenu3d.toMovie(300, 200))')), {x:300, y:200});
  run(`window.owMenu3d.apply({style:{setProperty(){}}}, 11, 640, 360)`);
  const centre = JSON.parse(run('JSON.stringify(window.owMenu3d.toMovie(640, 360))'));
  assert.deepEqual(centre, {x:640, y:360});
  const m = JSON.parse(run('JSON.stringify(window.owMenu3d.toMovie(300, 200))'));
  assert.ok(m.x > 300 && m.x < 320, `x ${m.x}`);
  const a = 11 * Math.PI / 180, d = m.x - 640, k = 1229 / (1229 + d * Math.sin(a));
  assert.ok(Math.abs(640 + d * Math.cos(a) * k - 300) < 1e-6 && Math.abs(360 + (m.y - 360) * k - 200) < 1e-6);
  const projected = JSON.parse(run(`JSON.stringify(window.owMenu3d.toScreen(${m.x},${m.y}))`));
  assert.ok(Math.abs(projected.x - 300) < 1e-6 && Math.abs(projected.y - 200) < 1e-6);
  const projectedBounds = JSON.parse(run("JSON.stringify(window.owMenu3d.projectBounds({xMin:300,yMin:200,xMax:340,yMax:230}))"));
  assert.ok(projectedBounds.xMin < 300 && projectedBounds.xMax < 340);
  run('window.owMenu3d.setScale(1)'); // back to no turn for later checks
  run(`window.owMenu3d.apply({style:{setProperty(){}}}, 0, 640, 360)`);
});
check('D5 Inspect fit, zoom and hint text track the measured targets (UNVERIFIED)', () => {
  assert.equal(run('INSPECT_CARD_LEFT'), 28); // movie-space target accounts for the visible frame inset
  assert.equal(run('INSPECT_CARD_TOP'), 43);
  assert.equal(run('INSPECT_ZOOM'), 1.54);
  assert.equal(run('INSPECT_HINT_FONT_SIZE'), 14);
  assert.equal(run('CARD_FIT.inspect.width * FRAME_INSET'), 316);
});
console.log(`${passed}/${passed} navigation checks passed (traversal from original-game observation; VM replies synthetic; UNVERIFIED cells noted in inventory.js)`);
