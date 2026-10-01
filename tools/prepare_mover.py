"""Prepare one installed Matinee mover binding; game output stays under local/.

Existing imported mesh/material/collision payloads are reused. This is not a
Kismet dispatcher: a developer interaction drives the selected action only.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess
import tempfile
from prepare_level import Scene

SCHEMA = '''InterpGroups=ObjectProperty
InterpTracks=ObjectProperty
VariableLinks=StructProperty:SeqVarLink
LinkedVariables=ObjectProperty
Points=StructProperty:InterpCurvePointVector
'''


def fields(tags):
    if not isinstance(tags, list):
        raise ValueError('Expected tagged properties')
    result = {}
    for tag in tags:
        if tag['name'] in result:
            raise ValueError('Duplicate property ' + tag['name'])
        if tag.get('status') == 'decoded':
            result[tag['name']] = tag['value']
    return result


def curve(tags, length):
    points = fields(tags)['Points']
    if not 2 <= len(points) <= 128:
        raise ValueError('Unsupported curve key count')
    result = []
    for tags in points:
        p = fields(tags)
        if set(p) != {'InVal', 'OutVal', 'ArriveTangent', 'LeaveTangent', 'InterpMode'}:
            raise ValueError('Unsupported movement curve point')
        if p['InterpMode'] not in ('CIM_Linear', 'CIM_Constant', 'CIM_CurveAutoClamped', 'CIM_CurveUser', 'CIM_CurveBreak', 'CIM_CurveAuto'):
            raise ValueError('Unsupported curve interpolation')
        q = {'time': p['InVal'], 'mode': p['InterpMode']}
        for name, source in [('value', 'OutVal'), ('arrive', 'ArriveTangent'), ('leave', 'LeaveTangent')]:
            if set(p[source]) != {'X', 'Y', 'Z'}:
                raise ValueError('Expected Vector curve field')
            q[name] = [p[source][axis] for axis in 'XYZ']
        numbers = [q['time'], *q['value'], *q['arrive'], *q['leave']]
        if any(isinstance(x, bool) or not isinstance(x, (int, float)) or not math.isfinite(x) or abs(x) > 1e8 for x in numbers):
            raise ValueError('Invalid curve numeric value')
        if q['time'] < 0 or q['time'] > length or (result and q['time'] <= result[-1]['time']):
            raise ValueError('Curve times must strictly increase within action duration')
        result.append(q)
    if result[0]['time'] != 0 or abs(result[-1]['time'] - length) > 1e-5:
        raise ValueError('Curve does not cover full action')
    return result


def event_keys(tags, length):
    """InterpTrackEvent.EventTrack -> [{'time', 'name'}], times inside the action."""
    result = []
    for key in tags:
        k = fields(key)
        if set(k) != {'Time', 'EventName'}:
            raise ValueError('Unsupported event key')
        time = k['Time']
        if isinstance(time, bool) or not isinstance(time, (int, float)) or not 0 <= time <= length + 1e-5:
            raise ValueError('Event key outside action duration')
        result.append({'time': time, 'name': k['EventName']})
    return result


def track_direction(track):
    """Which play directions fire a track's keys (InterpTrack defaults: both)."""
    direction = track.get('TrackPlayDirection', 'ETPD_Both')
    if direction not in ('ETPD_Both', 'ETPD_PlayOnlyForward', 'ETPD_PlayOnlyReverse'):
        raise ValueError('Unsupported track play direction')
    forward = direction != 'ETPD_PlayOnlyReverse' and track.get('bFireEventsWhenForwards', True)
    backward = direction != 'ETPD_PlayOnlyForward' and track.get('bFireEventsWhenBackwards', True)
    return {'play_direction': direction, 'fires_forward': bool(forward), 'fires_backward': bool(backward)}


# The binding manifest also needs the action's links and the event/audio track keys.
# Kept separate from SCHEMA so the door manifest above decodes exactly as before.
BINDING_SCHEMA = SCHEMA + '''InputLinks=StructProperty:SeqOpInputLink
OutputLinks=StructProperty:SeqOpOutputLink
Links=StructProperty:SeqOpOutputInputLink
EventTrack=StructProperty:EventTrackKey
AkEvents=StructProperty:AkEventTrackKey
Attached=ObjectProperty
'''


def prepare_binding(reader, game, package, action_path):
    """Data-only binding of one installed Matinee (every group, curves, event keys, links).

    For movers whose bound actor has no prepared mesh of its own (for example a
    hidden carrier InterpActor that other actors are attached to), so no scene
    component is required. Returns the manifest dict; game-derived, so callers
    write it under local/ only. Frame mapping is the door's first-key-delta
    convention and stays UNVERIFIED against the original game.
    """
    pkg = game / 'WillowGame/CookedPCConsole' / (package + '.upk')

    def call(*args):
        run = subprocess.run([str(reader.resolve()), str(pkg), *map(str, args)], capture_output=True, text=True, encoding='utf-8')
        if run.returncode:
            raise ValueError(run.stderr.strip())
        return json.loads(run.stdout)
    exports = {x['path']: x for x in call('--exports')}
    indices = {x['index']: x for x in exports.values()}
    with tempfile.TemporaryDirectory() as tmp:
        schema = Path(tmp) / 'binding.schema'
        schema.write_text(BINDING_SCHEMA)

        def properties(ref, expected_class, prefix=4):
            index = ref['index'] if isinstance(ref, dict) else ref
            if index not in indices or indices[index]['class'] != expected_class:
                raise ValueError('Wrong local binding class: ' + str(index))
            d = call('--properties', index, '--property-offset', prefix, '--array-schema', schema)
            # SeqAct_Interp carries a 4-byte native tail (also accepted by prepare()).
            if d.get('trailing_bytes', 0) and expected_class != 'Engine.SeqAct_Interp':
                raise ValueError('Unexpected native trailing bytes: ' + expected_class)
            return fields(d['properties'])

        def path(ref):
            return indices[ref['index']]['path'] if ref['index'] > 0 else ref['path']
        action = properties(exports[action_path]['index'], 'Engine.SeqAct_Interp')
        variables = {}
        for tags in action['VariableLinks']:
            link = fields(tags)
            if link['LinkDesc'] in variables:
                raise ValueError('Duplicate action variable link')
            variables[link['LinkDesc']] = link.get('LinkedVariables', [])
        if len(variables.get('Data', [])) != 1:
            raise ValueError('Ambiguous Matinee data')
        data = properties(variables['Data'][0], 'Engine.InterpData')
        length = data['InterpLength']
        if not isinstance(length, (int, float)) or not math.isfinite(length) or not 0 < length <= 60:
            raise ValueError('Unsupported duration')
        groups = []
        for group_ref in data['InterpGroups']:
            group = properties(group_ref, 'Engine.InterpGroup')
            name = group['GroupName']
            bound = variables.get(name, [])
            if len(bound) != 1:
                raise ValueError('Group must bind exactly one variable: ' + name)
            var = properties(bound[0], 'Engine.SeqVar_Object')
            actor_ref = var['ObjValue']
            actor = properties(actor_ref, 'Engine.InterpActor', 26)
            entry = {'group': name, 'actor': path(actor_ref),
                     'actor_hidden': bool(actor.get('bHidden', False)),
                     'attached': [path(r) for r in actor.get('Attached', [])],
                     'initial': {'location': [actor.get('Location', {}).get(a, 0) for a in 'XYZ'],
                                 'rotation_units': [actor.get('Rotation', {}).get(a, 0) for a in ('Pitch', 'Yaw', 'Roll')],
                                 'draw_scale': actor.get('DrawScale', 1)},
                     'event_tracks': [], 'omitted_tracks': []}
            for track_ref in group['InterpTracks']:
                cls = indices[track_ref['index']]['class']
                track = properties(track_ref, cls)
                if cls == 'Engine.InterpTrackMove':
                    if 'position' in entry:
                        raise ValueError('Expected one movement track per group')
                    if track.get('MoveFrame') != 'IMF_RelativeToInitial':
                        raise ValueError('Only relative-to-initial movement supported')
                    if any(fields(p).get('GroupName') != 'None' for p in fields(track['LookupTrack'])['Points']):
                        raise ValueError('Dynamic movement lookups unsupported')
                    entry['position'] = curve(track['PosTrack'], length)
                    entry['rotation'] = curve(track['EulerTrack'], length)
                elif cls == 'Engine.InterpTrackEvent':
                    entry['event_tracks'].append({'track': path(track_ref), **track_direction(track),
                                                  'keys': event_keys(track.get('EventTrack', []), length)})
                else:  # audio and anything else: listed, never played here
                    keys = [{'time': fields(k).get('Time'), 'event': fields(k).get('Event', {}).get('path')}
                            for k in track.get('AkEvents', [])]
                    entry['omitted_tracks'].append({'track': path(track_ref), 'class': cls, **track_direction(track), 'keys': keys})
            if 'position' not in entry:
                raise ValueError('Group has no movement track: ' + name)
            groups.append(entry)
        inputs = action.get('InputLinks')
        if inputs is None:  # not overridden: the class default object's inputs apply
            engine = game / 'WillowGame/CookedPCConsole/Engine.upk'
            run = subprocess.run([str(reader.resolve()), str(engine), '--exports'], capture_output=True, text=True, encoding='utf-8')
            default = [x for x in json.loads(run.stdout) if x['path'] == 'Default__SeqAct_Interp']
            if len(default) != 1:
                raise ValueError('SeqAct_Interp class default not found')
            run = subprocess.run([str(reader.resolve()), str(engine), '--properties', str(default[0]['index']),
                                  '--property-offset', '4', '--array-schema', str(schema)],
                                 capture_output=True, text=True, encoding='utf-8')
            if run.returncode:
                raise ValueError(run.stderr.strip())
            inputs = fields(json.loads(run.stdout)['properties'])['InputLinks']
        outputs = {}
        for tags in action.get('OutputLinks', []):
            link = fields(tags)
            outputs[link['LinkDesc']] = [{'op': path(fields(k)['LinkedOp']), 'input': fields(k).get('InputLinkIdx', 0)}
                                         for k in link.get('Links', [])]
        return {'schema': 'ow-mover-binding-v1', 'package': package, 'action': action_path,
                'duration': length, 'rewind_on_play': bool(action.get('bRewindOnPlay', False)),
                'looping': bool(action.get('bLooping', False)),
                'inputs': [fields(t)['LinkDesc'] for t in inputs],
                'outputs': outputs, 'groups': groups,
                'relative_frame': 'first-key deltas composed with placed actor (door convention); original-game parity UNVERIFIED'}


def prepare(reader, game, scene_path, package, action_path, group_name, output):
    output = output.resolve()
    root = Path(__file__).resolve().parents[1]
    if not output.is_relative_to(root / 'local'):
        raise ValueError('Game-derived mover output must stay under repository local/')
    cooked = game / 'WillowGame/CookedPCConsole'
    pkg = cooked / (package + '.upk')
    def call(*args):
        run = subprocess.run([str(reader.resolve()), str(pkg), *map(str, args)], capture_output=True, text=True, encoding='utf-8')
        if run.returncode:
            raise ValueError(run.stderr.strip())
        return json.loads(run.stdout)
    exports = {x['path']: x for x in call('--exports')}
    indices = {x['index']: x for x in exports.values()}
    with tempfile.TemporaryDirectory() as tmp:
        schema = Path(tmp) / 'mover.schema'
        schema.write_text(SCHEMA)
        def properties(ref, expected_class, prefix=4):
            index = ref['index'] if isinstance(ref, dict) else ref
            if index not in indices or indices[index]['class'] != expected_class:
                raise ValueError('Wrong local binding class: ' + str(index))
            d = call('--properties', index, '--property-offset', prefix, '--array-schema', schema)
            if d.get('trailing_bytes', 0) and expected_class not in ('Engine.SeqAct_Interp', 'Engine.StaticMeshComponent'):
                raise ValueError('Unexpected native trailing bytes: ' + expected_class)
            return fields(d['properties'])
        action = exports[action_path]
        a = properties(action['index'], 'Engine.SeqAct_Interp')
        links = {}
        for tags in a['VariableLinks']:
            link = fields(tags)
            if link['LinkDesc'] in links:
                raise ValueError('Duplicate action variable link')
            links[link['LinkDesc']] = link['LinkedVariables']
        if len(links['Data']) != 1 or len(links[group_name]) != 1:
            raise ValueError('Ambiguous Matinee binding')
        data = properties(links['Data'][0], 'Engine.InterpData')
        length = data['InterpLength']
        if not isinstance(length, (int, float)) or not math.isfinite(length) or not 0 < length <= 60:
            raise ValueError('Unsupported duration')
        groups = [(ref, properties(ref, 'Engine.InterpGroup')) for ref in data['InterpGroups']]
        selected = [(ref, p) for ref, p in groups if p['GroupName'] == group_name]
        if len(selected) != 1:
            raise ValueError('Ambiguous group')
        _, group = selected[0]
        tracks = group['InterpTracks']
        moves = [ref for ref in tracks if indices[ref['index']]['class'] == 'Engine.InterpTrackMove']
        if len(moves) != 1:
            raise ValueError('Expected one movement track')
        move = properties(moves[0], 'Engine.InterpTrackMove')
        if move.get('MoveFrame') != 'IMF_RelativeToInitial':
            raise ValueError('Only relative-to-initial movement supported')
        lookup = fields(move['LookupTrack'])['Points']
        if any(fields(p).get('GroupName') != 'None' for p in lookup):
            raise ValueError('Dynamic movement lookups unsupported')
        var = properties(links[group_name][0], 'Engine.SeqVar_Object')
        actor_ref = var['ObjValue']
        actor = properties(actor_ref, 'Engine.InterpActor', 26)
        component_ref = actor['StaticMeshComponent']
        # Scene preparation has already verified component/mesh/material chains.
        scene = json.loads(scene_path.read_text())
        candidates = [x for x in scene['actors'] if x['level'] == package and x['source'] == component_ref['path']]
        if len(candidates) != 1:
            raise ValueError('Expected one prepared mover component')
        placed = candidates[0]
        mesh = scene['meshes'][placed['mesh']]
        component = properties(component_ref, 'Engine.StaticMeshComponent', 8)
        # Reuse the scene resolver's loaded-package scope. Shared objects occur
        # in many cooked maps; a global first-match (e.g. another map's copy)
        # is not the canonical identity of this scene's prepared payload.
        resolver = Scene(reader, game, output.parent)
        resolver.load(package)
        resolver.load(mesh['source'].split(':', 1)[0])
        installed_identity = resolver.identity(resolver.resolve(package, component['StaticMesh']))
        if mesh['source'] != installed_identity:
            raise ValueError('Prepared component mesh differs from installed binding')
        if len(mesh['sections']) != 1 or not mesh['collision'].get('hulls') or not placed['collision_enabled']:
            raise ValueError('Bounded mover needs one section and existing convex collision')
        # Shader/geometry output is reused, never seeded or overwritten here.
        result = {'schema': 'ow-mover-v1', 'package': package, 'action': action_path,
                  'actor': actor_ref['path'], 'component': component_ref['path'], 'group': group_name,
                  'duration': length, 'position': curve(move['PosTrack'], length),
                  'rotation': curve(move['EulerTrack'], length), 'mesh': mesh['source'],
                  'initial': placed['transform'],
                  'omitted_tracks': [indices[r['index']]['path'] for r in tracks if r not in moves],
                  'scene_sha256': hashlib.sha256(scene_path.read_bytes()).hexdigest(),
                  'activation': 'developer interaction; mission/Kismet activation UNVERIFIED',
                  'relative_frame': 'first-key deltas composed with placed actor; original-game parity UNVERIFIED'}
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(result, indent=2) + '\n')
        return result


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--reader', type=Path, default=Path('build/Release/ow-package.exe'))
    p.add_argument('--game', type=Path, required=True)
    p.add_argument('--scene', type=Path, default=Path('local/sanctuary/scene.json'))
    p.add_argument('--package', required=True)
    p.add_argument('--action', required=True)
    p.add_argument('--group', help='group bound to a prepared mesh (door mode)')
    p.add_argument('--binding', action='store_true',
                   help='data-only binding of every group (no prepared mesh needed); writes --output')
    p.add_argument('--output', type=Path, default=Path('local/doors/mover.json'))
    args = p.parse_args()
    if args.binding:
        output = args.output.resolve()
        if not output.is_relative_to(Path(__file__).resolve().parents[1] / 'local'):
            raise SystemExit('Game-derived mover output must stay under repository local/')
        result = prepare_binding(args.reader, args.game, args.package, args.action)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(result, indent=2) + '\n')
        print(f"Prepared binding {args.action}: {result['duration']} s, groups {[g['group'] for g in result['groups']]}")
        raise SystemExit(0)
    if not args.group:
        p.error('--group is required unless --binding is given')
    result = prepare(args.reader, args.game, args.scene, args.package, args.action, args.group, args.output)
    print(f"Prepared {result['actor']}: {result['duration']} s, {len(result['position'])} position / {len(result['rotation'])} rotation keys; {len(result['omitted_tracks'])} omitted tracks")
