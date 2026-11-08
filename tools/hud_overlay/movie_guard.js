// Loads a converted movie into a Ruffle player without ever showing Ruffle's error panel. For a missing or broken movie Ruffle
// draws an opaque orange page over its whole player, and its load() still resolves (the 2026-11-06 playtest: a missing pickup card
// movie covered the game, and stayed). So the files are checked first, and a player that shows the panel at any time in its life is
// removed. A page without its movie carries on without it; the log line says what is missing and how to get it.
(function (root) {
  'use strict';
  // A server hiccup is not a missing file: these are asked again, a few times. Only a 404 counts as missing at once.
  const HEAD_TRIES = 4, HEAD_RETRY_MS = 500, HEAD_TIMEOUT_MS = 3000;
  // What each movie imports, from the converted movies' ImportAssets tags. tools/check_overlay_data.py checks the same files.
  const FILES = {
    statusMenu:['UI_StatusMenu/StatusMenu.swf', 'SharedComponents/ConsoleComponents.swf',
      'SharedWillowComponents/SharedWillowComponents.swf', 'SharedWillowInventory/SharedWillowInventory.swf'],
    hud:['UI_HUD/HUD.swf', 'SharedComponents/ConsoleComponents.swf', 'SharedWillowComponents/SharedWillowComponents.swf', 'gfxfontlib.swf'],
    missionScreen:['UI_Mission/MissionInterface.swf', 'SharedWillowMission/SharedWillowMission.swf', 'SharedComponents/ConsoleComponents.swf',
      'SharedWillowComponents/SharedWillowComponents.swf', 'SharedWillowInventory/SharedWillowInventory.swf'],
    pickupCard:['UI_Inworld/Weapon_Card.swf', 'SharedWillowComponents/SharedWillowComponents.swf'],
    // The shop movie, the two-panel library the page loads beside it, and what both import.
    vending:['UI_VendingMachine/VendingMachineInterface.swf', 'UI_VendingMachine/gfxfontlib.swf', 'gfxfontlib.swf',
      'UI_TwoPanelInterface/TwoPanelInterface.swf', 'UI_TwoPanelInterface/gfxfontlib.swf', 'SharedComponents/ConsoleComponents.swf',
      'SharedWillowComponents/SharedWillowComponents.swf', 'SharedWillowInventory/SharedWillowInventory.swf']
  };

  // Ruffle marks a failed movie by showing its #panic element.
  function panicShown(player) {
    const panic = player.shadowRoot && player.shadowRoot.querySelector('#panic');
    return Boolean(panic && !panic.classList.contains('hidden'));
  }

  const wait = ms => new Promise(resolve => root.setTimeout(resolve, ms));
  // 'present', 'missing' (the server said 404) or 'unavailable' (still no answer after the tries).
  function fileState(url, triesLeft = HEAD_TRIES) {
    const abort = new root.AbortController();
    const timer = root.setTimeout(() => abort.abort(), HEAD_TIMEOUT_MS); // a request that never answers counts as a failed try
    return root.fetch(url, {method:'HEAD', signal:abort.signal}).then(response => response.status, () => 0).then(status => {
      root.clearTimeout(timer);
      if (status >= 200 && status < 300) return 'present';
      if (status === 404) return 'missing';
      return triesLeft > 1 ? wait(HEAD_RETRY_MS).then(() => fileState(url, triesLeft - 1)) : 'unavailable';
    });
  }

  // Resolves true when the movie is loading, false when the page must do without it (the player has then been removed).
  // `requires`: other files the movie loads itself (a harness loads the real movie, which imports others).
  // onLost runs only if the panel appears after this has resolved true, however much later. The page gets one or the other.
  // The promise has cancel(): a caller that gives up (its own timeout, a later setup step failing) calls it. A load that
  // settles after that does nothing (no log, no second remove, no watch), and a watch already running is disconnected.
  // Removing the player is then the caller's job.
  function load(player, options, label, requires = [], onLost = null) {
    let cancelled = false, observer = null;
    const drop = reason => {
      console.log(`OpenWillow movie ${reason}: ${label} (${options.url}); tools/check_overlay_data.py says how to get it`);
      player.remove();
    };
    const urls = [options.url, ...requires];
    const loading = Promise.all(urls.map(url => fileState(url))).then(states => {
      if (cancelled) return false;
      const named = state => urls.filter((_, i) => states[i] === state).join(', ');
      if (states.includes('missing')) { drop(`missing (${named('missing')})`); return false; }
      if (states.includes('unavailable')) { drop(`unavailable (${named('unavailable')})`); return false; }
      return player.ruffle().load(options).then(() => {
        if (cancelled) return false;
        const lost = () => { drop('failed'); if (onLost) onLost(); };
        if (panicShown(player)) { drop('failed'); return false; } // the caller handles a false; onLost is for later
        // The panel is a class change or a new element in the player's shadow DOM, on any later movie or runtime error.
        observer = new root.MutationObserver(() => {
          if (!panicShown(player)) return;
          observer.disconnect();
          lost();
        });
        observer.observe(player.shadowRoot, {subtree:true, childList:true, attributes:true, attributeFilter:['class']});
        return true;
      }, error => { if (cancelled) return false; drop(`failed (${error})`); return false; });
    });
    loading.cancel = () => {
      cancelled = true;
      if (observer) observer.disconnect();
    };
    return loading;
  }

  root.owMovieGuard = {load, panicShown, FILES};
})(typeof window !== 'undefined' ? window : globalThis);
