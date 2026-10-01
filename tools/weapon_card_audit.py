"""Audit tools/weapon_stats.py against weapon cards observed in the real game.

Input: UI trace JSONL from the existing trace SDK (the game's own ItemCard
callbacks: SetCardUIStats, SetTitle, SetLevelRequirement, SetValue). A card
does not say which parts the item rolled, so for each card this tool takes
every balance of the card's weapon type and manufacturer whose title list can
produce the card's title, enumerates every legal part combination
(tools/weapon_recipe.merge, all game stages), evaluates each with
weapon_stats.evaluate at the card's level, and counts combinations that
reproduce the card. A stat is "reproduced" when some combination matching the
other main stats also matches it.

This is an oracle for the evaluator, not for the roll: the parts are inferred,
and a match is evidence only to the precision the card prints. Generated
reports are game-derived and go under ignored local/.
"""
import argparse
import html
import itertools
import json
import re
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import weapon_recipe  # noqa: E402
import weapon_stats  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
# Card ElementalIcon -> elemental part name suffix (GD_Weap_*.elemental.*_Elemental_<suffix>).
ELEMENT_SUFFIX = {'none': 'None', 'fire': 'Fire', 'shock': 'Shock', 'corrosive': 'Corrosive', 'amp': 'Slag',
                  'explosive': 'Explosive'}
MAIN = ('damage', 'fire_rate', 'reload_time', 'magazine')


def plain(text):
    return html.unescape(re.sub(r'<[^>]*>', '', str(text))).strip()


def number(text):
    match = re.search(r'-?\d+(?:\.\d+)?', text or '')
    return float(match.group()) if match else None


def weapon_cards(rows):
    """Unique weapon cards from trace rows: {title, type_icon, manufacturer, element, level, value, stats{}}."""
    pending, cards, seen = {}, [], set()
    for row in rows:
        function = row.get('func', '')
        if row.get('phase') != 'call' or 'ItemCardGFxObject:' not in function:
            continue
        method, obj, args = function.rsplit(':', 1)[-1], row.get('obj'), row.get('args', {})
        if method == 'SetCardUIStats':
            pending[obj] = {'stats': args.get('TopStats') or []}
            continue
        card = pending.get(obj)
        if card is None:
            continue
        if method == 'SetTitle':
            card['title'] = args
        elif method == 'SetValue':
            card['value'] = args.get('Amount')
        elif method == 'SetLevelRequirement':
            card['level'] = number(plain(args.get('RequirementText', '')))
        elif method == 'SetHeight':
            pending.pop(obj, None)
            title = card.get('title') or {}
            stats = {}
            for stat in card['stats']:
                label, value = plain(stat.get('LabelText', '')), stat.get('ValueText', '')
                projectiles = re.search(r'\[projectilecount\]x(\d+)', value)
                stats[label] = {'value': number(re.sub(r'\[projectilecount\].*', '', value)),
                                'icon': stat.get('IconName'), 'text': plain(value)}
                if projectiles:
                    stats[label]['projectiles'] = int(projectiles.group(1))
            item = {'title': title.get('Title'), 'type_icon': str(title.get('TypeIcon', '')).lower(),
                    'manufacturer': title.get('Manufacturer'), 'element': str(title.get('ElementalIcon', '')).lower(),
                    'level': card.get('level'), 'value': card.get('value'), 'stats': stats}
            key = json.dumps(item, sort_keys=True)
            if 'Damage' in stats and key not in seen:
                seen.add(key)
                cards.append(item)
    return cards


def observed(card):
    """Card -> the numbers weapon_stats' card fields are compared with."""
    stats, out = card['stats'], {}
    for field, label in (('damage', 'Damage'), ('accuracy', 'Accuracy'), ('fire_rate', 'Fire Rate'),
                         ('reload_time', 'Reload Speed'), ('magazine', 'Magazine Size')):
        if label in stats:
            out[field] = stats[label]['value']
    if 'projectiles' in stats.get('Damage', {}):
        out['projectiles'] = stats['Damage']['projectiles']
    for label, stat in stats.items():
        if label.endswith('Chance'):
            out['status_chance'] = stat['value']
        elif label.endswith('/ sec.'):
            out['status_dps'] = stat['value']
    if card.get('value') is not None:
        out['sale_value'] = card['value']
    return out


def agrees(field, card, real):
    """The model's card, rounded as the real card prints it (weapon_stats.display), equals the observation."""
    shown = card.get('display') or {}
    model = shown.get(field, card.get(field))
    if field == 'projectiles':
        model = card.get('projectiles') or 1
    if model is None or real is None:
        return False
    return abs(model - real) < 1e-6


class Catalogue:
    """Balances by (scaleform frame, manufacturer flash label), with their title texts."""

    def __init__(self, package):
        self.package = package
        balances = sorted(p for p, c in package.classes.items() if c == 'WillowGame.WeaponBalanceDefinition')
        # Batch-decode balances, part lists, parts, types, name parts and attribute definitions.
        package.crawl(balances, follow=lambda path, cls: cls.endswith('Definition'))
        self.balances = balances

    def describe(self, balance):
        merged = weapon_recipe.merge(self.package, balance, None)
        weapon_type = self.package.props(merged['weapon_type']) if merged['weapon_type'] else {}
        maker = self.package.props(merged['manufacturer']).get('FlashLabelName') if merged['manufacturer'] else None
        titles = set()
        for entries in merged['merged'].values():
            for part, _ in entries:
                for name in self.package.props(part).get('TitleList') or []:
                    titles.add(self.package.props(name).get('PartName'))
        for name in weapon_type.get('TitleList') or []:
            titles.add(self.package.props(name).get('PartName'))
        return merged, (weapon_type.get('ScaleformFrameName') or '').lower(), maker, titles

    def candidates(self, card):
        found = []
        for balance in self.balances:
            merged, frame, maker, titles = self.describe(balance)
            if frame == card['type_icon'] and maker == card['manufacturer'] and any(
                    t and (card['title'] == t or card['title'].endswith(' ' + t)) for t in titles):
                found.append((balance, merged))
        return found


def combinations(merged, element):
    slots = {}
    for slot, entries in merged['merged'].items():
        parts = list(dict.fromkeys(p for p, _ in entries))
        if slot == 'Elemental' and element in ELEMENT_SUFFIX:
            wanted = [p for p in parts if p.endswith('_' + ELEMENT_SUFFIX[element])]
            parts = wanted or parts
        if parts:
            slots[slot] = parts
    names = sorted(slots)
    for choice in itertools.product(*(slots[n] for n in names)):
        yield dict(zip(names, choice))


def audit_card(package, catalogue, card, levels, limit):
    real = observed(card)
    report = {'card': {k: card[k] for k in ('title', 'type_icon', 'manufacturer', 'element', 'level', 'value')},
              'observed': real, 'balances': [], 'evaluated': 0, 'full_matches': 0, 'examples': [],
              # field -> [agreeing, disagreeing] over the combinations that reproduce the main stats
              'extras_with_main_match': {}}
    stat_any = {field: False for field in real}
    for balance, merged in catalogue.candidates(card):
        report['balances'].append(balance)
        for parts in combinations(merged, card['element']):
            if report['evaluated'] >= limit:
                report['truncated'] = True
                break
            recipe = {'manufacturer': merged['manufacturer'], 'weapon_type': merged['weapon_type'],
                      'parts': {slot: {'part': part} for slot, part in parts.items()}}
            for level in levels(card):
                report['evaluated'] += 1
                model = weapon_stats.evaluate(package, recipe, level)['card']
                hits = {field: agrees(field, model, value) for field, value in real.items()}
                for field, ok in hits.items():
                    stat_any[field] = stat_any[field] or ok
                if all(hits.get(f, True) for f in MAIN):
                    report['full_matches'] += 1
                    for field, ok in hits.items():
                        extra = report['extras_with_main_match'].setdefault(field, [0, 0])
                        extra[0 if ok else 1] += 1
                    if len(report['examples']) < 5:
                        report['examples'].append({'balance': balance, 'level': level,
                                                   'parts': {s: p.rsplit('.', 1)[-1] for s, p in parts.items()},
                                                   'model': {f: model.get(f) for f in real},
                                                   'display': model.get('display'),
                                                   'agrees': hits})
    report['each_stat_reproduced_somewhere'] = stat_any
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True)
    parser.add_argument('--package', required=True, help='.../CookedPCConsole/Startup.upk')
    parser.add_argument('--trace', type=Path, action='append', required=True, help='UI trace JSONL (repeatable)')
    parser.add_argument('--level-offsets', default='0', help='item level = card level requirement + each offset')
    parser.add_argument('--limit', type=int, default=400000, help='evaluations per card')
    parser.add_argument('--output', type=Path, default=ROOT / 'local/weapon_audit/card_audit.json')
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to((ROOT / 'local').resolve()):
        parser.error('--output must stay under local/')
    schema = ROOT / 'local/weapon_audit/weapon_audit.schema'
    schema.parent.mkdir(parents=True, exist_ok=True)
    schema.write_text('\n'.join(weapon_stats.SCHEMA_LINES) + '\n', encoding='utf-8')
    package = weapon_recipe.Package(str(Path(args.reader).resolve()), Path(args.package), str(schema.resolve()))
    started = time.perf_counter()
    rows = []
    for path in args.trace:
        with path.open(encoding='utf-8') as stream:
            rows.extend(json.loads(line) for line in stream)
    cards = weapon_cards(rows)
    catalogue = Catalogue(package)
    offsets = [int(x) for x in args.level_offsets.split(',')]
    results = [audit_card(package, catalogue, card, lambda c: [int(c['level']) + o for o in offsets], args.limit)
               for card in cards]
    summary = {'cards': len(cards), 'with_full_match': sum(1 for r in results if r['full_matches']),
               'elapsed_seconds': round(time.perf_counter() - started, 1)}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps({'summary': summary, 'cards': results}, indent=1), encoding='utf-8')
    for r in results:
        print(json.dumps({'title': r['card']['title'], 'balances': len(r['balances']), 'evaluated': r['evaluated'],
                          'full_matches': r['full_matches'], 'each_stat': r['each_stat_reproduced_somewhere'],
                          'extras': r['extras_with_main_match']}))
    print(json.dumps(summary))


if __name__ == '__main__':
    main()
