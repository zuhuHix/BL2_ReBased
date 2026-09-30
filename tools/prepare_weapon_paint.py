"""Prepare one local UE paint input from an existing recipe/UModel export.

Uses the same explicitly approximate Master_Gun interpretation as the local
thumbnail renderer. Output is game-derived and must remain under local/.
"""
import argparse
import json
import math
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
        leaf = params['texture'].get(name)
        if not isinstance(leaf, str) or not leaf:
            raise ValueError(f'Missing texture parameter {name}')
        path = texture_dir / (leaf + '.png')
        if not path.is_file():
            raise ValueError(f'Missing {path}')
        paths[name] = str(path.resolve())
    required = [f'p_{zone}Color{tone}' for zone in 'ABC'
                for tone in ['Shadow', 'Midtone', 'Hilight']]
    required += ['p_PatternColor', 'p_PatternChannelScale', 'p_PatternScalePosition']
    for name in required:
        value = params['vector'].get(name)
        if not isinstance(value, (tuple, list)) or len(value) != 4 or not all(
                isinstance(component, (int, float)) and math.isfinite(component) for component in value):
            raise ValueError(f'Missing or invalid vector parameter {name}')
    return {'recipe_id': recipe_path.stem, 'material_identity': identity,
            'parent_chain': chain, 'weapon_class': kind, 'detail_channel': channel,
            'params': params, 'textures': paths, 'shader_verified': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--recipe', type=Path, nargs='+', required=True)
    parser.add_argument('--materials', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    local = Path(__file__).resolve().parents[1] / 'local'
    if not args.output.resolve().is_relative_to(local.resolve()):
        parser.error('Output must remain under local/')
    results = [prepare(recipe, args.materials) for recipe in args.recipe]
    ids = [result['recipe_id'] for result in results]
    if len(ids) != len(set(ids)):
        parser.error('Duplicate recipe IDs')
    result = results[0] if len(results) == 1 else results
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2))
    for entry in results:
        print(f"{entry['recipe_id']}: {len(entry['parent_chain'])} MICs, four textures; shader UNVERIFIED")
