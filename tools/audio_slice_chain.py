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
# Seen in one real-game session on a modded install (feature/slice-audio-capture, docs/verification/SLICE_AUDIO_LIVE.md), not read from a data reference.
# UNVERIFIED. The hit event is a switch container with many sources, one per surface presumably, and which plays for an enemy is unknown.
LIVE_ROLES = (
    {'key': 'role:phaselock_cast', 'event': 'Ake_FX_Player_Siren.Ak_Play_FX_Player_Phaselock_Activate',
     'seen': 'posted in the same millisecond as LiftActionSkill.PhaseLockTarget, one cast'},
    {'key': 'role:hit', 'event': 'Ake_Imp_Bullets.Ak_Play_Imp_Bullets',
     'seen': 'PlayImpactEffectSound 33 to 34 ms after 2 of 3 shots at a target; a surface sound, not shown to be the enemy'},
)


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


def mission_run_dialog(ue3):
    """Dialog effects of one scripted run of the mission (MISSION_STEPS) through ow-package --mission-run."""
    run = subprocess.run([ue3.reader, os.path.join(ue3.cooked, 'Startup.upk'), '--mission-run', MISSION, '--cooked', ue3.cooked, *MISSION_STEPS],
                         capture_output=True, encoding='utf-8')
    if run.returncode:
        raise RuntimeError('--mission-run: ' + run.stderr.strip())
    return [e for s in json.loads(run.stdout)['steps'] for e in s.get('effects', []) if e['kind'] == 'dialog']


def group_talk(ue3, group_dump, tag):
    """(talk data, how) for one DialogEvents tag of a dumped dialog group; (None, reason) when the tag is not there."""
    event = next((d for d in group_dump['dialogevents'] if d['Tag'] == tag), None)
    if not event:
        return None, 'event tag not in group'
    if event['OutputAction']:
        return ue3.dump('Startup', event['OutputAction'])['talkdata'], 'DialogEvents[Tag].OutputAction -> WillowDialogAct_Talk.TalkData'
    # events without an OutputAction pair, in order, with the group's inline TalkActs (UNVERIFIED pairing rule;
    # oracle: the talk event name matches the tag name for 7 of 7 in this group)
    empties = [d['Tag'] for d in group_dump['dialogevents'] if not d['OutputAction']]
    k = empties.index(tag)
    if k >= len(group_dump.get('talkacts') or []):
        return None, 'null OutputAction and no k-th inline TalkActs entry'
    return group_dump['talkacts'][k]['TalkData'], 'k-th null-OutputAction event <-> k-th inline TalkActs entry (UNVERIFIED pairing)'


def dialog_chain(ue3, run_effects=None):
    """Every line the mission's dialog groups can play -> Talk act(s) -> AkEvent (decoded by ow-package --object-dump).

    The groups are the mission's MissionDialogGroup plus any group a scripted run's dialog effects name. Every
    DialogEvents entry of those groups is resolved, not only the ones one fixed run reaches: which lines the slice
    host plays depends on timing and choices (seen: 03a, 04, 05 on some runs only), so a step list cannot be complete.
    `reached_in_scripted_run` marks the tags the fixed MISSION_STEPS run reached.
    """
    effects = mission_run_dialog(ue3) if run_effects is None else run_effects
    reached = {(e['b'], e['a']) for e in effects}
    groups = []
    own = ue3.dump('Startup', MISSION).get('missiondialoggroup')
    for group in [own] + [e['b'] for e in effects]:
        if group and group not in groups:
            groups.append(group)
    rows = []
    for group in groups:
        g = ue3.dump('Startup', group)
        tags = [d['Tag'] for d in g['dialogevents']]
        # A scripted-run tag missing from its group is kept, so the gap shows instead of vanishing.
        tags += [e['a'] for e in effects if e['b'] == group and e['a'] not in tags]
        for tag in dict.fromkeys(tags):
            name = next((e['c'] for e in effects if e['a'] == tag and e['b'] == group), None)
            talk, how = group_talk(ue3, g, tag)
            row = {'trigger': {'event_tag': tag, 'group': group, 'name_tag': name}, 'how': how, 'talk': talk,
                   'reached_in_scripted_run': (group, tag) in reached, 'ak_events': []}
            for t in talk or []:
                row['ak_events'].append({'name_tag': t['NameTag'], **ue3.ak_event('Startup', t['TalkAkEvent'])})
            rows.append(row)
    # Manifest keys and the host's lookup are `dialog:<event tag>` with no group, so one tag in two groups would
    # collide (the host keeps the last entry). Refuse until the key and the lookup carry the group.
    owners = {}
    for row in rows:
        owners.setdefault(row['trigger']['event_tag'], []).append(row['trigger']['group'])
    shared = {tag: groups for tag, groups in owners.items() if len(groups) > 1}
    if shared:
        raise ValueError('dialog event tag(s) in more than one included group; manifest keys need the group first: %s'
                         % '; '.join('%s in %s' % (tag, ', '.join(groups)) for tag, groups in sorted(shared.items())))
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


def live_role_ak(role):
    """An AkEvent summary from a name seen live. The object was not read from a package, so ShortId and the durations are unknown and the FNV-1 check
    cannot run (None, not True). The Wwise name is the object's name without its Ak_ prefix: a rule that holds for every event this tool reads from a package."""
    name = role['event'].split('.')[-1]
    if not name.startswith('Ak_'):
        raise ValueError('not an Ak_ event name: ' + role['event'])
    wwise = name[3:]
    return {'ue3_path': role['event'], 'package': None, 'wwise_name': wwise, 'short_id': ac.fnv1_32(wwise), 'fnv1_matches_short_id': None,
            'min_duration': None, 'max_duration': None, 'required_bank': None}


def live_role_chain():
    return [{'key': role['key'], 'seen': role['seen'], 'ak_event': live_role_ak(role)} for role in LIVE_ROLES]


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
            add('dialog:' + row['trigger']['event_tag'], 'dialog', row['trigger'], ak, how_resolved=row['how'], talker_name_tag=ak['name_tag'],
                reached_in_scripted_run=row.get('reached_in_scripted_run'))
    for row in chain['marcus_group']:
        for ak in row['ak_events']:
            add('marcus_group:' + row['trigger']['event_tag'], 'dialog_group_bark', row['trigger'], ak, talker_name_tag=ak['name_tag'])
    for row in chain['door']:
        add('door:' + ('open' if 'Open' in row['ak_event']['wwise_name'] else 'close'), 'interp_track_key', {'track': row['track'], 'time': row['time']}, row['ak_event'])
    for row in chain['weapon']:
        role = row['role'].split(' (')[0].replace(' ', '_').replace('@', 'at_')
        add('weapon:' + role, 'weapon', {'role': row['role']}, row['ak_event'])
    for row in chain.get('live_roles', []):
        add(row['key'], 'live_role', {'seen': row['seen']}, row['ak_event'], evidence='UNVERIFIED: one session on a modded install; not from a data reference')
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
    # Windows CreateProcess does not find a relative path written with forward slashes (build/Release/...).
    ue3 = Ue3(os.path.abspath(args.reader), os.path.join(args.game, 'WillowGame', 'CookedPCConsole'))
    inst = ac.Install(args.game)
    chain = {'dialog': dialog_chain(ue3), 'marcus_group': marcus_group_chain(ue3), 'door': door_chain(ue3), 'weapon': weapon_chain(ue3), 'live_roles': live_role_chain()}
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
