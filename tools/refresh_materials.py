"""Refresh Material v1 definitions in an existing local scene without re-extracting meshes."""
import argparse
import json
from pathlib import Path
from prepare_level import (Scene, apply_outer_shell_policy, hidden_visual_mesh, material_index,
                           native_skybox_placement)


def restore_sky_policy(manifest):
    """Reapply the placement-derived interior policy after replacing materials."""
    for actor in manifest['actors']:
        mesh = manifest['meshes'][actor['mesh']]
        effective = [actor['materials'][s['slot']]
                     if s['slot'] < len(actor['materials']) and actor['materials'][s['slot']]
                     else s['material'] for s in mesh['sections']]
        if native_skybox_placement(mesh['source'], effective, manifest['materials']):
            for name in effective:
                manifest['materials'][name]['two_sided'] = True


def restore_hidden_visual_policy(manifest):
    """Re-evaluate the render-only hide rule against the refreshed materials."""
    for actor in manifest['actors']:
        mesh = manifest['meshes'][actor['mesh']]
        effective = [actor['materials'][s['slot']]
                     if s['slot'] < len(actor['materials']) and actor['materials'][s['slot']]
                     else s['material'] for s in mesh['sections']]
        actor['hidden_visual'] = hidden_visual_mesh(
            mesh['source'], effective, manifest['materials'], actor['source'])


def restore_outer_shell_policy(manifest, enabled):
    """Re-apply (or withdraw) the opt-in outer-hull override policy."""
    for actor in manifest['actors']:
        mesh = manifest['meshes'][actor['mesh']]
        apply_outer_shell_policy(actor, mesh['source'], mesh['sections'], manifest['materials'], enabled)
    manifest['outer_shell_policy'] = ('mesh_default_for_teleported_overrides_v1'
                                      if enabled else 'placed_overrides')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--scene', type=Path, required=True)
    parser.add_argument('--reuse-textures', action='store_true',
                        help='Reuse existing PNGs; use only with the same unchanged game install')
    parser.add_argument('--outer-shell', action='store_true',
                        help='Render the observed outer hull meshes with their mesh-default materials '
                             'instead of the unrecoverable masked _Teleported overrides')
    args = parser.parse_args()
    if not (args.game / 'Binaries/Win32/Borderlands2.exe').is_file():
        parser.error('An installed Borderlands 2 is required')
    filename = args.scene / 'scene.json'
    manifest = json.loads(filename.read_text(encoding='utf-8'))
    if manifest['schema'] != 1 or manifest['dynamic_policy'] != 'frozen':
        parser.error('Unsupported scene schema/policy')
    scene = Scene(args.reader, args.game, args.scene,
                  include_dlc=manifest.get('package_scope') == 'base_and_dlc')
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
    restore_sky_policy(manifest)
    restore_outer_shell_policy(manifest, args.outer_shell)
    restore_hidden_visual_policy(manifest)
    manifest['issues'] = [issue for issue in manifest['issues']
                          if not any(issue['object'] == source or issue['object'].startswith(source + ':')
                                     for source in sources)] + scene.issues
    manifest['visual_validation'] = 'pending'
    temporary = filename.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    temporary.replace(filename)
    print(json.dumps({'materials': len(scene.materials), 'issues': len(manifest['issues']),
                      'inferred_diffuse': sum('diffuse_inference' in m for m in scene.materials.values()),
                      'sky_approximations': sum('sky_approximation' in m for m in scene.materials.values()),
                      'outer_shell_replaced': sum(len(a.get('outer_shell_replaced', [])) for a in manifest['actors']),
                      'hidden_visual': sum(1 for a in manifest['actors'] if a.get('hidden_visual'))}))


if __name__ == '__main__':
    main()
