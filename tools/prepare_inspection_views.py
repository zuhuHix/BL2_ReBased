"""Prepare bounded, repeatable host inspection viewpoints.

The input and output are local scene evidence.  This helper does not infer
original-game camera poses: it preserves the supplied viewpoints and, for the
known obstructed terrain investigation, adds host-side candidate poses that
look back at a recorded terrain stand point.  The UE test may select the first
candidate whose target trace reaches the requested terrain actor.
"""
import argparse
import copy
import json
import math
from pathlib import Path


MAX_VIEWS = 12
OBSTRUCTED_VIEW_TOKEN = 'Terrain_10'
OBSTRUCTION_POLICY = 'target_trace_candidate_v1'
# Camera offsets are deliberately broad and symmetric.  They are inspection
# candidates, not recovered original-game coordinates.
OBSTRUCTION_OFFSETS = (
    (-2400.0, -1800.0, 2200.0),
    (2200.0, -1800.0, 2200.0),
    (-2400.0, 1800.0, 2200.0),
    (2200.0, 1800.0, 2200.0),
)


def _finite_vector(value, name):
    if not isinstance(value, list) or len(value) != 3:
        raise ValueError(f'{name} must contain three numbers')
    result = [float(item) for item in value]
    if not all(math.isfinite(item) for item in result):
        raise ValueError(f'{name} must be finite')
    return result


def look_at(location, target):
    """Return UE pitch/yaw/roll degrees for a camera aimed at ``target``."""
    delta = [b - a for a, b in zip(location, target)]
    horizontal = math.hypot(delta[0], delta[1])
    if horizontal <= 1e-6 and abs(delta[2]) <= 1e-6:
        raise ValueError('camera target must differ from camera location')
    return [math.degrees(math.atan2(delta[2], horizontal)),
            math.degrees(math.atan2(delta[1], delta[0])), 0.0]


def _pose(row):
    location = _finite_vector(row.get('location'), 'location')
    rotation = _finite_vector(row.get('rotation'), 'rotation')
    fov = float(row.get('fov', 75))
    if not math.isfinite(fov) or not 10 <= fov <= 150:
        raise ValueError('fov must be finite and in 10..150')
    return {'location': location, 'rotation': rotation, 'fov': fov}


def _terrain_target(runtime, token):
    for probe in runtime.get('probes', []):
        source = str(probe.get('source', ''))
        if token not in source:
            continue
        stands = probe.get('stands') or ([probe['stand']] if probe.get('stand') else [])
        if not stands:
            continue
        stand = stands[0]
        point = _finite_vector(stand.get('point'), 'terrain stand point')
        surface = stand.get('surface')
        if not isinstance(surface, list) or len(surface) != 2:
            raise ValueError(f'{source} stand surface is missing')
        # Aim at the recorded surface band, not the elevated teleport point.
        surface = [float(value) for value in surface]
        if not all(math.isfinite(value) for value in surface):
            raise ValueError(f'{source} stand surface is not finite')
        return [point[0], point[1], max(surface)]
    raise ValueError(f'no terrain runtime stand matched {token!r}')


def repair_obstructed_view(row, runtime):
    """Add visibility candidates for one known host-obstructed terrain view."""
    result = copy.deepcopy(row)
    if OBSTRUCTED_VIEW_TOKEN not in str(result.get('name', '')):
        return result
    base = _pose(result)
    target = _terrain_target(runtime, OBSTRUCTED_VIEW_TOKEN)
    candidates = [base]
    for dx, dy, dz in OBSTRUCTION_OFFSETS:
        location = [target[0] + dx, target[1] + dy, target[2] + dz]
        candidates.append({'location': location, 'rotation': look_at(location, target),
                           'fov': base['fov']})
    result.update(base)
    result['target'] = target
    result['target_actor_contains'] = OBSTRUCTED_VIEW_TOKEN
    result['candidate_policy'] = OBSTRUCTION_POLICY
    result['candidates'] = candidates
    return result


def prepare_views(source, runtime=None, repair_obstructed=False):
    if not isinstance(source, dict) or not isinstance(source.get('views'), list):
        raise ValueError('inspection input requires a views array')
    rows = source['views']
    if not 1 <= len(rows) <= MAX_VIEWS:
        raise ValueError(f'inspection requires 1..{MAX_VIEWS} views')
    output = copy.deepcopy(source)
    prepared = []
    for row in rows:
        if not isinstance(row, dict):
            raise ValueError('inspection view must be an object')
        normalized = dict(row)
        normalized.update(_pose(row))
        if repair_obstructed:
            if runtime is None:
                raise ValueError('terrain runtime evidence is required for repair')
            normalized = repair_obstructed_view(normalized, runtime)
        prepared.append(normalized)
    output['views'] = prepared
    if repair_obstructed:
        output['policy'] = OBSTRUCTION_POLICY
        output['unverified'] = [
            'candidate selection checks host visibility only',
            'original-game camera and visual parity',
        ]
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scene', type=Path, required=True,
                        help='prepared local scene containing terrain-runtime.json')
    parser.add_argument('--input', type=Path,
                        help='existing inspection JSON; defaults to <scene>/inspection-views.json')
    parser.add_argument('--output', type=Path,
                        help='output JSON; defaults to <scene>/inspection-views.json')
    parser.add_argument('--repair-obstructed', action='store_true',
                        help='add host visibility candidates for the Terrain_10 view')
    args = parser.parse_args()
    source_path = args.input or (args.scene / 'inspection-views.json')
    output_path = args.output or (args.scene / 'inspection-views.json')
    try:
        source = json.loads(source_path.read_text(encoding='utf-8'))
        runtime = None
        if args.repair_obstructed:
            runtime = json.loads((args.scene / 'terrain-runtime.json').read_text(encoding='utf-8'))
        prepared = prepare_views(source, runtime, args.repair_obstructed)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(json.dumps(prepared, indent=2) + '\n', encoding='utf-8')
    except (OSError, TypeError, ValueError, json.JSONDecodeError) as error:
        parser.exit(1, f'inspection views: {error}\n')
    print(json.dumps({'output': str(output_path), 'views': len(prepared['views']),
                      'policy': prepared.get('policy', 'baseline')}))


if __name__ == '__main__':
    main()
