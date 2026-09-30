"""Prepare one local UE paint input from an existing recipe/UModel export.

Uses the same explicitly approximate Master_Gun interpretation as the local
thumbnail renderer. Output is game-derived and must remain under local/.
"""
import argparse
import json
from pathlib import Path
import re
from render_weapon_previews import resolve_material, weapon_class, DETAIL_CHANNELS


def prepare(recipe_path, materials):
    if not re.fullmatch(r'[A-Za-z0-9_]+', recipe_path.stem):
        raise ValueError('Invalid recipe ID')
    recipe = json.loads(recipe_path.read_text())
    identity = recipe['material']
    resolved = resolve_material(identity.split('.')[-1], [materials])
    if resolved is None:
        raise ValueError(f'No local export of {identity}')
    params, texture_dir, chain = resolved
    kind = weapon_class(recipe, params)
    channel = DETAIL_CHANNELS[(params['texture']['p_Diffuse'], kind)]
    paths = {}
    for name in ['p_Masks', 'p_Diffuse', 'p_NormalScopesEmissive', 'p_Pattern']:
        path = texture_dir / (params['texture'][name] + '.png')
        if not path.is_file():
            raise ValueError(f'Missing {path}')
        paths[name] = str(path.resolve())
    for zone in 'ABC':
        for tone in ['Shadow', 'Midtone', 'Hilight']:
            if f'p_{zone}Color{tone}' not in params['vector']:
                raise ValueError('Incomplete palette')
    return {'recipe_id': recipe_path.stem, 'material_identity': identity,
            'parent_chain': chain, 'weapon_class': kind, 'detail_channel': channel,
            'params': params, 'textures': paths, 'shader_verified': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--recipe', type=Path, required=True)
    parser.add_argument('--materials', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    local = Path(__file__).resolve().parents[1] / 'local'
    if not args.output.resolve().is_relative_to(local.resolve()):
        parser.error('Output must remain under local/')
    result = prepare(args.recipe, args.materials)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2))
    print(f"{result['recipe_id']}: {len(result['parent_chain'])} MICs, four textures; shader UNVERIFIED")
