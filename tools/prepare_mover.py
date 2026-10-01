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
    p.add_argument('--group', required=True)
    p.add_argument('--output', type=Path, default=Path('local/doors/mover.json'))
    args = p.parse_args()
    result = prepare(args.reader, args.game, args.scene, args.package, args.action, args.group, args.output)
    print(f"Prepared {result['actor']}: {result['duration']} s, {len(result['position'])} position / {len(result['rotation'])} rotation keys; {len(result['omitted_tracks'])} omitted tracks")
