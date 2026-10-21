// Scaleform 3D for the menu pages. The StatusMenu movie turns some clips about the vertical axis (`_yrotation`: 11 degrees on
// the inventory clip, 7.24 on the skills clip, both at the clip's registration point near the screen centre), which Ruffle
// ignores. A page applies the same turn to its player and hit layer as a CSS 3D transform (the elements opt in with the CSS
// variables set here), at the focal length of Flash's default 55-degree field of view: 640 / tan(27.5 deg) = 1229 px for a
// 1280 px stage. Scaleform's own value is not read (UNVERIFIED); on the 2026-10-04 backpack frame the panel's right edge is
// 0.963 of its left edge's height and this projection gives 0.962.
// Layout fits in the pages are screen positions measured on real frames; toMovie maps such a point back to the movie point
// the turn draws there, so it can be compared with the movie's own (unturned) getBounds.
window.owMenu3d = (() => {
  const FOCAL_LENGTH = 1229;
  let view = {deg:NaN, x:NaN, y:NaN}, scale = 1;
  return {
    // Returns true when the turn changed (the caller re-lays out).
    apply(element, deg, x, y) {
      if (![deg, x, y].every(Number.isFinite)) return false;
      if (deg === view.deg && x === view.x && y === view.y) return false;
      view = {deg, x, y};
      element.style.setProperty('--ow-rotation', `${deg}deg`);
      element.style.setProperty('--ow-origin', `${x * scale}px ${y * scale}px`);
      element.style.setProperty('--ow-focal', `${FOCAL_LENGTH * scale}px`);
      return true;
    },
    // The stage's CSS scale (letterboxing); forces the next apply.
    setScale(value) { scale = value; view = {deg:NaN, x:NaN, y:NaN}; },
    // Inverse of x' = cx + d cos(a) f / (f + d sin(a)), y' = cy + e f / (f + d sin(a)), with d = x - cx, e = y - cy.
    toMovie(xs, ys) {
      if (![view.deg, view.x, view.y].every(Number.isFinite)) return {x:xs, y:ys};
      const a = view.deg * Math.PI / 180, sin = Math.sin(a), cos = Math.cos(a), ds = xs - view.x;
      const d = ds * FOCAL_LENGTH / (cos * FOCAL_LENGTH - ds * sin);
      return {x:view.x + d, y:view.y + (ys - view.y) * (FOCAL_LENGTH + d * sin) / FOCAL_LENGTH};
    },
  };
})();
