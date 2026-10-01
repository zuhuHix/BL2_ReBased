"""Recover stock world placement for the Sanctuary Fire-mission slice.

AI-assisted. Clean-room: this file contains no game data. Everything it derives
from the installed packages is written to the ignored `local/slice/world.json`.
Schema and evidence: docs/verification/SLICE_WORLD_PLACEMENT.md.

Nothing is looked up by a remembered offset or position. Each item starts at
installed data the mission already names and follows references:

1a range trigger  WillowWaypoint whose WaypointInfo.LinkedObjective is the
                  mission's first objective (all levels of the map scanned).
1b Marcus walk    mission remote event -> WillowSeqAct_AIScripted -> Target
                  (SeqVar_Named -> the one SeqVar_Object of that VarName ->
                  placed pawn) and Destination (move node -> NextNodes chain);
                  SeqEvent_ArrivedAtMoveNode outputs on that chain.
1c target dummy   SeqEvent_PopulatedActor dens -> population definition ->
                  factory -> pawn balance -> archetype; the den whose
                  MissionPopulationAspect names one of the mission's objectives.
1d target mover   tools/prepare_mover.py binding of the Matinee the dummy's
                  spawn/complete behaviours drive, plus Toggle/dialog actions.
1e respawn        every TravelStation actor of the map, its teleport
                  destination and exit points; the selection rule decoded from
                  script (WillowPlayerPawn.GetBestPlayerPlacementPoint).

Coordinates: `ue3` is the serialized value (Unreal units, rotator units);
`host` is the conversion tools/prepare_level.py uses for the prepared scene
(location unchanged, rotation degrees = units * 360 / 65536 as
[Pitch, Yaw, Roll]). Property prefixes 4/8/26 are the established ones; the
first that decodes with exact consumption is used (UNVERIFIED semantics).
"""
import argparse
import datetime
import hashlib
import json
import math
from pathlib import Path
import subprocess
import tempfile

import prepare_mover
from prepare_level import transform

ROOT = Path(__file__).resolve().parents[1]
MISSION = 'GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire'
PERSISTENT = 'Sanctuary_P'
SEQ_PACKAGE = 'Sanctuary_Dynamic'
SEQUENCE = 'TheWorld.PersistentLevel.Main_Sequence.RocksPaperGenocide'
MOVE_EVENT = 'RocksPaper_MoveMarcusToRange'
PREFIXES = (4, 8, 26)
STATION_CLASSES = ('WillowGame.FastTravelStation', 'WillowGame.LevelTravelStation', 'WillowGame.ResurrectTravelStation')
SCHEMA = '''OutputLinks=StructProperty:SeqOpOutputLink
InputLinks=StructProperty:SeqOpInputLink
VariableLinks=StructProperty:SeqVarLink
Links=StructProperty:SeqOpOutputInputLink
LinkedVariables=ObjectProperty
Behaviors=ObjectProperty
ExitPoints=ObjectProperty
TouchVolumes=ObjectProperty
NextNodes=StructProperty:MoveNodeData
PreviousNodes=ObjectProperty
SpawnPoints=ObjectProperty
ActorArchetypeList=StructProperty:ActorArchetypeListEntry
ObjectiveSetRestrictions=ObjectProperty
SupportedRemoteEvents=NameProperty
ObjectiveDefs=ObjectProperty
ObjectiveDefinitions=ObjectProperty
Attached=ObjectProperty
Attachments=StructProperty:BodyInstanceDataUnion
PlayThroughs=StructProperty:AIPawnPlaythroughData
TransformedNames=StructProperty:AITransformedName
'''


# ---------------------------------------------------------------- pure helpers (synthetic tests)
def fields(tags):
    """Tagged property list -> {name: value} for decoded entries (nested structs too)."""
    if not isinstance(tags, list) or not all(isinstance(t, dict) and 'name' in t for t in tags):
        return tags
    return {t['name']: fields(t['value']) if isinstance(t.get('value'), list) and t['value']
            and all(isinstance(x, dict) and 'name' in x for x in t['value'])
            else [fields(x) for x in t['value']] if isinstance(t.get('value'), list)
            else t.get('value') for t in tags if t.get('status') == 'decoded'}


def ref(value):
    """Object property value -> path or None."""
    return value.get('path') if isinstance(value, dict) and value.get('index') else None


def vector(value):
    return [float((value or {}).get(axis, 0)) for axis in 'XYZ']


def placement(p):
    """Serialized pose plus the scene pipeline's host conversion."""
    rot = p.get('Rotation') or {}
    return {'ue3': {'location': vector(p.get('Location')),
                    'rotation_units': [int(rot.get(k, 0)) for k in ('Pitch', 'Yaw', 'Roll')]},
            'host': transform(p)}


def distance(a, b, planar=False):
    return math.dist(a[:2], b[:2]) if planar else math.dist(a, b)


def in_cylinder(point, center, radius, half_height):
    """UE3 CylinderComponent containment: planar radius and +-CollisionHeight about the centre."""
    return distance(point, center, planar=True) <= radius and abs(point[2] - center[2]) <= half_height


def rotate(rotation, v):
    """`v` given in a frame rotated by `rotation` ([Pitch, Yaw, Roll] degrees) -> the parent frame.

    Rows of Unreal's FRotationMatrix (the convention of tools/crosscheck_blcmm_dumps.py). Checked on the data by
    the holder-on-carrier oracle in dummy(): Base pose x RelativeLocation reproduces the holder's Location.
    """
    p, y, r = (math.radians(a) for a in rotation)
    sp, sy, sr, cp, cy, cr = math.sin(p), math.sin(y), math.sin(r), math.cos(p), math.cos(y), math.cos(r)
    rows = [[cp * cy, cp * sy, sp],
            [sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp],
            [-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp]]
    return [sum(v[k] * rows[k][j] for k in range(3)) for j in range(3)]


def child_location(parent, child):
    """World location of `child` ({'location', 'rotation'} relative to `parent`, which is the same shape in world)."""
    return [a + b for a, b in zip(parent['location'], rotate(parent['rotation'], child['location']))]


def attach_socket(body, components, bone):
    """The SocketComponent of a body composition whose SocketName is `bone`, as (path, local pose).

    `body` is a decoded BodyComposition ({'Attachments': [{'Data': {'ComponentData': {...}}}]}); `components`
    maps component path -> (class, decoded properties). Exactly one match is required. A socket attached to a
    mesh socket of the body is rejected: its pose would need that mesh.
    """
    found = []
    for entry in body.get('Attachments') or []:
        data = (entry.get('Data') or {}).get('ComponentData') or {}
        path = ref(data.get('Component'))
        cls, props = components.get(path, (None, {}))
        if cls == 'Engine.SocketComponent' and props.get('SocketName') == bone:
            found.append((path, data))
    if len(found) != 1:
        raise ValueError(f'Expected one SocketComponent named {bone}, found {len(found)}')
    path, data = found[0]
    if data.get('bAttachToMesh') or data.get('MeshSocketName') not in (None, 'None'):
        raise ValueError(f'{path} is attached to a mesh socket; its pose needs that mesh')
    pose = transform(components[path][1], component=True)
    if pose['scale'] != [1, 1, 1]:
        raise ValueError(f'{path} is scaled')
    return path, pose


def playthrough_names(balance):
    """AIPawnBalanceDefinition.PlayThroughs -> [{playthrough, display_name, transformed_names {EAITransformed: name}}]."""
    return [{'playthrough': t.get('PlayThrough'), 'display_name': t.get('DisplayName'),
             'transformed_names': {n.get('Type'): n.get('TransformedName') for n in t.get('TransformedNames') or []}}
            for t in balance.get('PlayThroughs') or []]


def follow_chain(nodes, start, limit=64):
    """Walk WillowAIMoveNode NextNodes from `start`; `nodes` maps path -> {'next': [path...]}.

    Only single-successor chains are accepted: a branch (more than one next
    node) needs the AI's weighted choice, which is not decoded.
    """
    chain, seen = [start], {start}
    while True:
        nxt = nodes[chain[-1]]['next']
        if not nxt:
            return chain
        if len(nxt) != 1:
            raise ValueError('Branching move-node chain at ' + chain[-1])
        if nxt[0] in seen or len(chain) >= limit:
            raise ValueError('Move-node chain loops at ' + nxt[0])
        chain.append(nxt[0])
        seen.add(nxt[0])


def point_segment_distance(p, a, b):
    ab = [b[i] - a[i] for i in range(3)]
    length = sum(x * x for x in ab)
    t = 0.0 if length == 0 else max(0.0, min(1.0, sum((p[i] - a[i]) * ab[i] for i in range(3)) / length))
    return math.dist(p, [a[i] + t * ab[i] for i in range(3)])


def choose_respawn(stations, death_location, active=None):
    """Mirror of WillowPlayerPawn.GetBestPlayerPlacementPoint (script, disassembled).

    1. GRI.ActiveRespawnCheckpointTeleportActor when set (`active`).
    2. Else the first station (actor iteration order) with bIsCurrentlyActive
       or bShouldBeActive (`runtime_active` here; runtime state).
    3. Else the nearest station whose CanResurrectHere(false) is true.
    4. Else the nearest other station.
    Returns the chosen station's teleport destination path and the rule used.
    """
    if active:
        return active, 'active checkpoint'
    for s in stations:
        if s.get('runtime_active'):
            return s['teleport_destination'], 'active station'
    best = {True: None, False: None}
    for s in stations:
        d = distance(death_location, s['location'])
        key = bool(s['can_resurrect'])
        if best[key] is None or d < best[key][0]:
            best[key] = (d, s['teleport_destination'])
    for key, rule in ((True, 'nearest resurrect-capable station'), (False, 'nearest station')):
        if best[key]:
            return best[key][1], rule
    return None, 'no station'


# ---------------------------------------------------------------- reader access
class Reader:
    def __init__(self, exe, game, schema):
        self.exe = Path(exe).resolve()
        self.cooked = Path(game) / 'WillowGame' / 'CookedPCConsole'
        self.schema = schema
        self._exports, self._props, self._imports = {}, {}, {}

    def run(self, pkg, *args):
        p = subprocess.run([str(self.exe), str(self.cooked / (pkg + '.upk')), *map(str, args)],
                           capture_output=True, text=True, encoding='utf-8')
        if p.returncode:
            raise ValueError(f'{pkg} {args[:2]}: {p.stderr.strip()[:200]}')
        return json.loads(p.stdout)

    def exports(self, pkg):
        if pkg not in self._exports:
            data = self.run(pkg, '--exports')
            self._exports[pkg] = {'by_path': {x['path']: x for x in data}, 'by_index': {x['index']: x for x in data}}
        return self._exports[pkg]

    def export(self, pkg, path):
        x = self.exports(pkg)['by_path'].get(path)
        if x is None:
            raise ValueError(f'{path} is not an export of {pkg}')
        return x

    def of_class(self, pkg, cls):
        return [x for x in self.exports(pkg)['by_index'].values() if x['class'] == cls]

    def children(self, pkg, path):
        outer = self.export(pkg, path)['index']
        return [x for x in self.exports(pkg)['by_index'].values() if x['outer_index'] == outer]

    def props(self, pkg, path, allow_tail=False):
        """Tagged properties of an export; prefix = first of 4/8/26 that decodes exactly."""
        key = (pkg, path)
        if key not in self._props:
            index = self.export(pkg, path)['index']
            # SeqAct_Interp has a 4-byte native tail (also accepted by tools/prepare_mover.py).
            allow_tail = allow_tail or self.export(pkg, path)['class'] == 'Engine.SeqAct_Interp'
            last = None
            for prefix in PREFIXES:
                try:
                    d = self.run(pkg, '--properties', index, '--property-offset', prefix, '--array-schema', self.schema)
                except ValueError as error:
                    last = error
                    continue
                if d.get('trailing_bytes', 0) == 0 or allow_tail:
                    self._props[key] = fields(d['properties'])
                    break
            else:
                raise ValueError(f'{pkg}:{path} did not decode with an established prefix ({last})')
        return self._props[key]

    def class_default(self, cls):
        """Class default object (package = the class's code package), prefix 4."""
        package, name = cls.split('.', 1)
        return self.props(package, 'Default__' + name)

    def archetype(self, pkg, path):
        x = self.export(pkg, path)
        a = x['archetype_index']
        if a > 0:
            return self.exports(pkg)['by_index'][a]['path']
        if a < 0:
            if pkg not in self._imports:
                self._imports[pkg] = {i['index']: i for i in self.run(pkg, '--imports')}
            return self._imports[pkg][a]['path']
        return None


def levels(reader, persistent):
    """Persistent map plus the streamed levels its LevelStreaming objects name."""
    names = [persistent]
    for x in reader.exports(persistent)['by_index'].values():
        if x['class'].startswith('Engine.LevelStreaming'):
            name = reader.props(persistent, x['path']).get('PackageName')
            if name and name != 'None' and name not in names and (reader.cooked / (name + '.upk')).exists():
                names.append(name)
    return names


def links(props, kind='VariableLinks'):
    """{LinkDesc: [linked paths]} for variable links, or [(desc, [(op, input)])] for outputs."""
    if kind == 'VariableLinks':
        return {l['LinkDesc']: [ref(v) for v in l.get('LinkedVariables') or []] for l in props.get(kind) or []}
    return {l['LinkDesc']: [(ref(k.get('LinkedOp')), k.get('InputLinkIdx', 0)) for k in l.get('Links') or []]
            for l in props.get(kind) or []}


def input_names(reader, pkg, op_path):
    props = reader.props(pkg, op_path, allow_tail=True)
    inputs = props.get('InputLinks')
    if inputs is None:
        inputs = reader.class_default(reader.export(pkg, op_path)['class']).get('InputLinks') or []
    return [i.get('LinkDesc') for i in inputs]


def describe_outputs(reader, pkg, op_path):
    result = {}
    for desc, targets in links(reader.props(pkg, op_path, allow_tail=True), 'OutputLinks').items():
        result[desc] = []
        for op, index in targets:
            names = input_names(reader, pkg, op)
            result[desc].append({'op': op, 'class': reader.export(pkg, op)['class'], 'input': index,
                                 'input_name': names[index] if index < len(names) else None})
    return result


def object_var(reader, pkg, var_path):
    """SeqVar_Object / SeqVar_Named -> (object path, how it was resolved)."""
    cls = reader.export(pkg, var_path)['class']
    p = reader.props(pkg, var_path)
    if cls == 'Engine.SeqVar_Object':
        return ref(p.get('ObjValue')), {'var': var_path, 'var_name': p.get('VarName')}
    if cls == 'Engine.SeqVar_Named':
        name = p.get('FindVarName')
        named = [x['path'] for x in reader.of_class(pkg, 'Engine.SeqVar_Object')
                 if reader.props(pkg, x['path']).get('VarName') == name]
        if len(named) != 1:
            raise ValueError(f'Named variable {name} resolves to {len(named)} objects')
        return ref(reader.props(pkg, named[0]).get('ObjValue')), {'var': var_path, 'find_var_name': name, 'resolved_var': named[0]}
    raise ValueError('Unsupported Kismet variable ' + cls)


def check(oracles, name, passed, detail):
    oracles.append({'name': name, 'passed': bool(passed), 'detail': detail})


# ---------------------------------------------------------------- items
def range_trigger(reader, level_names, objective):
    found, scanned = [], 0
    for level in level_names:
        for x in reader.of_class(level, 'WillowGame.WillowWaypoint'):
            scanned += 1
            p = reader.props(level, x['path'])
            if ref((p.get('WaypointInfo') or {}).get('LinkedObjective')) == objective:
                found.append((level, x['path'], p))
    if len(found) != 1:
        raise ValueError(f'Expected one waypoint for {objective}, found {len(found)}')
    level, path, p = found[0]
    cylinder = ref(p.get('CylinderComponent'))
    c = reader.props(level, cylinder) if cylinder else {}
    if c.get('CollisionRadius') is None or c.get('CollisionHeight') is None:
        # The class-default cylinder does not decode with an established prefix; do not guess.
        raise ValueError('Waypoint cylinder relies on undecoded defaults')
    defaults = reader.class_default('WillowGame.WillowWaypoint')
    info = p.get('WaypointInfo') or {}
    return {'object': f'{level}:{path}', 'class': 'WillowGame.WillowWaypoint', **placement(p),
            'shape': {'type': 'cylinder', 'component': cylinder, 'radius': c.get('CollisionRadius'),
                      'half_height': c.get('CollisionHeight')},
            'linked_objective': objective, 'objective_set_restrictions': [ref(r) for r in info.get('ObjectiveSetRestrictions') or []],
            'update_objective_on_player_touch': bool(p.get('bUpdateObjectiveOnPlayerTouch')),
            'enabled': bool(p.get('bEnabled', defaults.get('bEnabled', False))),
            'touch_volumes': [ref(r) for r in p.get('TouchVolumes') or []],
            'completion_rule': 'WillowWaypoint.Touch (script): if bUpdateObjectiveOnPlayerTouch and Other is a player-owned '
                               'Pawn -> ProcessPlayerTouch: if MissionTracker.IsMissionObjectiveActive(LinkedObjective) and '
                               '(no ObjectiveSetRestrictions or one of them active) -> MissionTracker.UpdateObjective(LinkedObjective)',
            'how_found': f'scanned {scanned} WillowWaypoint exports in {len(level_names)} levels for WaypointInfo.LinkedObjective'}


def scripted_walk(reader, pkg, action, pawn_location):
    p = reader.props(pkg, action)
    defaults = reader.class_default('WillowGame.WillowSeqAct_AIScripted')
    variables = links(p)
    if len(variables.get('Target', [])) != 1 or len(variables.get('Destination', [])) != 1:
        raise ValueError('AIScripted needs one Target and one Destination')
    pawn, target_how = object_var(reader, pkg, variables['Target'][0])
    destination, dest_how = object_var(reader, pkg, variables['Destination'][0])
    node_defaults = reader.class_default('WillowGame.WillowAIMoveNode')
    nodes = {}

    def node(path):
        if path not in nodes:
            n = reader.props(pkg, path)
            nodes[path] = {'props': n, 'next': [ref(x.get('Node')) for x in n.get('NextNodes') or []],
                           'previous': [ref(x) for x in n.get('PreviousNodes') or []]}
            for nxt in nodes[path]['next']:
                node(nxt)
        return nodes[path]
    node(destination)
    chain = follow_chain(nodes, destination)
    arrivals = {}
    for x in reader.children(pkg, SEQUENCE):
        if x['class'] == 'GearboxFramework.SeqEvent_ArrivedAtMoveNode':
            origin = ref(reader.props(pkg, x['path']).get('Originator'))
            arrivals.setdefault(origin, []).append({'event': x['path'], 'outputs': describe_outputs(reader, pkg, x['path'])})
    path = []
    for n in chain:
        q = nodes[n]['props']
        path.append({'node': f'{pkg}:{n}', **placement(q),
                     'pawn_arrival_radius': q.get('PawnArrivalRadius', node_defaults.get('PawnArrivalRadius')),
                     'ai_speed_percentage': q.get('AISpeedPercentageHere', node_defaults.get('AISpeedPercentageHere')),
                     'fuzzy_arrival': bool(q.get('bFuzzyArrival', node_defaults.get('bFuzzyArrival', False))),
                     'previous_nodes': nodes[n]['previous'], 'on_arrival': arrivals.get(n, [])})
    finished = []
    for op, _ in links(p, 'OutputLinks').get('Finished', []):
        entry = {'op': op, 'class': reader.export(pkg, op)['class'], 'behaviors': []}
        for b in reader.props(pkg, op).get('Behaviors') or []:
            bp = reader.props(pkg, ref(b))
            entry['behaviors'].append({'behavior': ref(b), 'class': reader.export(pkg, ref(b))['class'],
                                       'flag': ref(bp.get('FlagDef')), 'value': bool(bp.get('FlagValue', False))})
        finished.append(entry)
    return {'action': f'{pkg}:{action}', 'comment': p.get('ObjComment'),
            'focus_style': p.get('FocusStyle', defaults.get('FocusStyle')), 'stance': p.get('Stance', defaults.get('Stance')),
            'target': {'object': pawn, **target_how}, 'destination': {'object': destination, **dest_how},
            'start_leg': {'from': pawn_location, 'to': path[0]['ue3']['location'],
                          'note': 'pawn -> destination node is native navigation (PHYS_NavMeshWalking); straight line here UNVERIFIED'},
            'path': path, 'finished': finished}


def reaching_events(reader, pkg, ops, target):
    """Remote events whose 'Out' reaches `target` directly or through one intermediate op."""
    found = []
    for e in ops.values():
        if 'RemoteEvent' not in e['class']:
            continue
        first = [op for op, _ in links(reader.props(pkg, e['path']), 'OutputLinks').get('Out', [])]
        second = [o for op in first for o, _ in links(reader.props(pkg, op), 'OutputLinks').get('Out', [])]
        if target in first or target in second:
            p = reader.props(pkg, e['path'])
            found.append({'event': e['path'], 'name': p.get('EventName'), 'mission': ref(p.get('AssociatedMissionDefinition'))})
    return found


def marcus(reader, pkg, trigger):
    ops = {x['path']: x for x in reader.children(pkg, SEQUENCE)}
    entries = [x for x in ops.values() if x['class'] == 'WillowGame.WillowSeqEvent_MissionRemoteEvent'
               and reader.props(pkg, x['path']).get('EventName') == MOVE_EVENT
               and ref(reader.props(pkg, x['path']).get('AssociatedMissionDefinition')) == MISSION]
    if len(entries) != 1:
        raise ValueError(f'Expected one {MOVE_EVENT} listener for the mission, found {len(entries)}')
    actions = [op for op, _ in links(reader.props(pkg, entries[0]['path']), 'OutputLinks').get('Out', [])
               if ops[op]['class'] == 'WillowGame.WillowSeqAct_AIScripted']
    if len(actions) != 1:
        raise ValueError('Expected one scripted move behind ' + MOVE_EVENT)
    variables = links(reader.props(pkg, actions[0]))
    pawn_path, _ = object_var(reader, pkg, variables['Target'][0])
    pawn = reader.props(pkg, pawn_path)
    archetype = reader.archetype(pkg, pawn_path)
    ai_class = ref(reader.props(pkg, archetype).get('AIClass')) if archetype in reader.exports(pkg)['by_path'] else None
    ai = reader.props(pkg, ai_class) if ai_class else {}
    walk = scripted_walk(reader, pkg, actions[0], placement(pawn)['ue3']['location'])
    # Every other scripted move on the same pawn in this sequence (e.g. the walk back to the shop).
    others = []
    for x in ops.values():
        if x['class'] == 'WillowGame.WillowSeqAct_AIScripted' and x['path'] != actions[0]:
            other = scripted_walk(reader, pkg, x['path'], placement(pawn)['ue3']['location'])
            if other['target']['object'] == pawn_path:
                other['entry_events'] = reaching_events(reader, pkg, ops, x['path'])
                others.append(other)
    oracles = []
    check(oracles, 'target pawn archetype', archetype == 'GD_Marcus.Character.Pawn_Marcus', archetype)
    for i in range(1, len(walk['path'])):
        prev = walk['path'][i - 1]['node'].split(':', 1)[1]
        listed = walk['path'][i]['previous_nodes']
        check(oracles, f'path link {i}', not listed or prev in listed, f'{prev} in PreviousNodes of node {i}')
    end = walk['path'][-1]['ue3']['location']
    check(oracles, 'walk ends inside the range trigger cylinder',
          in_cylinder(end, trigger['ue3']['location'], trigger['shape']['radius'], trigger['shape']['half_height']),
          f"planar {round(distance(end, trigger['ue3']['location'], planar=True), 1)}, "
          f"dz {round(end[2] - trigger['ue3']['location'][2], 1)}")
    door_events = [(o['input_name'], o['op']) for n in walk['path'] for a in n['on_arrival']
                   for outs in a['outputs'].values() for o in outs]
    check(oracles, 'door opens then closes along the walk', [d[0] for d in door_events] == ['Play', 'Reverse']
          and len({d[1] for d in door_events}) == 1, door_events)
    if door_events:
        # The door actor should sit on the walk between the node that opens it and the one that closes it.
        door_vars = [v for desc, vs in links(reader.props(pkg, door_events[0][1])).items() if desc != 'Data' for v in vs]
        door = reader.props(pkg, object_var(reader, pkg, door_vars[0])[0])
        legs = [n['ue3']['location'] for n in walk['path'] if n['on_arrival']]
        gap = point_segment_distance(vector(door.get('Location')), legs[0], legs[-1])
        check(oracles, 'door lies between the opening and closing nodes', gap < 200,
              f'{object_var(reader, pkg, door_vars[0])[0]}: {round(gap, 1)} uu from the segment')
    for other in others:
        home = other['path'][-1]
        d = distance(home['ue3']['location'], placement(pawn)['ue3']['location'], planar=True)
        check(oracles, f'return walk {other["action"].rsplit(".", 1)[-1]} ends at the placed pawn',
              d <= (home['pawn_arrival_radius'] or 0) and home['ue3']['rotation_units'][1] == placement(pawn)['ue3']['rotation_units'][1],
              f'{round(d, 1)} uu, yaw {home["ue3"]["rotation_units"][1]} vs {placement(pawn)["ue3"]["rotation_units"][1]}')
    return {'pawn': {'object': f'{pkg}:{pawn_path}', 'archetype': archetype, **placement(pawn),
                     'how_found': f'{MOVE_EVENT} listener -> {actions[0].rsplit(".", 1)[-1]} Target -> named variable',
                     'note': 'placed WillowAIPawn (not population-spawned)'},
            'ai_class': {'object': ai_class, 'ground_speed': ai.get('GroundSpeed'), 'walking_pct': ai.get('WalkingPct'),
                         'physics': ai.get('Physics'), 'rotation_rate_units': ai.get('RotationRate'),
                         'slow_down_dist': ai.get('SlowDownDist'), 'slow_down_min_pct': ai.get('SlowDownMinPct')},
            'entry_event': f'{pkg}:{entries[0]["path"]}', 'walk_to_range': walk, 'other_walks': others, 'oracles': oracles}


def attachment(reader, pkg, step, holder_props, spawn_point):
    """Where SeqAct_AttachToActor puts the spawned dummy on the holder.

    The op names a BoneName; the holder is a WillowInteractiveObject whose definition's body composition has
    static meshes and one SocketComponent per named point (no skeleton), so the name resolves to that socket.
    Activated is native: placing the attached origin on the socket when no relative offset is tagged is the
    host's reading (UNVERIFIED), as is bUseConstructAttachment's meaning (recorded, not interpreted).
    """
    p = reader.props(pkg, step['op'])
    defaults = reader.class_default('Engine.SeqAct_AttachToActor')

    def flag(name):
        return bool(p.get(name, defaults.get(name, False)))
    definition = ref(holder_props.get('InteractiveObjectDefinition'))
    body = (reader.props(pkg, definition).get('BodyComposition') or {}) if definition else {}
    components = {}
    for entry in body.get('Attachments') or []:
        path = ref(((entry.get('Data') or {}).get('ComponentData') or {}).get('Component'))
        if path:
            cls = reader.export(pkg, path)['class']
            # Only sockets are decoded (the static mesh components do not decode with an established prefix).
            components[path] = (cls, reader.props(pkg, path) if cls == 'Engine.SocketComponent' else {})
    path, local = attach_socket(body, components, step.get('bone'))
    world = child_location(placement(holder_props)['host'], local)
    point = spawn_point['ue3']['location']
    return {'op': f'{pkg}:{step["op"]}', 'bone_name': step.get('bone'),
            'hard_attach': flag('bHardAttach'), 'detach': flag('bDetach'),
            'use_construct_attachment': flag('bUseConstructAttachment'),
            'use_relative_offset': flag('bUseRelativeOffset'), 'relative_offset': vector(p.get('RelativeOffset')),
            'use_relative_rotation': flag('bUseRelativeRotation'),
            'relative_rotation_units': [int((p.get('RelativeRotation') or {}).get(k, 0)) for k in ('Pitch', 'Yaw', 'Roll')],
            'socket': {'object': f'{pkg}:{path}', 'definition': definition, 'name': step.get('bone'), 'host': local},
            'socket_world': {'host': {'location': world}},
            'spawn_point_offset': {'planar': round(distance(world, point, planar=True), 2), 'dz': round(world[2] - point[2], 2)},
            'semantics': 'SeqAct_AttachToActor.Activated is native (no script). The BoneName resolves to the holder\'s '
                         'SocketComponent of that name; with no relative offset tagged the host puts the attached '
                         'origin on the socket: UNVERIFIED (bUseConstructAttachment not interpreted)'}


def dummy(reader, pkg, objectives):
    ops = reader.children(pkg, SEQUENCE)
    dens = []
    for x in ops:
        if x['class'] != 'GearboxFramework.SeqEvent_PopulatedActor':
            continue
        den_path = ref(reader.props(pkg, x['path']).get('Originator'))
        den = reader.props(pkg, den_path)
        aspect = reader.props(pkg, ref(den['Aspect'])) if ref(den.get('Aspect')) else {}
        pop = ref(den.get('PopulationDef'))
        archetypes = []
        for entry in reader.props(pkg, pop).get('ActorArchetypeList') or []:
            factory = ref(entry.get('SpawnFactory'))
            balance = ref(reader.props(pkg, factory).get('PawnBalanceDefinition'))
            b = reader.props(pkg, balance)
            # Target names (WillowAIPawn.GetTargetName, script): the balance's display name, or, when TransformType is
            # not 0, AIPawnBalanceDefinition.GetTransformedDisplayName (native; the per-playthrough table is the data).
            archetypes.append({'factory': factory, 'balance': balance, 'archetype': ref(b.get('AIPawnArchetype')),
                               'default_exp_level': ref((b.get('DefaultExpLevel') or {}).get('BaseValueAttribute')),
                               'playthroughs': playthrough_names(b)})
        points = []
        for point in den.get('SpawnPoints') or []:
            points.append({'object': f'{pkg}:{ref(point)}', **placement(reader.props(pkg, ref(point)))})
        default_enabled = reader.class_default('WillowGame.PopulationOpportunityDen').get('IsEnabled', True)
        dens.append({'den': f'{pkg}:{den_path}', 'event': f'{pkg}:{x["path"]}', 'description': den.get('Description'),
                     'enabled_initially': bool(den.get('IsEnabled', default_enabled)), **placement(den),
                     'population': pop, 'archetypes': archetypes, 'spawn_points': points,
                     'max_total_actors': (den.get('MaxTotalActorsFormula') or {}).get('BaseValueConstant'),
                     'spawn_radius': den.get('SpawnRadius'),
                     'game_stage': {'region': ref(den.get('GameStageRegion')), 'min': den.get('MinimumGameStageForRegion'),
                                    'max': den.get('MaximumGameStageForRegion')},
                     'mission_objective': ref(aspect.get('MissionObjective')),
                     'waypoint_setting': aspect.get('WaypointSetting')})
    chosen = [d for d in dens if d['mission_objective'] in objectives]
    if len(chosen) != 1:
        raise ValueError(f'Expected one den for {MISSION}, found {len(chosen)}')
    den = chosen[0]
    point = den['spawn_points'][0]['object'].split(':', 1)[1]
    after = []
    for x in ops:
        if x['class'] == 'GearboxFramework.SeqEvent_PopulatedPoint' and ref(reader.props(pkg, x['path']).get('Originator')) == point:
            todo = [op for op, _ in links(reader.props(pkg, x['path']), 'OutputLinks').get('Out', [])]
            while todo:
                op = todo.pop(0)
                p = reader.props(pkg, op)
                step = {'op': op, 'class': reader.export(pkg, op)['class'], 'bone': p.get('BoneName')}
                for desc, variables in links(p).items():
                    step[desc] = [object_var(reader, pkg, v)[0] or reader.props(pkg, v).get('VarName') for v in variables]
                after.append(step)
                todo += [o for o, _ in links(p, 'OutputLinks').get('Out', [])]
    holder = next((s['Target'][0] for s in after if s['class'] == 'Engine.SeqAct_AttachToActor'), None)
    hp = reader.props(pkg, holder) if holder else {}
    oracles = []
    arch = den['archetypes']
    check(oracles, 'population chain resolves to the target-dummy pawn',
          len(arch) == 1 and arch[0]['archetype'] == 'GD_TargetDummy.Character.Pawn_TargetDummy'
          and arch[0]['balance'] == 'GD_Population_Psycho.Balance.PawnBalance_TargetDummy', arch)
    attach = None
    if holder:
        d = distance(den['spawn_points'][0]['ue3']['location'], vector(hp.get('Location')))
        check(oracles, 'spawn point next to the attachment holder', d < 200, round(d, 1))
        if ref(hp.get('Base')) and hp.get('RelativeLocation') is not None:
            # Convention oracle for the socket composition below: the holder's serialized Location equals its Base's
            # pose applied to its RelativeLocation (the Base's DrawScale is not applied).
            base = reader.props(pkg, ref(hp['Base']))
            relative = {'location': vector(hp.get('RelativeLocation')),
                        'rotation': transform({'Rotation': hp.get('RelativeRotation') or {}})['rotation']}
            gap = distance(child_location(placement(base)['host'], relative), vector(hp.get('Location')))
            check(oracles, 'holder Location = Base pose x RelativeLocation', gap < 0.05, round(gap, 4))
        step = next(s for s in after if s['class'] == 'Engine.SeqAct_AttachToActor')
        attach = attachment(reader, pkg, step, hp, den['spawn_points'][0])
    return {'den': den, 'other_dens': [d for d in dens if d is not den], 'after_spawn': after,
            'holder': {'object': f'{pkg}:{holder}' if holder else None, **(placement(hp) if hp else {}),
                       'base': ref(hp.get('Base')), 'definition': ref(hp.get('InteractiveObjectDefinition')),
                       'relative_location': vector(hp.get('RelativeLocation')) if hp else None,
                       'attach': attach},
            'how_found': 'SeqEvent_PopulatedActor originators in the mission sequence; den chosen by MissionPopulationAspect objective',
            'oracles': oracles}


def target_mover(reader, game, pkg, holder_base):
    ops = {x['path']: x for x in reader.children(pkg, SEQUENCE)}
    interps = [p for p, x in ops.items() if x['class'] == 'Engine.SeqAct_Interp'
               and any(holder_base == object_var(reader, pkg, v)[0]
                       for vs in links(reader.props(pkg, p, allow_tail=True)).values() for v in vs
                       if reader.export(pkg, v)['class'] == 'Engine.SeqVar_Object')]
    if len(interps) != 1:
        raise ValueError(f'Expected one Matinee moving {holder_base}, found {len(interps)}')
    binding = prepare_mover.prepare_binding(reader.exe, Path(game), pkg, interps[0])
    drivers = []
    for path, x in ops.items():
        outs = links(reader.props(pkg, path, allow_tail=True), 'OutputLinks')
        for desc, targets in outs.items():
            for op, index in targets:
                if op == interps[0]:
                    drivers.append({'from': path, 'class': x['class'], 'output': desc, 'input': binding['inputs'][index],
                                    'event_name': reader.props(pkg, path).get('EventName')})
    # Events upstream of the Matinee (through conditions/delays/switches), breadth first.
    backward = {}
    for path in ops:
        for desc, targets in links(reader.props(pkg, path, allow_tail=True), 'OutputLinks').items():
            for op, index in targets:
                backward.setdefault(op, []).append((path, desc, index))
    upstream, todo, seen = [], [(interps[0], 0)], {interps[0]}
    while todo:
        op, depth = todo.pop(0)
        for source, desc, index in backward.get(op, []):
            if source in seen or depth >= 6:
                continue
            seen.add(source)
            if 'SeqEvent' in ops[source]['class']:
                upstream.append({'event': source, 'name': reader.props(pkg, source).get('EventName'), 'hops': depth + 1})
            else:
                todo.append((source, depth + 1))
    # Which behaviours fire those remote events (by exact name, this package).
    names = {u['name'] for u in upstream if u['name']}
    sources = [{'behavior': x['path'], 'event_name': reader.props(pkg, x['path']).get('EventName')}
               for x in reader.of_class(pkg, 'Engine.Behavior_RemoteEvent')
               if reader.props(pkg, x['path']).get('EventName') in names]
    toggles, dialogs = [], []
    for path, x in ops.items():
        if x['class'] == 'Engine.SeqAct_Toggle':
            p = reader.props(pkg, path)
            toggles.append({'op': f'{pkg}:{path}', 'targets': [object_var(reader, pkg, v)[0] for v in links(p).get('Target', [])],
                            'reached_by': [{'event': e, 'name': reader.props(pkg, e).get('EventName'),
                                            'mission': ref(reader.props(pkg, e).get('AssociatedMissionDefinition')), 'input': i}
                                           for e in ops for o, i in links(reader.props(pkg, e, allow_tail=True), 'OutputLinks').get('Out', [])
                                           if o == path]})
        elif x['class'] == 'GearboxFramework.GearboxSeqAct_TriggerDialogName':
            p = reader.props(pkg, path)
            dialogs.append({'op': f'{pkg}:{path}', 'group': ref(p.get('Group')), 'event_tag': ref(p.get('EventTag')),
                            'name_tag': ref(p.get('NameTag')),
                            'reached_by': [{'event': e, 'name': reader.props(pkg, e).get('EventName')}
                                           for e in ops for o, i in links(reader.props(pkg, e, allow_tail=True), 'OutputLinks').get('Out', [])
                                           if o == path]})
    return {'binding': binding, 'driven_by': drivers, 'upstream_events': upstream, 'remote_event_sources': sources,
            'toggles': toggles, 'dialogs': dialogs,
            'how_found': 'the Matinee whose bound group actor is the target holder\'s Base'}


def mission_event_listeners(reader, level_names, mission_events):
    """For each mission remote event name: Kismet listeners anywhere in the map (exact name)."""
    result = {name: [] for name in mission_events}
    for level in level_names:
        for cls in ('WillowGame.WillowSeqEvent_MissionRemoteEvent', 'Engine.SeqEvent_RemoteEvent'):
            for x in reader.of_class(level, cls):
                p = reader.props(level, x['path'])
                name = p.get('EventName')
                mission = ref(p.get('AssociatedMissionDefinition'))
                if name in result and (cls == 'Engine.SeqEvent_RemoteEvent' or mission == MISSION):
                    result[name].append(f'{level}:{x["path"]}')
    return result


def respawn(reader, level_names, startup_reader, references):
    stations = []
    for level in level_names:
        for cls in STATION_CLASSES:
            for x in reader.of_class(level, cls):
                p = reader.props(level, x['path'])
                dest = ref(p.get('TeleportDest'))
                d = reader.props(level, dest)
                exits = []
                for e in d.get('ExitPoints') or []:
                    ep = reader.props(level, ref(e))
                    exits.append({'object': f'{level}:{ref(e)}', 'class': reader.export(level, ref(e))['class'],
                                  'owner': ref(ep.get('Owner')), **placement(ep)})
                definition = ref(p.get('TravelDefinition'))
                if cls.endswith('ResurrectTravelStation'):
                    can, why = True, 'ResurrectTravelStation.CanResurrectHere = !bIsLevelTravel'
                elif cls.endswith('FastTravelStation'):
                    initially = bool(startup_reader.props('Startup', definition).get('bInitiallyActive')) if definition else False
                    can, why = initially, 'FastTravelStation.CanResurrectHere includes TravelDefinition.bInitiallyActive (runtime flags may add)'
                else:
                    can, why = False, 'TravelStation.CanResurrectHere = false'
                stations.append({'object': f'{level}:{x["path"]}', 'class': cls, **placement(p), 'location': vector(p.get('Location')),
                                 'definition': definition, 'interactive_object': ref(p.get('InteractiveObjectDefinition')),
                                 'touch_radius': p.get('StationTouchRadius'), 'touch_height': p.get('StationTouchHeight'),
                                 'teleport_destination': f'{level}:{dest}', 'destination_owner': ref(d.get('Owner')),
                                 'destination': placement(d), 'exit_points': exits,
                                 'can_resurrect': can, 'can_resurrect_rule': why})
    oracles = []
    for s in stations:
        own = s['object'].split(':', 1)[1]
        check(oracles, f'{own} destination owner', s['destination_owner'] == own, s['destination_owner'])
        dest = s['teleport_destination'].split(':', 1)[1]
        check(oracles, f'{own} exit points owned by its destination',
              all(e['owner'] == dest for e in s['exit_points']), len(s['exit_points']))
    choices = {}
    for label, location in references.items():
        chosen, rule = choose_respawn(stations, location)
        station = next(s for s in stations if s['teleport_destination'] == chosen)
        first_exit = next((e for e in station['exit_points'] if 'Vehicle' not in e['class']), None)
        choices[label] = {'death_location': location, 'teleport_destination': chosen, 'rule': rule,
                          'station': station['object'], 'distance': round(distance(location, station['location']), 1),
                          'first_exit_point': first_exit}
    for s in stations:
        s.pop('location')
    return {'stations': stations, 'fresh_state_choice': choices, 'oracles': oracles,
            'selection_rule': 'WillowPlayerPawn.GetBestPlayerPlacementPoint (script): active checkpoint '
                              '(GRI.ActiveRespawnCheckpointTeleportActor, set by TravelStation.ReplacePreviouslyActivatedStation) '
                              '-> first active station -> nearest CanResurrectHere -> nearest other; '
                              'exit point = TeleporterDestination.GetNextExitPoint round robin from ExitPointsCounter (0), '
                              'skipping vehicle exit points and occupied ones',
            'runtime_state': 'which station the player activated (touch/activation path) is runtime state; '
                             'fresh_state_choice assumes none: UNVERIFIED'}


def scene_bounds_check(locations, positions):
    """Loose oracle: planar distance to the nearest prepared static-mesh placement origin.

    (An AABB is useless here: sky/backdrop meshes span hundreds of km.) A small
    distance shows a position is among the map's geometry, not that it is on a floor.
    """
    checks = []
    for name, p in positions.items():
        nearest = min(distance(p, l, planar=True) for l in locations)
        checks.append({'name': name, 'nearest_placement_planar': round(nearest, 1)})
    return {'placements': len(locations), 'checks': checks}


def scene_locations(scene_path):
    scene = json.loads(Path(scene_path).read_text(encoding='utf-8'))
    locs = []
    for actor in scene['actors']:
        pose = actor['transform']
        loc = (pose.get('actor') or pose).get('location')
        if loc:
            locs.append(loc)
    return locs


def build(reader_exe, game, scene=None):
    with tempfile.TemporaryDirectory() as tmp:
        schema = Path(tmp) / 'slice.schema'
        schema.write_text(SCHEMA)
        reader = Reader(reader_exe, game, schema)
        level_names = levels(reader, PERSISTENT)
        mission = reader.props('Startup', MISSION)
        objectives = [ref(o) for o in mission.get('ObjectiveDefs') or []]
        initial = reader.props('Startup', ref(mission['InitialObjectiveSet']))
        first = [ref(o) for o in initial.get('ObjectiveDefinitions') or []]
        if len(first) != 1:
            raise ValueError('Expected one objective in the initial objective set')
        trigger = range_trigger(reader, level_names, first[0])
        m = marcus(reader, SEQ_PACKAGE, trigger)
        d = dummy(reader, SEQ_PACKAGE, objectives)
        mover = target_mover(reader, game, SEQ_PACKAGE, d['holder']['base'])
        group = mover['binding']['groups'][0]
        check(d['oracles'], 'holder Base is the Matinee group actor', group['actor'] == d['holder']['base'], group['actor'])
        check(d['oracles'], 'holder is attached to the group actor',
              d['holder']['object'].split(':', 1)[1] in group['attached'], group['attached'])
        listeners = mission_event_listeners(reader, level_names, mission.get('SupportedRemoteEvents') or [])
        refs = {'marcus_placed': m['pawn']['ue3']['location'], 'range_trigger': trigger['ue3']['location']}
        r = respawn(reader, level_names, reader, refs)
        result = {'schema': 'ow-slice-world-v1', 'mission': MISSION, 'map': PERSISTENT, 'levels': level_names,
                  'generated': datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
                  'reader_sha256': hashlib.sha256(Path(reader_exe).read_bytes()).hexdigest(),
                  'coordinates': 'ue3 = serialized; host = tools/prepare_level.py transform (location as-is, rotation degrees [P, Y, R])',
                  'range_trigger': trigger, 'marcus': m, 'target_dummy': d, 'target_mover': mover,
                  'mission_remote_event_listeners': listeners, 'respawn': r}
        if scene and Path(scene).exists():
            positions = {'range_trigger': trigger['ue3']['location'], 'marcus': m['pawn']['ue3']['location'],
                         'dummy_spawn': d['den']['spawn_points'][0]['ue3']['location'],
                         **{f'walk_{i}': n['ue3']['location'] for i, n in enumerate(m['walk_to_range']['path'])},
                         **{s['object']: s['host']['location'] for s in r['stations']}}
            result['scene_bounds'] = scene_bounds_check(scene_locations(scene), positions)
            result['scene_bounds']['scene_sha256'] = hashlib.sha256(Path(scene).read_bytes()).hexdigest()
        return result


def main():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument('--reader', type=Path, default=ROOT / 'build/Release/ow-package.exe')
    p.add_argument('--game', type=Path, required=True)
    p.add_argument('--scene', type=Path, default=ROOT / 'local/sanctuary/scene.json')
    p.add_argument('--output', type=Path, default=ROOT / 'local/slice/world.json')
    p.add_argument('--no-values', action='store_true', help='skip tools/slice_values.py')
    args = p.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / 'local'):
        raise SystemExit('Game-derived output must stay under repository local/')
    result = build(args.reader, args.game, args.scene)
    if not args.no_values:
        import slice_values
        result['values'] = slice_values.build(args.reader, args.game)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, indent=2) + '\n')
    oracles = [o for part in (result['marcus'], result['target_dummy'], result['respawn']) for o in part['oracles']]
    failed = [o for o in oracles if not o['passed']]
    print(f'wrote {output}: {len(oracles) - len(failed)}/{len(oracles)} oracles pass')
    for o in failed:
        print('FAILED', o['name'], o['detail'])
    return 1 if failed else 0


if __name__ == '__main__':
    raise SystemExit(main())
