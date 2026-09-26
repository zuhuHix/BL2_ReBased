"""Summarize an OpenWillow UI trace (tools/sdk_trace/openwillow_uitrace).

Prints how often each UI class and function ran, the Scaleform bridge calls
(GFxUI functions: what the game told the movie), the ext* callbacks (what the
movie told the game), and, with --timeline, the call sequence for one class.
Traces are game data; keep them and any output under local/.
"""
import argparse
import json
from collections import Counter
from pathlib import Path


def load(path):
    with open(path, encoding='utf-8') as handle:
        return [json.loads(line) for line in handle if line.strip()]


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('trace', help='uitrace_*.jsonl (default: newest in local/ui/traces)', nargs='?')
    parser.add_argument('--timeline', metavar='CLASS', help='print the call sequence of one class, e.g. StatusMenuExGFxMovie')
    parser.add_argument('--limit', type=int, default=400)
    args = parser.parse_args()
    path = Path(args.trace) if args.trace else max(
        (Path(__file__).resolve().parents[2] / 'local' / 'ui' / 'traces').glob('uitrace_*.jsonl'),
        key=lambda p: p.stat().st_mtime)
    records = load(path)
    calls = [r for r in records if r.get('phase') == 'call']
    stop = next((r for r in records if r.get('phase') == 'stop'), None)
    totals = Counter(stop['counts']) if stop else Counter(r['func'] for r in calls)
    print(f'{path.name}: {len(records)} records, {len(calls)} detailed calls, '
          f'{sum(totals.values())} calls counted, errors {sum(r.get("phase") == "error" for r in records)}')

    by_class = Counter()
    for func, n in totals.items():
        by_class[func.rsplit(':', 1)[0].split('.', 1)[-1]] += n
    print('\nUI classes by calls:')
    for name, n in by_class.most_common(25):
        print(f'  {n:8}  {name}')

    bridge = Counter({f: n for f, n in totals.items() if f.startswith('GFxUI.')})
    print('\nScaleform bridge calls (game -> movie):')
    for func, n in bridge.most_common(25):
        print(f'  {n:8}  {func}')

    callbacks = Counter({f: n for f, n in totals.items() if f.rsplit(':', 1)[-1].startswith('ext')})
    print('\nMovie callbacks (movie -> game):')
    for func, n in callbacks.most_common(40):
        print(f'  {n:8}  {func}')

    if args.timeline:
        print(f'\nTimeline for {args.timeline}:')
        shown = 0
        for r in records:
            if r.get('phase') not in ('call', 'return'):
                continue
            owner = r['func'].rsplit(':', 1)[0]
            if args.timeline not in owner and args.timeline not in json.dumps(r.get('obj', '')):
                continue
            detail = r.get('args') if r['phase'] == 'call' else {'ret': r.get('ret')}
            print(f"  {r['seq']:7} {r['t']:9.3f} {r['phase']:6} {r['func']}  {json.dumps(detail, ensure_ascii=False)[:220]}")
            shown += 1
            if shown >= args.limit:
                print('  ...')
                break


if __name__ == '__main__':
    main()
