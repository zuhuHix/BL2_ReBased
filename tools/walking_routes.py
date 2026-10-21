"""Walking route probes: stair candidates from a prepared scene, ground truth from the running game.

1. `candidates` reads local/<map>/scene.json and proposes one straight probe per placed
   stair ramp (a mesh whose only collision is one convex hull, sloping along one of its
   local horizontal axes): start 80 cm before the low edge, end 80 cm past the high edge.
   These are proposals only; nothing here says the original game can walk them.
2. tools/real_game/scripts/walking.py walks each probe in the running game with a
   steering autopilot and writes one JSON-lines trace per probe (game data, local/ only).
3. `routes` turns those traces into local/<map>/walk-routes.json for the host test
   OpenWillow.RouteWalking: the waypoints, whether the original game reached the end,
   its final position and the lowest and highest feet height it passed through.

The host test then asks one question per probe: does the host walker, steered the same
way, reach the same end without falling below the original game's lowest point?
"""
import argparse
import json
import math
from pathlib import Path

STAIR_MESHES = ('Prop_Stairs.Meshes.Sanc_Stairs', 'Prop_Stairs.StairMetal320')
APPROACH = 80.0      # cm before and after the ramp
STAND_HEIGHT = 100.0  # start/end points are given this far above the ramp surface
END_RADIUS = 40.0    # 2D distance that counts as reaching a waypoint


def rotation_axes(pitch, yaw, roll):
    """X, Y, Z axes of a UE rotator given in degrees (FRotationMatrix convention)."""
    p, y, r = (math.radians(v) for v in (pitch, yaw, roll))
    sp, cp, sy, cy, sr, cr = math.sin(p), math.cos(p), math.sin(y), math.cos(y), math.sin(r), math.cos(r)
    return ([cp * cy, cp * sy, sp],
            [sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp],
            [-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp])


def apply_pose(pose, v):
    axes = rotation_axes(*pose['rotation'])
    s = [v[i] * pose['scale'][i] for i in range(3)]
    return [pose['location'][k] + sum(s[j] * axes[j][k] for j in range(3)) for k in range(3)]


def placement_point(transform, v):
    """A mesh-local point in world space, using the scene's placement conventions."""
    if 'matrix' in transform:
        m, s = transform['matrix'], transform['scale']
        rows = [m[0:3], m[4:7], m[8:11]]
        return [m[12 + k] + sum(v[j] * s[j] * rows[j][k] for j in range(3)) for k in range(3)]
    return apply_pose(transform['actor'], apply_pose(transform['component'], v))


def placement_axis(transform, axis):
    """World direction of a mesh-local unit axis (0=X, 1=Y), horizontal and normalised."""
    o = placement_point(transform, [0, 0, 0])
    e = placement_point(transform, [1 if axis == 0 else 0, 1 if axis == 1 else 0, 0])
    d = [e[0] - o[0], e[1] - o[1]]
    n = math.hypot(*d)
    return [d[0] / n, d[1] / n] if n > 1e-6 else None


def ramp_probe(vertices, transform):
    """Straight probe across one ramp hull, or None if no single horizontal slope axis exists."""
    world = [placement_point(transform, v) for v in vertices]
    best = None
    for axis in (0, 1):
        d = placement_axis(transform, axis)
        if d is None:
            continue
        proj = [w[0] * d[0] + w[1] * d[1] for w in world]
        lo, hi = min(proj), max(proj)
        if hi - lo < 50:
            continue
        ends = []
        for target in (lo, hi):
            group = [w for w, p in zip(world, proj) if abs(p - target) < 5]
            ends.append((max(w[2] for w in group), group))
        rise = ends[1][0] - ends[0][0]
        if best is None or abs(rise) > abs(best[0]):
            best = (rise, d, ends, lo, hi)
    if best is None or abs(best[0]) < 20:
        return None
    rise, d, ends, lo, hi = best
    if rise < 0:
        d, ends = [-d[0], -d[1]], ends[::-1]
    points = []
    for (z, group), sign in zip(ends, (-1, 1)):
        cx = sum(w[0] for w in group) / len(group)
        cy = sum(w[1] for w in group) / len(group)
        points.append([round(cx + sign * APPROACH * d[0], 1), round(cy + sign * APPROACH * d[1], 1),
                       round(z + STAND_HEIGHT, 1)])
    run = hi - lo
    return {'points': points, 'rise': round(abs(rise), 1), 'run': round(run, 1),
            'slope_degrees': round(math.degrees(math.atan2(abs(rise), run)), 1)}


def candidates(scene):
    result = []
    for actor in scene['actors']:
        mesh = scene['meshes'][actor['mesh']]
        identity = mesh['source'].split(':', 1)[-1]
        hulls = mesh.get('collision', {}).get('hulls', [])
        if identity not in STAIR_MESHES or len(hulls) != 1 or not actor.get('collision_enabled'):
            continue
        probe = ramp_probe(hulls[0]['vertices'], actor['transform'])
        if probe:
            result.append(dict(probe, name='stair_%02d' % len(result), mesh=identity,
                               source=actor['level'] + ':' + actor['source']))
    return result


WALKING, FALLING = ('1', 'PHYS_Walking'), ('2', 'PHYS_Falling')  # the SDK reports the enum as its number


def ground(row, radius):
    """Height of the surface under the pawn's centre.

    A UE3 player cylinder standing on a slope rests on its uphill rim, so its feet are about
    radius * tan(slope) above the surface under its centre (observed on the Sanctuary ramps:
    feet 35-42 cm above the ramp hull at 33 degrees, radius 42). A UE5 capsule rests differently,
    so routes are compared on this ground height, not on feet height. Falling rows use the feet.
    """
    floor = row.get('floor')
    if row.get('phys') not in WALKING or not floor or not 0 < floor[2] <= 1:
        return row['feet']
    return row['feet'] - radius * math.tan(math.acos(floor[2]))


def summarize(rows, points, radius=42.0):
    """Ground truth from one live trace (rows of {'t', 'p', 'feet', 'phys', 'floor'})."""
    if not rows:
        raise ValueError('Empty trace')
    last = rows[-1]['p']
    reached = math.hypot(last[0] - points[-1][0], last[1] - points[-1][1]) <= END_RADIUS
    # The capture moves the pawn by writing its Location; the running game left it hovering at
    # the start until it moved, then it dropped. Heights count from its first landing after
    # such an initial drop, and the host starts counting at the same distance along the route.
    landing = 0
    falls = [i for i, r in enumerate(rows) if r.get('phys') in FALLING and r['t'] - rows[0]['t'] < 1.0]
    if falls:
        landing = next((i for i in range(falls[0], len(rows)) if rows[i].get('phys') in WALKING), len(rows) - 1)
    walked = rows[landing:]
    feet = [r['feet'] for r in walked]
    heights = [ground(r, radius) for r in walked]
    start = rows[landing]['p']
    return {'reached': reached, 'final': last, 'final_feet': rows[-1]['feet'],
            'min_feet': min(feet), 'max_feet': max(feet),
            'landing_distance': round(math.hypot(start[0] - points[0][0], start[1] - points[0][1]), 1),
            'final_ground': round(heights[-1], 2), 'min_ground': round(min(heights), 2),
            'duration': round(rows[-1]['t'] - rows[0]['t'], 3),
            'falling_samples': sum(1 for r in rows if r.get('phys') in FALLING)}


def routes(probes, trace_dir, capture):
    radius = float(capture.get('cylinder', {}).get('radius', 42.0))
    result, missing = [], []
    for probe in probes:
        trace = Path(trace_dir) / (probe['name'] + '.jsonl')
        rows = ([json.loads(line) for line in trace.read_text(encoding='utf-8').splitlines() if line.strip()]
                if trace.is_file() else [])
        if len(probe['points']) < 2 or not rows:
            missing.append(probe['name'])
            continue
        result.append(dict(probe, original=summarize(rows, probe['points'], radius)))
    # An interrupted capture must not produce a smaller manifest that still passes.
    if missing:
        raise ValueError('Probes without a usable trace: ' + ', '.join(missing))
    return {'schema': 1, 'capture': capture, 'end_radius': END_RADIUS, 'expected_routes': len(result),
            'routes': result}


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    sub = parser.add_subparsers(dest='command', required=True)
    c = sub.add_parser('candidates')
    c.add_argument('--scene', type=Path, required=True)
    c.add_argument('--output', type=Path, required=True)
    r = sub.add_parser('routes')
    r.add_argument('--probes', type=Path, required=True)
    r.add_argument('--traces', type=Path, required=True)
    r.add_argument('--capture', type=Path, required=True, help='walk_info JSON from the same session')
    r.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.command == 'candidates':
        scene = json.loads((args.scene / 'scene.json').read_text(encoding='utf-8'))
        probes = candidates(scene)
        args.output.write_text(json.dumps(probes, indent=1) + '\n', encoding='utf-8')
        print(json.dumps({'candidates': len(probes)}))
    else:
        probes = json.loads(args.probes.read_text(encoding='utf-8'))
        capture = json.loads(args.capture.read_text(encoding='utf-8'))
        data = routes(probes, args.traces, capture.get('info', capture))  # walk_dump holds {'info', 'volumes'}
        args.output.write_text(json.dumps(data, indent=1) + '\n', encoding='utf-8')
        print(json.dumps({'routes': len(data['routes']),
                          'reached': sum(r['original']['reached'] for r in data['routes'])}))


if __name__ == '__main__':
    main()
