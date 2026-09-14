"""Add observed body collision to an existing local scene without reimporting art."""
import argparse
import json
from pathlib import Path
from prepare_level import Scene, props
from collision_geometry import hulls


def refresh(reader, game, root):
    scene = Scene(reader, game, root)
    filename = root / 'scene.json'
    manifest = json.loads(filename.read_text(encoding='utf-8'))
    for level in manifest['levels']:
        scene.load(level)
    counts = {'supported': 0, 'absent': 0, 'unsupported': 0, 'hulls': 0}
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
                collision = {'status': 'supported', 'source': scene.identity(body),
                             'hulls': hulls(record['data']['properties'])}
            except (ValueError, KeyError) as error:
                collision = {'status': 'unsupported', 'hulls': [], 'reason': str(error)}
        mesh['collision'] = collision
        counts[collision['status']] += 1
        counts['hulls'] += len(collision['hulls'])
    for actor in manifest['actors']:
        matches = [r for r in scene.load(actor['level']).values() if r['path'] == actor['source']]
        if len(matches) != 1:
            raise ValueError('Ambiguous actor: ' + actor['source'])
        p = props(matches[0])
        actor['collision_enabled'] = p.get('BlockActors', True) and p.get('CollideActors', True)
    manifest['collision_policy'] = 'observed_convex_and_box_v1'
    tmp = filename.with_suffix('.json.tmp')
    tmp.write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    tmp.replace(filename)
    (root / 'collision-report.json').write_text(json.dumps(counts, indent=2) + '\n')
    print(json.dumps(counts), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--scene', type=Path, required=True)
    args = parser.parse_args()
    refresh(args.reader, args.game, args.scene.resolve())
