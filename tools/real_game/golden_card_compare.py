"""Compare tools/weapon_stats.py with the weapon cards the real game printed, using the EXACT rolled parts.

Input: local/realgame/cards/golden_cards.json (tools/real_game/golden_cards.py). Each entry holds the
real game's record of one weapon (balance, the part chosen per slot, game stage, prefix/title name
parts, sale value) and the card exactly as the game filled it. Unlike tools/weapon_card_audit.py,
which has to infer the parts from the card, this tool evaluates the host's evaluator on the parts the
game actually rolled, at the recorded game stage (the level requirement), renders each number the way
the card prints it (weapon_stats.display: damage rounded up, magazine rounded down, the rest to one
decimal, half up; a projectile count as `21x7`; a status chance with a percent sign) and compares the
text field by field: damage and projectile count, accuracy, fire rate, reload, magazine, element status
rows, sale value, name (prefix and title part names joined, plus the host's own name where the record
has one) and the level line.

Reuses weapon_stats.evaluate and weapon_recipe.Package by import. The oracle is the real game, so
this is evidence about the evaluator, not about the roll; the display rounding and the scale rule
stay UNVERIFIED (docs/verification/WEAPON_BALANCE_DECODE.md). A field the evaluator cannot produce
(unresolved attribute, launcher sale price, a type missing from the package) is reported as
`unsupported`, never guessed. The level line is compared in its naive form (always printed, equal to
the game stage) on purpose, so the report shows where the real game omits it. Reports are
game-derived and go under ignored local/.
"""
import argparse
import json
import re
import sys
import time
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
import weapon_recipe  # noqa: E402
import weapon_stats  # noqa: E402

FIELDS = ('damage', 'projectiles', 'accuracy', 'fire_rate', 'reload_time', 'magazine', 'status_dps',
          'status_chance', 'sale_value', 'name', 'host_name', 'level_line')
# Card row label -> field. Status rows are recognised by their suffix (Burn/Shock/Corrode ... / sec., Chance).
LABELS = {'Damage': 'damage', 'Accuracy': 'accuracy', 'Fire Rate': 'fire_rate', 'Reload Speed': 'reload_time',
          'Magazine Size': 'magazine'}
NUMERIC = ('damage', 'accuracy', 'fire_rate', 'reload_time', 'magazine', 'status_dps', 'status_chance', 'sale_value')


def number(text):
    """First number in a card text ('15.6%' -> 15.6, '21x7' -> 21), else None."""
    match = re.search(r'-?\d+(?:\.\d+)?', str(text or ''))
    return float(match.group()) if match else None


def real_fields(entry):
    """One golden entry -> {field: printed text} of the card the game filled (absent fields are omitted)."""
    card = entry['cards'][0]
    out = {}
    for stat in card.get('top_stats') or []:
        label, text = stat['label'], str(stat['value'])
        counted = re.search(r'\[projectilecount\]x(\d+)', text)
        if counted:
            out['projectiles'] = counted.group(1)
            text = re.sub(r'\[projectilecount\].*', '', text)
        if label in LABELS:
            out[LABELS[label]] = text.strip()
        elif label.endswith('/ sec.'):
            out['status_dps'] = text.strip()
        elif label.endswith('Chance'):
            out['status_chance'] = text.strip()
    out['sale_value'] = str(card.get('value'))
    out['name'] = (card.get('title') or {}).get('Title') or ''
    out['level_line'] = card.get('level_line') or ''
    return out


def render(model, level):
    """The evaluator's card -> {field: text} as the real card prints it (None where the model has no value)."""
    shown = model.get('display') or {}

    def text(field, template):
        return template.format(shown[field]) if shown.get(field) is not None else None
    out = {
        'damage': text('damage', '{:d}'),
        'projectiles': str(shown['projectiles']) if shown.get('projectiles') else '1',
        'accuracy': text('accuracy', '{:.1f}'),
        'fire_rate': text('fire_rate', '{:.1f}'),
        'reload_time': text('reload_time', '{:.1f}'),
        'magazine': text('magazine', '{:d}'),
        'status_dps': text('status_dps', '{:.1f}'),
        'status_chance': text('status_chance', '{:.1f}%'),
        'sale_value': str(model['sale_value']) if model.get('sale_value') is not None else None,
        'level_line': f'LEVEL REQUIREMENT: {level}',
    }
    return out


def part_name(package, path):
    """PartName of a WeaponNamePartDefinition, or None when the part is absent."""
    return package.props(path).get('PartName') if path else None


def name_from_parts(prefix, title):
    """Prefix and title part names joined by one space, skipping an absent part (as the host does)."""
    return ' '.join(x for x in (prefix, title) if x)


def compare_field(field, model_text, real_text):
    """Status of one field: match, mismatch, unsupported (the model has no value) or absent (neither side)."""
    if model_text is None:
        return 'unsupported' if real_text is not None else 'absent'
    if real_text is None:
        return 'mismatch'
    return 'match' if model_text == real_text else 'mismatch'


def compare(model_text, real_fields_, host_name=None):
    """{field: {status, model, real, diff}}; fields neither side has are left out.

    `diff` is model minus real for numeric fields (in the printed unit), so a group of mismatches shows
    its direction. The damage row's projectile count is its own field. `host_name` is the name the host
    produced on its own (slice guns only).
    """
    rows = {}
    model_text = dict(model_text)
    real_fields_ = dict(real_fields_)
    if host_name is not None:
        model_text['host_name'] = host_name
        real_fields_['host_name'] = real_fields_.get('name')
    for field in FIELDS:
        if field == 'projectiles' and 'damage' in real_fields_ and 'projectiles' not in real_fields_:
            real_fields_['projectiles'] = '1'
        real, model = real_fields_.get(field), model_text.get(field)
        if field in ('status_dps', 'status_chance') and real is None and model is None:
            continue
        if field == 'host_name' and host_name is None:
            continue
        status = compare_field(field, model, real)
        if status == 'absent':
            continue
        row = {'status': status, 'model': model, 'real': real}
        if field in NUMERIC and number(model) is not None and number(real) is not None:
            row['diff'] = round(number(model) - number(real), 4)
        rows[field] = row
    return rows


def summarize(results, key=lambda r: 'all'):
    """{group: {field: Counter(match/mismatch/unsupported)}} over per-entry comparison rows."""
    table = defaultdict(lambda: defaultdict(Counter))
    for result in results:
        for field, row in (result.get('fields') or {}).items():
            table[key(result)][field][row['status']] += 1
    return table


MAIN = ('damage', 'fire_rate', 'reload_time', 'magazine')
PRINTED = ('damage', 'projectiles', 'accuracy', 'fire_rate', 'reload_time', 'magazine', 'status_dps', 'status_chance')


def rollup(result):
    """Per entry: did the main four stats / every printed card stat / every field (name, price, level line) all match."""
    fields = result.get('fields') or {}

    def all_match(names):
        return bool(fields) and all(fields[n]['status'] == 'match' for n in names if n in fields)
    return {'main_four': all_match(MAIN), 'printed_stats': all_match(PRINTED), 'every_field': all_match(FIELDS)}


def entry_stratum(record):
    return record['source'] if record.get('stratum') is None else f"{record['source']}:{record['stratum']}"


def evaluate_entry(package, entry):
    """Evaluate one golden entry; returns the per-entry result (with `unsupported` set when it cannot)."""
    record = entry['record']
    result = {'name': record['name'], 'balance': record['balance'], 'stratum': entry_stratum(record),
              'level': record['game_stage'], 'class': package.classes.get(record['balance'])}
    recipe = {'manufacturer': record['manufacturer'], 'weapon_type': record['type'],
              'parts': {slot: {'part': path} for slot, path in record['parts'].items() if path}}
    try:
        evaluated = weapon_stats.evaluate(package, recipe, int(record['game_stage']))
        model = evaluated['card']
        prefix, title = part_name(package, record.get('prefix')), part_name(package, record.get('title'))
    except Exception as error:  # a type or part missing from the package, unresolved data
        result['unsupported'] = f'{type(error).__name__}: {error}'
        result['fields'] = {}
        return result
    texts = render(model, record['game_stage'])
    texts['name'] = name_from_parts(prefix, title)
    result['unresolved_attributes'] = evaluated['unresolved_attributes']
    result['sale_value_known'] = model.get('sale_value_known')
    result['model_raw'] = {k: model.get(k) for k in ('damage', 'projectiles', 'accuracy', 'fire_rate', 'reload_time',
                                                    'magazine', 'status_dps', 'status_chance', 'sale_value')}
    result['fields'] = compare(texts, real_fields(entry), record.get('host_name') if record['source'] == 'slice_exact' else None)
    return result


def print_table(table, order=FIELDS):
    cols = [f for f in order if any(f in fields for fields in table.values())]
    print(f"{'group':42}" + ''.join(f'{c[:10]:>12}' for c in cols))
    for group in sorted(table):
        cells = []
        for field in cols:
            counts = table[group].get(field)
            if not counts:
                cells.append('-')
                continue
            total = sum(counts.values())
            cell = f"{counts['match']}/{total}"
            if counts['unsupported']:
                cell += f"({counts['unsupported']}u)"
            cells.append(cell)
        print(f'{group:42}' + ''.join(f'{c:>12}' for c in cells))


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--package', required=True, help='.../CookedPCConsole/Startup.upk')
    parser.add_argument('--golden', type=Path, default=ROOT / 'local/realgame/cards/golden_cards.json')
    parser.add_argument('--output', type=Path, default=ROOT / 'local/realgame/cards/golden_compare.json')
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to((ROOT / 'local').resolve()):
        parser.error('--output must stay under local/')
    started = time.perf_counter()
    schema = args.output.parent / 'golden_compare.schema'
    schema.parent.mkdir(parents=True, exist_ok=True)
    schema.write_text('\n'.join(weapon_stats.SCHEMA_LINES) + '\n', encoding='utf-8')
    package = weapon_recipe.Package(str(Path(args.reader).resolve()), Path(args.package), str(schema.resolve()))
    entries = json.loads(args.golden.read_text(encoding='utf-8'))
    results = [evaluate_entry(package, entry) for entry in entries]
    by_stratum, overall = summarize(results, lambda r: r['stratum']), summarize(results)
    for result in results:
        result['rollup'] = rollup(result)
    totals = {k: sum(1 for r in results if r['rollup'][k]) for k in ('main_four', 'printed_stats', 'every_field')}
    report = {'entries': len(entries), 'unsupported_entries': sum(1 for r in results if r.get('unsupported')),
              'elapsed_seconds': round(time.perf_counter() - started, 1),
              'entries_fully_matching': totals,
              'overall': {f: dict(c) for f, c in overall['all'].items()},
              'by_stratum': {g: {f: dict(c) for f, c in fields.items()} for g, fields in by_stratum.items()},
              'results': results}
    args.output.write_text(json.dumps(report, indent=1), encoding='utf-8')
    print('fields: matched/compared (u = unsupported by the evaluator)')
    print_table(overall)
    print()
    print_table(by_stratum)
    print(f"\nentries matching in the main four stats / every printed stat / every field: "
          f"{totals['main_four']} / {totals['printed_stats']} / {totals['every_field']} of {len(results)}")
    print(f"{report['entries']} entries, {report['unsupported_entries']} unsupported, "
          f"{report['elapsed_seconds']} s; report: {args.output}")


if __name__ == '__main__':
    main()
