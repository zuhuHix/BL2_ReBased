// Loads a converted movie into a Ruffle player without ever showing Ruffle's error panel. For a missing or broken movie Ruffle
// draws an opaque orange page over its whole player, and its load() still resolves (the 2026-11-06 playtest: a missing pickup card
// movie covered the game, and stayed). So the files are checked first, and a player that shows the panel anyway is removed. A page
// without its movie carries on without it; the log line says what is missing and how to get it.
(function (root) {
  'use strict';
  const PANIC_CHECKS = 20, PANIC_CHECK_MS = 500;

  // Ruffle marks a failed movie by showing its #panic element.
  function panicShown(player) {
    const panic = player.shadowRoot && player.shadowRoot.querySelector('#panic');
    return Boolean(panic && !panic.classList.contains('hidden'));
  }

  // Resolves true when the movie is loading, false when the page must do without it (the player has then been removed).
  // `requires`: other files the movie loads itself (a harness loads the real movie from ActionScript).
  // onLost runs if the panel appears later, after this has resolved true.
  function load(player, options, label, requires = [], onLost = null) {
    const drop = reason => {
      console.log(`OpenWillow movie ${reason}: ${label} (${options.url}); tools/check_overlay_data.py says how to get it`);
      player.remove();
    };
    const urls = [options.url, ...requires];
    const present = url => root.fetch(url, {method:'HEAD'}).then(response => response.ok, () => false);
    return Promise.all(urls.map(present)).then(found => {
      if (found.includes(false)) {
        drop(`missing (${urls.filter((_, i) => !found[i]).join(', ')})`);
        return false;
      }
      return player.ruffle().load(options).then(() => {
        const watch = checksLeft => {
          if (panicShown(player)) { drop('failed'); if (onLost) onLost(); return; }
          if (checksLeft > 0) root.setTimeout(() => watch(checksLeft - 1), PANIC_CHECK_MS);
        };
        watch(PANIC_CHECKS);
        return true;
      }, error => { drop(`failed (${error})`); return false; });
    });
  }

  root.owMovieGuard = {load, panicShown};
})(typeof window !== 'undefined' ? window : globalThis);
