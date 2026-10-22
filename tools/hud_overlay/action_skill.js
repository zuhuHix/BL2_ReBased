// OpenWillow: drives the action-skill clips of BL2's HUD movie from the host's
// state. Our own code; loaded by index.html and by tests/hud_action_skill_test.js.
//
// The game drives these clips from native code (WillowHUDGFxMovie.UpdateActionSkill
// and friends), so the rules below were read from the live movie during real
// casts (docs/verification/ACTION_SKILL_HUD.md), not from its code:
//   p1.character       frame 6 while Maya plays (read live; the bench label
//                      `siren` is frame 5).
//   p1.character.cooldown  the icon: frame 100 ready (green), 2 while the skill
//                      runs (gold, flames), 3..99 a red veil that recedes as the
//                      cooldown runs out: frame = floor(2 + 98 * (1 - left)).
//   p1.character.cooldown.icon.placeholderTexture  gets the class's icon movie
//                      (UI_HUD_AAIcons.AA-Siren for Maya).
//   p1.activeAbility   `siren_action` (frame 3) from the lock to the skill's end,
//                      `none` (1) otherwise: the purple Siren emblem above the XP
//                      bar. Its `bar` drains right to left:
//                      frame = floor(1 + 99 * (1 - left)).
// `left` is a remaining fraction: of the lock time for the emblem, of the
// cooldown pool for the icon.
(function (root) {
  'use strict';
  const P1 = '_level1.p1';
  const clamp01 = v => Math.min(1, Math.max(0, v));

  function activeBarFrame(left) {
    return Math.min(100, Math.floor(1 + 99 * (1 - clamp01(left))));
  }

  function cooldownFrame(left) {
    return Math.min(100, Math.floor(2 + 98 * (1 - clamp01(left))));
  }

  // The movie calls for one state change. `prev` is what was last applied ({}
  // at start). Fields: `skillActive` (remaining lock fraction, null when the
  // emblem is off), `skillCooldown` (remaining cooldown fraction, 0 = ready),
  // `skillIcon` (icon movie path). Returns [target, op, a] tuples for the
  // harness's ow() callback.
  function plan(prev, next) {
    const calls = [];
    if (next.skillIcon && next.skillIcon !== prev.skillIcon)
      calls.push([P1 + '.character.cooldown.icon.placeholderTexture', 'loadMovie', next.skillIcon]);
    if ('skillActive' in next) {
      const was = prev.skillActive, now = next.skillActive;
      if (now === null || now === undefined) {
        if (was !== null) calls.push([P1 + '.activeAbility', 'gotoAndStop', 'none']);
      } else {
        if (was === null || was === undefined) calls.push([P1 + '.activeAbility', 'gotoAndStop', 'siren_action']);
        const frame = activeBarFrame(now);
        if (was === null || was === undefined || frame !== activeBarFrame(was))
          calls.push([P1 + '.activeAbility.bar', 'gotoAndStop', frame]);
      }
    }
    if ('skillCooldown' in next) {
      const frame = cooldownFrame(next.skillCooldown);
      if (!('skillCooldown' in prev) || frame !== cooldownFrame(prev.skillCooldown))
        calls.push([P1 + '.character.cooldown', 'gotoAndStop', frame]);
    }
    return calls;
  }

  const api = { activeBarFrame, cooldownFrame, plan };
  root.owActionSkill = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
