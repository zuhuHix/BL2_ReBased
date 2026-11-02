"""In-game confirmation of the weapon generator (ow-weapongen): sample, compare, pick-rule frequencies.

Our own code; no game data in it. Everything it reads or writes is game-derived and goes under the ignored
local/realgame/gen_confirm/ (the sample, the live records, the reports).

Subcommands:
- plan: choose a stratified sample of weapon balances (every weapon type, manufacturers spread, a few
  Legendary/Unique balances, stages 5/15/30/50/80), roll each with ow-weapongen and write sample.json
  (the requests with the generated gun) and sample.batch.
- compare: join the live records (tools/real_game/scripts/gen_confirm_live.py: weapon_record + the card
  trace) with the generator's display values, field by field. Each mismatch is classed as runtime-data when
  the gun's evaluation changes with the live overlay (live_weapon_data.json) and the printed value matches
  that overlay, else as a rule failure. Needs the cooked Startup.upk for the overlay check.
- frequency: observed part frequencies per slot from spawn_balance rolls (live) against the cooked slot
  weights (weapon_recipe.merge), chi-square per slot, plus item level vs stage and name choice vs
  weapon_recipe.choose_name_parts on the live parts.

The generator equals weapon_recipe.roll(CrtRand) + weapon_stats.evaluate (its own parity check), so the
expected values here are the generator's. The pick itself is the native rule read from code (UNVERIFIED
until the live frequencies agree); the name rule is UNVERIFIED in the same way.
"""
import argparse
import json
import math
import re
import struct
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'tools'))
sys.path.insert(0, str(HERE))
import weapon_recipe  # noqa: E402
import golden_card_compare as gcc  # noqa: E402
import golden_cards  # noqa: E402

FOLDER = ROOT / 'local/realgame/gen_confirm'
MANIFEST = ROOT / 'local/weapons/gen/weapon_gen_manifest.txt'
GENERATOR = ROOT / 'build/Release/ow-weapongen.exe'
STAGES = (5, 15, 30, 50, 80)
TYPES = ('Pistol', 'SMG', 'Shotgun', 'AssaultRifle', 'Sniper', 'Launcher')
SPECIAL_RARITY = ('Legendary', 'Unique')
CARD_FIELDS = ('name', 'rarity_colour', 'level_line', 'damage', 'projectiles', 'accuracy', 'fire_rate',
               'reload_time', 'magazine', 'status_dps', 'status_chance', 'sale_value')


# ---------------------------------------------------------------- manifest and sample

def read_balances(manifest=MANIFEST):
    """Every weapon balance in the generator manifest: {balance, type, type_short, manufacturer, mission, rarity}."""
    objects, rows, out = {}, [], []
    with Path(manifest).open(encoding='utf-8') as handle:
        for line in handle:
            fields = line.rstrip('\n').split('\t')
            if fields[0] == 'O':
                objects[int(fields[1])] = fields[3]
            elif fields[0] == 'B':
                rows.append(fields)
    for fields in rows:
        balance, weapon_type = objects[int(fields[1])], objects[int(fields[2])]
        out.append({'balance': balance, 'type': weapon_type, 'type_short': type_short(weapon_type),
                    'manufacturer': fields[3] if fields[3] != '-' else None, 'mission': fields[4] == '1',
                    'rarity': rarity_tag(balance)})
    return out


def type_short(weapon_type):
    """WeaponType_Maliwan_Pistol and WT_SMG_Hyperion both name their gun kind (Pistol, SMG, ...)."""
    for token in re.split(r'[_.]', weapon_type):
        if token in TYPES:
            return token
    return None


def rarity_tag(balance):
    for tag in ('Legendary', 'Unique', 'Uncommon', 'Rare', 'Elemental'):
        if tag in balance:
            return tag
    return 'Common'


def plan_sample(balances, count=40, base_seed=1000, stages=STAGES):
    """Stratified requests: one special (Legendary/Unique) per type where one exists, the rest round-robin over
    manufacturers, stages cycled. Mission balances are left out (their card has no level line)."""
    per_type = {t: [b for b in balances if b['type_short'] == t and not b['mission']] for t in TYPES}
    quota = {t: count // len(TYPES) + (1 if i < count % len(TYPES) else 0) for i, t in enumerate(TYPES)}
    picked = []
    for i, t in enumerate(TYPES):
        pool = per_type[t]
        chosen = []
        special = [b for b in pool if b['rarity'] in SPECIAL_RARITY]
        special.sort(key=lambda b: b['rarity'] != SPECIAL_RARITY[i % 2])  # Legendary and Unique alternate by type
        if special and quota[t]:
            chosen.append(special[0])
        buckets = defaultdict(list)
        for b in sorted(pool, key=lambda b: (b['manufacturer'] or '', b['balance'])):
            if b not in chosen:
                buckets[b['manufacturer']].append(b)
        names = sorted(buckets)
        while len(chosen) < quota[t] and any(buckets.values()):
            for name in names:
                if buckets[name] and len(chosen) < quota[t]:
                    chosen.append(buckets[name].pop(0))
        picked.extend(chosen)
    requests = []
    for i, b in enumerate(picked):
        requests.append({'index': i, 'balance': b['balance'], 'type_short': b['type_short'], 'type': b['type'],
                         'manufacturer': b['manufacturer'], 'rarity': b['rarity'],
                         'stage': stages[i % len(stages)], 'seed': base_seed + i})
    return requests


# Extra requests appended after the stratified sample (round 3 used Dahl assault rifles to check a cross-lane report of
# live damage above the card). They name stock balances, so they live in ignored local data:
# local/realgame/gen_confirm/extra_requests.json, a list of {balance, type_short, rarity, stage, seed, note}.
EXTRA_REQUESTS = Path(__file__).resolve().parents[2] / 'local/realgame/gen_confirm/extra_requests.json'


def lane_recipe_gun(package, recipe_path, stage=8):
    """The Weapons 3 lane's Dahl AR as a fixed part list (its coverage file), evaluated by weapon_stats.evaluate on
    the cooked package. It is not a roll: the current weapon_recipe.roll gives other parts for that balance, stage and
    seed (a stale coverage file), so the expected card is the evaluation of the lane's own parts."""
    import weapon_stats  # noqa: E402  (only this path needs the package)
    lane = json.loads(Path(recipe_path).read_text(encoding='utf-8'))
    recipe = {'manufacturer': lane['manufacturer'], 'weapon_type': lane['weapon_type'], 'balance': lane['balance'],
              'prefix': lane.get('prefix') and {'part': lane['prefix']['part']},
              'title': lane.get('title') and {'part': lane['title']['part']},
              'parts': {slot: {'part': v['part']} for slot, v in lane['parts'].items() if v and v.get('part')}}
    evaluated = weapon_stats.evaluate(package, recipe, stage)
    card = evaluated['card']
    return {'balance': lane['balance'], 'stage': stage, 'seed': None, 'manufacturer': lane['manufacturer'],
            'weapon_type': lane['weapon_type'], 'gestalt': lane.get('gestalt'), 'name': lane['name'],
            'prefix': recipe['prefix'], 'title': recipe['title'], 'parts': recipe['parts'],
            'gestalt_fragments': lane.get('gestalt_fragments', []), 'material': lane.get('material'),
            'notes': ['lane recipe: Weapons 3 coverage gun (fixed parts, evaluated, not rolled)'],
            'card': card}


def extra_requests(start_index, path=EXTRA_REQUESTS):
    """The extra requests from the local file (none when it is absent), numbered after the stratified sample."""
    listed = json.loads(Path(path).read_text(encoding='utf-8')) if Path(path).exists() else []
    out = []
    for i, r in enumerate(listed):
        out.append({'index': start_index + i, 'balance': r['balance'], 'type_short': r['type_short'],
                    'type': None, 'manufacturer': r.get('manufacturer'), 'rarity': r['rarity'], 'stage': r['stage'],
                    'seed': r['seed'], 'note': r['note']})
    return out


def write_batch(requests, path):
    Path(path).write_text(''.join(f"balance {r['balance']} {r['stage']} {r['seed']}\n" for r in requests),
                          encoding='utf-8')
    return path


def run_generator(requests, batch_path, manifest=MANIFEST, generator=GENERATOR):
    """ow-weapongen --batch: one gun per request line, in order."""
    import subprocess
    write_batch(requests, batch_path)
    done = subprocess.run([str(generator), str(manifest), '--batch', str(batch_path)],
                          capture_output=True, text=True, encoding='utf-8')
    if done.returncode:
        raise RuntimeError(f'ow-weapongen failed: {done.stderr.strip()}')
    guns = [json.loads(line) for line in done.stdout.splitlines() if line.strip()]
    if len(guns) != len(requests):
        raise RuntimeError(f'{len(guns)} guns for {len(requests)} requests')
    return guns


# ---------------------------------------------------------------- definition fields (for spawn_definition)

SLOTS = ('Body', 'Grip', 'Barrel', 'Sight', 'Stock', 'Elemental', 'Accessory1', 'Accessory2', 'Material')


def definition_fields(gun):
    """WeaponDefinitionData fields that give exactly the generated parts, manufacturer, level and names."""
    stage = int(gun['stage'])
    fields = {'BalanceDefinition': gun['balance'], 'WeaponTypeDefinition': gun['weapon_type'],
              'ManufacturerDefinition': gun['manufacturer'], 'GameStage': stage, 'ManufacturerGradeIndex': stage}
    for slot in SLOTS:
        part = (gun['parts'].get(slot) or {}).get('part')
        if part:
            fields[slot + 'PartDefinition'] = part
    if gun.get('prefix'):
        fields['PrefixPartDefinition'] = gun['prefix']['part']
    if gun.get('title'):
        fields['TitlePartDefinition'] = gun['title']['part']
    return fields


# ---------------------------------------------------------------- the generator's printed card

def expected_card(gun):
    """The generator's display values as the card prints them (same text rules as golden_card_compare.render)."""
    card, shown = gun['card'], gun['card']['display']
    level = card['level_requirement']
    return {
        'name': gun['name'],
        'rarity_colour': card['rarity_color'].upper(),
        'level_line': f'LEVEL REQUIREMENT: {level}' if card['level_line'] else '',
        'damage': f"{int(shown['damage'])}",
        'projectiles': str(card.get('projectiles') or 1),
        'accuracy': f"{shown['accuracy']:.1f}",
        'fire_rate': f"{shown['fire_rate']:.1f}",
        'reload_time': f"{shown['reload_time']:.1f}",
        'magazine': f"{int(shown['magazine'])}",
        'status_dps': f"{shown['status_dps']:.1f}" if 'status_dps' in shown else None,
        'status_chance': f"{shown['status_chance']:.1f}%" if 'status_chance' in shown else None,
        'sale_value': str(card['sale_value']) if card.get('sale_value_known') else None,
    }


def live_card(fill, record):
    """The card the game printed (a golden_cards.card_fills entry) -> the same field texts."""
    entry = {'record': record, 'cards': [fill]}
    fields = dict(gcc.real_fields(entry))
    title = fill.get('title') or {}
    rgb = title.get('Rarity') or {}
    fields['rarity_colour'] = ('#%02X%02X%02X' % (rgb['R'], rgb['G'], rgb['B'])) if rgb else None
    fields.setdefault('status_dps', None)
    fields.setdefault('status_chance', None)
    fields.setdefault('projectiles', '1')
    fields['name'] = title.get('Title') or ''
    return fields


def compare_card(expected, live):
    """{field: {status, model, real}} with the golden comparison's status rules; absent fields are left out."""
    rows = {}
    for field in CARD_FIELDS:
        model, real = expected.get(field), live.get(field)
        if field in ('status_dps', 'status_chance') and model is None and real is None:
            continue
        if field == 'level_line':
            model, real = model or '', real or ''
        if field == 'projectiles' and real is None:
            real = '1'
        status = gcc.compare_field(field, model, real)
        if status != 'absent':
            rows[field] = {'status': status, 'model': model, 'real': real}
    return rows


def join_cards(records, trace_rows):
    """Live records (weapon_record dicts) + the card trace -> {object path: [card fills]} for those records."""
    fills = golden_cards.card_fills(trace_rows)
    by_item = defaultdict(list)
    for fill in fills:
        item = golden_cards.object_name(fill['item'])
        if item in records:
            by_item[item].append(fill)
    return by_item


# ---------------------------------------------------------------- chi-square

def chi_square(observed, expected):
    """observed {category: count}, expected {category: probability}. Returns (statistic, df, p, zero_expected_hits).

    Categories with probability 0 cannot be expected: any count there is a hit and is reported separately
    instead of entering the statistic. p is the Wilson-Hilferty approximation (no scipy here)."""
    total = sum(observed.values())
    stat, cells, hits = 0.0, 0, {}
    small_o, small_e = 0, 0.0  # categories expected < 5 are pooled into one cell (chi-square validity)
    for category in set(observed) | set(expected):
        p = expected.get(category, 0.0)
        o = observed.get(category, 0)
        if p <= 0:
            if o:
                hits[category] = o
            continue
        e = total * p
        if e < 5:
            small_o, small_e = small_o + o, small_e + e
            continue
        stat += (o - e) ** 2 / e
        cells += 1
    if small_e > 0:
        stat += (small_o - small_e) ** 2 / small_e
        cells += 1
    df = max(cells - 1, 1)
    if total == 0:
        return 0.0, df, 1.0, hits
    x = stat / df
    z = ((x ** (1 / 3)) - (1 - 2 / (9 * df))) / math.sqrt(2 / (9 * df))
    p_value = 0.5 * math.erfc(z / math.sqrt(2))
    return stat, df, p_value, hits


def slot_expectation(package, balance, stage):
    """{slot: {part: probability}} from the cooked merged candidates (weapon_recipe.merge, same stage window)."""
    merged = weapon_recipe.merge(package, balance, stage)['merged']
    out = {}
    for slot, entries in merged.items():
        weights = [(p, w) for p, w in entries if w is not None and w > weapon_recipe.MIN_WEIGHT]
        total = sum(w for _, w in weights)
        if total > 0:
            out[slot] = {p: w / total for p, w in weights}
    return out


def frequency_verdict(stat, df, p_value, hits, alpha=0.01):
    """'consistent' when no zero-probability category was hit and p >= alpha; else 'differs'."""
    if hits:
        return 'differs (impossible category hit)'
    return 'consistent' if p_value >= alpha else 'differs'


# ---------------------------------------------------------------- CLI

def cmd_plan(args):
    FOLDER.mkdir(parents=True, exist_ok=True)
    balances = read_balances(args.manifest)
    requests = plan_sample(balances, count=args.count, base_seed=args.base_seed)
    requests += extra_requests(len(requests))
    guns = run_generator(requests, FOLDER / 'sample.batch', args.manifest, args.generator)
    sample = [dict(r, gun=g, fields=definition_fields(g)) for r, g in zip(requests, guns)]
    if args.lane_recipe:  # the lane's own Dahl parts, evaluated on the cooked package (not rolled)
        schema = FOLDER / 'gen_confirm.schema'
        FOLDER.mkdir(parents=True, exist_ok=True)
        import weapon_stats  # noqa: E402
        schema.write_text('\n'.join(weapon_stats.SCHEMA_LINES) + '\n', encoding='utf-8')
        package = weapon_recipe.Package(str(Path(args.reader).resolve()), Path(args.package), str(schema.resolve()))
        gun = lane_recipe_gun(package, args.lane_recipe)
        index = len(sample)
        sample.append({'index': index, 'balance': gun['balance'], 'type_short': 'AssaultRifle', 'type': gun['weapon_type'],
                       'manufacturer': 'Dahl', 'rarity': 'Common', 'stage': 8, 'seed': None,
                       'source': 'lane_recipe', 'gun': gun, 'fields': definition_fields(gun)})
    out = FOLDER / 'sample.json'
    out.write_text(json.dumps(sample, indent=1, ensure_ascii=False), encoding='utf-8')
    kinds = Counter(s['type_short'] for s in sample)
    elements = Counter(s['gun']['card']['element'] for s in sample)
    print(f'{len(sample)} guns -> {out}')
    print('types', dict(kinds))
    print('manufacturers', dict(Counter(s['manufacturer'] for s in sample)))
    print('rarity', dict(Counter(s['rarity'] for s in sample)), 'stages', dict(Counter(s['stage'] for s in sample)))
    print('elements', dict(elements))


def cmd_compare(args):
    import weapon_stats  # noqa: E402  (only for the overlay evaluation)
    sample = json.loads((FOLDER / 'sample.json').read_text(encoding='utf-8'))
    live = json.loads(Path(args.live).read_text(encoding='utf-8'))
    trace = [json.loads(line) for line in Path(args.trace).open(encoding='utf-8')]
    cards = join_cards({r['record']['object']: r for r in live}, trace)
    schema = FOLDER / 'gen_confirm.schema'
    schema.write_text('\n'.join(weapon_stats.SCHEMA_LINES) + '\n', encoding='utf-8')
    reader, pkg, sch = str(Path(args.reader).resolve()), Path(args.package), str(schema.resolve())
    cooked = weapon_recipe.Package(reader, pkg, sch)
    overlay = gcc.live_overlay.LivePackage(reader, pkg, sch, args.live_data) if args.live_data else None
    results, tally = [], Counter()
    for entry in live:
        record = entry['record']
        sample_row = sample[entry['index']]
        fills = cards.get(record['object'], [])
        if not fills:
            results.append({'index': entry['index'], 'name': record['name'], 'error': 'no card fill'})
            continue
        fill = fills[-1]
        exp, got = expected_card(sample_row['gun']), live_card(fill, record)
        rows = compare_card(exp, got)
        status = {f: r['status'] for f, r in rows.items()}
        mismatch = [f for f, s in status.items() if s != 'match']
        row = {'index': entry['index'], 'name': record['name'], 'type': sample_row['type_short'],
               'manufacturer': sample_row['manufacturer'], 'stage': sample_row['stage'],
               'rarity': sample_row['rarity'], 'element': sample_row['gun']['card']['element'],
               'fields': rows, 'mismatch': mismatch, 'class': None}
        if mismatch:
            row['class'] = classify(sample_row, mismatch, rows, cooked, overlay, record, fill)
        results.append(row)
        for f, s in status.items():
            tally[(f, s)] += 1
    report = {'guns': len(live), 'per_field': {f: {s: tally[(f, s)] for s in ('match', 'mismatch', 'unsupported')}
                                                for f in CARD_FIELDS}, 'results': results}
    out = FOLDER / 'compare.json'
    out.write_text(json.dumps(report, indent=1, ensure_ascii=False), encoding='utf-8')
    for f in CARD_FIELDS:
        c = report['per_field'][f]
        print(f"{f:14} match {c['match']:3} mismatch {c['mismatch']:3} unsupported {c['unsupported']:3}")
    classes = Counter(r.get('class') for r in results if r.get('mismatch'))
    print('mismatch classes', dict(classes))
    print(out)


def classify(sample_row, mismatch, rows, cooked, overlay, record, fill):
    """'runtime-data' when evaluating the same exact parts on the live overlay prints every mismatched field as
    the game did; 'rule-failure' otherwise. The overlay replaces only the values in live_weapon_data.json."""
    if overlay is None:
        return 'unclassified (no overlay)'
    entry = {'record': dict(record, source='sdk_spawn', stratum='gen_confirm', host_name=None), 'cards': [fill]}
    live_res = gcc.evaluate_entry(overlay, entry)
    if live_res.get('unsupported'):
        return 'unsupported'
    fields = live_res.get('fields') or {}
    if all(f in fields and fields[f]['status'] == 'match' for f in mismatch):
        return 'runtime-data'
    return 'rule-failure'


def cmd_frequency(args):
    live = json.loads(Path(args.live).read_text(encoding='utf-8'))
    reader, pkg = str(Path(args.reader).resolve()), Path(args.package)
    schema = FOLDER / 'gen_confirm.schema'
    schema.parent.mkdir(parents=True, exist_ok=True)
    import weapon_stats  # noqa: E402
    schema.write_text('\n'.join(weapon_stats.SCHEMA_LINES) + '\n', encoding='utf-8')
    package = weapon_recipe.Package(reader, pkg, str(schema.resolve()))
    report = {}
    for balance_group in live:
        balance, stage, rolls = balance_group['balance'], balance_group['stage'], balance_group['rolls']
        expect = slot_expectation(package, balance, stage)
        per_slot = {}
        for slot in SLOTS:
            observed = Counter(r['parts'].get(slot) or None for r in rolls)
            probs = dict(expect.get(slot, {}))
            if not probs:
                probs = {None: 1.0} if set(observed) <= {None} else {}
            elif None in observed:
                probs.setdefault(None, 0.0)
            stat, df, p, hits = chi_square(observed, probs)
            if len(probs) == 1:  # one candidate: a chi-square has nothing to test, so check the count directly
                (only, _), = probs.items()
                verdict = 'single candidate, every roll' if set(observed) <= {only} else 'differs (other values seen)'
                stat, df, p = 0.0, 0, None
            else:
                verdict = frequency_verdict(stat, df, p, hits) if probs else 'no candidates'
            per_slot[slot] = {'n': len(rolls), 'observed': {str(k): v for k, v in observed.most_common()},
                              'expected': {str(k): round(v * len(rolls), 2) for k, v in probs.items()},
                              'chi2': round(stat, 2), 'df': df, 'p': None if p is None else round(p, 4),
                              'impossible_hits': {str(k): v for k, v in hits.items()}, 'verdict': verdict}
        levels = Counter((stage, r['level']) for r in rolls)
        names = Counter()
        for r in rolls:
            prefix, title = weapon_recipe.choose_name_parts(package, r['type'], r['parts'], r['manufacturer'], r['level'])
            names['match' if (prefix or None) == (r['prefix'] or None) and (title or None) == (r['title'] or None) else 'mismatch'] += 1
        report[balance] = {'stage': stage, 'rolls': len(rolls), 'per_slot': per_slot,
                           'level_vs_stage': {f'{s}->{l}': n for (s, l), n in levels.items()},
                           'name_choice': dict(names)}
        print(f"{balance} stage {stage}: {len(rolls)} rolls; level vs stage {dict(levels)}; name choice {dict(names)}")
        for slot, row in per_slot.items():
            print(f"  {slot:10} chi2 {row['chi2']:8} df {row['df']:3} p {str(row['p']):<7} {row['verdict']}")
            if len(row['expected']) > 1:
                print('      expected ' + ', '.join(f"{k.rsplit('.', 1)[-1]}={v}" for k, v in row['expected'].items()))
                print('      observed ' + ', '.join(f"{k.rsplit('.', 1)[-1]}={v}" for k, v in row['observed'].items()))
    out = FOLDER / 'frequency.json'
    out.write_text(json.dumps(report, indent=1, ensure_ascii=False), encoding='utf-8')
    print(out)


# ---------------------------------------------------------------- evaluated stats (no card UI)

STAT_FIELDS = ('damage', 'fire_rate', 'reload_time', 'magazine', 'spread', 'accuracy', 'projectiles', 'shot_cost',
               'status_dps', 'status_chance', 'sale_value', 'rarity_level', 'level_requirement')
INTEGER_FIELDS = ('projectiles', 'sale_value', 'rarity_level', 'level_requirement')
DISPLAY_FIELDS = ('damage', 'magazine', 'fire_rate', 'reload_time', 'accuracy', 'status_dps', 'status_chance')
LIVE_ATTR = {'damage': 'InstantHitDamage', 'reload_time': 'ReloadTime', 'magazine': 'ClipSize', 'spread': 'Spread',
             'projectiles': 'ProjectilesPerShot', 'shot_cost': 'ShotCost', 'status_dps': 'StatusEffectDamage'}
STATUS_CHANCE_ATTRS = ('StatusEffectChance', 'StatusChance')
LIVE_METHOD = {'rarity_level': 'GetRarityLevel', 'level_requirement': 'GetControllerPlayerExpLevelRequiredToUse'}


def live_number(value):
    """A stats.json entry -> float: an attribute struct's evaluated Value, or a plain number; None when absent."""
    if isinstance(value, dict):
        value = value.get('Value')
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    return float(value)


def live_values(row, record):
    """One stats.json row (equipped reading) and its live record -> {stat field: live value or None}."""
    attrs, methods = row['equipped']['attrs'], row['equipped']['methods']
    out = {field: live_number(attrs.get(name)) for field, name in LIVE_ATTR.items()}
    interval = live_number(attrs.get('FireInterval'))
    out['fire_rate'] = weapon_recipe.f32(1.0 / interval) if interval else None
    out['status_chance'] = next((live_number(attrs[n]) for n in STATUS_CHANCE_ATTRS
                                 if live_number(attrs.get(n)) is not None), None)
    out['sale_value'] = float(record['sale_value']) if record.get('sale_value') is not None else None
    for field, method in LIVE_METHOD.items():
        out[field] = live_number(methods.get(method))
    out['accuracy'] = None  # from Spread by the game's remap (accuracy_percent), filled in by the caller
    return out


def f32_ordinal(value):
    """Single-precision bit pattern as an ordered integer, so the distance between neighbours is 1."""
    bits = struct.unpack('<i', struct.pack('<f', weapon_recipe.f32(value)))[0]
    return bits if bits >= 0 else -(bits & 0x7fffffff)


def float_relation(model, live):
    """'bitwise' (equal single-precision bits), 'ulp1' (neighbours), 'larger', 'absent' (live missing) or 'na'."""
    if model is None and live is None:
        return 'na'
    if model is None or live is None:
        return 'absent' if live is None else 'larger'
    if struct.pack('<f', weapon_recipe.f32(model)) == struct.pack('<f', weapon_recipe.f32(live)):
        return 'bitwise'
    return 'ulp1' if abs(f32_ordinal(model) - f32_ordinal(live)) <= 1 else 'larger'


def exact_relation(model, live):
    if model is None and live is None:
        return 'na'
    if model is None or live is None:
        return 'absent' if live is None else 'larger'
    return 'bitwise' if model == live else 'larger'


def stat_relation(field, model, live):
    return exact_relation(model, live) if field in INTEGER_FIELDS else float_relation(model, live)


def stat_recipe(record):
    """The recipe weapon_stats.evaluate takes, built from a live weapon record (as golden_card_compare does)."""
    return {'manufacturer': record['manufacturer'], 'weapon_type': record['type'], 'balance': record['balance'],
            'prefix': {'part': record['prefix']} if record.get('prefix') else None,
            'title': {'part': record['title']} if record.get('title') else None,
            'parts': {slot: {'part': path} for slot, path in record['parts'].items() if path}}


def evaluated_card(package, record):
    """weapon_stats.evaluate on a package (cooked or live overlay): the unrounded card, or None when unsupported."""
    import weapon_stats  # noqa: E402  (imported here: only the evaluation needs the package)
    try:
        return weapon_stats.evaluate(package, stat_recipe(record), int(record['game_stage']))['card']
    except Exception:  # a part or type missing from the package
        return None


def card_stats(card):
    """The unrounded card values in STAT_FIELDS' names (None where the card has no such value)."""
    out = {field: card.get(field) for field in STAT_FIELDS}
    out['projectiles'] = card.get('projectiles') or 1
    out['rarity_level'] = card.get('rarity_level')
    out['level_requirement'] = card.get('level_requirement')
    return out


def classify_stat(field, cooked, overlay, live):
    """Why a stat differs from the live read: 'runtime-data' when the overlay evaluation agrees with the game,
    'unsupported' when the live read lacks the value, else 'rule-failure'."""
    if stat_relation(field, cooked, live) in ('bitwise', 'ulp1', 'na'):
        return None
    if live is None:
        return 'unsupported'
    if stat_relation(field, overlay, live) in ('bitwise', 'ulp1'):
        return 'runtime-data'
    return 'rule-failure'


def cmd_stats(args):
    import weapon_stats  # noqa: E402
    sample = json.loads((FOLDER / 'sample.json').read_text(encoding='utf-8'))
    live = json.loads(Path(args.live).read_text(encoding='utf-8'))
    rows = json.loads(Path(args.stats).read_text(encoding='utf-8'))
    records = {entry['index']: entry['record'] for entry in live}
    schema = FOLDER / 'gen_confirm.schema'
    schema.write_text('\n'.join(weapon_stats.SCHEMA_LINES) + '\n', encoding='utf-8')
    reader, pkg, sch = str(Path(args.reader).resolve()), Path(args.package), str(schema.resolve())
    cooked = weapon_recipe.Package(reader, pkg, sch)
    overlay = gcc.live_overlay.LivePackage(reader, pkg, sch, args.live_data) if args.live_data else None
    cooked_rounding = weapon_stats.presentation_rounding(cooked)
    overlay_rounding = weapon_stats.presentation_rounding(overlay) if overlay is not None else None
    results = []
    tally = defaultdict(Counter)
    for row in rows:
        index = row['index']
        if 'equipped' not in row:
            results.append({'index': index, 'error': row.get('error', 'no reading')})
            continue
        record, gun = records[index], sample[index]['gun']
        sample_card = gun['card']
        live_vals = live_values(row, record)
        spread = live_vals['spread']
        live_vals['accuracy'] = weapon_stats.accuracy_percent(cooked, spread) if spread is not None else None
        over_card = evaluated_card(overlay, record) if overlay is not None else None
        py_cooked = evaluated_card(cooked, record)
        over_stats = card_stats(over_card) if over_card else {}
        py_stats = card_stats(py_cooked) if py_cooked else {}
        model = card_stats(sample_card)
        fields, classes = {}, {}
        for field in STAT_FIELDS:
            relation = stat_relation(field, model.get(field), live_vals.get(field))
            ov_rel = stat_relation(field, over_stats.get(field), live_vals.get(field)) if over_stats else 'no overlay'
            fields[field] = {'cooked': model.get(field), 'python_cooked': py_stats.get(field),
                             'overlay': over_stats.get(field), 'live': live_vals.get(field),
                             'cooked_vs_live': relation, 'overlay_vs_live': ov_rel}
            tally[('unrounded', 'cooked_vs_live')][relation] += 1
            if over_stats:
                tally[('unrounded', 'overlay_vs_live')][ov_rel] += 1
            tally[('unrounded', 'generator_vs_python_cooked')][stat_relation(field, model.get(field), py_stats.get(field))] += 1
            if relation not in ('bitwise', 'ulp1', 'na'):
                classes[field] = classify_stat(field, model.get(field), over_stats.get(field), live_vals.get(field))
        display_live = weapon_stats.display({k: v for k, v in live_vals.items() if v is not None and k in DISPLAY_FIELDS},
                                            cooked_rounding)
        display_over = weapon_stats.display({k: v for k, v in over_stats.items() if v is not None and k in DISPLAY_FIELDS},
                                            overlay_rounding) if over_stats else {}
        shown = sample[index]['gun']['card']['display']
        for field in DISPLAY_FIELDS:
            if field not in shown:
                continue
            ok = display_live.get(field) == shown[field]
            tally[('display', 'cooked_vs_live')]['match' if ok else ('absent' if field not in display_live else 'mismatch')] += 1
            if over_stats:
                ok_o = display_over.get(field) == shown[field]
                tally[('display', 'overlay_vs_live')]['match' if ok_o else ('absent' if field not in display_over else 'mismatch')] += 1
        results.append({'index': index, 'name': record['name'], 'type': sample[index]['type_short'],
                        'source': 'equipped (readied in a quick slot)', 'fields': fields, 'classes': classes,
                        'display_shown': shown, 'display_live': display_live})
    report = {'guns': len(rows), 'tally': {f'{k[0]}/{k[1]}': dict(v) for k, v in tally.items()},
              'results': results}
    out = FOLDER / 'stats_compare.json'
    out.write_text(json.dumps(report, indent=1, ensure_ascii=False), encoding='utf-8')
    for key, counts in report['tally'].items():
        print(f'{key:40} ' + '  '.join(f'{k} {v}' for k, v in sorted(counts.items())))
    print('classes', dict(Counter(c for r in results for c in r.get('classes', {}).values())))
    print(out)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest='cmd', required=True)
    p = sub.add_parser('plan', help='stratified sample, rolled by ow-weapongen')
    p.add_argument('--manifest', type=Path, default=MANIFEST)
    p.add_argument('--generator', type=Path, default=GENERATOR)
    p.add_argument('--count', type=int, default=40)
    p.add_argument('--base-seed', type=int, default=1000)
    p.add_argument('--lane-recipe', type=Path, default=None,
                   help='a lane coverage file (fixed parts, e.g. the Weapons 3 Dahl AR); needs --reader and --package')
    p.add_argument('--reader', default=None)
    p.add_argument('--package', default=None, help='cooked Startup.upk')
    p.set_defaults(func=cmd_plan)
    c = sub.add_parser('compare', help='live cards vs the generator display')
    c.add_argument('--reader', required=True)
    c.add_argument('--package', required=True, help='cooked Startup.upk')
    c.add_argument('--live', type=Path, default=FOLDER / 'live.json')
    c.add_argument('--trace', type=Path, default=FOLDER / 'cardtrace.jsonl')
    c.add_argument('--live-data', type=Path, default=ROOT / 'local/realgame/cards/live_weapon_data.json')
    c.set_defaults(func=cmd_compare)
    f = sub.add_parser('frequency', help='live pick frequencies vs cooked slot weights')
    f.add_argument('--reader', required=True)
    f.add_argument('--package', required=True)
    f.add_argument('--live', type=Path, default=FOLDER / 'frequency_rolls.json')
    f.set_defaults(func=cmd_frequency)
    s = sub.add_parser('stats', help='evaluated stats read live (no card UI) vs the generator and the overlay')
    s.add_argument('--reader', required=True)
    s.add_argument('--package', required=True, help='cooked Startup.upk')
    s.add_argument('--stats', type=Path, default=FOLDER / 'stats.json')
    s.add_argument('--live', type=Path, default=FOLDER / 'live.json')
    s.add_argument('--live-data', type=Path, default=ROOT / 'local/realgame/cards/live_weapon_data.json')
    s.set_defaults(func=cmd_stats)
    args = parser.parse_args()
    if getattr(args, 'live_data', None) is not None and not args.live_data.is_file():
        args.live_data = None
    args.func(args)


if __name__ == '__main__':
    main()
