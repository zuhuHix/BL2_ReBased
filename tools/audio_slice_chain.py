#!/usr/bin/env python3
"""Identity chain for the Sanctuary slice sounds (AI-assisted, clean room, read-only).

UE3 side (our own reader, `ow-package`): mission dialog triggers -> dialog group -> Talk act -> AkEvent,
the door's InterpTrackAkEvent keys, the lent pistol's fire sound and reload notifies.
Wwise side (`tools/audio_census.py`): AkEvent.WwiseName -> FNV-1 id -> bank(s) -> actions -> sounds -> .pck bytes.
Output (ignored, game-derived): local/audio/slice_chain.json and local/slice/audio.json.
Raw .wem files are copied to local/audio/wem/ ; decoding is a separate, still undecided step.

  python tools/audio_slice_chain.py [--game DIR] [--reader build/Release/ow-package.exe]
"""
import argparse
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import audio_census as ac  # noqa: E402

MISSION = 'GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire'
# `kickoff` plays the kickoff (Default id 12) after acceptance, as the slice host does (NATIVE_MISSION_DISPATCH.md B9).
MISSION_STEPS = ['accept', 'kickoff', 'tick:1', 'tick:3', 'obj:RockPaper_GoToRange', 'tick:5', 'obj:Fire', 'turnin']
DOOR_TRACKS = ['TheWorld.PersistentLevel.Main_Sequence.RocksPaperGenocide.InterpData_2.InterpGroup_0.InterpTrackAkEvent_0',
               'TheWorld.PersistentLevel.Main_Sequence.RocksPaperGenocide.InterpData_2.InterpGroup_0.InterpTrackAkEvent_1']
WEAPON_TYPE = 'GD_Weap_Pistol.A_Weapons.WeaponType_Maliwan_Pistol'
ANIM_SET = 'Anim_1st_Person.Pistol'
MARCUS_GROUP = 'GD_Dialog_NPCImplementation.Groups.DialogGroup_NPC_Marcus'


class Ue3:
    def __init__(self, reader, cooked):
        self.reader, self.cooked, self._exports = reader, cooked, {}

    def run(self, pkg, *args):
        done = subprocess.run([self.reader, os.path.join(self.cooked, pkg + '.upk'), *map(str, args)], capture_output=True, encoding='utf-8')
        if done.returncode:
            raise RuntimeError('%s %s: %s' % (pkg, args, done.stderr.strip()))
        return json.loads(done.stdout)

    def index(self, pkg):
        if pkg not in self._exports:
            self._exports[pkg] = {e['path'].lower(): e for e in self.run(pkg, '--exports')}
        return self._exports[pkg]

    def find(self, pkg, path):
        return self.index(pkg)[path.lower()]

    def dump(self, pkg, path):
        return self.run(pkg, '--object-dump', self.find(pkg, path)['index'], 4, '--cooked', self.cooked)['properties']

    def ak_event(self, pkg, path):
        """AkEvent export -> {path, wwise_name, short_id, min/max duration}; checks FNV-1 against ShortId."""
        exp = self.find(pkg, path)
        raw = self.run(pkg, '--properties', exp['index'], '--property-offset', 4)['properties']
        p = {x['name']: x['value'] for x in raw if x['status'] == 'decoded'}
        short = p['ShortId'] & 0xffffffff
        return {'ue3_path': exp['path'], 'package': pkg, 'wwise_name': p['WwiseName'], 'short_id': short,
                'fnv1_matches_short_id': ac.fnv1_32(p['WwiseName']) == short, 'min_duration': p.get('MinDuration'),
                'max_duration': p.get('MaxDuration'), 'required_bank': p.get('RequiredBank')}


def dialog_chain(ue3):
    """The mission's dialog effects -> group -> Talk act(s) -> AkEvent (decoded by ow-package --object-dump)."""
    run = subprocess.run([ue3.reader, os.path.join(ue3.cooked, 'Startup.upk'), '--mission-run', MISSION, '--cooked', ue3.cooked, *MISSION_STEPS],
                         capture_output=True, encoding='utf-8')
    effects = [e for s in json.loads(run.stdout)['steps'] for e in s.get('effects', []) if e['kind'] == 'dialog']
    groups, rows = {}, []
    for e in effects:
        tag, group, name = e['a'], e['b'], e['c']
        if group not in groups:
            groups[group] = ue3.dump('Startup', group)
        g = groups[group]
        event = next((d for d in g['dialogevents'] if d['Tag'] == tag), None)
        talk = None
        if event and event['OutputAction']:
            talk = ue3.dump('Startup', event['OutputAction'])['talkdata']
            how = 'DialogEvents[Tag].OutputAction -> WillowDialogAct_Talk.TalkData'
        elif event:
            # events without an OutputAction pair, in order, with the group's inline TalkActs (UNVERIFIED pairing rule;
            # oracle: the talk event name matches the tag name for 7 of 7 in this group)
            empties = [d['Tag'] for d in g['dialogevents'] if not d['OutputAction']]
            talk = g['talkacts'][empties.index(tag)]['TalkData']
            how = 'k-th null-OutputAction event <-> k-th inline TalkActs entry (UNVERIFIED pairing)'
        row = {'trigger': {'event_tag': tag, 'group': group, 'name_tag': name}, 'how': how if event else 'event tag not in group',
               'talk': talk, 'ak_events': []}
        for t in talk or []:
            pkg_path = t['TalkAkEvent']
            row['ak_events'].append({'name_tag': t['NameTag'], **ue3.ak_event('Startup', pkg_path)})
        rows.append(row)
    return rows


def marcus_group_chain(ue3):
    """Marcus's own dialog group (mission-giver barks); not triggered by the Fire mission's steps."""
    g = ue3.dump('Sanctuary_Dynamic', MARCUS_GROUP)
    rows = []
    for d in g['dialogevents']:
        talk = ue3.dump('Sanctuary_Dynamic', d['OutputAction'])['talkdata'] if d['OutputAction'] else []
        rows.append({'trigger': {'event_tag': d['Tag'], 'group': MARCUS_GROUP},
                     'ak_events': [{'name_tag': t['NameTag'], **ue3.ak_event('Sanctuary_Dynamic', t['TalkAkEvent'])} for t in talk]})
    return rows


def door_chain(ue3):
    rows = []
    for track in DOOR_TRACKS:
        for key in ue3.dump('Sanctuary_Dynamic', track)['akevents']:
            rows.append({'track': track, 'time': key['Time'], 'ak_event': ue3.ak_event('Sanctuary_Dynamic', key['Event'])})
    return rows


def weapon_chain(ue3):
    wt = ue3.dump('Startup', WEAPON_TYPE)
    rows = [{'role': 'fire (WeaponTypeDefinition.FireSounds)', 'ak_event': ue3.ak_event('Startup', fs['Event'])} for fs in wt['firesounds']]
    rows += [{'role': 'equip (PickupAndEquipSounds)', 'ak_event': ue3.ak_event('Startup', fs['Event'])} for fs in wt['pickupandequipsounds']]
    names = [r['AnimationName'] for r in wt['weaponreloadanimations']]
    for exp in ue3.index('Startup').values():
        if exp['class'] != 'Engine.AnimSequence' or not exp['path'].lower().startswith(ANIM_SET.lower() + '.animsequence_'):
            continue
        seq = ue3.run('Startup', '--object-dump', exp['index'], 4, '--cooked', ue3.cooked)['properties']
        if seq.get('sequencename') not in names:
            continue
        for n in seq.get('notifies', []):
            note = ue3.dump('Startup', n['Notify'])
            if 'akevent' in note:
                rows.append({'role': 'reload anim %s @%.3fs' % (seq['sequencename'], n['Time']), 'ak_event': ue3.ak_event('Startup', note['akevent'])})
    return rows


def wwise_side(inst, ak, out_wem):
    """Resolve one AkEvent summary in the Wwise banks, copy the raw .wem files, peek at the RIFF headers."""
    res = ac.resolve(inst, '0x%x' % ak['short_id'])
    ak['wwise'] = []
    for row in res['rows']:
        entry = {k: row.get(k) for k in ('pck', 'bank', 'action_type', 'target_kind')}
        entry['bank_names'] = [n for b in inst.banks() if b['bkhd_id'] == row['bank'] for _, n in b['names']]
        entry['sounds'] = []
        for s in row.get('sounds', []):
            info = {'source': s['source'], 'stream': s['stream'], 'where': s['where'], 'bytes': s.get('size')}
            if s['where'] in ('bank', 'stream'):
                blob = ac.read_media(inst, s)
                os.makedirs(out_wem, exist_ok=True)
                path = os.path.join(out_wem, '%d.wem' % s['source'])
                with open(path, 'wb') as f:
                    f.write(blob)
                info.update(wem=path, **ac.riff_info(blob[:256]))
            entry['sounds'].append(info)
        ak['wwise'].append(entry)
    ak['event_found_in_banks'] = len({r['bank'] for r in res['rows']})
    return ak


def media_entries(ak, decoded_dir):
    """One record per distinct source id behind an AkEvent (several sources = random/switch container, selection rule UNVERIFIED)."""
    out, seen = [], set()
    for w in ak.get('wwise', []):
        for s in w['sounds']:
            if s['source'] in seen or 'wem' not in s:
                continue
            seen.add(s['source'])
            decoded = next((os.path.join(decoded_dir, '%d%s' % (s['source'], ext)) for ext in ('.wav', '.ogg', '.flac')
                            if os.path.exists(os.path.join(decoded_dir, '%d%s' % (s['source'], ext)))), None)
            out.append({k: s.get(k) for k in ('source', 'wem', 'bytes', 'codec', 'channels', 'sample_rate', 'duration_s')} | {'decoded': decoded})
    return out


def build_manifest(chain, decoded_dir):
    """local/slice/audio.json: one entry per (stock trigger, AkEvent). `state` never claims more than was checked."""
    entries = []

    def add(key, kind, trigger, ak, **extra):
        media = media_entries(ak, decoded_dir)
        lo, hi = ak.get('min_duration'), ak.get('max_duration')
        in_range = bool(media) and lo is not None and all(m['duration_s'] is not None and lo - 0.01 <= m['duration_s'] <= hi + 0.01 for m in media)
        decoded = bool(media) and all(m['decoded'] for m in media)
        state = 'unresolved' if not media else ('decoded_not_listened' if decoded else 'extracted_undecoded')
        entries.append({'key': key, 'kind': kind, 'trigger': trigger, 'ak_event': {k: ak[k] for k in ('ue3_path', 'wwise_name', 'short_id')},
                        'banks': sorted({n for w in ak.get('wwise', []) for n in w['bank_names']}), 'media': media,
                        'duration_range_from_akevent': [lo, hi], 'checks': {'fnv1_matches_short_id': ak['fnv1_matches_short_id'],
                        'wem_header_duration_within_akevent_range': in_range}, 'state': state, **extra})

    for row in chain['dialog']:
        for ak in row['ak_events']:
            add('dialog:' + row['trigger']['event_tag'], 'dialog', row['trigger'], ak, how_resolved=row['how'], talker_name_tag=ak['name_tag'])
    for row in chain['marcus_group']:
        for ak in row['ak_events']:
            add('marcus_group:' + row['trigger']['event_tag'], 'dialog_group_bark', row['trigger'], ak, talker_name_tag=ak['name_tag'])
    for row in chain['door']:
        add('door:' + ('open' if 'Open' in row['ak_event']['wwise_name'] else 'close'), 'interp_track_key', {'track': row['track'], 'time': row['time']}, row['ak_event'])
    for row in chain['weapon']:
        role = row['role'].split(' (')[0].replace(' ', '_').replace('@', 'at_')
        add('weapon:' + role, 'weapon', {'role': row['role']}, row['ak_event'])
    return {'schema': 'ow-audio-v1', 'note': 'game-derived, ignored; state values: unresolved | extracted_undecoded | decoded_not_listened',
            'entries': entries}


def main():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--game', default=os.environ.get('OPENWILLOW_BL2'))
    ap.add_argument('--decoded-dir', help='folder holding <source id>.wav/.ogg once a decoder has been approved and run')
    ap.add_argument('--reader', default=os.path.join(root, 'build', 'Release', 'ow-package.exe'))
    args = ap.parse_args()
    if not args.game:
        sys.exit('set OPENWILLOW_BL2 or pass --game')
    ue3 = Ue3(args.reader, os.path.join(args.game, 'WillowGame', 'CookedPCConsole'))
    inst = ac.Install(args.game)
    chain = {'dialog': dialog_chain(ue3), 'marcus_group': marcus_group_chain(ue3), 'door': door_chain(ue3), 'weapon': weapon_chain(ue3)}
    wem = os.path.join(root, 'local', 'audio', 'wem')
    seen = {}
    for section in chain.values():
        for row in section:
            for ak in ([row['ak_event']] if 'ak_event' in row else row.get('ak_events', [])):
                if ak['short_id'] not in seen:
                    seen[ak['short_id']] = wwise_side(inst, ak, wem)
                else:
                    ak.update({k: v for k, v in seen[ak['short_id']].items() if k in ('wwise', 'event_found_in_banks')})
    os.makedirs(os.path.join(root, 'local', 'audio'), exist_ok=True)
    with open(os.path.join(root, 'local', 'audio', 'slice_chain.json'), 'w') as f:
        json.dump(chain, f, indent=1)
    manifest = build_manifest(chain, args.decoded_dir or os.path.join(root, 'local', 'audio', 'decoded'))
    os.makedirs(os.path.join(root, 'local', 'slice'), exist_ok=True)
    with open(os.path.join(root, 'local', 'slice', 'audio.json'), 'w') as f:
        json.dump(manifest, f, indent=1)
    print('manifest entries', len(manifest['entries']), 'states', sorted({e['state'] for e in manifest['entries']}))
    print(json.dumps({k: len(v) for k, v in chain.items()}), 'distinct AkEvents', len(seen))


if __name__ == '__main__':
    main()
