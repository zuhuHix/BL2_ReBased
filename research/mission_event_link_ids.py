"""
mission_event_link_ids.py - structural oracle for the mission behavior-event link ids.

AI-assisted. Clean-room: no game data in this file; it reads the installed packages and prints aggregate
counts only (pass --examples to also print a few mission/event names locally; do not commit that output).

The high byte of LinkIdAndLinkedBehavior on an *event's* output links selects which occasion the link belongs
to (docs/verification/NATIVE_MISSION_DISPATCH.md, read from native code, UNVERIFIED). That reading predicts
which ids may appear on each kind of event in a mission's BehaviorProviderDefinition:

  objective-named events  0, 1 (level-load replay: not complete / complete), 2 completed, 3 progress updated,
                          4 decremented, 5 cleared
  set-named events        0, 1, 2, 3 (level-load replay), 4 became active, 5 completed, 6 collection completed
  the "Default" event     0-5 (level-load replay: current status), 7-11 (status changed: 6 + new status),
                          12 kickoff, 13 kickoff dialog only, 14 turn-in; 6 is never fired
  any other event         0 (custom events run through MissionTracker.RunMissionCustomEvent)

The oracle counts the ids found per event kind and reports every link outside its predicted set. A clean result
is consistent with the reading; it does not prove it (the installed game is the only proof).

Usage: python research/mission_event_link_ids.py [--packages Startup ...] [--examples]
"""
import argparse
import collections
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import behavior_census as bc  # noqa: E402  package loader and tag walker

PREDICTED = {
    'objective': set(range(0, 6)),
    'set': set(range(0, 7)),
    'Default': set(range(0, 6)) | set(range(7, 15)),
    'other': {0},
}


def props(pkg, index):
    e = pkg.exports[index - 1]
    return bc.walk_tags(pkg, pkg.data, e['off'] + 4, e['off'] + e['size'])[0]


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[1])
    ap.add_argument('--packages', nargs='*', default=['Startup'])
    ap.add_argument('--examples', action='store_true', help='print up to two mission:event names per id (local only)')
    args = ap.parse_args()
    counts = collections.defaultdict(collections.Counter)
    examples = collections.defaultdict(list)
    missions = unreadable = 0
    for name in args.packages:
        pkg = bc.Pkg(os.path.join(bc.GAME, name + '.upk'))
        for i in range(1, len(pkg.exports) + 1):
            if pkg.class_name(i) != 'MissionDefinition':
                continue
            try:
                mission = props(pkg, i)
                provider = mission.get('BehaviorProvider', 0)
                # Every objective set / objective object inside the mission (collection sub-sets are not always
                # listed in ObjectiveSetDefs).
                children = [j for j, e in enumerate(pkg.exports, 1) if e['outer'] == i]
                sets = {pkg.exports[j - 1]['name'] for j in children if pkg.class_name(j).startswith('MissionObjectiveSet')}
                objectives = {pkg.exports[j - 1]['name'] for j in children if pkg.class_name(j) == 'MissionObjectiveDefinition'}
                sequences = props(pkg, provider).get('BehaviorSequences', []) if provider > 0 else []
            except (ValueError, IndexError, KeyError, TypeError):
                unreadable += 1
                continue
            missions += 1
            for sequence in sequences:
                links = sequence.get('ConsolidatedOutputLinkData', [])
                for event in sequence.get('EventData2', []):
                    event_name = event.get('UserData', {}).get('EventName', '?')
                    kind = ('Default' if event_name == 'Default' else 'set' if event_name in sets
                            else 'objective' if event_name in objectives else 'other')
                    first, length = bc.unpack(event.get('OutputLinks', {}))
                    for k in range(first, first + length):
                        link_id = (links[k].get('LinkIdAndLinkedBehavior', 0) & 0xFFFFFFFF) >> 24
                        counts[kind][link_id] += 1
                        if len(examples[(kind, link_id)]) < 2:
                            examples[(kind, link_id)].append(pkg.path(i).split('.')[-1] + ':' + event_name)
    print(f'missions read: {missions}, unreadable: {unreadable}')
    outside = 0
    for kind in ('objective', 'set', 'Default', 'other'):
        ids = counts.get(kind, collections.Counter())
        bad = {k: v for k, v in ids.items() if k not in PREDICTED[kind]}
        outside += sum(bad.values())
        print(f'{kind:<10} links {sum(ids.values()):>5}  ids {dict(sorted(ids.items()))}  outside prediction {bad or "none"}')
    if args.examples:
        for key, names in sorted(examples.items()):
            print(key, names)
    print('links outside the predicted id sets:', outside)
    return 0


if __name__ == '__main__':
    sys.exit(main())
