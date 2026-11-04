// Shared item-card logic. The inventory page's cards (inventory.js) and the HUD's world pickup card (pickup_card.js) fill
// BL2's own ItemCard clip with the same rows, rounding, colours and compare arrows. Moved unchanged from inventory.js in
// round 24 (2026-11-04); the rules and their UNVERIFIED labels are as documented there and in INVENTORY_CARD_STATS.md.
(function (root) {
  'use strict';
  // Rarity colours (white, white, green, blue, purple, orange) for items without a host `rarityColor`.
  const colors = [0xffffff, 0xffffff, 0x39ff14, 0x3c8dff, 0xb43cff, 0xffb400];
  const gearItemTypes = new Set(['shield', 'grenade_mod', 'class_mod', 'relic']);
  const escapeHtml = text => String(text).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
    // `rounding` is the stat's presentation rounding (docs/verification/NATIVE_WEAPON_RULES.md section 2,
    // read from native code and data; tools/weapon_stats.py present() is the reference): damage up, magazine
    // down, the rest half up to one decimal. The golden cards print accuracy with one decimal and no '%'.
    const weaponCardStats = [
      {key:'damage', label:'Damage', decimals:0, rounding:'ceil', higherIsBetter:true, icon:'weaponDamage'},
      // Optional host field (the presentation remap of the evaluated spread).
      {key:'accuracy', label:'Accuracy', decimals:1, rounding:'half', higherIsBetter:true, icon:'weaponAccuracy'},
      {key:'fireRate', label:'Fire Rate', decimals:1, rounding:'half', higherIsBetter:true, icon:'weaponFireRate'},
      {key:'reloadTime', label:'Reload Speed', decimals:1, rounding:'half', higherIsBetter:false, icon:'weaponsReloadSpeed'},
      {key:'magazine', label:'Magazine Size', decimals:0, rounding:'floor', higherIsBetter:true, icon:'weaponClipSize'}
    ];
    // A stat as the card prints it, on the stored single-precision value (as present() in tools/weapon_stats.py):
    // sizes under 1e-8 print as 0; 'ceil' and 'floor' to an integer; 'half' half up to `decimals`.
    function cardRound(value, rounding, decimals) {
      let stored = Math.fround(value);
      if (Math.abs(stored) < 1e-8) stored = 0;
      if (rounding === 'ceil') return Math.ceil(stored);
      if (rounding === 'floor') return Math.floor(stored);
      // Half up with every step in single precision, as the game's rounding does (weapon_stats.half_up).
      const scale = Math.fround(10 ** decimals);
      return Math.floor(Math.fround(Math.fround(stored * scale) + 0.5)) / scale;
    }
    // An elemental weapon's card adds two rows after the five above: the status effect's damage per second and
    // its chance. Labels and icons are the ones the real cards printed for fire, shock and corrosive guns
    // (golden cards, 2026-10-03); the label of other elements (slag) was not seen, so they get no rows.
    // Both values are Float, one decimal, half up (docs/verification/NATIVE_WEAPON_RULES.md section 2).
    const statusRows = new Map([
      ['fire', {dps:'Burn Damage / sec.', chance:'Ignite Chance', icon:'elementFire'}],
      ['shock', {dps:'Shock Damage / sec.', chance:'Electrocute Chance', icon:'elementShock'}],
      ['corrosive', {dps:'Corrode Damage / sec.', chance:'Corrode Chance', icon:'elementCorrosive'}]
    ]);
    const statIcons = new Map([
      ['capacity','shieldCapacity'], ['rechargerate','shieldRechargeRate'],
      ['rechargedelay','shieldRechargeDelay'], ['ampdamage','weaponDamage'],
      ['ampshotdrain','impactshield']
    ]);
    const weaponTypeIcons = new Map([
      ['pistol','Pistol'], ['smg','SMG'], ['shotgun','Shotgun'],
      ['assault rifle','ar'], ['ar','ar'], ['sniper rifle','Sniper'],
      ['sniper','Sniper'], ['rocket launcher','Rocket'], ['rocket','Rocket'], ['launcher','Rocket']
    ]);
    // Frame names probed in the bench (setting the card's type icon frame and looking at it): Pistol, SMG,
    // Shotgun, ar, Sniper and Rocket draw an icon; 'Sniper Rifle' and 'Rocket Launcher' draw nothing.
    // Card-local pixels of the flavour block the movie's own layout already leaves room for
    // (found empirically in the bench, so UNVERIFIED against the original).
    const FUN_STATS_OVERLAP = 6;
    const ARROW_VALUE_GAP = 2;
    // Real card text is about 17 px against 15 px here, values bolder (critic round 11, measured on the 2026-10-04 frames).
    const CARD_TEXT_SIZE = 16;
    const PROJECTILE_COUNT_SIZE = 10, PROJECTILE_COUNT_COLOUR = '#e6d223';
    function itemColor(item) {
      const raw = item?.rarityColor;
      if (typeof raw === 'string' && /^#[\da-f]{6}$/i.test(raw)) return Number.parseInt(raw.slice(1), 16);
      if (Number.isInteger(item?.rarity) && item.rarity >= 0 && item.rarity < colors.length)
        return item.rarityKnown === false ? colors[0] : colors[item.rarity];
      return colors[0];
    }
    // Card type-icon frames were probed in the bench: 'Shield', 'Grenade' and 'Artifact' (the relic)
    // draw an icon; no frame named like 'Class Mod' does, so class mods show none (frame name
    // unknown, UNVERIFIED).
    function movieTypeIcon(item) {
      if (item?.itemType === 'shield') return 'Shield';
      if (item?.itemType === 'grenade_mod') return 'Grenade';
      if (item?.itemType === 'class_mod') return 'Class Mod';
      if (item?.itemType === 'relic') return 'Artifact';
      const type = String(item?.type || 'pistol').trim().toLowerCase();
      return weaponTypeIcons.get(type) || 'Pistol';
    }
    function normalizeStatKey(value) { return String(value || '').toLowerCase().replace(/[^a-z0-9]/g, ''); }

    function cardStats(item) {
      if (Array.isArray(item?.stats)) return item.stats.slice(0,5).map(stat => {
        const key = normalizeStatKey(stat.key || stat.label);
        return {
          key,
          label:String(stat.label || 'Stat'),
          value:stat.value,
          decimals:Number.isInteger(stat.decimals) ? stat.decimals : statDecimals(stat.value),
          higherIsBetter:typeof stat.higherIsBetter === 'boolean' ? stat.higherIsBetter : undefined,
          icon:stat.icon || statIcons.get(key)
        };
      });
      // Weapon values arrive unrounded; the printed number (and so the compare delta) is the rounded one.
      const stats = weaponCardStats.filter(stat => item?.[stat.key] !== undefined && item?.[stat.key] !== null)
        .map(stat => {
          const raw = item[stat.key];
          const value = typeof raw === 'number' && Number.isFinite(raw) ? cardRound(raw, stat.rounding, stat.decimals) : raw;
          // A multi-projectile weapon prints its projectile count after the damage ("21x7").
          const projectiles = stat.key === 'damage' && Number.isInteger(item.projectiles) && item.projectiles > 1 ? item.projectiles : 0;
          return projectiles ? {...stat, value, projectiles} : {...stat, value};
        });
      const status = statusRows.get(String(item?.element || '').toLowerCase());
      if (status && Number.isFinite(item.statusDps) && Number.isFinite(item.statusChance)) {
        stats.push({key:'statusdps', label:status.dps, value:cardRound(item.statusDps, 'half', 1), decimals:1,
          higherIsBetter:true, icon:status.icon});
        stats.push({key:'statuschance', label:status.chance, value:cardRound(item.statusChance, 'half', 1), decimals:1,
          suffix:'%', higherIsBetter:true, icon:status.icon});
      }
      return stats;
    }

    // Gear stat values arrive as display strings ("+88%", "6 m"); their number is compared as-is.
    const statNumber = value => typeof value === 'number' ? value
      : typeof value === 'string' && /^\s*[-+]?\d/.test(value) ? Number.parseFloat(value) : NaN;
    function statDecimals(value) {
      if (typeof value !== 'string') return 1;
      const match = /^\s*[-+]?\d*\.(\d+)/.exec(value);
      return match ? match[1].length : 0;
    }

    function statValueText(stat) {
      if (typeof stat.value === 'string') return stat.value;
      const value = Number(stat.value);
      if (stat.value !== '' && stat.value !== null && Number.isFinite(value))
        return value.toFixed(stat.decimals) + (stat.suffix || '') + (stat.projectiles ? `x${stat.projectiles}` : '');
      return String(stat.value ?? '');
    }

    // The compare arrow of one row. The real game shows an arrow only (green up = better, red down = worse,
    // none when equal) and no difference figure: every golden card's aux field is empty and the 2026-10-04
    // compare captures print none. A multi-projectile weapon's damage compares as damage times projectiles
    // (a 21x7 gun showed the better arrow against 25x2, which the per-projectile figures would not).
    function statComparison(stat, otherStats) {
      const other = otherStats.find(candidate => candidate.key === stat.key);
      if (!other) return null;
      const value = statNumber(stat.value) * (stat.projectiles || 1), otherValue = statNumber(other.value) * (other.projectiles || 1);
      if (!Number.isFinite(value) || !Number.isFinite(otherValue)) return null;
      const delta = value - otherValue;
      if (Math.abs(delta) < 1e-9 || typeof stat.higherIsBetter !== 'boolean') return {arrow:'blank'};
      const better = stat.higherIsBetter ? delta > 0 : delta < 0;
      return {arrow:better ? 'up' : 'down'};
    }


  // A card writer drives one Ruffle player through its harness API ({call, get, set}, as inventory.js defines them).
  function createCardWriter(api) {
    const {call, get, set} = api;
    const statMainX = new Map();
    const statVisibility = new Map();
    function text(path, value, size = 15, tint = 0xffffff, align = '') {
      richText(path, escapeHtml(value), size, tint, align);
    }
    function richText(path, html, size = 15, tint = 0xffffff, align = '') {
      const body = `<font face="$WillowBody" size="${size}" color="#${tint.toString(16).padStart(6,'0')}">${html}</font>`;
      set(path, 'htmlText', align ? `<p align="${align}">${body}</p>` : body);
    }
    const color = itemColor;
    // Frame style: the movie's card background has three frames, named by SetBackgroundStyle (probed in
    // the bench: default, 'highlight' = yellow, 'compare' = green). The game trace calls
    // SetBackgroundStyle('highlight') on both cards while the menu opens, and both reference captures
    // show a yellow frame around the selected card whatever its rarity, so the frame is a state, not a
    // rarity colour: the main card is 'highlight'. That the game uses 'compare' for the second card is
    // only inferred from the frame name (UNVERIFIED). Rarity colours the name, manufacturer and type icon.
    function configureCard(card, item, compareItem = null, style = 'highlight', equipped = false, playerLevel = NaN) {
      call(card, 'SetVisible_', Boolean(item));
      if (!item) return;
      call(card, 'SetBackgroundStyle', style);
      const element = String(item.element || 'None');
      call(card, 'SetTitle', String(item.manufacturer || '').toLowerCase(), escapeHtml(item.name), color(item),
        movieTypeIcon(item), element === 'None' ? 'none' : element.toLowerCase(),
        equipped);
      // SetTitle counts the name's lines with Scaleform's getLineLength, which Ruffle lacks, so every name counted as two lines
      // and the stats started a line low (real cards put them right under a one-line name). The field's bottomScroll is the
      // number of lines it shows (all of them: the field is two lines tall).
      const titleLines = Number(get(`${card}.titleLine`, 'bottomScroll'));
      if (titleLines >= 1 && titleLines <= 2) call(card, 'SetRequiredTitleLines', titleLines);
      call(card, 'TurnOffAllTopStats');
      const stats = cardStats(item), otherStats = cardStats(compareItem || {});
      for (let index=0; index<7; index++) {
        for (const field of ['mainField', 'auxField', 'arrow']) statVisibility.set(`${card}.stat${index+1}.${field}`, false);
        set(`${card}.stat${index+1}.arrow`, '_visible', false);
        set(`${card}.stat${index+1}.auxField`, '_visible', false);
      }
      stats.forEach((stat, index) => {
        const formatted = statValueText(stat);
        const compare = compareItem ? statComparison(stat, otherStats) : null;
        const arrow = compare?.arrow || 'blank';
        call(card, 'SetTopStat', index, stat.label, formatted, arrow, '', stat.icon || 'none');
        const row = `${card}.stat${index+1}`;
        text(`${row}.labelField`, stat.label, CARD_TEXT_SIZE, 0xa4e8f3);
        // Values print in the label colour; a projectile count follows the damage in gold at a smaller size
        // (both measured on a real card, UNVERIFIED as exact game values).
        const mainHtml = stat.projectiles
          ? `${escapeHtml(formatted.slice(0, -`x${stat.projectiles}`.length))}<font size="${PROJECTILE_COUNT_SIZE}" color="${PROJECTILE_COUNT_COLOUR}">x${stat.projectiles}</font>`
          : escapeHtml(formatted);
        richText(`${row}.mainField`, `<b>${mainHtml}</b>`, CARD_TEXT_SIZE, 0xa4e8f3);
        statVisibility.set(`${row}.mainField`, true);
        const arrowShown = compare?.arrow === 'up' || compare?.arrow === 'down';
        statVisibility.set(`${row}.auxField`, false);
        statVisibility.set(`${row}.arrow`, arrowShown);
        if (arrowShown) set(`${row}.arrow`, '_visible', true);
        // On real cards a value without an arrow ends at the arrow column's right edge and one with an arrow ends just before
        // the arrow (2026-10-21 elemental and compare cards); the movie's field ended at the arrow's left edge, so values sat
        // about 15 px left of the real ones. The field's natural x is remembered per field.
        if (!statMainX.has(`${row}.mainField`)) statMainX.set(`${row}.mainField`, Number(get(`${row}.mainField`, '_x')));
        const arrowWidth = Number(get(`${row}.arrow`, '_width')) || 0;
        set(`${row}.mainField`, '_x', statMainX.get(`${row}.mainField`) + arrowWidth - (arrowShown ? arrowWidth + ARROW_VALUE_GAP : 0));
      });
      // Legacy plain-text payloads are '; '-joined. The weapon contract puts red
      // text first; observed gear formatting below overrides this fallback.
      const funLines = typeof item.funStats === 'string' ? item.funStats.split(/;\s*/).filter(Boolean) : [];
      const redFirst = !gearItemTypes.has(item?.itemType);
      const funStats = funLines.map((line, index) =>
        `• <font color="${redFirst && index === 0 ? '#dc4646' : '#ffffff'}">${escapeHtml(line)}</font>\n`).join('');
      // Preserve observed gear colours/emphasis in the movie's TextField. This payload
      // is passed only to Flash; it must never be inserted into browser HTML.
      call(card, 'SetFunStats', typeof item.funStatsMarkup === 'string' && item.funStatsMarkup ? item.funStatsMarkup : funStats);
      // ItemCardGFxObject.SetItemCardEx (script): the strip shows only for a required level above 1; `met` is
      // IsLevelRequirementMet. The movie's own SetLevelRequirement turns the card red when it is not met. Overpower levels
      // and DLC-restricted items have their own texts there; the host has neither.
      const levelKnown = item.levelKnown !== false && Number.isFinite(Number(item.level));
      const showLevel = levelKnown && Number(item.level) > 1;
      call(card, 'SetLevelRequirement', showLevel, !levelKnown || Number(item.level) <= Number(playerLevel), false,
        showLevel ? `LEVEL REQUIREMENT: ${item.level}` : '');
      // Show the sale value whenever the host sends a number (valueKnown only says how sure it is).
      const valueKnown = item.value !== null && item.value !== '' && item.value !== undefined && Number.isFinite(Number(item.value));
      call(card, 'SetValue', valueKnown ? Number(item.value) : 0);
      call(card, 'SetEridiumValue', 0);
      set(card + '.valueClip', '_visible', valueKnown);
      set(card + '.eridiumCounter', '_visible', false);
      call(card, 'SetHeight');
      fitFunStats(card, funLines.length > 0);
    }

    // Ruffle has no TextField.numLines, which ItemCard.CalculateHeight needs to size the flavour
    // block, so the card comes out short and the manufacturer/type row lands on the flavour text.
    // Grow the card by the text's real height and push that row (and the value plate) down.
    function fitFunStats(card, hasFunStats) {
      if (!hasFunStats) return;
      const textHeight = Number(get(`${card}.funstats`, 'textHeight'));
      const height = Number(get(card, 'dynamicHeight'));
      if (!(textHeight > 0) || !Number.isFinite(height)) return;
      const extra = textHeight - FUN_STATS_OVERLAP;
      for (const row of ['manufacturer', 'typeIcon', 'elementalIcon'])
        set(`${card}.${row}`, '_y', Number(get(`${card}.${row}`, '_y')) + extra);
      call(card, 'SetDirectHeight', height + extra);
    }

    return {configureCard, fitFunStats, statVisibility, statMainX};
  }

  root.owItemCard = {colors, gearItemTypes, escapeHtml, weaponCardStats, cardRound, statusRows, statIcons, weaponTypeIcons,
    FUN_STATS_OVERLAP, ARROW_VALUE_GAP, CARD_TEXT_SIZE, PROJECTILE_COUNT_SIZE, PROJECTILE_COUNT_COLOUR,
    itemColor, movieTypeIcon, normalizeStatKey, cardStats, statNumber, statDecimals, statValueText, statComparison, createCardWriter};
})(typeof window !== 'undefined' ? window : globalThis);
