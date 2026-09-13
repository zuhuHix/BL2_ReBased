"""Refresh Material v1 definitions in an existing local scene without re-extracting meshes."""
import argparse
import json
from pathlib import Path
from prepare_level import Scene, material_index


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--scene', type=Path, required=True)
    parser.add_argument('--reuse-textures', action='store_true',
                        help='Reuse existing PNGs; use only with the same unchanged game install')
    args = parser.parse_args()
    if not (args.game / 'Binaries/Win32/Borderlands2.exe').is_file():
        parser.error('An installed Borderlands 2 is required')
    filename = args.scene / 'scene.json'
    manifest = json.loads(filename.read_text(encoding='utf-8'))
    if manifest['schema'] != 1 or manifest['dynamic_policy'] != 'frozen':
        parser.error('Unsupported scene schema/policy')
    scene = Scene(args.reader, args.game, args.scene)
    if args.reuse_textures:
        decode = scene.texture

        def texture(key, channel):
            if key is not None:
                cached = scene.filename(key, '_' + channel + '.png')
                if (scene.output / cached).is_file():
                    return cached
            return decode(key, channel)

        scene.texture = texture
    sources = {m['source'] for m in manifest['materials'].values()}
    for number, (name, material) in enumerate(manifest['materials'].items(), 1):
        package, path = material['source'].split(':', 1)
        index = material_index(scene.load(package), path)
        if scene.material((package, index)) != name:
            raise ValueError('Material identity changed: ' + material['source'])
        if number % 25 == 0:
            print(f'Refreshed {number}/{len(manifest["materials"])} materials', flush=True)
    manifest['materials'] = scene.materials
    manifest['issues'] = [issue for issue in manifest['issues']
                          if not any(issue['object'] == source or issue['object'].startswith(source + ':')
                                     for source in sources)] + scene.issues
    manifest['visual_validation'] = 'pending'
    temporary = filename.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    temporary.replace(filename)
    print(json.dumps({'materials': len(scene.materials), 'issues': len(manifest['issues']),
                      'inferred_diffuse': sum('diffuse_inference' in m for m in scene.materials.values())}))


if __name__ == '__main__':
    main()
