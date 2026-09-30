"""Turn observed ItemCard callbacks into local gear-card payloads.

Reads the existing UI Trace SDK's JSONL observations, not executable code.
Cards are observations only: package balance, rolled parts and visual identity
remain unresolved. No equip state, gameplay bonuses or mesh is inferred.
Generated output must stay under this checkout's ignored local/ directory.
"""
import argparse
import html
import json
import re
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TYPES = {'shield': 'shield', 'comm': 'class_mod', 'artifact': 'relic', 'grenade': 'grenade_mod'}


def plain(value):
    return html.unescape(re.sub(r'<[^>]*>', '', str(value))).strip()


def observed_cards(rows, source):
    """Keep each callback object's transaction separate, ending at SetHeight."""
    pending, cards = {}, []
    for row in rows:
        if row.get('phase') != 'call':
            continue
        function = row.get('func', '')
        if 'ItemCardGFxObject:' not in function:
            continue
        method = function.rsplit(':', 1)[-1]
        obj, args = row.get('obj'), row.get('args', {})
        if method == 'SetCardUIStats':
            pending[obj] = {'start': row.get('seq'), 'stats': args.get('TopStats', [])}
            continue
        card = pending.get(obj)
        if card is None:
            continue
        if method == 'SetTitle':
            card['title'] = args
        elif method == 'SetFunStats':
            card['funStatsMarkup'] = args.get('FunStatsText', '')
            card['funStats'] = '; '.join(plain(line).lstrip('• ').strip()
                                       for line in args.get('FunStatsText', '').splitlines() if plain(line))
        elif method == 'SetValue':
            card['saleValue'] = args.get('Amount')
        elif method == 'SetLevelRequirement':
            match = re.fullmatch(r'LEVEL REQUIREMENT: (\d+)', plain(args.get('RequirementText', '')))
            if args.get('bHasRequirement') and match:
                card['level'] = int(match.group(1))
        elif method == 'SetHeight':
            pending.pop(obj, None)
            title = card.get('title', {})
            kind = TYPES.get(str(title.get('TypeIcon', '')).lower())
            if not kind or not title.get('Title'):
                continue
            slug = re.sub(r'[^a-z0-9]+', '-', title['Title'].lower()).strip('-')
            item = {'id': f'{kind}-{slug}-trace-{card["start"]}',
                    'itemType': kind, 'name': title['Title'],
                    'manufacturer': title.get('Manufacturer', ''),
                    'funStats': card.get('funStats', ''),
                    'funStatsMarkup': card.get('funStatsMarkup', ''),
                    'stats': [{'label': plain(s.get('LabelText', '')), 'value': plain(s.get('ValueText', '')),
                               'icon': str(s.get('IconName', ''))} for s in card['stats'] if s.get('LabelText')],
                    'provenance': {'kind': 'movie_callback_observation', 'file': source,
                                   'sequenceStart': card['start'], 'sequenceEnd': row.get('seq'),
                                   'packageResolved': False, 'visualIdentityResolved': False}}
            for field in ('level', 'saleValue'):
                if isinstance(card.get(field), int) and card[field] >= 0:
                    item[field] = card[field]
            color = title.get('Rarity')
            if isinstance(color, dict) and all(isinstance(color.get(c), int) and 0 <= color[c] <= 255 for c in 'RGB'):
                item['rarityColor'] = '#%02X%02X%02X' % tuple(color[c] for c in 'RGB')
            cards.append(item)
    return cards


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('traces', nargs='+', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'local/inventory/observed_gear.json')
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to((ROOT/'local').resolve()):
        parser.error('output must stay under this checkout\'s ignored local/')
    started, found, unique = time.perf_counter(), [], {}
    for path in args.traces:
        with path.open(encoding='utf-8') as stream:
            found.extend(observed_cards((json.loads(line) for line in stream), str(path)))
    for item in found:
        key = json.dumps({k:v for k,v in item.items() if k not in ('id', 'provenance')}, sort_keys=True)
        unique.setdefault(key, item)
    result = {'schemaVersion': 1, 'items': list(unique.values())}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False)+'\n', encoding='utf-8')
    counts = {kind: sum(i['itemType'] == kind for i in unique.values()) for kind in TYPES.values()}
    print(json.dumps({'inputs': len(args.traces), 'observations': len(found), 'uniqueCards': len(unique),
                      'duplicates': len(found)-len(unique), 'types': counts,
                      'elapsedSeconds': round(time.perf_counter()-started, 3), 'output': str(args.output)}))


if __name__ == '__main__':
    main()
