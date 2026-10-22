// OpenWillow: draws the 3D turn of BL2's HUD clusters as a real perspective warp of the Ruffle canvas. Our own code;
// loaded by index.html and by tests/hud_warp_test.js.
//
// The model is hud3d.js's (docs/verification/ACTION_SKILL_HUD.md, "HUD layout"): a clip with turn `a` about the
// vertical axis through its stage origin x = ox is seen at X = CX + (ox + u cos a - CX) F / (D + u sin a),
// Y = CY + (y - CY) F / (D + u sin a), u = x - ox, F = 914 (View3DFOV 70), F / D = 1.095 (fitted, UNVERIFIED). In u, y
// and 1 both numerators and the denominator are linear, so a turned clip's plane maps to the screen by one 3x3
// homography; oy does not enter (the axis is vertical).
//
// Ruffle draws no 3D. The page leaves the turned clips flat inside `p1`, which hud3d.js scales by 1.095 about the stage
// centre (so every unturned clip is already where the game shows it), hides the Ruffle player (opacity 0, it keeps
// drawing) and redraws its canvas into an overlay WebGL canvas: every source pixel once, either in place or, inside a
// turned clip's region, through that clip's homography composed with the inverse of Ruffle's 1.095 scale. Regions are
// rectangles in priority order; a pixel belongs to the first one that holds it. Without WebGL or a readable Ruffle
// canvas the page keeps hud3d.js's affine stand-in.
(function (root) {
  'use strict';
  const CX = 640, CY = 360, STAGE_W = 1280, STAGE_H = 720;
  const FOCAL = CX / Math.tan(35 * Math.PI / 180);   // View3DFOV 70 across the 1280 px stage
  const SCALE = 1.095;                                // F / D, fitted (UNVERIFIED)
  const MARGIN = 12;                                  // stage px around a clip's bounds for glows (filters are not in getBounds)
  const MAX_REGIONS = 16;
  const SUPERSAMPLE = 2;                              // the hidden player renders at this multiple of the page size

  // 3x3 matrices as row-major arrays of 9.
  const mul = (A, B) => {
    const C = new Array(9);
    for (let r = 0; r < 3; r++) for (let c = 0; c < 3; c++)
      C[3 * r + c] = A[3 * r] * B[c] + A[3 * r + 1] * B[3 + c] + A[3 * r + 2] * B[6 + c];
    return C;
  };
  function inv(M) {
    const [a, b, c, d, e, f, g, h, i] = M;
    const A = e * i - f * h, B = -(d * i - f * g), C = d * h - e * g;
    const det = a * A + b * B + c * C;
    if (!det) return null;
    return [A / det, -(b * i - c * h) / det, (b * f - c * e) / det,
            B / det, (a * i - c * g) / det, -(a * f - c * d) / det,
            C / det, -(a * h - b * g) / det, (a * e - b * d) / det];
  }
  const apply = (M, x, y) => {
    const w = M[6] * x + M[7] * y + M[8];
    return [(M[0] * x + M[1] * y + M[2]) / w, (M[3] * x + M[4] * y + M[5]) / w];
  };

  // The flat stage point (x, y) of a clip turned `a` degrees about x = ox to the screen (stage px), as the formula above.
  function projectPoint(a, ox, x, y, focal = FOCAL, scale = SCALE) {
    const r = a * Math.PI / 180, u = x - ox, w = focal / scale + u * Math.sin(r);
    return [CX + (ox + u * Math.cos(r) - CX) * focal / w, CY + (y - CY) * focal / w];
  }

  // The same mapping as a homography of (x, y, 1).
  function clipHomography(a, ox, focal = FOCAL, scale = SCALE) {
    const r = a * Math.PI / 180, s = Math.sin(r), c = Math.cos(r), D = focal / scale, F = focal;
    const kx = CX * s + F * c;
    return [kx, 0, -kx * ox + CX * D + F * (ox - CX),
            CY * s, F, -CY * s * ox - F * CY + CY * D,
            s, 0, D - s * ox];
  }

  // Ruffle's drawing of the flat stage: `p1` scaled by `scale` about the stage centre.
  const ruffleMatrix = (scale = SCALE) => [scale, 0, CX * (1 - scale), 0, scale, CY * (1 - scale), 0, 0, 1];

  // A turned clip's warp in canvas pixels (canvas w x h showing the 1280x720 stage): `fwd` takes a pixel Ruffle drew
  // to where the game shows it, `back` is its inverse (used per output pixel). `rect` is the clip's flat stage
  // bounds [x0, y0, x1, y1] (before p1's scale); the source region is Ruffle's drawing of it plus MARGIN.
  function clipWarp(clip, w = STAGE_W, h = STAGE_H, opts = {}) {
    const focal = opts.focal || FOCAL, scale = opts.scale || SCALE, margin = opts.margin ?? MARGIN;
    const K = [w / STAGE_W, 0, 0, 0, h / STAGE_H, 0, 0, 0, 1], Ki = inv(K), R = ruffleMatrix(scale);
    const fwd = mul(K, mul(clipHomography(clip.a, clip.ox, focal, scale), mul(inv(R), Ki)));
    const [x0, y0, x1, y1] = clip.rect;
    const p = (x, y) => apply(mul(K, R), x, y);
    const box = m => {
      const [sx0, sy0] = p(x0 - m, y0 - m), [sx1, sy1] = p(x1 + m, y1 + m);
      return [Math.max(0, Math.floor(sx0)), Math.max(0, Math.floor(sy0)), Math.min(w, Math.ceil(sx1)), Math.min(h, Math.ceil(sy1))];
    };
    const src = box(margin), core = box(0);
    const quad = [[src[0], src[1]], [src[2], src[1]], [src[2], src[3]], [src[0], src[3]]].map(([x, y]) => apply(fwd, x, y));
    return {name: clip.name, a: clip.a, ox: clip.ox, fwd, back: inv(fwd), src, core, quad};
  }

  // The region that draws source pixel (x, y), -1 for none (drawn in place): the first whose clip bounds (`core`)
  // hold it, else the first whose bounds plus margin (`src`) do. Bounds first, so a clip's glow margin never takes
  // pixels of a neighbour's own content: neighbours turned about different origins (the icon, 6 px from health's)
  // would otherwise draw those pixels up to 1.7 px off and leave a seam.
  function owner(regions, x, y) {
    for (const key of ['core', 'src'])
      for (let k = 0; k < regions.length; k++) {
        const r = regions[k][key];
        if (x >= r[0] && x < r[2] && y >= r[1] && y < r[3]) return k;
      }
    return -1;
  }

  // The clips the warp draws, in priority order, from the clip records the page reads ({name, a, ox, rect, area}).
  // Smaller regions first, so a small clip (the action-skill icon) keeps its pixels where a larger one's margin
  // overlaps it; the largest go last. Empty or degenerate rectangles are dropped; at most MAX_REGIONS are kept.
  // `pinned` ({name, rect} in drawn stage px, e.g. the minimap's stand-in) come first and keep their pixels in place;
  // the result's `pinned` is how many there are.
  function partition(clips, w = STAGE_W, h = STAGE_H, opts = {}, pinned = []) {
    const valid = r => r && r[2] > r[0] && r[3] > r[1];
    const ok = clips.filter(c => c.a && valid(c.rect));
    const area = c => (c.rect[2] - c.rect[0]) * (c.rect[3] - c.rect[1]);
    ok.sort((p, q) => area(p) - area(q) || (p.name < q.name ? -1 : 1));
    const margin = opts.margin ?? MARGIN, sx = w / STAGE_W, sy = h / STAGE_H;
    const box = (r, m) => [Math.max(0, Math.floor((r[0] - m) * sx)), Math.max(0, Math.floor((r[1] - m) * sy)),
      Math.min(w, Math.ceil((r[2] + m) * sx)), Math.min(h, Math.ceil((r[3] + m) * sy))];
    const keep = pinned.filter(c => valid(c.rect)).slice(0, MAX_REGIONS)
      .map(c => ({name: c.name, pinned: true, core: box(c.rect, 0), src: box(c.rect, margin)}));
    const out = keep.concat(ok.slice(0, MAX_REGIONS - keep.length).map(c => clipWarp(c, w, h, opts)));
    out.pinned = keep.length;
    return out;
  }

  // Which path draws the turn: 'warp' needs a WebGL context for the overlay and a Ruffle canvas whose pixels can be
  // read every frame (a WebGL context we created with preserveDrawingBuffer, or a 2D canvas); else 'affine'.
  function choosePath(env) {
    if (env && env.disabled) return {path: 'affine', why: 'warp turned off by the page (?hud3d=affine)'};
    if (!env || !env.overlayGl) return {path: 'affine', why: 'no WebGL for the overlay canvas'};
    if (!env.ruffleCanvas) return {path: 'affine', why: 'no Ruffle canvas found'};
    if (env.ruffleContext === '2d') return {path: 'warp', why: 'Ruffle canvas renderer'};
    if ((env.ruffleContext === 'webgl' || env.ruffleContext === 'webgl2') && env.preserved)
      return {path: 'warp', why: `Ruffle ${env.ruffleContext} with preserveDrawingBuffer`};
    return {path: 'affine', why: `Ruffle context ${env.ruffleContext || 'unknown'} is not readable`};
  }

  // ---- Reading the clips through the harness callback `ow(target, op, a, b)` ------------------------------------
  // getBounds counts hidden children (badassToken's and challenges' hidden panels span a third of the screen), so a
  // clip's region is the union of what is visible: a clip without children gives its bounds; otherwise the union of
  // its visible children's (recursively, at most VIS_DEPTH levels), plus its own bounds whole when they reach outside
  // all its children (shapes of its own that cannot be separated). Text fields have no getBounds: _x, _y, _width,
  // _height. Ruffle reports an empty clip as a point near 6.7e6.
  const VIS_DEPTH = 4;
  const union = (A, B) => !A ? B : !B ? A : [Math.min(A[0], B[0]), Math.min(A[1], B[1]), Math.max(A[2], B[2]), Math.max(A[3], B[3])];
  const inside = (A, B, eps) => A && B && B[0] >= A[0] - eps && B[1] >= A[1] - eps && B[2] <= A[2] + eps && B[3] <= A[3] + eps;
  function boundsOf(b) {
    if (!b || typeof b !== 'object') return null;
    const r = [Number(b.xMin), Number(b.yMin), Number(b.xMax), Number(b.yMax)];
    if (!r.every(Number.isFinite) || r[2] < r[0] || r[3] < r[1] || r.some(v => Math.abs(v) > 1e6)) return null;
    return r;
  }
  // A rectangle in a child's space to its parent's: scale, rotation (degrees), translation.
  function toParent(r, x, y, xs, ys, rot) {
    if (!r) return null;
    const t = (rot || 0) * Math.PI / 180, c = Math.cos(t), s = Math.sin(t);
    const pts = [[r[0], r[1]], [r[2], r[1]], [r[2], r[3]], [r[0], r[3]]].map(([u, v]) => [x + c * u * xs - s * v * ys, y + s * u * xs + c * v * ys]);
    return [Math.min(...pts.map(p => p[0])), Math.min(...pts.map(p => p[1])), Math.max(...pts.map(p => p[0])), Math.max(...pts.map(p => p[1]))];
  }
  const shown = (ow, p) => ow(p, 'get', '_visible') !== false && Number(ow(p, 'get', '_alpha')) > 0;

  // The visible content of `path` in its own coordinates, or null.
  function visibleRect(ow, path, depth = 0) {
    const own = boundsOf(ow(path, 'apply', 'getBounds', []));
    const kids = String(ow(path, 'keys') || '').split(',').filter(k => k && ow(`${path}.${k}`, 'get', '_name') === k);
    if (!kids.length || depth >= VIS_DEPTH) return own;
    let all = null, vis = null;
    for (const k of kids) {
      const p = `${path}.${k}`;
      const x = Number(ow(p, 'get', '_x')), y = Number(ow(p, 'get', '_y'));
      const kb = ow(p, 'apply', 'getBounds', []);
      if (kb === null || kb === undefined) {   // a text field: _width/_height already scaled, in the parent's space
        const w = Number(ow(p, 'get', '_width')), h = Number(ow(p, 'get', '_height'));
        const r = [x, y, x + w, y + h].every(Number.isFinite) && w > 0 && h > 0 ? [x, y, x + w, y + h] : null;
        all = union(all, r);
        if (r && shown(ow, p) && String(ow(p, 'get', 'text') ?? '') !== '') vis = union(vis, r);
        continue;
      }
      const xs = Number(ow(p, 'get', '_xscale')) / 100, ys = Number(ow(p, 'get', '_yscale')) / 100, rot = Number(ow(p, 'get', '_rotation'));
      all = union(all, toParent(boundsOf(kb), x, y, xs, ys, rot));
      if (shown(ow, p)) vis = union(vis, toParent(visibleRect(ow, p, depth + 1), x, y, xs, ys, rot));
    }
    if (own && !inside(all, own, 0.5)) vis = union(vis, own);
    return vis;
  }

  // The turned children of `p1` the warp draws, as {name, a, ox, rect} in flat stage px. `origin` is p1's stage
  // position before hud3d.js scales it; `skip` names clips left to other code (the minimap keeps its stand-in).
  function readClips(ow, p1, names, origin, skip = new Set()) {
    const out = [];
    for (const name of names) {
      if (skip.has(name)) continue;
      const clip = `${p1}.${name}`, a = Number(ow(clip, 'get', '_yrotation'));
      if (!a || Number(ow(clip, 'get', '_rotation')) || !shown(ow, clip)) continue;
      const x = Number(ow(clip, 'get', '_x')), y = Number(ow(clip, 'get', '_y'));
      const xs = Number(ow(clip, 'get', '_xscale')) / 100, ys = Number(ow(clip, 'get', '_yscale')) / 100;
      if (![x, y, xs, ys].every(Number.isFinite)) continue;
      const r = toParent(visibleRect(ow, clip), origin.x + x, origin.y + y, xs, ys, 0);
      if (r) out.push({name, a, ox: origin.x + x, rect: r});
    }
    return out;
  }

  // Where a clip left to other code (the minimap's stand-in) is drawn now, in stage px: p1's and the clip's current
  // transforms (both set by hud3d.js) over its visible content. The warp keeps those pixels in place.
  function drawnRect(ow, p1, name) {
    const clip = `${p1}.${name}`, n = k => Number(ow(clip, 'get', k)), q = k => Number(ow(p1, 'get', k));
    if (!shown(ow, clip)) return null;
    const inner = toParent(visibleRect(ow, clip), n('_x'), n('_y'), n('_xscale') / 100, n('_yscale') / 100, n('_rotation'));
    return toParent(inner, q('_x'), q('_y'), q('_xscale') / 100, q('_yscale') / 100, q('_rotation'));
  }

  // ---- Browser part --------------------------------------------------------------------------------------------
  // Ruffle's WebGL context must keep its drawing buffer so it can be read outside Ruffle's own frame callback.
  // Install before ruffle.js creates the player.
  const seen = {context: null, preserved: false, canvas: null};
  function patchContexts() {
    if (typeof HTMLCanvasElement === 'undefined' || HTMLCanvasElement.prototype.__owWarp) return;
    const original = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.__owWarp = true;
    HTMLCanvasElement.prototype.getContext = function (type, attrs) {
      if (this.__owOverlay) return original.call(this, type, attrs);
      if (type === 'webgl' || type === 'webgl2' || type === 'experimental-webgl') {
        attrs = Object.assign({}, attrs, {preserveDrawingBuffer: true});
        const ctx = original.call(this, type, attrs);
        if (ctx) Object.assign(seen, {context: type === 'experimental-webgl' ? 'webgl' : type, canvas: this,
          preserved: !!(ctx.getContextAttributes && ctx.getContextAttributes().preserveDrawingBuffer)});
        return ctx;
      }
      const ctx = original.call(this, type, attrs);
      if (ctx && (type === '2d' || type === 'webgpu' || type === 'bitmaprenderer'))
        Object.assign(seen, {context: type, canvas: this, preserved: type === '2d'});
      return ctx;
    };
  }

  const VS = `attribute vec2 p; uniform vec2 size; void main() {
    gl_Position = vec4(p.x / size.x * 2.0 - 1.0, 1.0 - p.y / size.y * 2.0, 0.0, 1.0); }`;
  // `back` maps an output pixel (top-left origin) to its source pixel; mode -1 draws in place (pixels no region owns
  // and those of the first `pinned` regions). A source pixel is used only by the region that owns it (owner() below),
  // so each one is drawn once.
  const FS = `precision highp float;
    uniform sampler2D tex; uniform vec2 size; uniform mat3 back; uniform int mode; uniform int count; uniform int pinned;
    uniform vec4 cores[${MAX_REGIONS}]; uniform vec4 rects[${MAX_REGIONS}];
    bool holds(vec4 r, vec2 s) { return s.x >= r.x && s.x < r.z && s.y >= r.y && s.y < r.w; }
    int ownerOf(vec2 s) {   // as owner() in JS
      for (int k = 0; k < ${MAX_REGIONS}; k++) { if (k >= count) break; if (holds(cores[k], s)) return k; }
      for (int k = 0; k < ${MAX_REGIONS}; k++) { if (k >= count) break; if (holds(rects[k], s)) return k; }
      return -1;
    }
    void main() {
      vec2 d = vec2(gl_FragCoord.x, size.y - gl_FragCoord.y);
      vec2 s = d;
      if (mode >= 0) { vec3 h = back * vec3(d, 1.0); s = h.xy / h.z; }
      int o = ownerOf(s);
      if (mode < 0 ? o >= pinned : o != mode) discard;   // in place: unowned and pinned pixels
      gl_FragColor = texture2D(tex, s / size);
    }`;

  function createRenderer(canvas) {
    canvas.__owOverlay = true;
    const opts = {premultipliedAlpha: true, alpha: true, antialias: false, depth: false, stencil: false};
    const gl = canvas.getContext('webgl', opts);
    if (!gl) return null;
    const shader = (type, src) => {
      const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
      return s;
    };
    const prog = gl.createProgram();
    gl.attachShader(prog, shader(gl.VERTEX_SHADER, VS)); gl.attachShader(prog, shader(gl.FRAGMENT_SHADER, FS));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog));
    gl.useProgram(prog);
    const loc = n => gl.getUniformLocation(prog, n);
    const U = {size: loc('size'), back: loc('back'), mode: loc('mode'), count: loc('count'), pinned: loc('pinned'), rects: loc('rects'), cores: loc('cores'), tex: loc('tex')};
    const buf = gl.createBuffer(), attr = gl.getAttribLocation(prog, 'p');
    gl.bindBuffer(gl.ARRAY_BUFFER, buf); gl.enableVertexAttribArray(attr); gl.vertexAttribPointer(attr, 2, gl.FLOAT, false, 0, 0);
    const tex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, tex);
    for (const [k, v] of [[gl.TEXTURE_MIN_FILTER, gl.LINEAR], [gl.TEXTURE_MAG_FILTER, gl.LINEAR],
      [gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE], [gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE]]) gl.texParameteri(gl.TEXTURE_2D, k, v);
    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
    gl.uniform1i(U.tex, 0);
    gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);   // premultiplied "over"
    const fan = pts => { gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(pts.flat()), gl.DYNAMIC_DRAW); gl.drawArrays(gl.TRIANGLE_FAN, 0, pts.length); };
    // GLSL mat3 is column-major: pass the transpose of a row-major matrix.
    const mat3 = M => [M[0], M[3], M[6], M[1], M[4], M[7], M[2], M[5], M[8]];

    function draw(source, regions) {
      const w = source.width, h = source.height;
      if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
      gl.viewport(0, 0, w, h);
      gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
      gl.uniform2f(U.size, w, h);
      const rects = new Float32Array(4 * MAX_REGIONS), cores = new Float32Array(4 * MAX_REGIONS);
      regions.forEach((r, k) => { rects.set(r.src, 4 * k); cores.set(r.core, 4 * k); });
      gl.uniform4fv(U.rects, rects); gl.uniform4fv(U.cores, cores); gl.uniform1i(U.count, regions.length);
      gl.uniform1i(U.pinned, regions.pinned || 0);
      gl.uniform1i(U.mode, -1);
      gl.uniformMatrix3fv(U.back, false, mat3([1, 0, 0, 0, 1, 0, 0, 0, 1]));
      fan([[0, 0], [w, 0], [w, h], [0, h]]);
      regions.forEach((r, k) => {
        if (r.pinned) return;
        gl.uniform1i(U.mode, k);
        gl.uniformMatrix3fv(U.back, false, mat3(r.back));
        fan(r.quad);
      });
    }
    return {draw, gl};
  }

  // Starts the overlay: `ruffleElement` is the <ruffle-player>; returns {path, why, setClips(clips), stats} or the
  // affine choice. `setClips` takes the clip records ({name, a, ox, rect} in flat stage px) whenever they change.
  function start(ruffleElement, log = console.log, disabled = false) {
    const canvas = document.createElement('canvas');
    canvas.style.cssText = 'position:fixed;left:0;top:0;width:100%;height:100%;pointer-events:none';
    let renderer = null, err = '';
    try { renderer = createRenderer(canvas); } catch (e) { err = String(e); renderer = null; }
    const choice = choosePath({disabled, overlayGl: !!renderer, ruffleCanvas: seen.canvas, ruffleContext: seen.context, preserved: seen.preserved});
    if (err) choice.why += ` (${err})`;
    log(`OpenWillow HUD 3D path ${choice.path}: ${choice.why}`);
    if (choice.path !== 'warp') return choice;
    document.body.appendChild(canvas);
    ruffleElement.style.opacity = '0';
    // Supersampling: the hidden player renders at SUPERSAMPLE times the page size and the overlay canvas keeps that
    // size, so the browser's downscale averages the resampled pixels (one bilinear tap per output pixel at 1:1 left
    // the turned bars and their digits visibly softer than the game's on a 2026-10-22 host frame).
    Object.assign(ruffleElement.style, {position: 'fixed', left: '0', top: '0', width: `${100 * SUPERSAMPLE}%`,
      height: `${100 * SUPERSAMPLE}%`});
    let regions = [], last = {clips: [], pinned: [], w: 0, h: 0};
    const split = () => {
      last.w = seen.canvas.width; last.h = seen.canvas.height;
      regions = partition(last.clips, last.w, last.h, {}, last.pinned);
      return regions;
    };
    const stats = {frames: 0, ms: 0};
    const frame = () => {
      const t0 = performance.now();
      // Ruffle resizes its canvas a frame or more after its element changes size: split the regions again for it.
      if (seen.canvas.width !== last.w || seen.canvas.height !== last.h) split();
      try { renderer.draw(seen.canvas, regions); } catch (e) { log('OpenWillow HUD 3D warp draw failed: ' + e); }
      stats.frames++; stats.ms += performance.now() - t0;
      requestAnimationFrame(frame);
    };
    requestAnimationFrame(frame);
    return Object.assign(choice, {
      stats,
      setClips(clips, pinned = []) { last.clips = clips; last.pinned = pinned; return split(); },
    });
  }

  const api = {CX, CY, FOCAL, SCALE, MARGIN, MAX_REGIONS, SUPERSAMPLE, mul, inv, apply, projectPoint, clipHomography, ruffleMatrix,
    clipWarp, owner, partition, choosePath, visibleRect, readClips, drawnRect, patchContexts, start, seen};
  root.owHudWarp = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  if (typeof window !== 'undefined') patchContexts();
})(typeof window !== 'undefined' ? window : globalThis);
