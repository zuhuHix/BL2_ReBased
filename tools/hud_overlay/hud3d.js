// OpenWillow: a 2D stand-in for the 3D turn of BL2's HUD clusters. Our own code; loaded by index.html and by
// tests/hud_action_skill_test.js.
//
// In the running game (docs/verification/ACTION_SKILL_HUD.md, "HUD layout") no HUD clip is moved or scaled in 2D:
// the side clusters carry the movie's own `_yrotation` (+25 on health, shield and the action-skill icon, -25 on
// ammo and grenades, ...), which Ruffle stores but does not draw, and the whole movie appears 1.095 times larger
// about the screen centre. Model fitted to 13 points on a live 1280x720 frame (rms about 3 px): turn each clip about
// its origin, x1 = ox + u cos(a), z = u sin(a), then project about the stage centre,
// X = 640 + (x1 - 640) F / (D + z), Y = 360 + (y - 360) F / (D + z). F is the movie definition's View3DFOV 70 across
// the 1280 px stage (read in the game); the scale F / D = 1.095 is fitted (UNVERIFIED), and how Scaleform derives it
// was not read. Ruffle draws affine transforms only, so each turned clip gets the scale and position that meet that
// projection at its bounds' left and right edges: the trapezoid (its near edge taller than its far edge) is not drawn.
// When hud_warp.js can read Ruffle's canvas it draws the true perspective of the turned clips and this stand-in is
// kept for the minimap (and for every turned clip when the warp is unavailable).
(function (root) {
  'use strict';
  const CX = 640, CY = 360;
  const FOCAL = CX / Math.tan(35 * Math.PI / 180);   // View3DFOV 70
  const SCALE = 1.095;                                // F / D, fitted
  const DEPTH = FOCAL / SCALE;

  // A clip with turn `a` (degrees), stage origin (ox, oy) and its bounds' left and right edges u0, u1 as stage-pixel
  // offsets from that origin: the stage mapping X = x0 + sx * u, Y = y0 + sy * v that meets the projection at both
  // edges horizontally and at their middle's depth vertically.
  function project(a, ox, oy, u0, u1) {
    const r = a * Math.PI / 180, sin = Math.sin(r), cos = Math.cos(r);
    const X = u => CX + (ox + u * cos - CX) * FOCAL / (DEPTH + u * sin);
    const sx = u1 > u0 ? (X(u1) - X(u0)) / (u1 - u0) : FOCAL * cos / DEPTH;
    const w = DEPTH + (u0 + u1) / 2 * sin, sy = FOCAL / w;
    return {x0: X(u0) - sx * u0, y0: CY + (oy - CY) * sy, sx, sy};
  }

  // Applies the stand-in once: `p1` is scaled by SCALE about the stage centre (the unturned plane) and every direct
  // child with a turn gets its projection relative to that, except the clips named in `skip` (left flat: hud_warp.js
  // draws their turn). `ow(target, op, a, b)` is the harness callback. Returns the names of the clips given the
  // stand-in (null when the movie is not ready).
  function apply(ow, p1, names, skip) {
    const px = ow(p1, 'get', '_x'), py = ow(p1, 'get', '_y');
    if (![px, py].every(Number.isFinite)) return null;
    const turned = [];
    for (const name of names) {
      const clip = `${p1}.${name}`, a = Number(ow(clip, 'get', '_yrotation'));
      if (!a || Number(ow(clip, 'get', '_rotation')) || (skip && skip.has(name))) continue;
      const b = ow(clip, 'apply', 'getBounds', []);
      const x = ow(clip, 'get', '_x'), y = ow(clip, 'get', '_y');
      const xs = ow(clip, 'get', '_xscale') / 100, ys = ow(clip, 'get', '_yscale') / 100;
      if (!b || ![x, y, xs, ys].every(Number.isFinite)) continue;
      const m = project(a, px + x, py + y, xs * b.xMin, xs * b.xMax);
      // Undo the scaled parent: stage = P1' + SCALE * child.
      const p1x = CX + (px - CX) * SCALE, p1y = CY + (py - CY) * SCALE;
      ow(clip, 'set', '_x', (m.x0 - p1x) / SCALE);
      ow(clip, 'set', '_y', (m.y0 - p1y) / SCALE);
      ow(clip, 'set', '_xscale', 100 * xs * m.sx / SCALE);
      ow(clip, 'set', '_yscale', 100 * ys * m.sy / SCALE);
      turned.push(name);
    }
    ow(p1, 'set', '_x', CX + (px - CX) * SCALE);
    ow(p1, 'set', '_y', CY + (py - CY) * SCALE);
    ow(p1, 'set', '_xscale', 100 * SCALE);
    ow(p1, 'set', '_yscale', 100 * SCALE);
    return turned;
  }

  const api = {project, apply, FOCAL, SCALE};
  root.owHud3d = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
