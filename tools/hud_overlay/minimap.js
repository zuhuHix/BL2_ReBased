// OpenWillow: drives the minimap of BL2's HUD movie (p1.minimapEcho.minimap) from the host's state. Our own code;
// loaded by index.html and by tests/hud_minimap_test.js.
//
// The game's driver (HUDWidget_Minimap.Init/Update/SetMapMovie) is native, so these rules were read from the running
// game (docs/verification/HUD_MINIMAP.md: the widget's properties and the clips it writes, Sanctuary, 2026-10-22):
//   - the level's map movie (WillowMapInfo.TacticalMapMovie) goes into `mapPlaceholder` through the clip's own
//     MinimapController.setMapMovie; its onLoadInit calls extInitMinimapFogOfWar, and the widget then toggles each
//     fog-of-war blob (toggleFogOfWarBlob(name, HasBeenUncovered) for the player's DiscoveredWorldAreas);
//   - the map turns about the player: mapPlaceholder._rotation = -yaw, its scale is WORLD_UU_PER_MOVIE_UNIT / UU_PER_UNIT
//     (102.4 %), and its position puts the player's map point at the clip origin. The map movie's (0, 0) is the
//     centre of the WillowTacticalMapVolume's bounds; movie x follows world +Y and movie y world -X, 128 uu per unit;
//   - one minimap unit is WorldRadius / 48 = 125 uu; a point's offset (dX, dY) from the player is (dY, -dX) / 125,
//     turned by -yaw (so the view direction is up);
//   - the panel is a box of half size 58 x 48 units: shops, health, travel and customization icons hide outside it,
//     mission and objective icons are pulled back onto its edge along their direction;
//   - the objective icon's scale is 140 % up to WorldRadius and 140 * (1 - (d - 6000) / 18000) beyond (seen to 9000 uu);
//   - the "N" marker sits where north's direction (world +X for a north offset of 0) meets the box edge, plus
//     NorthMarkerOffset (-16, -18.8); DirArrow keeps _rotation 0 (player-relative mode, the default).
// Icon clips are attached from the movie's own library symbols under the names the game uses (shop1, hstation0,
// ftstation0, cstation0, eligible0, objective0); which symbol the game attaches for each kind was matched by size,
// not read, and the kinds the capture did not show (level travel, turn-in) are UNVERIFIED.
(function (root) {
  'use strict';
  const MM = '_level1.p1.minimapEcho.minimap';
  const WORLD_RADIUS = 6000;            // HUDWidget_Minimap.WorldRadius (read live; vehicles change it, not modelled)
  const HALF_W = 58, HALF_H = 48;       // the panel box in minimap units (icon clamps and hides; mapmask 116 x 96)
  const UU_PER_UNIT = WORLD_RADIUS / HALF_H;           // 125
  const UU_PER_MOVIE_UNIT = 128;        // HUDWidget_Minimap.UnrealUnitsPerPixel (read live)
  const MAP_SCALE = UU_PER_MOVIE_UNIT / UU_PER_UNIT;   // 1.024
  const NORTH_OFFSET = [-16, -18.800003051757812];   // HUDWidget_Minimap.NorthMarkerOffset (read live, a float)
  const OBJECTIVE_SCALE = 140, OBJECTIVE_FALLOFF = 18000;   // fitted to five live reads, exact to 0.05 %
  // kind: [library symbol, clip name prefix, clamp onto the box edge (true) or hide outside it (false)]
  const KINDS = {
    shop: ['icon_shop', 'shop', false],
    health: ['icon_healthstation', 'hstation', false],
    fasttravel: ['icon_travel station', 'ftstation', false],
    customize: ['icon_customize', 'cstation', false],
    leveltravel: ['icon_level transition', 'ltstation', false],   // UNVERIFIED: not on screen in the capture
    eligible: ['icon_new mission', 'eligible', true],
    turnin: ['icon_mission turnin', 'redeemable', true],          // UNVERIFIED: not on screen in the capture
    objective: ['objective clip', 'objective', true],
  };
  // Stacking seen in the game: map, travel/shop icons, mission icons, objective, then the arrow and "N" on top.
  const LAYER = {shop: 0, health: 0, fasttravel: 0, customize: 0, leveltravel: 0, eligible: 1, turnin: 1, objective: 2};
  const BASE_DEPTH = 100, LAYER_DEPTHS = 100;

  const twip = v => Math.floor(v * 20 + 1e-6) / 20;   // Scaleform keeps twips, rounded down (live reads)
  const rad = deg => deg * Math.PI / 180;

  // Screen-up is the view direction: rotate a minimap-unit vector by -yaw (AS2 rotation, y down, clockwise positive).
  function turn(x, y, yaw) {
    const t = rad(-yaw), c = Math.cos(t), s = Math.sin(t);
    return [x * c - y * s, x * s + y * c];
  }

  // Offset of a world point from the player in minimap units, before turning.
  function offset(px, py, wx, wy) {
    return [(wy - py) / UU_PER_UNIT, -(wx - px) / UU_PER_UNIT];
  }

  function wrap180(deg) {
    let r = ((deg % 360) + 360) % 360;
    if (r >= 180) r -= 360;   // the game reads -180 at yaw 180
    return r;
  }

  // Scale factor that brings (x, y) onto the box edge (1 when inside).
  function edgeFactor(x, y) {
    const fx = Math.abs(x) > HALF_W ? HALF_W / Math.abs(x) : 1;
    const fy = Math.abs(y) > HALF_H ? HALF_H / Math.abs(y) : 1;
    return Math.min(fx, fy);
  }

  function objectiveScale(distance) {
    if (distance <= WORLD_RADIUS) return OBJECTIVE_SCALE;
    return Math.max(0, OBJECTIVE_SCALE * (1 - (distance - WORLD_RADIUS) / OBJECTIVE_FALLOFF));
  }

  // The clip values for one state: {map: {x, y, rotation, scale}, north: {x, y}, icons: [{name, symbol, layer, x, y,
  // visible, scale}]}. `m`: {x, y, yaw, centre: [cx, cy], icons: [{kind, x, y}]} in world units and degrees.
  function layout(m) {
    const yaw = m.yaw || 0;
    const [cx, cy] = m.centre || [0, 0];
    const mapPoint = [(m.y - cy) / UU_PER_MOVIE_UNIT * MAP_SCALE, -(m.x - cx) / UU_PER_MOVIE_UNIT * MAP_SCALE];
    const [mx, my] = turn(mapPoint[0], mapPoint[1], yaw);
    const out = {map: {x: twip(-mx), y: twip(-my), rotation: wrap180(-yaw), scale: MAP_SCALE * 100}, icons: []};
    const [nx, ny] = turn(0, -1, yaw);
    const nf = edgeFactor(nx * 1e3, ny * 1e3) * 1e3;
    out.north = {x: twip(nx * nf + NORTH_OFFSET[0]), y: twip(ny * nf + NORTH_OFFSET[1])};
    const counts = {};
    for (const icon of m.icons || []) {
      const kind = KINDS[icon.kind];
      if (!kind) continue;
      const index = counts[icon.kind] = (counts[icon.kind] ?? -1) + 1;
      let [x, y] = turn(...offset(m.x, m.y, icon.x, icon.y), yaw);
      const f = edgeFactor(x, y);
      const visible = kind[2] || f === 1;
      if (kind[2]) { x *= f; y *= f; }
      const scale = icon.kind === 'objective' ? objectiveScale(Math.hypot(icon.x - m.x, icon.y - m.y)) : 100;
      out.icons.push({name: kind[1] + index, symbol: kind[0], layer: LAYER[icon.kind], x: twip(x), y: twip(y),
        visible, scale: Math.round(scale * 100) / 100});
    }
    return out;
  }

  // The movie calls for one state change: `prev` and `next` are the {minimap} states (prev {} at start), `attached`
  // the set of icon clip names already attached (updated in place). Returns [target, op, a, b] tuples for the
  // harness's ow() callback; values that did not change are not sent again.
  function plan(prev, next, attached) {
    const calls = [];
    if (!next || next.x === undefined) return calls;
    if (next.movie && next.movie !== prev.movie) calls.push([MM, 'setMapMovie', next.movie]);
    const a = layout(next), b = prev.x === undefined || next.movie !== prev.movie ? null : layout(prev);
    const P = MM + '.mapPlaceholder';
    if (!b || a.map.x !== b.map.x) calls.push([P, 'set', '_x', a.map.x]);
    if (!b || a.map.y !== b.map.y) calls.push([P, 'set', '_y', a.map.y]);
    if (!b || a.map.rotation !== b.map.rotation) calls.push([P, 'set', '_rotation', a.map.rotation]);
    if (!b) { calls.push([P, 'set', '_xscale', a.map.scale]); calls.push([P, 'set', '_yscale', a.map.scale]); }
    if (!b) calls.push([MM + '.DirArrow', 'set', '_rotation', 0]);
    if (!b || a.north.x !== b.north.x) calls.push([MM + '.north', 'set', '_x', a.north.x]);
    if (!b || a.north.y !== b.north.y) calls.push([MM + '.north', 'set', '_y', a.north.y]);
    const old = {};
    if (b) for (const i of b.icons) old[i.name] = i;
    const seen = new Set();
    let added = false;
    for (const i of a.icons) {
      seen.add(i.name);
      const clip = MM + '.' + i.name, o = attached.has(i.name) ? old[i.name] : undefined;
      if (!attached.has(i.name)) {
        const depth = BASE_DEPTH + i.layer * LAYER_DEPTHS + attached.size;
        calls.push([MM, 'apply', 'attachMovie', [i.symbol, i.name, depth]]);
        // The objective clip's height arrows, edge arrow and "optional" mark read hidden in the game (not driven).
        if (i.symbol === KINDS.objective[0])
          for (const sub of ['above', 'below', 'edgeArrow', 'objective.optional'])
            calls.push([clip + '.' + sub, 'set', '_visible', false]);
        attached.add(i.name);
        added = true;
      }
      if (!o || o.visible !== i.visible) calls.push([clip, 'set', '_visible', i.visible]);
      if (!o || o.x !== i.x) calls.push([clip, 'set', '_x', i.x]);
      if (!o || o.y !== i.y) calls.push([clip, 'set', '_y', i.y]);
      if (!o || o.scale !== i.scale) {
        calls.push([clip, 'set', '_xscale', i.scale]);
        calls.push([clip, 'set', '_yscale', i.scale]);
      }
    }
    // Slots the host no longer reports are hidden, as the widget hides unused slots.
    for (const name of attached) if (!seen.has(name) && (!b || (old[name] && old[name].visible)))
      calls.push([MM + '.' + name, 'set', '_visible', false]);
    // Keep the arrow, "N" and the frame flash above every attached icon.
    if (added) for (const top of ['DirArrow', 'north', 'flash'])
      calls.push([MM + '.' + top, 'apply', 'swapDepths', [BASE_DEPTH + 3 * LAYER_DEPTHS + ['DirArrow', 'north', 'flash'].indexOf(top)]]);
    return calls;
  }

  // The fog-of-war toggles the widget sends once the map movie has loaded (extInitMinimapFogOfWar).
  function fogCalls(blobs) {
    return Object.keys(blobs || {}).map(name => [MM, 'toggleFogOfWarBlob', name, !!blobs[name]]);
  }

  // The page's side: keeps what was applied, answers the map movie's extInitMinimapFogOfWar callback, and logs what the
  // movie now shows (the state applied and values read back from its clips) when that changes, for the in-engine check
  // (tools/check_hud_minimap.py). `call(target, op, a, b)` is the harness callback.
  const page = {state: {}, attached: new Set(), lastLine: '', log: line => console.log(line)};
  function apply(call, next) {
    if (!next || next.x === undefined) return;
    if (!page.echo0) {
      const echo = MM.slice(0, MM.lastIndexOf('.'));
      const v = k => Number(call(echo, 'get', k));
      page.echo0 = {x: v('_x'), y: v('_y'), xs: v('_xscale') / 100, ys: v('_yscale') / 100};
    }
    for (const c of plan(page.state, next, page.attached)) call(c[0], c[1], c[2], c[3]);
    page.state = next;
    const get = (clip, p) => Number(call(MM + clip, 'get', p));
    const n = v => (Number.isFinite(v) ? v : NaN).toFixed(2);
    const objective = page.attached.has('objective0')
      ? ` objective=${call(MM + '.objective0', 'get', '_visible') ? 1 : 0},${n(get('.objective0', '_x'))},${n(get('.objective0', '_y'))},${n(get('.objective0', '_xscale'))}` : '';
    const line = `OWMINIMAPPAGE x=${n(next.x)} y=${n(next.y)} yaw=${n(next.yaw)}`
      + ` map=${n(get('.mapPlaceholder', '_x'))},${n(get('.mapPlaceholder', '_y'))},${n(get('.mapPlaceholder', '_rotation'))},${n(get('.mapPlaceholder', '_xscale'))}`
      + ` mapw=${n(get('.mapPlaceholder', '_width'))} north=${n(get('.north', '_x'))},${n(get('.north', '_y'))}`
      + ` icons=${[...page.attached].filter(c => call(MM + '.' + c, 'get', '_visible')).length}${objective}`;
    if (line !== page.lastLine) { page.log(line); page.lastLine = line; }
  }
  // hud3d.js's stand-in for minimapEcho's turn (_yrotation -20) sizes it from getBounds, which counts the movie's
  // hidden template icons parked about 400 px right of the panel (the game reads minimap._width 731 too), so the panel
  // lands too far right and too high, and a scale alone cannot show the turned panel's slant (its top edge rising to
  // the right, its near right side taller). This places the panel by the same projection as hud3d.js (turn about the
  // clip's origin, perspective about the stage centre, F = View3DFOV 70, F / D fitted) but over the panel's own extent
  // (the map mask) and with a full 2D affine map fitted to it by least squares on a grid of points: the closest a 2D
  // player can come to the trapezoid. A general affine map needs two nested clips: minimapEcho gets rotation and
  // scale and the minimap inside it a rotation (L = R(phi) diag(s1, s2) R(theta)). The authored transforms are kept
  // from the first state applied (before the stand-in runs). Returns the transform set and the fit's worst residual
  // in stage px, or null when there is nothing to do yet.
  const GRID = 5;
  function affineFit(points) {
    // points: [[u, v, X, Y]]; least squares X = a u + c v + tx, Y = b u + d v + ty (normal equations, 3x3).
    const N = [[0, 0, 0], [0, 0, 0], [0, 0, 0]], rx = [0, 0, 0], ry = [0, 0, 0];
    for (const [u, v, X, Y] of points) {
      const r = [u, v, 1];
      for (let i = 0; i < 3; i++) {
        for (let j = 0; j < 3; j++) N[i][j] += r[i] * r[j];
        rx[i] += r[i] * X; ry[i] += r[i] * Y;
      }
    }
    const solve = rhs => {   // Cramer's rule on the symmetric 3x3
      const det = M => M[0][0] * (M[1][1] * M[2][2] - M[1][2] * M[2][1]) - M[0][1] * (M[1][0] * M[2][2] - M[1][2] * M[2][0])
        + M[0][2] * (M[1][0] * M[2][1] - M[1][1] * M[2][0]);
      const D = det(N);
      return [0, 1, 2].map(k => det(N.map((row, i) => row.map((v, j) => (j === k ? rhs[i] : v)))) / D);
    };
    const [a, c, tx] = solve(rx), [b, d, ty] = solve(ry);
    return {a, b, c, d, tx, ty};
  }
  // M = [[a, c], [b, d]] (columns: images of the u and v axes) = R(phi) diag(s1, s2) R(theta), AS2 rotations (y down).
  function splitAffine(a, b, c, d) {
    const E = (a + d) / 2, F = (a - d) / 2, G = (b + c) / 2, H = (b - c) / 2;
    const Q = Math.hypot(E, H), R = Math.hypot(F, G);
    const a1 = Math.atan2(G, F), a2 = Math.atan2(H, E);
    return {s1: Q + R, s2: Q - R, phi: (a2 + a1) / 2 * 180 / Math.PI, theta: (a2 - a1) / 2 * 180 / Math.PI};
  }
  function placeEcho(call, p1, hud3d) {
    const echo = p1 + '.minimapEcho', e = page.echo0;
    if (!e || !hud3d) return null;
    const num = (t, k) => Number(call(t, 'get', k));
    const angle = num(echo, '_yrotation'), S = hud3d.SCALE;
    const b = call(MM + '.mapmask', 'apply', 'getBounds', []);
    if (!angle || !b) return null;
    const tm = [num(MM, '_x'), num(MM, '_y')];                       // the minimap's place in minimapEcho
    const mk = [tm[0] + num(MM + '.mapmask', '_x'), tm[1] + num(MM + '.mapmask', '_y')];
    const p1x = num(p1, '_x'), p1y = num(p1, '_y');
    const px = 640 + (p1x - 640) / S, py = 360 + (p1y - 360) / S;   // p1 before the stand-in's 1.095 scale
    const ox = px + e.x, oy = py + e.y, r = angle * Math.PI / 180, D = hud3d.FOCAL / S;
    const project = (u, v) => {   // echo-local point -> stage px, as hud3d.js's model
      const su = e.xs * u, w = D + su * Math.sin(r);
      return [640 + (ox + su * Math.cos(r) - 640) * hud3d.FOCAL / w, 360 + (oy + e.ys * v - 360) * hud3d.FOCAL / w];
    };
    const points = [];
    for (let i = 0; i < GRID; i++) for (let j = 0; j < GRID; j++) {
      const u = mk[0] + b.xMin + (b.xMax - b.xMin) * i / (GRID - 1), v = mk[1] + b.yMin + (b.yMax - b.yMin) * j / (GRID - 1);
      points.push([u, v, ...project(u, v)]);
    }
    const A = affineFit(points);
    const residual = Math.max(...points.map(([u, v, X, Y]) =>
      Math.hypot(A.a * u + A.c * v + A.tx - X, A.b * u + A.d * v + A.ty - Y)));
    // In p1's units: stage = P1' + S * child.
    const L = [A.a / S, A.b / S, A.c / S, A.d / S], T = [(A.tx - p1x) / S, (A.ty - p1y) / S];
    const k = splitAffine(...L);
    const ph = k.phi * Math.PI / 180;
    // minimapEcho's own linear part R(phi) diag(s1, s2); its translation keeps the minimap's origin where L puts it.
    const Le = [Math.cos(ph) * k.s1, Math.sin(ph) * k.s1, -Math.sin(ph) * k.s2, Math.cos(ph) * k.s2];
    const out = {x: T[0] + L[0] * tm[0] + L[2] * tm[1] - (Le[0] * tm[0] + Le[2] * tm[1]),
      y: T[1] + L[1] * tm[0] + L[3] * tm[1] - (Le[1] * tm[0] + Le[3] * tm[1]),
      rotation: k.phi, xscale: 100 * k.s1, yscale: 100 * k.s2, minimapRotation: k.theta, residual};
    for (const key of ['x', 'y', 'rotation', 'xscale', 'yscale']) call(echo, 'set', '_' + key, out[key]);
    call(MM, 'set', '_rotation', out.minimapRotation);
    page.log(`OWMINIMAPPAGE echo ${['x', 'y', 'rotation', 'xscale', 'yscale', 'minimapRotation', 'residual']
      .map(key => out[key].toFixed(3)).join(',')}`);
    return out;
  }
  // With hud_warp.js's perspective warp the minimap is drawn like the other turned clusters (a true trapezoid). The
  // warp's homography depends only on the clip's turn and origin; its region rectangle decides which drawn pixels it
  // owns. readClips takes the union of the visible children, which here includes the movie's parked library icons
  // (fastTravel, customize, newMission, ... about 400 px right of the panel and 190 px above it, not hidden by the
  // movie's HideAllIcons), so minimapEcho's rectangle is replaced by the panel's own: the map mask plus WARP_PAD for
  // edge-clamped icons and the "N" marker above it, in flat stage px (minimapEcho is left unturned by hud3d.js then).
  const WARP_PAD = {left: 14, right: 14, top: 26, bottom: 14};
  function warpRect(call, p1, origin) {
    const echo = p1 + '.minimapEcho', num = (t, k) => Number(call(t, 'get', k));
    const b = call(MM + '.mapmask', 'apply', 'getBounds', []);
    if (!b) return null;
    const ex = origin.x + num(echo, '_x'), ey = origin.y + num(echo, '_y');
    const xs = num(echo, '_xscale') / 100, ys = num(echo, '_yscale') / 100;
    const mx = num(MM, '_x') + num(MM + '.mapmask', '_x'), my = num(MM, '_y') + num(MM + '.mapmask', '_y');
    const r = [ex + xs * (mx + b.xMin - WARP_PAD.left), ey + ys * (my + b.yMin - WARP_PAD.top),
      ex + xs * (mx + b.xMax + WARP_PAD.right), ey + ys * (my + b.yMax + WARP_PAD.bottom)];
    return r.every(Number.isFinite) ? r : null;
  }
  function warpClips(clips, call, p1, origin) {
    return clips.map(c => {
      if (c.name !== 'minimapEcho') return c;
      const rect = warpRect(call, p1, origin);
      return rect ? {...c, rect} : c;
    });
  }
  function initFog(call) {
    for (const c of fogCalls(page.state.blobs)) call(c[0], c[1], c[2], c[3]);
    page.log('OWMINIMAPPAGE fog ' + JSON.stringify(page.state.blobs || {}));
  }

  const api = {layout, plan, fogCalls, apply, initFog, placeEcho, warpRect, warpClips, affineFit, splitAffine, page, turn, edgeFactor, objectiveScale, MM, WORLD_RADIUS, HALF_W, HALF_H,
    UU_PER_UNIT, MAP_SCALE, KINDS};
  root.owMinimap = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
