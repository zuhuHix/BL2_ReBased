#!/usr/bin/env python3
"""Census of the Sanctuary ambient NPCs (the civilians who wander the town), read with our own reader.

AI-assisted (Claude), 2026-10-04. Clean room: no game data lives in this file; the output is game-derived and
goes to the ignored ``local/slice/ambient_npcs.json``. Every number is read from the installed packages with
``ow-package`` (``--properties`` with the established prefixes 4/8/26 for placed actors, ``--object-dump`` for archetypes
and definitions). Nothing here says what the native population/AI code does with the data; that is `UNVERIFIED`
and written up in ``docs/verification/SANCTUARY_AMBIENT_NPCS.md``.

What it collects, for the levels streamed into ``Sanctuary_P`` that hold population data:

* ``dens``       every ``PopulationOpportunityDen`` (population definition, enabled flag, limits, spawn points)
* ``points``     every ``WillowPopulationPoint`` a den uses: pose and ``InitialActionDestinations``
* ``nodes``      every ``WillowAIMoveNode`` / ``Perch`` actor: pose, arrival radius, weighted ``NextNodes``, ``PerchDef``
* ``pop_defs``   population definition -> factories -> pawn balance -> pawn archetype, flags set on spawn
* ``pawns``      pawn archetype -> AI class (speed, name, AI definition) and mesh component (mesh, anim sets, tree, materials)
* ``perch_defs`` perch definition -> start/idle/stop special moves -> animation set and clip names
* ``kismet``     ``SeqEvent_PopulatedPoint`` events with the operations they reach, and ``WillowSeqAct_AIScripted`` walks
* ``routes``     for each spawn point with a destination, the node chain reached by following ``NextNodes``
* ``named``      the individually placed ``WillowAIPawn`` actors of the town (archetype and pose)

  python tools/census_ambient_npcs.py [--game DIR] [--reader build/Release/ow-package.exe] [--levels A B ...]
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import prepare_slice_world as W  # noqa: E402
from prepare_level import transform  # noqa: E402

TOOL = 'tools/census_ambient_npcs.py'
OUT = ROOT / 'local/slice/ambient_npcs.json'
LEVELS = ['Sanctuary_Combat', 'Sanctuary_Dynamic', 'Sanctuary_Side']
SCHEMA = W.SCHEMA + 'InitialActionDestinations=ObjectProperty\n'
ROUTE_LIMIT = 24
# Definitions are copies in several packages; read them from the smallest first (object dumps in Dynamic are slow).
HOME_ORDER = ['Sanctuary_Combat', 'Sanctuary_Side', 'Sanctuary_Dynamic']


def now():
    return datetime.datetime.now().astimezone().isoformat(timespec='seconds')


class Census:
    def __init__(self, exe, game):
        self.tmp = tempfile.TemporaryDirectory()
        schema = Path(self.tmp.name) / 'ambient.schema'
        schema.write_text(SCHEMA)
        self.reader = W.Reader(exe, game, schema)
        self.exe, self.cooked = self.reader.exe, self.reader.cooked
        self._dumps = {}

    # ------------------------------------------------------------------ reader access
    def dump(self, package, path):
        """Class defaults + tagged overrides of one export (--object-dump, prefix 4 then 8); None if undecodable."""
        key = (package, path)
        if key not in self._dumps:
            result = None
            try:
                index = self.reader.export(package, path)['index']
            except ValueError:
                self._dumps[key] = None
                return None
            for prefix in (4, 8):
                try:
                    run = subprocess.run([str(self.exe), str(self.cooked / f'{package}.upk'), '--object-dump', str(index), str(prefix),
                                          '--cooked', str(self.cooked)], capture_output=True, text=True, encoding='utf-8', timeout=120)
                except subprocess.TimeoutExpired:
                    continue
                if run.returncode == 0:
                    result = json.loads(run.stdout)
                    result.pop('log', None)
                    break
            self._dumps[key] = result
        return self._dumps[key]

    def props(self, package, path):
        try:
            return self.reader.props(package, path)
        except ValueError as error:
            return {'_error': str(error)[:160]}

    def tail(self, path):
        return path.rsplit('.', 1)[-1] if path else None


def host_pose(p):
    pose = transform(p)
    return {'location': [round(v, 3) for v in pose['location']], 'rotation': [round(v, 3) for v in pose['rotation']]}


def short(path, prefix='TheWorld.PersistentLevel.'):
    return path[len(prefix):] if path and path.startswith(prefix) else path


# ---------------------------------------------------------------------------------------------------- per level
def level_data(c, package):
    reader = c.reader
    exports = reader.exports(package)['by_index']
    result = {'package': package, 'counts': {}, 'dens': [], 'points': {}, 'nodes': {}, 'encounters': [], 'kismet': {}, 'named': []}
    by_class = {}
    for x in exports.values():
        if x['path'].startswith('TheWorld.PersistentLevel.') and x['path'].count('.') == 2:
            by_class.setdefault(x['class'], []).append(x)
    for cls in ('WillowGame.PopulationOpportunityDen', 'WillowGame.WillowPopulationPoint', 'WillowGame.WillowAIMoveNode',
                'WillowGame.Perch', 'WillowGame.WillowPopulationEncounter', 'WillowGame.WillowAIPawn'):
        result['counts'][cls] = len(by_class.get(cls, []))

    def point(path):
        name = short(path)
        if name in result['points']:
            return name
        p = c.props(package, path)
        dest = [short(d.get('path')) if isinstance(d, dict) else d for d in (p.get('InitialActionDestinations') or [])]
        result['points'][name] = {'pose': host_pose(p) if 'Location' in p else None, 'destinations': dest,
                                  **({'error': p['_error']} if '_error' in p else {})}
        return name

    for x in sorted(by_class.get('WillowGame.PopulationOpportunityDen', []), key=lambda e: e['path']):
        p = c.props(package, x['path'])
        pop = W.ref(p.get('PopulationDef'))
        spawn = [point(sp['path']) if sp.get('path') else None for sp in (p.get('SpawnPoints') or [])]
        result['dens'].append({
            'den': short(x['path']), 'population_def': pop, 'is_enabled': p.get('IsEnabled', 'class default'),
            'max_active_normal': p.get('MaxActiveActorsIsNormal'), 'max_active_threatened': p.get('MaxActiveActorsThreatened'),
            'respawn_delay': p.get('RespawnDelay'), 'spawn_radius': p.get('SpawnRadius'), 'patrol_radius': p.get('PatrolRadius'),
            'description': p.get('Description'), 'parent_encounter': short(W.ref(p.get('ParentEncounter'))),
            'game_stage_region': W.ref(p.get('GameStageRegion')), 'critical': p.get('bIsCriticalActor'),
            'pose': host_pose(p) if 'Location' in p else None, 'spawn_points': spawn})
    for x in by_class.get('WillowGame.WillowPopulationEncounter', []):
        p = c.props(package, x['path'])
        result['encounters'].append({'encounter': short(x['path']), 'is_enabled': p.get('IsEnabled', 'class default'),
                                     'pose': host_pose(p) if 'Location' in p else None})
    for cls in ('WillowGame.WillowAIMoveNode', 'WillowGame.Perch'):
        for x in sorted(by_class.get(cls, []), key=lambda e: e['path']):
            p = c.props(package, x['path'])
            nxt = []
            for entry in p.get('NextNodes') or []:
                if isinstance(entry, dict) and W.ref(entry.get('Node')):
                    nxt.append({'node': short(W.ref(entry['Node'])), 'weight': entry.get('Weight', 1)})
            result['nodes'][short(x['path'])] = {
                'class': cls.split('.')[-1], 'pose': host_pose(p) if 'Location' in p else None,
                'arrival_radius': p.get('PawnArrivalRadius'), 'face_node_direction': p.get('bFaceNodeDirection'),
                'fuzzy_arrival': p.get('bFuzzyArrival'), 'next': nxt,
                'previous': [short(W.ref(v)) for v in (p.get('PreviousNodes') or []) if W.ref(v)],
                'perch_def': W.ref(p.get('PerchDef')),
                'loop_time_override': ([p['LoopTimeOverride'].get('MinVal'), p['LoopTimeOverride'].get('MaxVal')]
                                       if p.get('bOverrideLoopTime') and isinstance(p.get('LoopTimeOverride'), dict) else None),
                **({'error': p['_error']} if '_error' in p else {})}
    for x in sorted(by_class.get('WillowGame.WillowAIPawn', []), key=lambda e: e['path']):
        p = c.props(package, x['path'])
        arch = reader.archetype(package, x['path'])
        result['named'].append({'actor': short(x['path']), 'archetype': arch, 'pose': host_pose(p) if 'Location' in p else None,
                                'size': x['size']})
    return result


# ---------------------------------------------------------------------------------------------------- Kismet
def kismet_data(c, package):
    reader = c.reader
    out = {'populated_point_events': [], 'ai_scripted': [], 'leaving_move_node_events': []}
    seen_ops = {}

    def op_brief(path):
        x = reader.export(package, path)
        p = c.props(package, path)
        brief = {k: v for k, v in p.items() if k in ('Stance', 'ToggleName', 'EventName', 'bRunning')
                 and not isinstance(v, (dict, list))}
        return {'op': short(path, 'TheWorld.PersistentLevel.Main_Sequence.'), 'class': x['class'], **brief}

    def follow(path, depth):
        if depth > 3 or path in seen_ops and depth > 0:
            return []
        seen_ops[path] = True
        steps = []
        try:
            outputs = W.describe_outputs(reader, package, path)
        except (ValueError, KeyError):
            return steps
        for desc, targets in outputs.items():
            for t in targets:
                steps.append({'via': desc, **op_brief(t['op']), 'then': follow(t['op'], depth + 1)})
        return steps

    for x in sorted(reader.of_class(package, 'GearboxFramework.SeqEvent_PopulatedPoint'), key=lambda e: e['path']):
        p = c.props(package, x['path'])
        seen_ops.clear()
        out['populated_point_events'].append({
            'event': short(x['path'], 'TheWorld.PersistentLevel.Main_Sequence.'), 'point': short(W.ref(p.get('Originator'))),
            'reaches': follow(x['path'], 0)})
    for x in sorted(reader.of_class(package, 'WillowGame.WillowSeqAct_AIScripted'), key=lambda e: e['path']):
        p = c.props(package, x['path'], )
        links = W.links(p)
        targets, dests = [], []
        for kind, bucket in (('Target', targets), ('Destination', dests)):
            for var in links.get(kind, []):
                try:
                    value, how = W.object_var(reader, package, var)
                    bucket.append(short(value))
                except ValueError as error:
                    bucket.append(f'unresolved: {error}')
        try:
            finished = W.describe_outputs(reader, package, x['path'])
        except (ValueError, KeyError):
            finished = {}
        out['ai_scripted'].append({'action': short(x['path'], 'TheWorld.PersistentLevel.Main_Sequence.'), 'targets': targets,
                                   'destinations': dests, 'stance': p.get('Stance'),
                                   'finished_to': [t['op'].rsplit('.', 1)[-1] for t in finished.get('Finished', [])]})
    for x in reader.of_class(package, 'GearboxFramework.SeqEvent_LeavingMoveNode'):
        p = c.props(package, x['path'])
        out['leaving_move_node_events'].append({'event': short(x['path'], 'TheWorld.PersistentLevel.Main_Sequence.'),
                                                'node': short(W.ref(p.get('Originator')))})
    return out


# ---------------------------------------------------------------------------------------------------- definitions
def structure(c, package, pop_path):
    """Population definition -> factories -> balance -> pawn archetype."""
    d = c.dump(package, pop_path)
    if not d:
        return {'error': 'not decodable here'}
    entries = []
    for entry in d['properties'].get('actorarchetypelist', []):
        factory = entry.get('SpawnFactory')
        f = c.dump(package, factory) if factory else None
        row = {'factory': factory, 'probability': entry.get('Probability')}
        if f:
            fp = f['properties']
            row['factory_class'] = f['class']
            row['pawn_balance'] = fp.get('pawnbalancedefinition')
            row['flags_to_set'] = [x.get('FlagToSet') for x in fp.get('flagstoset', []) if isinstance(x, dict)]
            row['on_actor_spawn'] = [b.rsplit('.', 1)[-1] for b in fp.get('onactorspawn', [])]
            bal = c.dump(package, fp['pawnbalancedefinition']) if fp.get('pawnbalancedefinition') else None
            if bal:
                bp = bal['properties']
                row['pawn_archetype'] = bp.get('aipawnarchetype')
                row['display_names'] = [{'playthrough': pt.get('PlayThrough'), 'display_name': pt.get('DisplayName'),
                                         'transformed': [t.get('TransformedName') for t in pt.get('TransformedNames', [])]}
                                        for pt in bp.get('playthroughs', [])]
        entries.append(row)
    return {'factories': entries}


def pawn_data(c, package, archetype):
    """Pawn archetype -> AI class and mesh component, read from their own tagged properties (fast; --object-dump of
    the big Sanctuary_Dynamic package takes about a minute per object)."""
    pawn = c.props(package, archetype)
    if '_error' in pawn:
        return {'error': pawn['_error']}
    row = {'ai_class': W.ref(pawn.get('AIClass')), 'body_class': W.ref(pawn.get('BodyClass')), 'allegiance': W.ref(pawn.get('Allegiance')),
           'mesh_component': W.ref(pawn.get('Mesh'))}
    if row['ai_class']:
        ai = c.props(package, row['ai_class'])
        row['ai'] = {'display_name': ai.get('DefaultDisplayName'), 'ground_speed': ai.get('GroundSpeed'), 'walking_pct': ai.get('WalkingPct'),
                     'ai_definition': W.ref(ai.get('AIDef')), 'rotation_rate': ai.get('RotationRate'),
                     'slow_down_dist': ai.get('SlowDownDist'), 'slow_down_min_pct': ai.get('SlowDownMinPct')}
    if row['mesh_component']:
        comp = c.props(package, row['mesh_component'])
        row['mesh'] = {'skeletal_mesh': W.ref(comp.get('SkeletalMesh')), 'anim_sets': [W.ref(a) for a in comp.get('AnimSets') or []],
                       'anim_tree': W.ref(comp.get('AnimTreeTemplate')), 'materials': [W.ref(m) for m in comp.get('Materials') or []],
                       'translation': comp.get('Translation'), 'physics_asset': W.ref(comp.get('PhysicsAsset'))}
    return row


def perch_data(c, package, perch_def):
    d = c.dump(package, perch_def)
    if not d:
        return {'error': 'not decodable here'}
    p = d['properties']
    row = {'loop_time': p.get('looptime'), 'lerp_time': p.get('lerptime'), 'use_collision': p.get('busecollision'), 'anim_map': []}
    for entry in p.get('animmap', []):
        item = {'body_tag': entry.get('Key')}
        for role in ('StartAnim', 'IdleAnim', 'StopAnim'):
            item[role] = special_move(c, package, entry.get(role))
        row['anim_map'].append(item)
    return row


def special_move(c, package, path):
    """SpecialMove_Perch / _PerchLoop / _PerchRandomLoop -> clip name(s)."""
    if not path:
        return None
    d = c.dump(package, path)
    if not d:
        return {'path': path, 'error': 'not decodable here'}
    p = d['properties']
    row = {'class': d['class'].split('.')[-1], 'anim_set': p.get('animset'), 'clip': p.get('animname')}
    if p.get('randomlist'):
        row['random'] = []
        for item in p['randomlist']:
            sub = special_move(c, package, item.get('SMD'))
            row['random'].append({'weight': item.get('Weight'), **(sub or {})})
    return row


# ---------------------------------------------------------------------------------------------------- routes
def routes(level):
    """Follow NextNodes (all branches, depth first, no repeats) from every spawn point's first destination."""
    nodes = level['nodes']
    out = []
    for den in level['dens']:
        for name in den['spawn_points']:
            point = level['points'].get(name) if name else None
            if not point or not point['destinations']:
                continue
            for start in point['destinations']:
                if not start or start not in nodes:
                    out.append({'den': den['den'], 'point': name, 'start': start, 'error': 'destination is not a node of this level'})
                    continue
                seen, order, queue = {start}, [start], [start]
                edges = 0
                while queue and len(order) < ROUTE_LIMIT * 4:
                    node = queue.pop(0)
                    for n in nodes[node]['next']:
                        edges += 1
                        if n['node'] in nodes and n['node'] not in seen:
                            seen.add(n['node'])
                            order.append(n['node'])
                            queue.append(n['node'])
                out.append({'den': den['den'], 'point': name, 'start': start, 'reachable_nodes': len(order), 'edges': edges,
                            'reachable': order[:ROUTE_LIMIT], 'truncated': len(order) > ROUTE_LIMIT,
                            'population_def': den['population_def']})
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--game', default=os.environ.get('OPENWILLOW_BL2'))
    ap.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    ap.add_argument('--levels', nargs='*', default=LEVELS)
    ap.add_argument('--output', default=str(OUT))
    ap.add_argument('--definitions-from', nargs='*', help='levels whose definitions are read (default all)')
    ap.add_argument('--from-levels', action='store_true', help='reuse the saved level stage (definitions are re-read)')
    args = ap.parse_args()
    if not args.game:
        sys.exit('Pass --game or set OPENWILLOW_BL2')
    out = Path(args.output).resolve()
    if not out.is_relative_to(ROOT / 'local'):
        sys.exit('Output must stay under the repository local/')
    c = Census(args.reader, args.game)
    result = {'schema': 'ow-ambient-census-v1', 'tool': TOOL, 'generated': now(), 'levels': {}, 'pop_defs': {}, 'pawns': {},
              'perch_defs': {}, 'routes': {}}
    stage = out.with_name(out.stem + '_levels.json')
    if args.from_levels and stage.is_file():
        saved = json.loads(stage.read_text(encoding='utf-8'))
        result['levels'], result['routes'] = saved['levels'], saved['routes']
    else:
        for package in args.levels:
            print('level', package, flush=True)
            level = level_data(c, package)
            level['kismet'] = kismet_data(c, package)
            result['levels'][package] = level
            result['routes'][package] = routes(level)
        stage.parent.mkdir(parents=True, exist_ok=True)
        stage.write_text(json.dumps({'levels': result['levels'], 'routes': result['routes']}, indent=1), encoding='utf-8')
    # Definitions referenced by the dens/nodes. Each lives in some package of the town; use the level that has it.
    def resolve(path, package_order):
        for package in package_order:
            if path and package in result['levels'] and c.reader.exports(package)['by_path'].get(path):
                return package
        return None
    # Object dumps in the big Sanctuary_Dynamic package take about a minute each; --definitions-from limits which levels'
    # dens/nodes/placed pawns have their definitions read.
    wanted = set(args.definitions_from or result['levels'])
    for package, level in result['levels'].items():
        if package not in wanted:
            continue
        for den in level['dens']:
            pop = den['population_def']
            if pop and pop not in result['pop_defs']:
                home = resolve(pop, HOME_ORDER)
                result['pop_defs'][pop] = structure(c, home, pop) if home else {'error': 'not in the town packages'}
                result['pop_defs'][pop]['package'] = home
        for node in level['nodes'].values():
            pd = node['perch_def']
            if pd and pd not in result['perch_defs']:
                home = resolve(pd, HOME_ORDER)
                result['perch_defs'][pd] = perch_data(c, home, pd) if home else {'error': 'not in the town packages'}
                result['perch_defs'][pd]['package'] = home
    archetypes = {}
    for pop in result['pop_defs'].values():
        for f in pop.get('factories', []):
            if f.get('pawn_archetype'):
                archetypes[f['pawn_archetype']] = pop['package']
    for level in (l for l in result['levels'].values() if l['package'] in wanted):
        for named in level['named']:
            if named['archetype']:
                archetypes.setdefault(named['archetype'], level['package'])
    for arch, home in sorted(archetypes.items()):
        result['pawns'][arch] = {**pawn_data(c, home, arch), 'package': home}
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(result, indent=1), encoding='utf-8')
    print('wrote', out)
    for package, level in result['levels'].items():
        print(package, level['counts'], 'routes', len(result['routes'][package]))


if __name__ == '__main__':
    main()
