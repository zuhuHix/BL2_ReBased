// Developer-only probe. Load into the inventory browser after the movie is
// ready; never loaded by inventory.html. Uses original synthetic geometry.
window.owProbeScaleform3D = async function () {
  if (!ready) throw new Error('Inventory movie must be ready');
  const path = ROOT + '.owProjectionProbe' + Date.now();
  const name = path.slice(ROOT.length + 1);
  const started = performance.now();
  const results = [];
  call(ROOT, 'createEmptyMovieClip', name, 29000);
  try {
    call(path, 'beginFill', 0x00ff00, 100);
    for (const point of [[0,0], [100,0], [100,100], [0,100], [0,0]])
      call(path, 'lineTo', ...point);
    call(path, 'endFill');
    set(path, '_x', 100);
    set(path, '_y', 100);
    for (const [label, properties] of [
      ['flat', {}], ['yrotation45', {_yrotation:45}],
      ['depth300', {_z:-300}], ['rotation45', {_rotation:45}]
    ]) {
      for (const [key, value] of Object.entries(properties)) set(path, key, value);
      await new Promise(resolve => setTimeout(resolve, 100));
      results.push({label, bounds:readBounds(path),
        yrotation:get(path, '_yrotation'), z:get(path, '_z')});
    }
    return {milliseconds:performance.now()-started,
      gfxExtensions:get('_global', 'gfxExtensions'), results};
  } finally {
    call(path, 'removeMovieClip');
  }
};
