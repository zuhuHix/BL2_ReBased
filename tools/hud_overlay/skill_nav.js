// Pure skill-tree focus movement, shared by the browser adapter and Node tests.
// UNVERIFIED fit: an action skill sits over column 1; vertical movement chooses
// the closest occupied column in the adjacent tier, ties go to the lower cell,
// and a missing tier or tree edge stops movement. Only the four-Down captured
// destination has been checked against the original game so far.
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.owSkillNav = api;
})(globalThis, function () {
  function nearestSkill(skills, column) {
    let best = null;
    for (const skill of skills) {
      if (!Number.isInteger(skill.cell)) continue;
      const distance = Math.abs(skill.cell - column);
      if (!best || distance < best.distance ||
          (distance === best.distance && skill.cell < best.skill.cell)) {
        best = { skill, distance };
      }
    }
    return best && best.skill;
  }

  function move(branches, branchIndex, selected, direction) {
    if (!Array.isArray(branches) || !Number.isInteger(branchIndex) || !selected ||
        (direction !== 'up' && direction !== 'down')) return null;
    const tiers = branches[branchIndex]?.tiers;
    if (!Array.isArray(tiers)) return null;

    const fromAction = selected.branch < 0;
    const currentTier = fromAction ? -1 : selected.tier;
    if (!Number.isInteger(currentTier)) return null;
    const nextTier = currentTier + (direction === 'down' ? 1 : -1);
    if (nextTier < -1) return null;
    if (nextTier === -1) return fromAction ? null : { branch: -1, tier: -1, cell: -1 };
    if (nextTier >= tiers.length) return null;

    const skills = tiers[nextTier]?.skills;
    if (!Array.isArray(skills) || skills.length === 0) return null;
    const column = fromAction ? 1 : selected.skill?.cell;
    if (!Number.isInteger(column)) return null;
    const target = nearestSkill(skills, column);
    return target ? { branch: branchIndex, tier: nextTier, cell: target.cell } : null;
  }

  return { move };
});
