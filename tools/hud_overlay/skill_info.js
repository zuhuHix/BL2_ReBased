// Info-box HTML for one skill, in the form the game sends to the StatusMenu
// movie's InformationBox.SetInfo (observed in a local real-game UI trace,
// DECISIONS.md 2026-09-27). Our own code; it only arranges data that
// tools/prepare_skill_tree.py read from the install. Works in the page and in
// Node (tests/skill_info_test.js).
//
// skill.stats[g] lists the stat lines at grade g as [before, number, after].
// state: { grade, locked, bonus } where bonus is the class mod's extra grades.
// strings: { nextLevel, classModBonus, classModUntrained, pointSingular,
// pointPlural } from the install's WillowGame.int.
(function (root) {
  const CURRENT = "#cc6600";   // number colour below max grade
  const MAXED = "#00cc00";     // number colour at max grade
  const LOCKED = "#a3a3b0";    // whole text while the skill's tier is locked
  const CLASS_MOD = "#33FFFF"; // class-mod bonus note

  function lines(stats, grade) {
    return stats[Math.min(grade, stats.length - 1)] || [];
  }

  function skillInfoHtml(skill, state, strings) {
    const grade = Math.max(0, Math.floor(state.grade || 0));
    const bonus = Math.max(0, Math.floor(state.bonus || 0));
    const stats = skill.stats || [];
    const blocks = [];
    // Class-mod grades count once the skill has a point, and in the next-level
    // preview unless the skill is locked.
    if (grade > 0) {
      // Green only when the trained grade is maxed: the maintainer's capture
      // shows Ward at 2 + 4 class-mod grades (+30%) in the orange colour.
      const colour = grade >= skill.maxGrade ? MAXED : CURRENT;
      const current = lines(stats, grade + bonus);
      if (current.length)
        blocks.push(current.map(([before, number, after]) =>
          `${before}<font color='${colour}'>${number}</font>${after}`).join("\n"));
    }
    if (grade < skill.maxGrade) {
      const next = lines(stats, grade + 1 + (state.locked ? 0 : bonus));
      if (next.length) {
        // Next-level lines drop the space a number-less line starts with.
        const text = next.map(([before, number, after]) => `${before}${number}${after}`.replace(/^ /, ""));
        const colour = state.locked ? LOCKED : CURRENT;
        blocks.push(`<font size="15" color="${colour}"><i>${strings.nextLevel}\n${text.join("\n")}</i></font>`);
      }
    }
    let html = [skill.description || "", ...blocks].join("\n\n");
    if (state.locked) html = `<font color="${LOCKED}">${html}</font>`;
    if (bonus > 0 && strings.classModBonus) {
      const points = bonus === 1 ? strings.pointSingular : strings.pointPlural;
      let note = strings.classModBonus.replace("%n", String(bonus)).replace("%s", points);
      if (grade === 0 && strings.classModUntrained) note += "\n" + strings.classModUntrained;
      html += `\n\n<font color='${CLASS_MOD}'>${note}</font>`;
    }
    return html;
  }

  if (typeof module !== "undefined" && module.exports) module.exports = { skillInfoHtml };
  else root.skillInfoHtml = skillInfoHtml;
})(this);
