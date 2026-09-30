// Developer-only benchmark; never loaded by inventory.html. This tests an
// isolated native-art player inside a CSS plane, not stock projection values.
window.owProbePanelProjection = async function (visualHoldMs = 0) {
  if (!Number.isFinite(visualHoldMs) || visualHoldMs < 0 || visualHoldMs > 10000)
    throw new Error('Visual hold must be 0..10000 ms');
  window.owPanelProjectionProbeState = {phase:'loading'};
  const began = performance.now();
  const host = document.createElement('div');
  Object.assign(host.style, {position:'fixed', inset:'0', zIndex:'20000',
    perspective:'1200px', pointerEvents:'none'});
  const plane = document.createElement('div');
  Object.assign(plane.style, {position:'absolute', left:'0', top:'0',
    width:'1280px', height:'720px', transformOrigin:'640px 360px',
    transform:'rotateY(20deg)'});
  host.appendChild(plane);
  const isolated = window.RufflePlayer.newest().createPlayer();
  Object.assign(isolated.style, {maskImage:'none', webkitMaskImage:'none'});
  plane.appendChild(isolated);
  document.body.appendChild(host);
  const ow = (path, op, ...args) => isolated.ow(path, op, ...args);
  const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
  try {
    await isolated.ruffle().load({url:'UI_StatusMenu/harness.swf', base:'UI_StatusMenu/'});
    window.owPanelProjectionProbeState.phase = 'waiting-for-movie';
    const deadline = performance.now() + 20000;
    while (typeof isolated.ow !== 'function' || !Number(ow('_level1', 'get', '_totalframes'))
      || Number(ow('_level1', 'get', '_framesloaded')) < Number(ow('_level1', 'get', '_totalframes')))
      { if (performance.now() > deadline) throw new Error('Isolated movie did not initialize'); await pause(50); }
    ow('_level1', 'gotoAndStop', 'inventory');
    window.owPanelProjectionProbeState.phase = 'waiting-for-panel';
    while (!Number(ow('_level1.inventory.equippedPanel.cell1', 'get', '_totalframes')))
      { if (performance.now() > deadline) throw new Error('Native panel not available');
        window.owPanelProjectionProbeState.movie = {
          frame:ow('_level1','get','_currentframe'), inventory:ow('_level1.inventory','get','_totalframes'),
          panel:ow('_level1.inventory.equippedPanel','get','_totalframes')};
        await pause(50); }
    await pause(1000);
    for (const clip of ['header','tooltips','ring','scanlines']) ow('_level1.'+clip,'set','_visible',false);
    for (const clip of ['storagePanel','mainCard','compareCard','ammo','currencyPanel','arrowLeft','arrowRight'])
      ow('_level1.inventory.'+clip,'set','_visible',false);
    const panel = '_level1.inventory.equippedPanel';
    const nativeBounds = ow(panel+'.cell1.hitTestClip','apply','getBounds',['_level1']);
    if (!nativeBounds || !Number.isFinite(nativeBounds.xMin)) throw new Error('Missing cell bounds');
    const target = document.createElement('button');
    Object.assign(target.style, {position:'absolute', left:nativeBounds.xMin+'px', top:nativeBounds.yMin+'px',
      width:(nativeBounds.xMax-nativeBounds.xMin)+'px', height:(nativeBounds.yMax-nativeBounds.yMin)+'px',
      background:'transparent', border:'2px solid lime', pointerEvents:'auto'});
    target.textContent = 'Synthetic hit probe';
    plane.appendChild(target);
    await pause(100);
    const box = target.getBoundingClientRect();
    const hit = document.elementFromPoint((box.left+box.right)/2, (box.top+box.bottom)/2) === target;
    const intervals = [];
    window.owPanelProjectionProbeState.phase = 'measuring';
    let previous = performance.now();
    for (let frame=0; frame<60; frame++) {
      await new Promise(requestAnimationFrame);
      const now = performance.now(); intervals.push(now-previous); previous=now;
    }
    const result = {milliseconds:performance.now()-began, players:2, syntheticYaw:20,
      syntheticPerspective:1200, nativeBounds, transformedHitBounds:
        {left:box.left, top:box.top, width:box.width, height:box.height}, centerHit:hit,
      meanFrameMs:intervals.reduce((a,b)=>a+b,0)/intervals.length,
      maxFrameMs:Math.max(...intervals), stockProjectionVerified:false};
    window.owPanelProjectionProbeState = {phase:'complete', result};
    if (visualHoldMs) await pause(visualHoldMs);
    return result;
  } catch (error) {
    window.owPanelProjectionProbeState = {phase:'failed', error:String(error)};
    throw error;
  } finally {
    host.remove();
  }
};
