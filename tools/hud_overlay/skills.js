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
let classModText = '';
let selectedBranch = 1; // Harmony is in the middle on the movie's initial frame.
let hitTargets = [];
let displayedStates = new Map();

function setGradeText(path, value) {
  // Some movie text fields embed a digits-only WillowBody subset. Selecting
  // its full imported alias also renders the slash in ranks such as 0/5.
  const color = (Number(get(path, 'textColor')) || 0xb6cee2).toString(16).padStart(6, '0');
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
  if (typeof state.classModText === 'string') {
    classModText = state.classModText;
    if (ready) call(SKILLS, 'SetCharacter', classModText, data.className, data.portrait);
  }
  if (ready) call(SKILLS, 'SetSkillPoints', points);
  if (ready && ranksChanged) renderRanks();
};
window.owPlayer = player; // Useful for local inspection, not a game interface.

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

function showInfo(skill) {
  if (!skill || !ready) return;
  const description = (skill.description || '').replaceAll('<StringAliasMap:Action.ActionSkill>', 'F');
  call(SKILLS + '.InformationBox', 'SetInfo', skill.name || '', description);
}

function drawBranch(index, branch) {
  call(SKILLS, 'SetTreeName', index, branch.name);
  branch.tiers.forEach((tier, tierIndex) => {
    tier.skills.forEach(skill => {
      const cell = `${SKILLS}.Tree${index + 1}.SkillRow${tierIndex + 1}.Cell${skill.cell + 1}`;
      call(SKILLS, 'SetCellVisible', index, tierIndex, skill.cell);
      hitTargets.push({ path: cell, skill, branch: index, tier: tierIndex });
    });
  });
}

function rank(skill) {
  const value = Number(grades[skill.id]) || 0;
  return Math.min(skill.maxGrade, Math.max(0, Math.floor(value)));
}

function renderRanks() {
  // The movie owns the badge art and colour frames. The host owns grades.
  // Tiers show as enabled by the host's rule: the action skill has its
  // unlock points and the branch holds every lower tier's points.
  const treesOpen = actionGrade >= (data.actionSkillPoints ?? 1);
  data.branches.forEach((branch, branchIndex) => {
    const invested = branch.tiers.reduce((sum, tier) =>
      sum + tier.skills.reduce((tierSum, skill) => tierSum + rank(skill), 0), 0);
    let required = 0;
    branch.tiers.forEach((tier, tierIndex) => {
      for (const skill of tier.skills) {
        const grade = rank(skill);
        const status = grade >= skill.maxGrade ? 'maxed'
          : grade > 0 ? 'some' : treesOpen && invested >= required ? 'enabled' : 'disabled';
        const cell = `${SKILLS}.Tree${branchIndex + 1}.SkillRow${tierIndex + 1}.Cell${skill.cell + 1}`;
        const state = `${skill.killSkill ? 'KillSkill_' : ''}${status}`;
        if (displayedStates.get(cell) !== state) {
          call(cell, 'SetState', state);
          if (skill.icon) call(`${cell}.iconContainer`, 'loadMovie', skill.icon);
          displayedStates.set(cell, state);
        }
        if (status !== 'disabled') setGradeText(`${cell}.points`, `${grade}/${skill.maxGrade}`);
      }
      required += tier.pointsToUnlockNext || 0;
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
  call(SKILLS, 'BubbleSortBranchDepths', selectedBranch + 1);
  data.branches.forEach((_, i) => {
    const t = branchTween(i - selectedBranch);
    call(SKILLS, 'TweenBranch', i + 1, immediate, 0.3, t.x, t.y, 0, t.scale, t.scale, t.alpha);
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
  if (onEnter) button.addEventListener('pointerenter', onEnter);
  if (onClick) button.addEventListener('click', onClick);
  parent.appendChild(button);
}

function layoutHits() {
  if (!ready) return;
  const layer = document.getElementById('hit-layer');
  layer.replaceChildren();
  // ActiveAbility's bounds reach over the first tier (an invisible child), so
  // it goes first and the skill cells stack above it.
  addHit(`${SKILLS}.ActiveAbility`, () => showInfo(data.actionSkill), () => {
    showInfo(data.actionSkill);
    reportSpend(-1, -1, -1);
  }, data.actionSkill.name, layer);
  for (const hit of hitTargets.filter(hit => hit.branch === selectedBranch))
    addHit(hit.path, () => showInfo(hit.skill), () => {
      showInfo(hit.skill);
      reportSpend(hit.branch, hit.tier, hit.skill.cell);
    }, hit.skill.name, layer, 'skill-hit');
  addHit(`${SKILLS}.arrowLeft`, null, () => updateBranch(selectedBranch - 1),
    'Previous skill tree', layer);
  addHit(`${SKILLS}.arrowRight`, null, () => updateBranch(selectedBranch + 1),
    'Next skill tree', layer);
  // This route is intercepted by the UE browser before it navigates.
  addHit(`${ROOT}.header.pcCloseButton`, null, closeSkills, 'Close skills', layer);
}

function closeSkills() {
  if (window.owCloseSkills) window.owCloseSkills();
  else location.href = '/__ow_close_skills';
}

window.addEventListener('keydown', event => {
  if (event.key === 'Escape' || event.key.toLowerCase() === 'k') {
    event.preventDefault();
    closeSkills();
  } else if (event.key === 'ArrowLeft') {
    event.preventDefault();
    updateBranch(selectedBranch - 1);
  } else if (event.key === 'ArrowRight') {
    event.preventDefault();
    updateBranch(selectedBranch + 1);
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
  data.branches.forEach((branch, index) => drawBranch(index, branch));
  call(`${SKILLS}.InformationBox`, 'SetRemainingPointsTitle', data.pointsTitle);
  renderRanks();
  set(`${ROOT}.tooltips.tooltips`, 'htmlText',
    '<font face="$WillowBody" size="15" color="#a4e8f3">[LEFT/RIGHT] Rotate Trees     [ESC] Close</font>');
  ready = true;
  showInfo(data.actionSkill);
  updateBranch(selectedBranch);
  call(`${ROOT}.sway`, 'BeginSway');
  layoutHits();
  setTimeout(layoutHits, 600); // The movie's opening branch tween moves the hit areas.
  setTimeout(() => {
    const loading = document.getElementById('loading');
    loading.classList.add('finished');
    setTimeout(() => { loading.hidden = true; }, 300);
  }, 700);
  console.log(`OpenWillow Skills movie ready: ${data.branches.length} branches, ${hitTargets.length} skills`);
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
