"""Add observed body collision to an existing local scene without reimporting art."""
import argparse
import json
from pathlib import Path
from prepare_level import Scene, placement_collision, props
from collision_geometry import shapes
from collision_volumes import apply_live_state, collect as collect_volumes, live_observations


def refresh(reader, game, root, live_volumes=None):
    scene = Scene(reader, game, root)
    filename = root / 'scene.json'
    manifest = json.loads(filename.read_text(encoding='utf-8'))
    for level in manifest['levels']:
        scene.load(level)
    counts = {'supported': 0, 'absent': 0, 'unsupported': 0, 'hulls': 0, 'spheres': 0, 'capsules': 0}
    for name, mesh in manifest['meshes'].items():
        package, path = mesh['source'].split(':', 1) if ':' in mesh['source'] else (None, None)
        # identity() uses Package:path; require a unique StaticMesh export.
        if package is None:
            raise ValueError('Unsupported mesh identity: ' + mesh['source'])
        matches = [r for r in scene.load(package).values()
                   if r['path'] == path and r['class'].rsplit('.', 1)[-1] == 'StaticMesh']
        if len(matches) != 1:
            raise ValueError('Ambiguous mesh: ' + mesh['source'])
        probe = root / (name + '_collision_probe.obj')
        data = scene.call(package, '--mesh', matches[0]['index'], '--property-offset', 4, '--output', probe)
        # Refuse to attach collision to changed geometry from another installation.
        original = root / (name + '.obj')
        if not original.is_file() or original.read_bytes() != probe.read_bytes():
            raise ValueError('Mesh provenance mismatch: ' + mesh['source'])
        probe.unlink()
        body = scene.resolve(package, data['body_setup'])
        collision = {'status': 'absent', 'hulls': []}
        if body:
            try:
                record = scene.load(body[0])[body[1]]
                if record['class'] != 'Engine.RB_BodySetup' or 'error' in record:
                    raise ValueError('Invalid body: ' + str(record.get('error')))
                collision = dict(shapes(record['data']['properties']), status='supported',
                                 source=scene.identity(body))
            except (ValueError, KeyError) as error:
                collision = {'status': 'unsupported', 'hulls': [], 'reason': str(error)}
        # Render-triangle floors chosen by prepare_level stay as they are.
        if mesh.get('collision', {}).get('status') == 'triangle_mesh' and collision['status'] == 'absent':
            collision = mesh['collision']
        mesh['collision'] = collision
        counts[collision['status']] = counts.get(collision['status'], 0) + 1
        for kind in ('hulls', 'spheres', 'capsules'):
            counts[kind] += len(collision.get(kind, []))
    for actor in manifest['actors']:
        matches = [r for r in scene.load(actor['level']).values() if r['path'] == actor['source']]
        if len(matches) != 1:
            raise ValueError('Ambiguous actor: ' + actor['source'])
        # Same rule as prepare_level, including its observed per-placement exceptions.
        actor['collision_enabled'] = placement_collision(actor['level'], actor['source'], props(matches[0]))
    manifest['collision_policy'] = 'observed_convex_and_box_v1'
    volumes, _ = collect_volumes(scene, manifest['levels'])
    if live_volumes:
        apply_live_state(volumes, live_observations(json.loads(Path(live_volumes).read_text(encoding='utf-8'))))
    manifest['volumes'] = volumes
    manifest['volume_policy'] = 'observed_blocking_volumes_v1'
    counts['volumes'] = {s: sum(v['status'] == s for v in volumes)
                         for s in ('blocking', 'dynamic_unobserved', 'non_blocking', 'live_mismatch', 'unsupported')}
    counts['volumes']['blocking_hulls'] = sum(len(v['hulls']) for v in volumes if v['status'] == 'blocking')
    tmp = filename.with_suffix('.json.tmp')
    tmp.write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    tmp.replace(filename)
    (root / 'collision-report.json').write_text(json.dumps(counts, indent=2) + '\n')
    write_volume_runtime(root, volumes)
    print(json.dumps(counts), flush=True)


def write_volume_runtime(root, volumes):
    """volumes-runtime.json for the host's AOpenWillowVolumes: blocking volumes only."""
    runtime = [{'source': v['level'] + ':' + v['source'], 'location': v['location'],
                'hulls': [h['vertices'] for h in v['hulls']],
                # The BrushComponent's BlockZeroExtent as observed live; False until observed (UNVERIFIED).
                'block_zero_extent': bool(v.get('live_state', {}).get('block_zero_extent', False))}
               for v in volumes if v['status'] == 'blocking']
    (root / 'volumes-runtime.json').write_text(json.dumps({'schema': 1, 'volumes': runtime}, indent=1) + '\n',
                                                encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--scene', type=Path, required=True)
    parser.add_argument('--live-volumes', type=Path,
                        help='walk_dump JSON from tools/real_game/scripts/walking.py (local/realgame/walking/)')
    args = parser.parse_args()
    refresh(args.reader, args.game, args.scene.resolve(), args.live_volumes)
