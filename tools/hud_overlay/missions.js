// probe skeleton (replaced below)
const player = window.RufflePlayer.newest().createPlayer();
document.body.appendChild(player);
player.ruffle().load({ url: 'UI_StatusMenu/harness.swf', base: 'UI_StatusMenu/' });
const ROOT = '_level1';
window.owPlayer = player;
window.owCalls = [];
window.owScreen = name => window.owCalls.push(['screen', name]);
const timer = setInterval(() => {
  try {
    if (typeof player.ow !== 'function') return;
    const total = player.ow(ROOT, 'get', '_totalframes');
    if (!total || player.ow(ROOT, 'get', '_framesloaded') < total) return;
    clearInterval(timer);
    for (const name of ['extSetCurrentScreen', 'extSetupNoMissionsPage', 'extLogListRollover', 'extLogListTrack', 'extLogUp', 'extLogDown'])
      window[name] = (...args) => { window.owCalls.push([name, ...args]); };
    player.ow(ROOT, 'gotoAndStop', 'missionlog');
    window.owLoaded = true;
  } catch (e) { console.warn(e); }
}, 100);
for (const name of ['extSetCurrentScreen', 'extSetupNoMissionsPage', 'extLogListRollover', 'extLogListTrack', 'extLogUp', 'extLogDown'])
  window['owProbe_' + name] = (...args) => window.owCalls.push([name, ...args]);
