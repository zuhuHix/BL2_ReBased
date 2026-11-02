#!/usr/bin/env python3
"""Native census of the Fire mission's script call graph (ROADMAP Phase 2, "native dispatch table").

Runs `ow-package --native-census` over tools/slice_native_census_entries.txt (the entry functions, see src/census.hpp for the
format), merges what the VM counted while running them (dynamic) with what the decoded bytecode reaches (static), writes the
full result under local/p2/A/ (ignored, game-derived function names and counts) and prints a ranked table.

Ranking: dynamic call count, then static call sites in closure A (primary edges: direct calls, numbered natives, the
method visible at a receiver's static class), then in closure B (adds subclass overrides and interface implementers, an
upper bound). Core operators (+, ==, &&, ...) are counted but listed separately unless --operators is given.

Nothing here is a measurement of the real game: the dynamic side runs on default objects with every unimplemented native a
zero-result stub, and the static side is an estimate. This file contains no game data.
"""
import argparse
import collections
import json
import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def group_of(package, owner, operator):
    if operator:
        return 'Core.operator'
    if package == 'WillowGame' and 'GFx' in owner:
        return 'GFxUI'
    return {'GFxUI': 'GFxUI', 'Core': 'Core', 'Engine': 'Engine', 'GearboxFramework': 'GearboxFramework',
            'WillowGame': 'WillowGame', 'GameFramework': 'Engine', 'IpDrv': 'Online', 'OnlineSubsystemSteamworks': 'Online',
            'AkAudio': 'Audio'}.get(package, package)


def run_census(reader, package, cooked, entries, steps, static):
    command = [str(reader), str(Path(cooked) / package), '--native-census', str(entries), '--cooked', str(cooked)]
    if steps:
        command += ['--steps', str(steps)]
    if not static:
        command.append('--no-static')
    started = time.time()
    done = subprocess.run(command, capture_output=True, text=True, encoding='utf-8')
    if done.returncode != 0:
        sys.exit(f'native census failed ({done.returncode}): {done.stderr.strip() or done.stdout[:300]}')
    return json.loads(done.stdout), time.time() - started


def merge(report):
    rows = []
    for path, info in report['natives'].items():
        group = group_of(info['package'], info['owner'], info['operator'])
        rows.append({
            'native': f"{info['owner']}.{info['name']}", 'path': path, 'package': info['package'], 'group': group,
            'operator': info['operator'], 'implemented': info['implemented'], 'key': info['key'],
            'dynamic_calls': info['dynamic_calls'], 'dynamic_entries': sorted(int(k) for k in info['dynamic_by_entry']),
            'static_sites_a': info['static_sites_a'], 'static_sites_b': info['static_sites_b'],
            'static_entries_a': info['static_entries_a'], 'static_entries_b': info['static_entries_b'],
            'top_callers': info['top_callers'],
        })
    rows.sort(key=lambda r: (-r['dynamic_calls'], -r['static_sites_a'], -r['static_sites_b'], r['path']))
    return rows


def totals(rows):
    """Per package: distinct natives and calls/sites, by closure."""
    out = collections.OrderedDict()
    for row in sorted(rows, key=lambda r: r['package']):
        t = out.setdefault(row['package'], collections.Counter())
        t['reached_dynamic'] += row['dynamic_calls'] > 0
        t['dynamic_calls'] += row['dynamic_calls']
        t['in_closure_a'] += row['static_sites_a'] > 0
        t['in_closure_b'] += row['static_sites_b'] > 0
        t['sites_a'] += row['static_sites_a']
        t['sites_b'] += row['static_sites_b']
        t['implemented_of_a'] += row['implemented'] and row['static_sites_a'] > 0
        t['implemented_of_b'] += row['implemented'] and row['static_sites_b'] > 0
        t['stub_calls'] += 0 if row['implemented'] else row['dynamic_calls']
    return out


def table(rows, limit):
    head = f"{'#':>3}  {'native':<60} {'package':<18} {'dyn':>5} {'ent':>4} {'siteA':>6} {'siteB':>6}  impl  group"
    lines = [head, '-' * len(head)]
    for rank, row in enumerate(rows[:limit], 1):
        lines.append(f"{rank:>3}  {row['native'][:60]:<60} {row['package']:<18} {row['dynamic_calls']:>5} {len(row['dynamic_entries']):>4} {row['static_sites_a']:>6} "
                     f"{row['static_sites_b']:>6}  {'yes' if row['implemented'] else 'no':<4}  {row['group']}")
    return '\n'.join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(ROOT / 'build' / 'Release' / 'ow-package.exe'))
    parser.add_argument('--cooked', default=None, help='cooked package directory (default: $OPENWILLOW_BL2/WillowGame/CookedPCConsole)')
    parser.add_argument('--package', default='Startup.upk', help='package passed to the reader (the mission definition lives in Startup)')
    parser.add_argument('--entries', default=str(ROOT / 'tools' / 'slice_native_census_entries.txt'))
    parser.add_argument('--out', default=str(ROOT / 'local' / 'p2' / 'A'), help='output directory (keep it under local/)')
    parser.add_argument('--steps', type=int, default=0, help='step limit per entry (0: the reader default)')
    parser.add_argument('--no-static', action='store_true', help='dynamic run only')
    parser.add_argument('--top', type=int, default=60)
    parser.add_argument('--worklist', type=int, default=40, help='rows of the not-implemented table')
    parser.add_argument('--operators', action='store_true', help='include Core operators in the ranked table')
    args = parser.parse_args()

    cooked = args.cooked
    if not cooked:
        game = os.environ.get('OPENWILLOW_BL2')
        if not game:
            sys.exit('set OPENWILLOW_BL2 or pass --cooked')
        cooked = str(Path(game) / 'WillowGame' / 'CookedPCConsole')
    report, seconds = run_census(args.reader, args.package, cooked, args.entries, args.steps, not args.no_static)
    rows = merge(report)
    shown = rows if args.operators else [r for r in rows if not r['operator']]

    entries = report['entries']
    status = collections.Counter(e['status'] for e in entries)
    reasons = collections.Counter(e['stop_reason'] for e in entries if e['status'] != 'completed')
    dynamic_scripts = sum(1 for v in report['script_functions'].values() if v['dynamic_calls'])
    static = report.get('static', {})
    summary = {
        'generated_by': 'tools/slice_native_census.py', 'seconds': round(seconds, 1), 'entries': len(entries),
        'status': dict(status), 'stop_reasons': dict(reasons),
        'script_functions_entered': dynamic_scripts,
        'closure_a_script_functions': sum(1 for v in report['script_functions'].values() if v['static_closure'] == 'A'),
        'closure_b_script_functions': sum(1 for v in report['script_functions'].values() if v['static_closure'] in 'AB'),
        'natives_distinct': len(rows),
        'natives_reached_dynamic': sum(1 for r in rows if r['dynamic_calls']),
        'natives_in_closure_a': sum(1 for r in rows if r['static_sites_a']),
        'natives_in_closure_b': sum(1 for r in rows if r['static_sites_b']),
        'non_operator_dynamic': sum(1 for r in shown if r['dynamic_calls']),
        'non_operator_in_closure_a': sum(1 for r in shown if r['static_sites_a']),
        'non_operator_in_closure_b': sum(1 for r in shown if r['static_sites_b']),
        'dynamic_calls_implemented': sum(r['dynamic_calls'] for r in rows if r['implemented']),
        'dynamic_calls_stub': sum(r['dynamic_calls'] for r in rows if not r['implemented']),
        'unresolved_virtual_sites': sum(static.get('unresolved_virtual', {}).values()),
        'undecodable_reachable': static.get('undecodable', []),
        'dynamic_outside_static': static.get('dynamic_outside_static', []),
    }
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / 'native_census.json').write_text(json.dumps({'summary': summary, 'ranked': rows, 'by_package': totals(rows), 'raw': report},
                                                       indent=1), encoding='utf-8')
    with open(out / 'native_census_ranked.tsv', 'w', encoding='utf-8') as tsv:
        tsv.write('rank\tnative\tpackage\tgroup\toperator\timplemented\tdynamic_calls\tstatic_sites_a\tstatic_sites_b\n')
        for rank, row in enumerate(rows, 1):
            tsv.write(f"{rank}\t{row['native']}\t{row['package']}\t{row['group']}\t{int(row['operator'])}\t{int(row['implemented'])}\t"
                      f"{row['dynamic_calls']}\t{row['static_sites_a']}\t{row['static_sites_b']}\n")

    print(f"entries {summary['entries']}: {dict(status)} {dict(reasons)}; {seconds:.1f}s")
    print(f"script functions: entered {dynamic_scripts}; closure A {summary['closure_a_script_functions']}, "
          f"closure B {summary['closure_b_script_functions']}")
    print(f"natives: {summary['natives_distinct']} distinct reached in B; dynamic {summary['natives_reached_dynamic']}, "
          f"closure A {summary['natives_in_closure_a']}, closure B {summary['natives_in_closure_b']} "
          f"(without operators: {summary['non_operator_dynamic']} / {summary['non_operator_in_closure_a']} / {summary['non_operator_in_closure_b']})")
    print(f"dynamic native calls: {summary['dynamic_calls_implemented']} implemented, {summary['dynamic_calls_stub']} stub")
    print()
    print('ranked by dynamic calls, then closure A sites, then closure B sites:')
    print(table(shown, args.top))
    print()
    work = sorted((r for r in shown if not r['implemented']),
                  key=lambda r: (-r['static_sites_a'], -r['dynamic_calls'], -r['static_sites_b'], r['path']))
    print('not implemented, ranked by closure A sites (a work list for the native dispatch table):')
    print(table(work, args.worklist))
    print()
    print('per package (distinct natives; operators included):')
    for package, t in totals(rows).items():
        print(f"  {package:<26} dynamic {t['reached_dynamic']:>4} ({t['dynamic_calls']:>5} calls, {t['stub_calls']:>5} stub)  "
              f"A {t['in_closure_a']:>4} (impl {t['implemented_of_a']:>3})  B {t['in_closure_b']:>4} (impl {t['implemented_of_b']:>3})")
    print(f"\nfull report: {out / 'native_census.json'}")


if __name__ == '__main__':
    main()
