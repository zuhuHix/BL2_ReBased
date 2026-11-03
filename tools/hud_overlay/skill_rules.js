// Pure skill-tree spend rules, shared by the Skills page (skills.js) and Node tests. They restate the host's rule
// (UOpenWillowSkills::TrySpend, DECISIONS.md 2026-09-27) so the page can show which tiers and skills accept a point;
// the host still decides every spend.
//   - the trees open once the action skill holds the root tier's points (actionSkillPoints, 1 for Maya);
//   - a tier opens once its branch holds the sum of every lower tier's pointsToUnlockNext (5 per tier in the
//     installed trees, read from data);
//   - a spend needs a free point and a grade below the skill's maximum.
// tree: { actionSkill: {maxGrade}, actionSkillPoints, branches: [{tiers: [{pointsToUnlockNext, skills: [{id, maxGrade}]}]}] }
// state: { points, actionGrade, grades: {id: grade} }
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.owSkillRules = api;
})(globalThis, function () {
  function rank(state, skill) {
    const value = Number(state.grades && state.grades[skill.id]) || 0;
    return Math.min(skill.maxGrade, Math.max(0, Math.floor(value)));
  }

  function invested(tree, state, branchIndex) {
    return tree.branches[branchIndex].tiers.reduce((sum, tier) =>
      sum + tier.skills.reduce((tierSum, skill) => tierSum + rank(state, skill), 0), 0);
  }

  function treesOpen(tree, state) {
    return (state.actionGrade || 0) >= (tree.actionSkillPoints ?? 1);
  }

  // Points the branch must hold before tierIndex opens.
  function tierRequirement(tree, branchIndex, tierIndex) {
    return tree.branches[branchIndex].tiers.slice(0, tierIndex)
      .reduce((sum, tier) => sum + (tier.pointsToUnlockNext || 0), 0);
  }

  function tierOpen(tree, state, branchIndex, tierIndex) {
    return treesOpen(tree, state) && invested(tree, state, branchIndex) >= tierRequirement(tree, branchIndex, tierIndex);
  }

  // Why a spend would be refused, in the host's words, or '' when it would be accepted. branch < 0 is the action skill.
  function refusal(tree, state, branchIndex, tierIndex, skill) {
    if (branchIndex < 0) {
      if ((state.actionGrade || 0) >= tree.actionSkill.maxGrade) return 'action skill at max grade';
      return (state.points || 0) < 1 ? 'no skill points' : '';
    }
    if (rank(state, skill) >= skill.maxGrade) return 'skill at max grade';
    if ((state.points || 0) < 1) return 'no skill points';
    if (!treesOpen(tree, state)) return 'action skill not unlocked';
    const have = invested(tree, state, branchIndex), need = tierRequirement(tree, branchIndex, tierIndex);
    return have < need ? `tier locked (${have} of ${need} points in branch)` : '';
  }

  return { rank, invested, treesOpen, tierRequirement, tierOpen, refusal };
});
