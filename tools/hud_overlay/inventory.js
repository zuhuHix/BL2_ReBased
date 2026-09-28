// Executes the installed StatusMenu movie. The host owns equipment and stats;
// this adapter only displays snapshots and requests an equip by stable item id.
const player = window.RufflePlayer.newest().createPlayer();
document.body.prepend(player);
window.owPlayer = player;
const ROOT = '_level1', INV = ROOT + '.inventory';
const call = (path, method, ...args) => player.ow(path, 'apply', method, args);
const get = (path, member) => player.ow(path, 'get', member);
const set = (path, member, value) => player.ow(path, 'set', member, value);
const colors = [0xffffff, 0xffffff, 0x39ff14, 0x3c8dff, 0xb43cff, 0xffb400];
let ready = false, state = null, selectedId = null, targetSlot = 0, page = 0;
let lastState = '';
const escapeHtml = text => String(text).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
const color = item => colors[item?.rarity] || colors[0];
const itemById = id => state?.items.find(item => item.id === id);

window.owInventory = snapshot => {
  if (!snapshot || !Array.isArray(snapshot.items) || !Array.isArray(snapshot.slots)
      || snapshot.slots.length !== 4) return;
  const serial = JSON.stringify(snapshot);
  if (serial === lastState) return;
  lastState = serial;
  state = snapshot;
  if (!itemById(selectedId)) {
    targetSlot = Number.isInteger(state.activeSlot) && state.activeSlot >= 0 && state.activeSlot < 4 ? state.activeSlot : 0;
    selectedId = state.slots[targetSlot] || state.items[0]?.id || null;
  }
  if (ready) render();
};

function text(path, value, size = 15, tint = 0xffffff) {
  set(path, 'htmlText', `<font face="$WillowBody" size="${size}" color="#${tint.toString(16).padStart(6,'0')}">${escapeHtml(value)}</font>`);
}

function select(id) {
  selectedId = id;
  drawCard();
}

function equip() {
  const item = itemById(selectedId);
  if (item && item.level <= state.level) console.log('OWEQUIP ' + JSON.stringify({id:selectedId, slot:targetSlot}));
}

function closeInventory() { location.href = '/__ow_close_inventory'; }

function hit(path, label, click, hover, tint) {
  const bounds = call(path, 'getBounds', ROOT);
  if (!bounds || !Number.isFinite(bounds.xMin) || !Number.isFinite(bounds.xMax)) return;
  const button = document.createElement('button');
  button.className = 'cell'; button.type = 'button'; button.textContent = label;
  button.setAttribute('aria-label', label || 'Close inventory');
  Object.assign(button.style, {left:`${bounds.xMin/12.8}%`,top:`${bounds.yMin/7.2}%`,
    width:`${(bounds.xMax-bounds.xMin)/12.8}%`,height:`${(bounds.yMax-bounds.yMin)/7.2}%`});
  if (tint) button.style.color = '#' + tint.toString(16).padStart(6,'0');
  button.addEventListener('click', click);
  if (hover) { button.addEventListener('pointerenter', hover); button.addEventListener('focus', hover); }
  document.getElementById('controls').appendChild(button);
}

function drawCard() {
  const item = itemById(selectedId), card = INV + '.mainCard';
  call(card, 'SetVisible_', Boolean(item));
  if (!item) return;
  call(card, 'SetTitle', String(item.manufacturer || '').toLowerCase(), escapeHtml(item.name), color(item), 'pistol',
    item.element === 'None' ? 'none' : String(item.element || 'none').toLowerCase(), state.slots.includes(item.id));
  call(card, 'TurnOffAllTopStats');
  // Accuracy is not derived from spread: that formula is not verified.
  const stats = [['Damage', item.damage, 0], ['Fire Rate', item.fireRate, 1],
    ['Reload Speed', item.reloadTime, 1], ['Magazine Size', item.magazine, 0]];
  stats.forEach(([label,value,decimals], index) => {
    call(card, 'SetTopStat', index, label, Number(value || 0).toFixed(decimals), 'none', '', 'none');
    text(`${card}.stat${index+1}.labelField`, label, 14, 0xa4e8f3);
    text(`${card}.stat${index+1}.mainField`, Number(value || 0).toFixed(decimals), 14);
    set(`${card}.stat${index+1}.arrow`, '_visible', false);
  });
  call(card, 'SetFunStats', ''); // No invented red text or elemental DPS.
  call(card, 'SetLevelRequirement', true, item.level <= state.level, false, `LEVEL REQUIREMENT: ${item.level}`);
  call(card, 'SetValue', 0); call(card, 'SetEridiumValue', 0);
  set(card + '.valueClip', '_visible', false); set(card + '.eridiumCounter', '_visible', false);
  call(card, 'SetHeight');
  const action = item.level <= state.level ? '[Enter] Equip' : `Requires level ${item.level}`;
  text(ROOT + '.tooltips.tooltips', `[1–4] Target slot: ${targetSlot+1}   ${action}   [Escape] Close`, 15, 0xa4e8f3);
}

function render() {
  if (!ready || !state) return;
  const layer = document.getElementById('controls'); layer.replaceChildren();
  const equipped = INV + '.equippedPanel';
  call(equipped, 'SetCellInfo', ...state.slots.map(id => itemById(id) ? color(itemById(id)) : 0), 0,0,0,0);
  for (let i=0;i<8;i++) {
    const cell = `${equipped}.cell${i+1}`, item = itemById(state.slots[i]);
    call(cell,'SetSoldOut',false);call(cell,'SetEmptyCell',!item);
    call(cell,'SetTrashFavoriteMark',0);
    call(cell,'SetSelected',i===targetSlot);
    // Thumbnail capture is still pending. Names are explicit temporary labels.
    if (i<4) hit(cell, item?.name || `Slot ${i+1}`, () => {targetSlot=i;select(item?.id || selectedId);render();},
      item ? () => select(item.id) : null, color(item));
  }
  const backpack = state.items.filter(item => !state.slots.includes(item.id));
  page = Math.min(page, Math.max(0, Math.ceil(backpack.length/6)-1));
  // Use the movie's imported cell art inside its storage panel. Its native
  // list data provider needs unimplemented thumbnail callbacks; no AS copied.
  const panel = INV + '.storagePanel';
  const panelBounds = call(panel+'.bkgd','getBounds',panel);
  for(let row=0;row<6;row++) {
    const path = `${panel}.owRow${row}`;
    call(path,'removeMovieClip');
    const item = backpack[page*6+row];
    if (!item) continue;
    call(panel,'attachMovie','inventory - cell',`owRow${row}`,2000+row);
    const cellBounds = call(path,'getBounds',path);
    set(path,'_x',panelBounds.xMin+24-cellBounds.xMin);
    set(path,'_y',panelBounds.yMin+75+row*66-cellBounds.yMin);
    call(path,'SetSoldOut',false);call(path,'SetEmptyCell',false);call(path,'SetRarityColor',color(item));
    call(path,'SetTrashFavoriteMark',0);call(path,'SetSelected',item.id===selectedId);
    hit(path,item.name,()=>{select(item.id);equip();},()=>select(item.id),color(item));
  }
  call(INV,'SetStorageInfoCardData',String(backpack.length));
  hit(ROOT+'.header.pcCloseButton','',closeInventory);
  if(page>0)hit(panel+'.moreUp','Previous',()=>{page--;render();});
  if((page+1)*6<backpack.length)hit(panel+'.moreDown','Next',()=>{page++;render();});
  drawCard();
  document.getElementById('loading').hidden = true;
  console.log(`OpenWillow Inventory state: ${state.items.length} items, ${backpack.length} backpack`);
}

window.addEventListener('keydown',event=>{
  if(event.key==='Escape'||event.key.toLowerCase()==='i'){event.preventDefault();closeInventory();}
  else if(/^[1-4]$/.test(event.key)){targetSlot=Number(event.key)-1;event.preventDefault();render();}
  else if(event.key==='Enter'){event.preventDefault();equip();}
});
window.addEventListener('resize',()=>render());

let enteredInventory=false;
player.ruffle().load({url:'UI_StatusMenu/harness.swf',base:'UI_StatusMenu/'}).catch(showError);
const timer=setInterval(()=>{
  if(typeof player.ow!=='function')return;
  const total=get(ROOT,'_totalframes');
  if(!total||get(ROOT,'_framesloaded')<total)return;
  if(!enteredInventory) {
    enteredInventory=true;player.ow(ROOT,'gotoAndStop','inventory');return;
  }
  if(!get(INV+'.storagePanel','_totalframes'))return;
  clearInterval(timer);
  // Imported clips finish onLoad after they first appear; allow their own
  // initialization and opening tween to complete before applying host state.
  setTimeout(initialize,700);
},100);
function initialize() {
  call(INV,'ConfigureForPlayer',0);
  call(INV+'.storagePanel','SetBackground','inventory');
  call(INV+'.storagePanel','SetSortLabel','BACKPACK');
  call(INV,'SetLeftRightArrowVisibility',false,false);
  call(INV+'.compareCard','SetVisible_',false);
  // These default movie counters are sample values, not host state.
  set(INV+'.ammo','_visible',false);set(INV+'.currencyPanel','_visible',false);
  call(INV+'.mainCard','SetVisible_',false);
  setTimeout(()=>{
    ready=true;render();
    setTimeout(render,400);
    console.log('OpenWillow Inventory movie ready');
  },400);
}
function showError(error) {
  clearInterval(timer);
  document.getElementById('loading').textContent='Inventory unavailable. Press Esc to close.';
  console.error('OpenWillow Inventory:',error);
}
setTimeout(()=>{if(!ready)showError('movie library did not initialize');},25000);
