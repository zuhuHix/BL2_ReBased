#!/usr/bin/env python3
"""Host manifest for the Sanctuary ambient NPCs: spawns, node graph, perches, imported assets.

AI-assisted (Claude), 2026-10-04. Clean room: no game data lives in this file; the output ``local/slice/ambient_world.json``
(schema ``ow-ambient-world-v1``, read by host/ue5/OpenWillow/Source/OpenWillow/OpenWillowAmbient.cpp) is game-derived and
stays under the ignored ``local/``. It joins three things made earlier, all read from the installed packages by our reader:

* ``local/slice/ambient_npcs.json``      tools/census_ambient_npcs.py (dens, population points, node graph, perch definitions,
                                         Kismet population-point events and scripted-move actions)
* ``local/slice/ambient/ambient_assets.json``  tools/ambient_npc_assets.py (UE paths of the imported meshes and clips)
* ``Sanctuary_Combat`` itself            the body's patrol-stance speed scale (three small object dumps)

What is realised, and the rules the host applies to it, are stand-ins unless the sources say otherwise (details and the
`UNVERIFIED` list in docs/verification/SANCTUARY_AMBIENT_NPCS.md):

* a den realises MaxActiveActorsIsNormal (class default 1) of its population points, chosen by a seeded shuffle (the
  native population picks at random);
* the pawn kind is a seeded weighted draw over the population definition's factories;
* a point with ``InitialActionDestinations`` starts there; a point without one starts at the destination of the
  ``WillowSeqAct_AIScripted`` its ``SeqEvent_PopulatedPoint`` reaches (the Kismet that sends the crowd around the town);
  a point with neither is not realised;
* ``Flag_IdleNPC`` (PopDef_NPC_ScriptedIdle) = "Perch Only AI": snapped to the perch and stays; otherwise the pawn follows
  NextNodes (weighted); ``Flag_NPCDoNotThrottleMovement`` (PopDef_NPC_NoThrottle) = not load-balanced, any other walker waits
  for the NPCLoadBalancer (docs/verification/NATIVE_AMBIENT_NPC.md, UNVERIFIED in game).

* ``--observed <dump.json> --samples <sample.jsonl>`` (tools/real_game/scripts/ambient_npcs.py output from the real game, game
  at mission Plan B): instead of the seeded realisation above, the live "Sanctuary Citizen" pawns of that capture become the
  spawns (kind from the mesh, position and yaw as found, walker/idle from whether the pawn moved during the sample and whether
  it stood on a perch). This replaces the native population's own choice of which points are live; the rest of the rules stay.

  python tools/prepare_ambient_world.py [--game DIR] [--reader build/Release/ow-package.exe]
                                        [--observed local/realgame/ambient/t0_start.json --samples local/realgame/ambient/walk_a.jsonl]
"""
import argparse
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
CENSUS = ROOT / 'local/slice/ambient_npcs.json'
ASSETS = ROOT / 'local/slice/ambient/ambient_assets.json'
OUT = ROOT / 'local/slice/ambient_world.json'
LEVEL = 'Sanctuary_Combat'
POP_PREFIX = 'GD_Population_NPC.Population.'
KIND_OF_ARCHETYPE = {'GD_GenericNPCMale.Character.Pawn_GenericNPCMale': 'CitizenMale',
                     'GD_GenericNPCFemale.Character.Pawn_GenericNPCFemale': 'CitizenFemale'}
# Class defaults (installed): WillowAIMoveNode.PawnArrivalRadius 128, Perch.PawnArrivalRadius 32, Perch.bFaceNodeDirection true.
MOVE_NODE_RADIUS, PERCH_RADIUS = 128.0, 32.0
SPAWN_LIMIT_DEFAULT = 1          # PopulationOpportunityDen class default MaxActiveActorsIsNormal


def seeded(name, salt=''):
    return int(hashlib.sha1((name + salt).encode()).hexdigest()[:8], 16)


def weighted_choice(options, name):
    """options = [(item, weight)]; deterministic from `name`."""
    total = sum(max(w, 0) for _, w in options) or 1.0
    pick = (seeded(name, 'kind') % 100000) / 100000.0 * total
    for item, weight in options:
        pick -= max(weight, 0)
        if pick < 0:
            return item
    return options[-1][0]


def patrol_speed_scale(reader, cooked, body_class_path):
    """BodyClass.PatrolStance -> StanceTypeDefinition.SpeedScale (object dumps of Sanctuary_Combat)."""
    def dump(path):
        index = next(x['index'] for x in exports if x['path'] == path)
        run = subprocess.run([str(reader), str(cooked / f'{LEVEL}.upk'), '--object-dump', str(index), '4', '--cooked', str(cooked)],
                             capture_output=True, text=True, encoding='utf-8')
        if run.returncode:
            raise RuntimeError(f'{path}: {run.stderr.strip()[:160]}')
        return json.loads(run.stdout)['properties']
    exports = json.loads(subprocess.run([str(reader), str(cooked / f'{LEVEL}.upk'), '--exports'], capture_output=True, text=True,
                                        encoding='utf-8').stdout)
    body = dump(body_class_path)
    stance = dump(body['patrolstance'])
    return float(stance['speedscale'])


def ai_scripted_destination(level, point):
    """The first WillowSeqAct_AIScripted that the point's SeqEvent_PopulatedPoint reaches -> (destination node, action name)."""
    actions = {a['action']: a for a in level['kismet']['ai_scripted']}
    for event in level['kismet']['populated_point_events']:
        if event['point'] != point:
            continue
        stack = list(event['reaches'])
        while stack:
            step = stack.pop(0)
            if step['class'].endswith('WillowSeqAct_AIScripted'):
                action = actions.get(step['op'])
                if action and action['destinations']:
                    return action['destinations'][-1], action['action']   # the last Destination entry is the move target
            stack.extend(step.get('then', []))
    return None, None


MESH_KIND = {'Skel_GenericMale': 'CitizenMale', 'Skel_GenericFemale': 'CitizenFemale'}


def observed_spawns(args, nodes, perches, kinds):
    """Spawns copied from a real-game capture: one per live Sanctuary Citizen (see the module doc)."""
    t0 = json.loads(Path(args.observed).read_text(encoding='utf-8'))
    first, far = {}, {}
    for line in Path(args.samples).read_text(encoding='utf-8').splitlines():
        row = json.loads(line)
        for path, v in row['pawns'].items():
            first.setdefault(path, v)
            far[path] = max(far.get(path, 0.0), math.dist(v[:2], first[path][:2]))
    spawns, seen = [], set()
    for pawn in t0['pawns']:
        kind = MESH_KIND.get(pawn.get('mesh'))
        if pawn.get('name') != 'Sanctuary Citizen' or pawn['loc'] == [0.0, 0.0, 0.0] or kind not in kinds:
            continue
        loc = pawn['loc']
        nearest = sorted(nodes, key=lambda n: math.dist(n['location'][:2], loc[:2]))
        node = nearest[0]
        gap = math.dist(node['location'][:2], loc[:2])
        moved = far.get(pawn['path'], 0.0)
        on_perch = bool(node.get('perch')) and gap < 80.0
        if moved <= 150.0 and on_perch:
            mode = dict(wander=False, load_balanced=False, hold=False)
        elif moved <= 150.0:
            mode = dict(wander=False, load_balanced=False, hold=True)     # standing where the Kismet left it: no node to use
        else:
            mode = dict(wander=True, load_balanced=True, hold=False)
        spawn_loc, spawn_yaw, fixed_z = loc, (pawn['yaw'] or 0) * 360.0 / 65536.0, True
        if mode['wander']:
            fixed_z = False
        elif on_perch:
            # The game snaps an idle pawn onto its perch and the start clip's root motion then walks it to where it was found, so
            # the host starts at the node (x, y, yaw) and keeps the observed height.
            spawn_loc, spawn_yaw = [node['location'][0], node['location'][1], loc[2]], node['yaw']
        key = (kind, round(loc[0]), round(loc[1]))
        if key in seen:
            continue
        seen.add(key)
        spawns.append({'pawn_path': pawn['path'], 'id': f'obs{len(spawns):02d}_{kind}_{node["name"]}', 'den': 'observed in the real game', 'population': 'PopDef_NPC_*',
                       'kind': kind, 'location': spawn_loc, 'yaw': spawn_yaw, 'fixed_z': fixed_z, 'start_node': node['name'],
                       'route_source': f'real game t0: {"stood on the perch" if on_perch else "moved %.0f uu in the sample" % moved if moved > 150 else "stood"}',
                       **mode})
    return spawns


def showcase_for(spawns, nodes_by_name, stops=None):
    """The round-1 stops when given, else up to 6 idle pawns on different perch definitions; then one walker (found by the host)."""
    if stops:
        female = next((s['id'] for s in spawns if s['kind'] == 'CitizenFemale' and not s['wander'] and not s.get('hold') and s['id'] not in stops
                       and s['id'].startswith('obs')), None)
        return list(stops) + ([female] if female else []) + ['@walker']
    chosen, used = [], set()
    for s in spawns:
        node = nodes_by_name[s['start_node']]
        if not s['wander'] and not s.get('hold') and node.get('perch') and node['perch'] not in used:
            used.add(node['perch'])
            chosen.append(s['id'])
        if len(chosen) == 6:
            break
    return chosen + ['@walker']



HAIR_GAIN = 1.5     # UNVERIFIED stand-in: hair tint = the pawn's zone-A midtone times this (the original shades three colour zones from p_Masks)


def attachment_transforms(kind, bones):
    """Relative transform of each bone's attachment frame in the UE5 skeleton, per bone: {bone: ([x, y, z], [qx, qy, qz, qw])}.

    The static meshes are modelled in the UE3 bone frame; after UModel's glTF export and the UE5 import their vertices carry the same Y
    mirror C as the skeletal mesh (md5 -> UE5 is the mirror, checked by slice_npc_assets.convert_subset). So the world transform in UE5
    is C W C^-1 for the md5 bind world W of the bone, and relative to the imported bone's bind world T it is T^-1 C W C^-1.
    UNVERIFIED until looked at on a head."""
    import numpy as np
    from prepare_character_anims import MIRROR, matrix, to_quat, ue_matrix
    from prepare_character_pose import read_joints
    from prepare_sanctuary_pillar import numbers  # noqa: F401  (imported for read_joints' dependencies)
    md5 = next((ROOT / 'local/external/umodel/slice-npc' / f'Amb_{kind}').rglob('*.md5mesh'))
    joints = {j[0]: j for j in read_joints(md5.read_text(encoding='utf-8'))}
    ref = {b['name']: b for b in json.loads((ROOT / f'local/slice/ambient/ref_pose_{kind}.json').read_text(encoding='utf-8'))}
    c = MIRROR
    out = {}
    for bone in bones:
        _, _, position, orientation = joints[bone]
        w = matrix(position, orientation)
        t = ue_matrix(ref[bone]['loc'], ref[bone]['quat'])
        rel = np.linalg.inv(t) @ c @ w @ np.linalg.inv(c)
        out[bone] = ([round(float(v), 4) for v in rel[:3, 3]], [round(float(v), 6) for v in to_quat(rel[:3, :3])])
    return out


def rotator_matrix(pitch, yaw, roll):
    """UE rotator (65536 units per turn) to a column-convention rotation matrix: v' = R v (FRotationMatrix, transposed)."""
    import numpy as np
    p, y, r = (v * 2.0 * math.pi / 65536.0 for v in (pitch, yaw, roll))
    sp, cp, sy, cy, sr, cr = math.sin(p), math.cos(p), math.sin(y), math.cos(y), math.sin(r), math.cos(r)
    m = np.array([[cp * cy, cp * sy, sp],
                  [sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp],
                  [-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp]])
    return m.T


def component_transform(bone_rel, att):
    """Bone-relative placement of one attached static mesh: the bone attachment frame (bone_rel, from attachment_transforms) followed by
    the component's own Translation / Rotation / Scale x Scale3D (UE3, in the bone frame). The component transform is carried into the
    imported (Y-mirrored) frame by conjugation with the mirror; older captures without those fields keep the bone frame alone."""
    import numpy as np
    from prepare_character_anims import MIRROR, to_quat
    rel = np.eye(4)
    loc, quat = bone_rel
    x, y, z, w = quat
    rel[:3, :3] = [[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
                   [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
                   [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]]
    rel[:3, 3] = loc
    if 'comp_translation' not in att:
        return list(loc), list(quat), [1.0, 1.0, 1.0]
    t3 = np.eye(4)
    scale = [att['comp_scale'] * v for v in att['comp_scale3d']]
    t3[:3, :3] = rotator_matrix(*att['comp_rotation']) @ np.diag(scale)
    t3[:3, 3] = att['comp_translation']
    t5 = MIRROR @ t3 @ MIRROR
    total = rel @ t5
    columns = total[:3, :3]
    sizes = np.linalg.norm(columns, axis=0)
    rotation = columns / sizes
    return [round(float(v), 4) for v in total[:3, 3]], [round(float(v), 6) for v in to_quat(rotation)], [round(float(v), 5) for v in sizes]


def annotate_looks(spawns, compose_file, kinds, report):
    """Add head material and attachments (hair, hats, gear) to the observed spawns from an amb_compose capture."""
    compose = {row['path']: row for row in json.loads(Path(compose_file).read_text(encoding='utf-8'))}
    transforms = {kind: attachment_transforms(kind, {'Head', 'Jaw', 'Spine3'}) for kind in kinds}
    missing = 0
    for spawn in spawns:
        row = compose.get(spawn.get('pawn_path'))
        if not row:
            missing += 1
            continue
        head = row['materials'][0] if row['materials'] else None
        body = row['materials'][1] if len(row['materials']) > 1 else None
        if head and head.get('textures', {}).get('p_Diffuse'):
            key = f'{spawn["kind"]}_{head["textures"]["p_Diffuse"].rsplit(".", 1)[-1]}'
            if key in report['heads']:
                spawn['head_material'] = report['heads'][key]
                if report['zone'].get('head:' + key):
                    spawn['head_vectors'] = {k: v[:3] for k, v in (head.get('vectors') or {}).items()}
        if body and report.get('bodies', {}).get(spawn['kind']):
            spawn['body_material'] = report['bodies'][spawn['kind']]
            if report['zone'].get('body:' + spawn['kind']):
                spawn['body_vectors'] = {k: v[:3] for k, v in (body.get('vectors') or {}).items()}
        items = []
        for att in row['attachments']:
            mesh = att.get('static_mesh')
            info = report['meshes'].get(mesh.rsplit('.', 1)[-1]) if mesh else None
            if not info or att['bone'] not in transforms[spawn['kind']]:
                continue
            mat = None
            mats = [m for m in att.get('materials') or [] if m]
            if mats and mats[0].get('parent'):
                mat = report['materials'].get('MI_' + mats[0]['parent'].rsplit('.', 1)[-1])
            mat = mat or report['materials'].get(f'MI_{mesh.rsplit(".", 1)[-1]}_0')
            loc, quat, scale = component_transform(transforms[spawn['kind']][att['bone']], att)
            item = {'mesh': info['asset'], 'bone': att['bone'], 'location': loc, 'quat': quat, 'scale': scale, 'material': mat}
            zone = bool(mat) and report['zone'].get(mat.rsplit('/', 1)[-1], False)
            if zone and mats and mats[0].get('vectors'):
                item['vectors'] = {k: v[:3] for k, v in mats[0]['vectors'].items()}
            if not zone and 'Hair' in mesh and mats and mats[0].get('vectors', {}).get('p_AColorMidtone'):
                a = mats[0]['vectors']['p_AColorMidtone']
                item['tint'] = [round(min(1.0, v * HAIR_GAIN), 4) for v in a[:3]]
            items.append(item)
        spawn['attachments'] = items
    return missing


# The stops of the round-1 review (same perches, so frames compare): (perch node, kind). The population of a later real-game session
# does not repeat the earlier one, so when nobody stands on such a perch in the capture a pawn is added there at the pose the
# first capture found (--stops-from), with the look (head, hair, hats) of an observed pawn of that kind, taken in turn.
ROUND1_STOPS = [('Perch_66', 'CitizenMale'), ('Perch_186', 'CitizenMale'), ('Perch_115', 'CitizenMale'), ('Perch_166', 'CitizenFemale'),
                ('Perch_4', 'CitizenMale')]


def add_round1_stops(spawns, args, nodes_by_name):
    first = json.loads(Path(args.stops_from).read_text(encoding='utf-8'))
    added = []
    donors = {}
    for s in spawns:
        if 'head_material' in s:
            donors.setdefault(s['kind'], []).append(s)
    for perch, kind in ROUND1_STOPS:
        taken = next((s for s in spawns if s['start_node'] == perch and not s['wander'] and not s.get('hold') and s['kind'] == kind), None)
        if not taken and any(s['start_node'] == perch and not s['wander'] and not s.get('hold') for s in spawns):
            # Somebody of the other kind already stands here: that pawn is the stop (a second one would overlap it).
            taken = next(s for s in spawns if s['start_node'] == perch and not s['wander'] and not s.get('hold'))
        if taken:
            added.append(taken['id'])
            continue
        node = nodes_by_name[perch]
        live = [p for p in first['pawns'] if p.get('name') == 'Sanctuary Citizen' and p['loc'] != [0.0, 0.0, 0.0]
                and math.dist(p['loc'][:2], node['location'][:2]) < 60.0]
        z = live[0]['loc'][2] if live else node['location'][2] + 30.0
        donor = donors[kind][len(added) % len(donors[kind])]
        spawn = {'pawn_path': None, 'id': f'stop_{perch}', 'den': 'round-1 stop', 'population': 'PopDef_NPC_*', 'kind': kind,
                 'location': [node['location'][0], node['location'][1], z], 'yaw': node['yaw'], 'fixed_z': True, 'start_node': perch,
                 'route_source': f'round-1 stop: nobody on this perch in this capture; pose from the first capture, look borrowed from {donor["id"]}',
                 'wander': False, 'load_balanced': False, 'hold': False, 'head_material': donor.get('head_material'),
                 'attachments': donor.get('attachments', [])}
        spawns.append(spawn)
        added.append(spawn['id'])
    return added


def build(args):
    census = json.loads(CENSUS.read_text(encoding='utf-8'))
    assets = json.loads(ASSETS.read_text(encoding='utf-8'))
    level = census['levels'][LEVEL]
    nodes_in = level['nodes']
    game = Path(args.game)
    cooked = game / 'WillowGame/CookedPCConsole'
    kinds = {}
    for name, use in assets['use'].items():
        if use['import_status'] != 'imported' or not use['anims'].get('idle') or not use['anims'].get('walk'):
            continue
        ident = json.loads((ROOT / 'local/slice/ambient/identity.json').read_text(encoding='utf-8'))['npcs'][name]
        ai = ident['ai_class_properties']
        scale = patrol_speed_scale(args.reader, cooked, ident['pawn_properties']['bodyclass'])
        speed = float(ai['groundspeed']) * float(ai.get('walkingpct', 1)) * scale
        tracks = json.loads((ROOT / f'local/slice/ambient/anim_tracks_{name}.json').read_text(encoding='utf-8'))
        # Root-bone travel of each clip (last frame minus first, UE units, mesh frame): the stock perch clips walk the pawn onto its
        # wall or counter and back (BangOnWall start +25.8 forward). The host moves the actor by this when a clip ends, so the next
        # clip (whose root starts at zero) continues where the last one stopped.
        root_end = {}
        for role in use['anims']:
            root = (tracks.get(role) or {}).get('tracks', {}).get('Root')
            if root and root['pos']:
                a, b = root['pos'][0], root['pos'][-1]
                root_end[role] = [round(b[i] - a[i], 3) for i in range(3)]
        kinds[name] = {'root_end': root_end, 'display_name': ai['defaultdisplayname'], 'mesh': use['skeletal_mesh'], 'mesh_offset': use['mesh_offset'],
                       'speed': round(speed, 3), 'speed_note': f'GroundSpeed {ai["groundspeed"]} x WalkingPct {ai.get("walkingpct", 1)} x patrol SpeedScale {scale} (the product equals the Velocity read from one live walking citizen in the real game, 150; the native SetPawnMovementSpeed was not read)',
                       'yaw_rate': float(ai['rotationrate']['Yaw']) * 360.0 / 65536.0, 'clips': dict(use['anims'])}
    if not kinds:
        sys.exit('no imported ambient kind in ambient_assets.json (run tools/seed_ambient_npc_assets.ps1)')

    # Perch definitions the level's perches use, as clip roles every kind has.
    available = set.intersection(*[set(k['clips']) for k in kinds.values()])
    perches, dropped = {}, []
    for path, info in census['perch_defs'].items():
        entry = next((e for e in info.get('anim_map', []) if e['body_tag'].endswith('BodyTag_Human')), None)
        if not entry:
            dropped.append(path)
            continue
        def role(move):
            return 'perch_' + move['clip'].lower() if move and move.get('clip') else None
        idle_move = entry['IdleAnim']
        idle = [(role(m), m['weight']) for m in idle_move['random']] if idle_move and idle_move.get('random') else [(role(idle_move), 1.0)]
        roles = [role(entry['StartAnim']), role(entry['StopAnim'])] + [r for r, _ in idle]
        if any(r and r not in available for r in roles) or not idle or not idle[0][0]:
            dropped.append(path)
            continue
        loop = info.get('loop_time') or {'MinVal': 3, 'MaxVal': 5}
        perches[path.rsplit('.', 1)[-1]] = {'start': role(entry['StartAnim']) or '', 'stop': role(entry['StopAnim']) or '',
                                            'idle': [{'role': r, 'weight': w} for r, w in idle], 'loop_time': [loop['MinVal'], loop['MaxVal']],
                                            'lerp_time': info.get('lerp_time', 0.2), 'definition': path}

    # Spawns.
    spawns, notes = [], {'dens_seen': 0, 'dens_with_npc_population': 0, 'points_realised': 0, 'points_without_route': 0}
    for den in level['dens']:
        notes['dens_seen'] += 1
        pop = den['population_def']
        if not pop or not pop.startswith(POP_PREFIX):
            continue
        notes['dens_with_npc_population'] += 1
        factories = census['pop_defs'][pop]['factories']
        flags = {f.rsplit('.', 1)[-1] for f in factories[0].get('flags_to_set', [])}
        idle_flag, free_flag = 'Flag_IdleNPC' in flags, 'Flag_NPCDoNotThrottleMovement' in flags
        limit = int(den['max_active_normal'] or SPAWN_LIMIT_DEFAULT)
        points = [p for p in den['spawn_points'] if p]
        order = sorted(points, key=lambda p: seeded(den['den'] + p, 'shuffle'))
        taken = 0
        for point in order:
            if taken >= limit:
                break
            info = level['points'][point]
            start, source = None, None
            if info['destinations'] and info['destinations'][0] in nodes_in:
                start, source = info['destinations'][0], 'InitialActionDestinations'
            else:
                dest, action = ai_scripted_destination(level, point)
                if dest in nodes_in:
                    start, source = dest, f'Kismet {action}'
            if not start or not info['pose']:
                notes['points_without_route'] += 1
                continue
            options = [(KIND_OF_ARCHETYPE.get(f.get('pawn_archetype')), float((f.get('probability') or {}).get('BaseValueConstant', 1)))
                       for f in factories if KIND_OF_ARCHETYPE.get(f.get('pawn_archetype')) in kinds]
            if not options:
                continue
            kind = weighted_choice(options, point)
            loc, rot = info['pose']['location'], info['pose']['rotation']
            spawns.append({'id': f'{den["den"]}/{point}', 'den': den['den'], 'population': pop.rsplit('.', 1)[-1], 'kind': kind,
                           'location': loc, 'yaw': rot[1], 'start_node': start, 'route_source': source,
                           'wander': not idle_flag, 'load_balanced': (not idle_flag) and (not free_flag)})
            taken += 1
            notes['points_realised'] += 1

    # Node graph: every node of the level (the routes are reachable subsets of it).
    nodes = []
    for name, n in sorted(nodes_in.items()):
        is_perch = n['class'] == 'Perch'
        pd = n['perch_def'].rsplit('.', 1)[-1] if n.get('perch_def') else ''
        entry = {'name': name, 'location': n['pose']['location'], 'yaw': n['pose']['rotation'][1],
                 'arrival_radius': float(n['arrival_radius']) if n.get('arrival_radius') else (PERCH_RADIUS if is_perch else MOVE_NODE_RADIUS),
                 'face_node_direction': bool(n['face_node_direction']) if n.get('face_node_direction') is not None else is_perch,
                 'next': [{'node': e['node'], 'weight': e['weight']} for e in n['next'] if e['node'] in nodes_in]}
        if pd and pd in perches:
            entry['perch'] = pd
        if n.get('loop_time_override'):
            entry['loop_override'] = n['loop_time_override']
        nodes.append(entry)
    if args.observed:
        spawns = observed_spawns(args, nodes, perches, kinds)
        if args.compose:
            report = json.loads((ROOT / 'local/slice/ambient/editor_report_attach.json').read_text(encoding='utf-8'))
            notes['compose_missing'] = annotate_looks(spawns, args.compose, kinds, report)
            if args.stops_from:
                notes['round1_stops'] = add_round1_stops(spawns, args, {n['name']: n for n in nodes})
        notes['points_realised'] = len(spawns)
    names = {n['name'] for n in nodes}
    missing = [s['id'] for s in spawns if s['start_node'] not in names]
    if missing:
        sys.exit(f'spawns with a start node outside the manifest: {missing[:5]}')

    result = {'schema': 'ow-ambient-world-v1', 'tool': 'tools/prepare_ambient_world.py', 'generated': datetime.datetime.now().astimezone().isoformat(timespec='seconds'),
              'level': LEVEL, 'kinds': kinds, 'perches': perches, 'perch_definitions_dropped': dropped, 'nodes': nodes, 'spawns': spawns,
              'views': views_for(spawns, {n['name']: n for n in nodes}), 'showcase': showcase_for(spawns, {n['name']: n for n in nodes}, notes.get('round1_stops')),
              'counts': notes, 'observed': bool(args.observed)}
    return result


def views_for(spawns, nodes):
    """Capture viewpoints: in front of a few perch groups and beside the loop walkers. First guesses; refined from the frames."""
    views = []
    wander = [s for s in spawns if s['wander']]
    idle = [s for s in spawns if not s['wander']]

    def add(name, anchor, yaw_deg, distance=420.0, height=70.0):
        yaw = math.radians(yaw_deg)
        cam = [anchor[0] + math.cos(yaw) * distance, anchor[1] + math.sin(yaw) * distance, anchor[2] + height]
        views.append({'name': name, 'camera': cam, 'look': [anchor[0], anchor[1], anchor[2] + 80.0]})

    # Densest idle clusters.
    chosen = []
    for s in sorted(idle, key=lambda s: -sum(1 for t in idle if math.dist(t['location'][:2], s['location'][:2]) < 700)):
        if all(math.dist(s['location'][:2], c['location'][:2]) > 1500 for c in chosen):
            chosen.append(s)
        if len(chosen) == 3:
            break
    for i, s in enumerate(chosen):
        node = nodes[s['start_node']]
        add(f'idle{i + 1}', node['location'], node['yaw'])
    # Walk views: the nodes most walkers pass (following the first NextNode up to 80 steps), camera 350 to the side of the segment.
    usage = {}
    for s in wander:
        seen, name = set(), s['start_node']
        for _ in range(80):
            if name in seen or not nodes[name]['next']:
                break
            seen.add(name)
            name = nodes[name]['next'][0]['node']
        for name in seen:
            usage[name] = usage.get(name, 0) + 1
    picked = []
    for name, count in sorted(usage.items(), key=lambda kv: (-kv[1], kv[0])):
        node = nodes[name]
        if node['perch'] if 'perch' in node else False:
            continue
        if all(math.dist(node['location'][:2], nodes[o]['location'][:2]) > 1500 for o in picked):
            picked.append(name)
        if len(picked) == 3:
            break
    for i, name in enumerate(picked):
        node = nodes[name]
        nxt = nodes[node['next'][0]['node']]['location']
        heading = math.degrees(math.atan2(nxt[1] - node['location'][1], nxt[0] - node['location'][0]))
        add(f'walk{i + 1}', node['location'], heading + 90.0, distance=350.0)
    return views


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--game', default=os.environ.get('OPENWILLOW_BL2'))
    ap.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    ap.add_argument('--observed', help='real-game dump (ambient_npcs.py amb_dump) whose live citizens become the spawns')
    ap.add_argument('--samples', help='real-game position samples (ambient_npcs.py amb_sample) used with --observed')
    ap.add_argument('--compose', help='real-game amb_compose output (hair, hats, heads of the observed pawns); needs the attach step')
    ap.add_argument('--stops-from', help='an earlier real-game dump: pose of pawns added at the round-1 review perches')
    ap.add_argument('--output', default=str(OUT))
    args = ap.parse_args()
    if not args.game:
        sys.exit('Pass --game or set OPENWILLOW_BL2')
    out = Path(args.output).resolve()
    if not out.is_relative_to(ROOT / 'local'):
        sys.exit('Output must stay under the repository local/')
    if bool(args.observed) != bool(args.samples):
        sys.exit('--observed and --samples go together')
    result = build(args)
    out.write_text(json.dumps(result, indent=1), encoding='utf-8')
    kinds = {}
    for s in result['spawns']:
        kinds[s['kind']] = kinds.get(s['kind'], 0) + 1
    print('wrote', out, '| spawns', len(result['spawns']), kinds, '| wanderers', sum(1 for s in result['spawns'] if s['wander']),
          '| nodes', len(result['nodes']), '| perches', len(result['perches']), '| dropped perch defs', len(result['perch_definitions_dropped']),
          '| views', len(result['views']), '|', result['counts'])


if __name__ == '__main__':
    main()
