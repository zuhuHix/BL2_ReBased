// Drive the installed game's StatusMenu Skills frame through its observed
// movie interface. Movie and skill data are served from ignored local/ui/run.
const player = window.RufflePlayer.newest().createPlayer();
document.body.appendChild(player);
player.ruffle().load({ url: 'UI_StatusMenu/harness.swf', base: 'UI_StatusMenu/' });

const ROOT = '_level1'; // The harness loads StatusMenu at level 1.
const SKILLS = ROOT + '.skills';
const STAGE_WIDTH = 1280;
const STAGE_HEIGHT = 720;
const query = new URLSearchParams(location.search);
const treeName = /^[a-z0-9_]+$/i.test(query.get('tree') || 'siren')
  ? (query.get('tree') || 'siren') : 'siren';
const call = (path, method, ...args) => player.ow(path, 'apply', method, args);
const get = (path, member) => player.ow(path, 'get', member);
const set = (path, member, value) => player.ow(path, 'set', member, value);

let data;
let ready = false;
let loadedBranches = new Set();
let points = Math.max(0, Number(query.get('points') || 0) || 0);
let actionGrade = Math.max(0, Number(query.get('action') || 0) || 0);
let grades = {};
let bonuses = {}; // class-mod bonus grades by skill id, from the host
let classModText = '';
let selected = null; // the highlighted skill: { skill, branch, tier, cell, highlight }
let selectedBranch = 1; // Harmony is in the middle on the movie's initial frame.
let hitTargets = [];
let displayedStates = new Map();
// Q toggles the overview: the three trees side by side at one size, no selection and no info box (the
// original's SkillTreeGFxObject.ToggleOverviewMode; arrows and Enter do nothing there). The numbers are the
// installed UI_SkillTree.Definitions.Gfx_SkillTree defaults (OverviewOffset.X 235, OverviewGlobalOffset.X -50,
// OverviewScale 85); how the game combines them with the selected tree's X is not read, so the layout below is
// a fit (UNVERIFIED) checked by eye against a third-party overview screenshot.
let overview = false;
const OVERVIEW_HINT_DROP = 15, OVERVIEW_LOCKED_ALPHA = 55; // host choices read off the overview screenshot
const OVERVIEW = {offsetX:265, globalX:5, scale:95, y:50}; // offset/global/scale defaults are 235/-50/85; enlarged to the third-party overview screenshot (fit)

// Open-time instrumentation, the same "OWINVTIME js_<event> ..." lines inventory.js sends through the
// console bridge. The host preloads this page hidden at level start (as it does the inventory page) and
// calls owSkillsOpened(<Unix ms of the open request>) when the player opens it, so `populated` is true
// at js_skills_open once the preload has finished. js_skills_painted is two animation frames later.
const startupAt = performance.now();
let openTiming = null, populatedAt = 0;
function timeLog(event, extra = '') {
  const since = openTiming ? Date.now() - openTiming.hostEpoch : -1;
  console.log(`OWINVTIME js_${event} sinceOpen=${since} pageMs=${Math.round(performance.now() - startupAt)} epoch=${Date.now()} ready=${ready} ${extra}`);
}
timeLog('skills_page_start');

function setGradeText(path, value, colour = null) {
  // Some movie text fields embed a digits-only WillowBody subset. Selecting
  // its full imported alias also renders the slash in ranks such as 0/5.
  const color = colour || (Number(get(path, 'textColor')) || 0xb6cee2).toString(16).padStart(6, '0');
  set(path, 'html', true);
  set(path, 'htmlText', `<font face="$WillowBody" size="13" color="#${color}">${value}</font>`);
}

// Host state may arrive before the movie loads. The host owns level, skill
// points and grades; this page only presents them and reports clicks with
// reportSpend. ?points=N&action=N is for a standalone visual check.
window.owSkills = state => {
  let ranksChanged = false;
  if (typeof state.points === 'number' && Number.isFinite(state.points))
    points = Math.max(0, Math.floor(state.points));
  if (typeof state.actionGrade === 'number' && Number.isFinite(state.actionGrade)) {
    actionGrade = Math.max(0, Math.floor(state.actionGrade));
    ranksChanged = true;
  }
  if (state.grades && typeof state.grades === 'object' && !Array.isArray(state.grades)) {
    grades = state.grades;
    ranksChanged = true;
  }
  if (state.bonuses && typeof state.bonuses === 'object' && !Array.isArray(state.bonuses)) {
    bonuses = state.bonuses;
    ranksChanged = true;
  }
  if (typeof state.classModText === 'string') {
    classModText = state.classModText;
    if (ready) call(SKILLS, 'SetCharacter', classModText, data.className, data.portrait);
  }
  if (ready) call(SKILLS, 'SetSkillPoints', points);
  if (ready && ranksChanged) renderRanks();
  if (ready) refreshSelection();
};
window.owPlayer = player; // Useful for local inspection, not a game interface.

// The host shows the (already loaded) page again. Open on the action skill and the middle tree, as a
// fresh page does, then report when a frame of the populated screen has been composited.
window.owSkillsOpened = hostEpoch => {
  openTiming = { hostEpoch };
  timeLog('skills_open', `populated=${ready} size=${innerWidth}x${innerHeight} visibility=${document.visibilityState}`);
  if (ready) {
    if (overview) toggleOverview();
    select(actionTarget());
    updateBranch(1, true);
    refreshSelection();
  }
  const timing = openTiming;
  requestAnimationFrame(() => requestAnimationFrame(() => {
    if (timing === openTiming) timeLog('skills_painted', `populated=${ready}`);
  }));
};

// A click on a skill, reported the way the movie's own extCellClicked
// reports it: (branch, tier, cell), with -1, -1, -1 for the action skill
// (observed in a real-game UI trace). The UE host reads this console line,
// validates the spend and answers with owSkills(state).
function reportSpend(branch, tier, cell) {
  console.log('OWSKILL ' + JSON.stringify({ branch, tier, cell }));
}
window.owCellClicked = (branch, tier, cell) => {
  if ([branch, tier, cell].every(Number.isInteger)) reportSpend(branch, tier, cell);
};

// Selection, as traced: the highlight clip goes to "over" ("over_KillSkill"
// for kill skills) and the cell tweens to Z 200 over 0.2 s; the previous one
// goes back to "up" and Z 0. The screen opens with the action skill selected.
function actionTarget() {
  return { skill: data.actionSkill, branch: -1, tier: -1, cell: `${SKILLS}.ActiveAbility`,
    highlight: `${SKILLS}.ActiveAbility.highlight` };
}

function select(target) {
  if (!ready || !target) return;
  if (!selected || selected.cell !== target.cell) {
    if (selected) {
      player.ow(selected.highlight, 'gotoAndStop', 'up');
      call(selected.cell, 'TweenZPos', 0, 0.2);
    }
    player.ow(target.highlight, 'gotoAndStop', target.skill.killSkill ? 'over_KillSkill' : 'over');
    call(target.cell, 'TweenZPos', 200, 0.2);
    selected = target;
  }
  refreshSelection();
}

function skillState(target) {
  if (target.branch < 0) return { grade: Math.min(data.actionSkill.maxGrade, actionGrade), locked: false, bonus: 0 };
  return { grade: rank(target.skill), locked: !tierOpen(target.branch, target.tier),
    bonus: Math.max(0, Math.floor(Number(bonuses[target.skill.id]) || 0)) };
}

function canSpend(target) {
  const state = skillState(target);
  return points > 0 && !state.locked && state.grade < target.skill.maxGrade;
}

// Info box text (tools/hud_overlay/skill_info.js reproduces the traced
// SetInfo HTML) and the footer, built from the install's own strings.
// The real description text is a little smaller than the movie's default as scaled by the card fit (critic round 12).
const DESCRIPTION_SIZE = 16;
function refreshSelection() {
  if (!ready) return;
  applyInfoCard();
  if (!selected) return;
  if (overview) { showTips(); return; }
  // The info box's embedded font is a subset without ' : + %. Scaleform falls
  // back to the imported font library for missing glyphs and Ruffle does not,
  // so the page selects the imported $WillowBody alias itself (as for badges).
  const face = html => `<font face="$WillowBody">${html}</font>`;
  const description = html => `<font face="$WillowBody" size="${DESCRIPTION_SIZE}">${html}</font>`;
  const name = selected.skill.name || '';
  call(SKILLS + '.InformationBox', 'SetInfo', name,
    description(skillInfoHtml(selected.skill, skillState(selected), data.strings)));
  // SetInfo writes the name as plain text; re-set it as HTML for the font.
  const escaped = name.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  set(`${SKILLS}.InformationBox.infoWrapper.SkillName`, 'htmlText', face(escaped));
  showTips();
}

// Footer: the original reads "[Q] Toggle Overview   [Escape] Close" (2026-10-04 capture); the spend hint is the
// host's addition while a point can be spent.
function showTips() {
  const tips = [!overview && selected && canSpend(selected) ? data.strings.spendPoint : '', '[Q] Toggle Overview',
    data.strings.close].filter(Boolean);
  set(`${ROOT}.tooltips.tooltips`, 'htmlText',
    `<font face="$WillowBody" size="15" color="#a4e8f3">${tips.join('   ')}</font>`);
}

function toggleOverview() {
  if (!ready) return;
  overview = !overview;
  if (overview && selected) {
    player.ow(selected.highlight, 'gotoAndStop', 'up');
    call(selected.cell, 'TweenZPos', 0, 0.2);
    selected = null;
  }
  set(`${SKILLS}.InformationBox`, '_visible', !overview);
  // The footer sits clearly below the trees in the overview, and tiers that are not open yet are drawn dimmer.
  set(`${ROOT}.tooltips`, '_y', Number(get(`${ROOT}.tooltips`, '_y')) + (overview ? OVERVIEW_HINT_DROP : -OVERVIEW_HINT_DROP));
  for (const hit of hitTargets) set(hit.cell, '_alpha', overview && !tierOpen(hit.branch, hit.tier) ? OVERVIEW_LOCKED_ALPHA : 100);
  if (overview) updateOverview(); else { select(actionTarget()); updateBranch(selectedBranch); }
  showTips();
  layoutHits();
  setTimeout(layoutHits, 600);
}

function updateOverview(immediate = false) {
  call(SKILLS, 'BubbleSortBranchDepths', selectedBranch + 1);
  data.branches.forEach((_, i) => {
    const x = 15 + OVERVIEW.globalX + OVERVIEW.offsetX * (i - 1);
    call(SKILLS, 'TweenBranch', i + 1, immediate, 0.3, x, OVERVIEW.y, 0, OVERVIEW.scale, OVERVIEW.scale, 100);
  });
}

function spendSelected() {
  if (selected && canSpend(selected)) reportSpend(selected.branch, selected.tier, selected.branch < 0 ? -1 : selected.skill.cell);
}

function drawBranch(index, branch) {
  call(SKILLS, 'SetTreeName', index, branch.name);
  branch.tiers.forEach((tier, tierIndex) => {
    tier.skills.forEach(skill => {
      const cell = `${SKILLS}.Tree${index + 1}.SkillRow${tierIndex + 1}.Cell${skill.cell + 1}`;
      call(SKILLS, 'SetCellVisible', index, tierIndex, skill.cell);
      hitTargets.push({ skill, branch: index, tier: tierIndex, cell,
        highlight: `${SKILLS}.Tree${index + 1}.SkillRow${tierIndex + 1}.Highlight${skill.cell + 1}` });
    });
  });
}

function rank(skill) {
  const value = Number(grades[skill.id]) || 0;
  return Math.min(skill.maxGrade, Math.max(0, Math.floor(value)));
}

function invested(branchIndex) {
  return data.branches[branchIndex].tiers.reduce((sum, tier) =>
    sum + tier.skills.reduce((tierSum, skill) => tierSum + rank(skill), 0), 0);
}

// The host's rule: trees open once the action skill has its unlock points
// (the traced info text is grey for tree skills before that), and a tier
// opens once its branch holds every lower tier's points.
function treesOpen() {
  return actionGrade >= (data.actionSkillPoints ?? 1);
}

function tierOpen(branchIndex, tierIndex) {
  const tiers = data.branches[branchIndex].tiers;
  const required = tiers.slice(0, tierIndex).reduce((sum, tier) => sum + (tier.pointsToUnlockNext || 0), 0);
  return treesOpen() && invested(branchIndex) >= required;
}

// ProgressBackground frame at the bottom of each tier row, measured in
// Ruffle (the lit band grows about 0.43 px per frame). The maintainer's
// real-game capture has 10 points in Harmony and the band ending at the
// bottom of tier 3, the deepest open tier. Filling part of the way toward the
// next tier is an assumption (UNVERIFIED).
const PROGRESS_FRAMES = [173, 333, 490, 646, 815, 962];
function progressFrame(branchIndex) {
  if (!treesOpen()) return 1;
  const tiers = data.branches[branchIndex].tiers, spent = invested(branchIndex);
  let required = 0;
  for (let k = 0; k < tiers.length; k++) {
    const next = required + (tiers[k].pointsToUnlockNext || 0);
    const here = PROGRESS_FRAMES[Math.min(k, PROGRESS_FRAMES.length - 1)];
    if (k === tiers.length - 1 || spent < next) {
      if (k === tiers.length - 1 || next <= required) return here;
      const toward = PROGRESS_FRAMES[Math.min(k + 1, PROGRESS_FRAMES.length - 1)];
      return Math.round(here + (toward - here) * (spent - required) / (next - required));
    }
    required = next;
  }
  return 1;
}

function renderRanks() {
  // The movie owns the badge art and colour frames. The host owns grades.
  data.branches.forEach((branch, branchIndex) => {
    const frame = progressFrame(branchIndex);
    if (displayedStates.get(`progress${branchIndex}`) !== frame) {
      call(SKILLS, 'SetBranchProgression', branchIndex, frame);
      displayedStates.set(`progress${branchIndex}`, frame);
    }
    branch.tiers.forEach((tier, tierIndex) => {
      const open = tierOpen(branchIndex, tierIndex);
      for (const skill of tier.skills) {
        const grade = rank(skill);
        const status = grade >= skill.maxGrade ? 'maxed'
          : grade > 0 ? 'some' : open ? 'enabled' : 'disabled';
        const cell = `${SKILLS}.Tree${branchIndex + 1}.SkillRow${tierIndex + 1}.Cell${skill.cell + 1}`;
        const state = `${skill.killSkill ? 'KillSkill_' : ''}${status}`;
        if (displayedStates.get(cell) !== state) {
          call(cell, 'SetState', state);
          if (skill.icon) call(`${cell}.iconContainer`, 'loadMovie', skill.icon);
          displayedStates.set(cell, state);
        }
        // A trained skill's badge counts class-mod grades, drawn in the
        // class-mod cyan (9/5 in the maintainer's capture; colour by eye).
        const bonus = grade > 0 ? Math.max(0, Math.floor(Number(bonuses[skill.id]) || 0)) : 0;
        if (status !== 'disabled')
          setGradeText(`${cell}.points`, `${grade + bonus}/${skill.maxGrade}`, bonus ? '33ffff' : null);
      }
    });
  });
  const grade = Math.min(data.actionSkill.maxGrade, actionGrade);
  const action = `${SKILLS}.ActiveAbility.BackgroundState`;
  const actionState = grade >= data.actionSkill.maxGrade ? 4 : grade > 0 ? 3 : points ? 2 : 1;
  if (displayedStates.get(action) !== actionState) {
    player.ow(action, 'gotoAndStop', actionState);
    if (data.actionSkill.icon) call(`${action}.iconContainer`, 'loadMovie', data.actionSkill.icon);
    displayedStates.set(action, actionState);
  }
  if (grade || points) setGradeText(`${action}.points`, `${grade}/${data.actionSkill.maxGrade}`);
}

// Branch layout from the real-game UI trace (DECISIONS.md 2026-09-27): the
// trees stay in one row and slide; the selected one comes forward. For offset
// d from the selected branch the game tweens X = 15 + 330d, Y = 17,
// Z = -5500|d|, scale 100, alpha 100 - 15|d|, and first calls the movie's
// BubbleSortBranchDepths(selected + 1) so the front tree draws on top.
// Ruffle ignores the Z coordinate, so the page projects it in 2D: scale by
// f = PERSPECTIVE / (PERSPECTIVE + |Z|) about PROJECTION (parent-local
// coordinates). Both constants were fitted to one real-game capture
// (Cataclysm at 70.6% behind Harmony); they are not read from the movie.
const PERSPECTIVE = 13200;
const PROJECTION = { x: -209, y: 12 };
function branchTween(offset) {
  const x = 15 + 330 * offset, y = 17, z = -5500 * Math.abs(offset);
  const f = PERSPECTIVE / (PERSPECTIVE - z);
  return {
    x: PROJECTION.x + (x - PROJECTION.x) * f,
    y: PROJECTION.y + (y - PROJECTION.y) * f,
    scale: 100 * f,
    alpha: 100 - 15 * Math.abs(offset),
  };
}

function updateBranch(which, immediate = false) {
  if (!ready) return;
  // The trace never pressed an arrow at either end, so whether the row wraps
  // there is unobserved (UNVERIFIED); a row with a fixed order suggests not.
  selectedBranch = Math.max(0, Math.min(data.branches.length - 1, which));
  if (overview) return;
  call(SKILLS, 'BubbleSortBranchDepths', selectedBranch + 1);
  data.branches.forEach((_, i) => {
    const t = branchTween(i - selectedBranch);
    call(SKILLS, 'TweenBranch', i + 1, immediate, 0.3, t.x, t.y, 0, t.scale, t.scale, t.alpha);
    // The tree left of the selected one has its name behind the Siren plate in the original.
    set(`${SKILLS}.Tree${i + 1}.TreeName`, '_visible', !(i < selectedBranch));
  });
  setTimeout(layoutHits, immediate ? 50 : 600);
}

function addHit(path, onEnter, onClick, label, parent, kind = '') {
  const bounds = call(path, 'getBounds', ROOT);
  if (!bounds || !Number.isFinite(bounds.xMin) || !Number.isFinite(bounds.yMin)) return;
  const button = document.createElement('button');
  button.type = 'button';
  button.className = `movie-hit ${kind}`;
  button.setAttribute('aria-label', label);
  button.style.left = `${100 * bounds.xMin / STAGE_WIDTH}%`;
  button.style.top = `${100 * bounds.yMin / STAGE_HEIGHT}%`;
  button.style.width = `${100 * (bounds.xMax - bounds.xMin) / STAGE_WIDTH}%`;
  button.style.height = `${100 * (bounds.yMax - bounds.yMin) / STAGE_HEIGHT}%`;
  if (onEnter) {
    button.addEventListener('pointerenter', onEnter);
    button.addEventListener('focus', onEnter);
  }
  if (onClick) button.addEventListener('click', onClick);
  parent.appendChild(button);
}

function layoutHits() {
  if (!ready) return;
  const layer = document.getElementById('hit-layer');
  layer.replaceChildren();
  // ActiveAbility's bounds reach over the first tier (an invisible child), so
  // it goes first and the skill cells stack above it.
  // Hover selects (the movie reports extCellRolledOver in the game); a click
  // spends, as the traced extCellClicked on release does.
  const action = actionTarget();
  if (!overview) {
    addHit(action.cell, () => select(action), () => { select(action); spendSelected(); },
      data.actionSkill.name, layer);
    for (const hit of hitTargets.filter(hit => hit.branch === selectedBranch))
      addHit(hit.cell, () => select(hit), () => { select(hit); spendSelected(); },
        hit.skill.name, layer, 'skill-hit');
  }
  // The tree arrows are hidden in the overview, as in the original.
  set(`${SKILLS}.arrowLeft`, '_visible', !overview);
  set(`${SKILLS}.arrowRight`, '_visible', !overview);
  if (!overview) {
    addHit(`${SKILLS}.arrowLeft`, null, () => updateBranch(selectedBranch - 1),
      'Previous skill tree', layer);
    addHit(`${SKILLS}.arrowRight`, null, () => updateBranch(selectedBranch + 1),
      'Next skill tree', layer);
  }
  // This route is intercepted by the UE browser before it navigates.
  addHit(`${ROOT}.header.pcCloseButton`, null, closeSkills, 'Close skills', layer);
  // Header tabs (nav1..nav5 = Missions, Map, Inventory, Skills, Challenges). Only Inventory is another
  // page this host has; the UE browser intercepts the route. The others have no host data yet.
  addHit(`${ROOT}.header.nav3`, null, () => { location.href = '/__ow_tab_inventory'; }, 'Inventory tab', layer);
}

function closeSkills() {
  if (window.owCloseSkills) window.owCloseSkills();
  else location.href = '/__ow_close_skills';
}

window.addEventListener('keydown', event => {
  if (event.key === 'Escape' || event.key.toLowerCase() === 'k' || event.key === 'Tab') {
    event.preventDefault();
    closeSkills();
  } else if (event.key.toLowerCase() === 'q') {
    event.preventDefault();
    toggleOverview();
  } else if (event.key.toLowerCase() === 'i') {
    event.preventDefault();
    location.href = '/__ow_tab_inventory';
  } else if (event.key === 'Enter') {
    event.preventDefault();
    if (!overview) spendSelected();
  } else if (event.key === 'ArrowLeft') {
    event.preventDefault();
    if (!overview) updateBranch(selectedBranch - 1);
  } else if (event.key === 'ArrowRight') {
    event.preventDefault();
    if (!overview) updateBranch(selectedBranch + 1);
  }
});
window.addEventListener('resize', () => { if (ready) layoutHits(); });

window.owInitTree = (number, path) => {
  if (path !== `${SKILLS}.Tree${number + 1}`) return;
  loadedBranches.add(number);
  console.log(`OpenWillow Skills tree initialized: ${number + 1}/3`);
  if (loadedBranches.size === 3) populate();
};

function populate() {
  if (ready || !data) return;
  call(SKILLS, 'SetCharacter', classModText, data.className, data.portrait);
  call(SKILLS, 'SetSkillPoints', points);
  call(SKILLS, 'SetAllSkillIconsInvisible');
  applySkillsLayout();
  data.branches.forEach((branch, index) => drawBranch(index, branch));
  // The highlight clips start on their cyan "outline" frame; the original shows that only on the selected tile
  // (locked tiles have a plain dark border in the 2026-10-04 capture).
  for (const hit of hitTargets) player.ow(hit.highlight, 'gotoAndStop', 'up');
  call(`${SKILLS}.InformationBox`, 'SetRemainingPointsTitle', data.pointsTitle);
  ready = true;
  renderRanks();
  select(actionTarget());
  updateBranch(selectedBranch);
  call(`${ROOT}.sway`, 'BeginSway');
  layoutHits();
  setTimeout(layoutHits, 600); // The movie's opening branch tween moves the hit areas.
  setTimeout(() => {
    const loading = document.getElementById('loading');
    loading.classList.add('finished');
    setTimeout(() => { loading.hidden = true; }, 300);
  }, 700);
  populatedAt = performance.now() - startupAt;
  timeLog('skills_populated', `branches=${data.branches.length} skills=${hitTargets.length}`);
  console.log(`OpenWillow Skills movie ready: ${data.branches.length} branches, ${hitTargets.length} skills`);
}

// Placement measured on the 2026-10-04 real capture (1280x720): the header group is a little smaller and centred about
// x 695; the trees column sits 130 px right of where the harness puts it; the Phaselock card is 320 px wide at (205, 140);
// the Siren / Skill Points block is a child of the card clip, so it grows with it (the original's own 20% bigger plate sits
// at (225, 530); here it follows the card). Moves are done on the movie clips in
// ROOT coordinates (a host fit; the original gets these from its 3D camera).
const SKILLS_LAYOUT = {headerScale:1.02, headerDY:5, headerCentreX:707, treesDX:130, treesDY:25, hintDY:56, hintDX:137, // the real footer is centred near x 575 and sits at y 657, clear of the Siren plate
  card:{left:205, top:140, width:320, boundsShare:0.867, insetLeft:0.0685, insetTop:0.0106}};
let layoutApplied = false;
function boundsOf(path) { const b = call(path, 'getBounds', ROOT); return b && Number.isFinite(b.xMin) && b.xMax > b.xMin ? b : null; }
function moveClip(path, parent, dx, dy) {
  const scale = Number(get(parent, '_xscale')) / 100 || 1;
  set(path, '_x', Number(get(path, '_x')) + dx / scale);
  set(path, '_y', Number(get(path, '_y')) + dy / scale);
}
// The Phaselock card (InformationBox) is fitted by the bounds of its background clip, whose margin around the visible frame is
// known from the round-18/19 frames (the visible card is 0.867 of those bounds' width, starts 0.0685 of it from the left and
// 0.0106 from the top). The movie resets this clip's scale and position when it tweens the trees, so the fit is applied
// again on every selection and when the page opens; it changes nothing once the card is within a pixel.
const CARD_Y_STRETCH = 1.045; const PLATE_UP = 22; // the card ended at y 380 against 390 in the real frame
let wrapperHomeY = null;
function applyInfoCard() {
  const info = `${SKILLS}.InformationBox`, wrapper = `${info}.infoWrapper`, bkgd = `${wrapper}.DescriptionBkgd`, card = SKILLS_LAYOUT.card;
  // The Siren plate is a sibling of the card (infoWrapper) inside the clip, 18-33 px lower than the real one; the card is moved down
  // inside the clip by PLATE_UP and the clip up by the same amount, so the card stays and the plate rises.
  if (wrapperHomeY === null) { const y = Number(get(wrapper, '_y')); if (Number.isFinite(y)) wrapperHomeY = y; }
  if (wrapperHomeY !== null) {
    const unit = (Number(get(info, '_yscale')) / 100 || 1) * (Number(get(SKILLS, '_xscale')) / 100 || 1);
    set(wrapper, '_y', wrapperHomeY + PLATE_UP / unit);
  }
  const b = boundsOf(bkgd);
  if (!b) return;
  const boundsWidth = card.width / card.boundsShare;
  if (Math.abs((b.xMax - b.xMin) - boundsWidth) > 1.5) {
    const factor = boundsWidth / (b.xMax - b.xMin);
    if (!(factor > 0.3 && factor < 4)) return;
    set(info, '_xscale', Number(get(info, '_xscale')) * factor);
    set(info, '_yscale', Number(get(info, '_yscale')) * factor * CARD_Y_STRETCH);
  }
  const f = boundsOf(bkgd);
  if (!f) return;
  const dx = card.left - card.insetLeft * boundsWidth - f.xMin, dy = card.top - card.insetTop * boundsWidth - f.yMin;
  if (Math.abs(dx) > 1 || Math.abs(dy) > 1) moveClip(info, SKILLS, dx, dy);
}
function applySkillsLayout() {
  if (layoutApplied) return;
  layoutApplied = true;
  const header = `${ROOT}.header`;
  set(header, '_xscale', SKILLS_LAYOUT.headerScale * 100);
  set(header, '_yscale', SKILLS_LAYOUT.headerScale * 100);
  const hb = boundsOf(header);
  if (hb) moveClip(header, ROOT, SKILLS_LAYOUT.headerCentreX - (hb.xMin + hb.xMax) / 2, SKILLS_LAYOUT.headerDY);
  moveClip(SKILLS, ROOT, SKILLS_LAYOUT.treesDX, SKILLS_LAYOUT.treesDY);
  moveClip(`${ROOT}.tooltips`, ROOT, SKILLS_LAYOUT.hintDX, SKILLS_LAYOUT.hintDY); // the real footer is at y 657, below the Siren plate
  applyInfoCard();
  setTimeout(applyInfoCard, 700); setTimeout(applyInfoCard, 1500);
}

async function boot() {
  const response = await fetch(`skilltree_${treeName}.json`);
  if (!response.ok) throw new Error(`Missing local skill tree: skilltree_${treeName}.json`);
  data = await response.json();
  const timer = setInterval(() => {
    try {
      if (typeof player.ow !== 'function') return;
      const total = get(ROOT, '_totalframes');
      if (!total || get(ROOT, '_framesloaded') < total) return;
      clearInterval(timer);
      console.log(`OpenWillow Skills StatusMenu loaded: ${total} frames`);
      player.ow(ROOT, 'gotoAndStop', 'skills');
      player.ow(SKILLS, 'forward', 'extInitTree', 'owInitTree');
      // The page's hit targets cover the cells, so this fires only if the
      // movie itself sees a release; that was not observed in the UE check.
      player.ow(SKILLS, 'forward', 'extCellClicked', 'owCellClicked');
    } catch (error) {
      console.warn('Waiting for StatusMenu:', error);
    }
  }, 100);
  setTimeout(() => {
    if (!ready) {
      document.querySelector('#loading small').textContent = 'Could not load the skill tree. Press Esc to return.';
      console.error('OpenWillow Skills: movie did not initialize all three branches');
    }
  }, 25000);
}

boot().catch(error => {
  document.querySelector('#loading small').textContent = 'Could not load the skill tree. Press Esc to return.';
  console.error('OpenWillow Skills:', error);
});
