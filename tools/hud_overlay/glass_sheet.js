// Own-made glass-sheet stand-in for the StatusMenu pages. UNVERIFIED: the
// original ring brightens the scene, while this page can only composite alpha.
// Measured on local/realgame/d20 D1, D2g and D7 (1280x720): scanline pitch is
// about 6 px; the curved bright edges sit around y=25..70 and y=653..690, with
// the side edges slanting inward toward the bottom.
(() => {
  const WIDTH = 1280, HEIGHT = 720, SCANLINE_PITCH = 6;
  const presentation = document.getElementById('presentation');
  const parent = presentation || document.body;
  const canvas = document.createElement('canvas');
  canvas.id = 'ow-glass-sheet';
  canvas.className = 'ow-glass-sheet';
  canvas.setAttribute('aria-hidden', 'true');
  Object.assign(canvas.style, {
    position: presentation ? 'absolute' : 'fixed', inset: '0',
    display: 'block', width: '100%', height: '100%',
    zIndex: '0', pointerEvents: 'none'
  });
  parent.insertBefore(canvas, parent.firstChild);

  const context = canvas.getContext('2d');
  let visible = true;
  function paint() {
    if (!context) return;
    const bounds = canvas.getBoundingClientRect();
    const width = Math.max(1, bounds.width || innerWidth || WIDTH);
    const height = Math.max(1, bounds.height || innerHeight || HEIGHT);
    const pixelRatio = Math.min(2, window.devicePixelRatio || 1);
    const pixelWidth = Math.max(1, Math.round(width * pixelRatio));
    const pixelHeight = Math.max(1, Math.round(height * pixelRatio));
    if (canvas.width !== pixelWidth || canvas.height !== pixelHeight) {
      canvas.width = pixelWidth;
      canvas.height = pixelHeight;
    }
    context.setTransform(pixelWidth / WIDTH, 0, 0, pixelHeight / HEIGHT, 0, 0);
    context.clearRect(0, 0, WIDTH, HEIGHT);

    // Normal-alpha blue wash approximates the brightening blend over a dark scene.
    const wash = context.createLinearGradient(0, 0, 0, HEIGHT);
    wash.addColorStop(0, 'rgba(28, 122, 204, 0.15)');
    wash.addColorStop(0.2, 'rgba(20, 103, 188, 0.11)');
    wash.addColorStop(0.78, 'rgba(17, 99, 188, 0.11)');
    wash.addColorStop(1, 'rgba(33, 135, 211, 0.16)');
    context.fillStyle = wash;
    context.fillRect(0, 0, WIDTH, HEIGHT);

    // A very light sheet body keeps the colour more cyan near its curved edges.
    context.beginPath();
    context.moveTo(-12, 68);
    context.bezierCurveTo(290, 25, 950, 20, 1290, 56);
    context.lineTo(1238, 658);
    context.bezierCurveTo(930, 689, 320, 682, 30, 655);
    context.closePath();
    context.fillStyle = 'rgba(44, 146, 208, 0.045)';
    context.fill();

    // The local captures show fine horizontal lines about six pixels apart.
    context.fillStyle = 'rgba(3, 22, 54, 0.075)';
    for (let y = 0; y < HEIGHT; y += SCANLINE_PITCH) context.fillRect(0, y, WIDTH, 1);
    context.fillStyle = 'rgba(125, 220, 244, 0.025)';
    for (let y = 2; y < HEIGHT; y += SCANLINE_PITCH) context.fillRect(0, y, WIDTH, 1);

    // Shallow curved top/bottom and slanted side edges; subtle at the measured
    // screen positions so the stand-in does not become a second menu frame.
    context.save();
    context.lineWidth = 1.25;
    context.lineCap = 'round';
    context.strokeStyle = 'rgba(105, 226, 246, 0.38)';
    context.shadowColor = 'rgba(70, 213, 242, 0.32)';
    context.shadowBlur = 5;
    context.beginPath();
    context.moveTo(-12, 68);
    context.bezierCurveTo(290, 25, 950, 20, 1290, 56);
    context.moveTo(-12, 653);
    context.bezierCurveTo(320, 682, 930, 689, 1290, 656);
    context.moveTo(-12, 68);
    context.bezierCurveTo(5, 238, 18, 476, 30, 655);
    context.moveTo(1290, 56);
    context.bezierCurveTo(1274, 238, 1256, 487, 1238, 658);
    context.stroke();
    context.restore();
  }

  function setVisible(value) {
    visible = Boolean(value);
    canvas.style.display = visible ? 'block' : 'none';
    if (visible) paint();
  }
  window.owGlassSheet = Object.freeze({setVisible, redraw:paint});
  window.addEventListener('resize', paint);
  if (typeof ResizeObserver === 'function') new ResizeObserver(paint).observe(parent);

  // Skills creates its Ruffle player after this file runs. Keep it at the same
  // stack level as this first-child canvas, where DOM order puts the movie above it.
  if (!presentation && typeof MutationObserver === 'function') {
    const layerMovie = () => {
      for (const player of document.querySelectorAll('ruffle-player')) {
        player.style.position = 'relative';
        player.style.zIndex = '0';
      }
    };
    new MutationObserver(layerMovie).observe(document.body, {childList:true});
    layerMovie();
  }
  paint();
})();
