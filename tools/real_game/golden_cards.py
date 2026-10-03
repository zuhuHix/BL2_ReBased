"""Join a real-game weapon card capture into one golden file.

Inputs (all under the capture folder, normally the ignored local/realgame/cards, written by
tools/real_game/scripts/weapon_cards.py and the inventory walk in tools/real_game/realgame.ps1):
- *_weapons.json / spawned_extra.json: one record per weapon (balance, parts by slot, names, value);
- the card trace (ItemCardGFxObject calls, uitrace row format);
- bp_index.json (optional): screenshot file and the trace sequence number when it was taken.

Output: golden_cards.json, one entry per weapon with its record, the card exactly as the game filled
it (title, manufacturer, type and element icons, level line, sale value, top stats as label/value
text, fun-text lines) and the screenshots that show it. Game data: keep it under local/.
"""
import argparse
import html
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def plain(text):
    text = re.sub(r'<br\s*/?>', '\n', str(text or ''), flags=re.I)
    return html.unescape(re.sub(r'<[^>]*>', '', text))


def card_fills(rows):
    """Each SetItemCardEx call followed by the card setters it triggered."""
    fills, current = [], None
    for row in rows:
        if row.get('phase') != 'call':
            continue
        method, args = row['func'].rsplit(':', 1)[-1], row.get('args', {})
        if method == 'SetItemCardEx':
            current = {'seq': row['seq'], 'item': args.get('InventoryItem'),
                       'compare': args.get('CompareAgainstInventoryItem')}
            fills.append(current)
        elif current is None:
            continue
        elif method == 'SetCardUIStats':
            current['top_stats'] = [{'label': s.get('LabelText'), 'value': s.get('ValueText'),
                                     'aux': s.get('AuxText'), 'icon': s.get('IconName'), 'arrow': s.get('Arrow')}
                                    for s in args.get('TopStats') or []]
        elif method == 'SetTitle':
            current['title'] = {k: args.get(k) for k in args}
        elif method == 'SetFunStats':
            text = next((v for v in args.values() if isinstance(v, str)), '')
            current['fun_stats_raw'] = text
            current['fun_lines'] = [line.strip() for line in plain(text).split('\n') if line.strip()]
        elif method == 'SetValue':
            current['value'] = args.get('Amount', args)
        elif method == 'SetLevelRequirement':
            current['level_line'] = plain(next((v for v in args.values() if isinstance(v, str)), ''))
    return [f for f in fills if 'top_stats' in f]


def object_name(value):
    match = re.search(r"'([^']+)'", value or '')
    return match.group(1) if match else value


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--folder', type=Path, default=ROOT / 'local/realgame/cards')
    parser.add_argument('--trace', default='cardtrace_inventory.jsonl')
    args = parser.parse_args()
    folder = args.folder

    records = {}
    for name in ('own_weapons.json', 'spawned_weapons.json', 'spawned_extra.json'):
        path = folder / name
        if path.is_file():
            for record in json.loads(path.read_text(encoding='utf-8')):
                records[record['object']] = record
    rows = [json.loads(line) for line in (folder / args.trace).open(encoding='utf-8')]
    fills = card_fills(rows)
    shots = json.loads((folder / 'bp_index.json').read_text(encoding='utf-8')) if (folder / 'bp_index.json').is_file() else []

    golden, unmatched = {}, []
    for fill in fills:
        key = object_name(fill['item'])
        if key not in records:
            unmatched.append(key)
            continue
        entry = golden.setdefault(key, {'record': records[key], 'cards': [], 'screenshots': []})
        card = {k: v for k, v in fill.items() if k not in ('item',)}
        if not any(c.get('top_stats') == card.get('top_stats') and c.get('fun_lines') == card.get('fun_lines')
                   and c.get('compare') == card.get('compare') for c in entry['cards']):
            entry['cards'].append(card)
    # A screenshot shows the last card filled before it was taken.
    for shot in shots:
        before = [f for f in fills if f['seq'] <= shot['seq']]
        if before:
            key = object_name(before[-1]['item'])
            if key in golden:
                golden[key]['screenshots'].append(shot['file'])
    out = folder / 'golden_cards.json'
    out.write_text(json.dumps(list(golden.values()), indent=1, ensure_ascii=False), encoding='utf-8')
    missing = [r['name'] for k, r in records.items() if k not in golden]
    print(f'{len(golden)} weapons with cards, {len(fills)} card fills, {len(unmatched)} fills for other items, '
          f'{len(missing)} weapons without a card: {missing}')
    print(out)


if __name__ == '__main__':
    main()
