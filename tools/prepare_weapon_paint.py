"""Prepare one local UE paint input from an existing recipe/UModel export.

Uses the same explicitly approximate Master_Gun interpretation as the local
thumbnail renderer. Output is game-derived and must remain under local/.
As in the renderer, a MIC chain without p_Pattern (or with no pattern weight)
gets the zone colours only (`pattern_used` false). `mesh` names the UE
skeletal mesh the importer paints.

With --reader/--package the owned reader adds two installed facts UModel's
MIC export lacks: the base Material's own parameter defaults (its surviving
MaterialExpression*Parameter objects; params["source"] says where each value
came from) and the decal texture's address modes. The decal layer (`decal`)
is an UNVERIFIED reading, see DECAL_READING; the graph that consumes it is
stripped from the cooked data.
"""
import argparse
import json
import math
from pathlib import Path
import re
import subprocess
import tempfile
from render_weapon_previews import resolve_material, weapon_class, DETAIL_CHANNELS


MESH_ROOT = '/Game/OpenWillow/Weapons/'
VALUE_PARAMETERS = {'MaterialExpressionScalarParameter': 'scalar', 'MaterialExpressionVectorParameter': 'vector'}
ZERO = {'scalar': 0.0, 'vector': (0.0, 0.0, 0.0, 0.0)}
DECAL_READING = ('UNVERIFIED: p_Decal sampled at UV1 * xy + zw of p_DecalScalePosition (as p_PatternScalePosition), '
                 'with the installed texture address modes; weight saturate(dot(p_Masks.rgb, p_DecalChannel.rgb)) '
                 'times decal alpha; Decal.rgb * p_DecalColor multiplies the zone colours, moving to a replace '
                 '(times the detail tone) as p_ReplaceDecal goes to 1. p_DecalRotate and sw_FlipDecalOnRightSide '
                 'are not applied.')


def as_value(kind, value):
    return float(value) if kind == 'scalar' else tuple(float(value[c]) for c in 'RGBA')


def base_defaults_from(material, children, class_defaults):
    """Scalar/vector parameter defaults of one base Material from reader property records.

    children: (class name, --properties record) for the Material's expression exports. UE3 omits a
    property equal to its class default, so an expression without DefaultValue takes class_defaults[kind]
    (read from Default__MaterialExpression*Parameter). A name with two different defaults is a conflict
    and is left out rather than picked.
    """
    found, source, conflicts = {'scalar': {}, 'vector': {}}, {'scalar': {}, 'vector': {}}, set()
    for klass, record in children:
        kind = VALUE_PARAMETERS.get(klass.split('.')[-1])
        props = {prop['name']: prop for prop in record['properties']}
        name = props.get('ParameterName', {}).get('value')
        if kind is None or not name:
            continue
        given = props.get('DefaultValue')
        if given is not None and given.get('status') != 'decoded':
            raise ValueError(f"Undecoded DefaultValue on {record['path']}")
        value = as_value(kind, given['value']) if given else class_defaults[kind]
        origin = f"{material}:{record['path'].split('.')[-1]}" + ('' if given else ' (class default)')
        if name in found[kind] and found[kind][name] != value:
            conflicts.add((kind, name))
        found[kind].setdefault(name, value)
        source[kind].setdefault(name, origin)
    for kind, name in conflicts:
        del found[kind][name], source[kind][name]
    return {'path': material, **found, 'source': source, 'conflicts': sorted(name for _, name in conflicts)}


class PackageReader:
    """ow-package over one installed package; its output is only held in memory."""

    def __init__(self, reader, package):
        self.reader, self.package = str(reader), Path(package)
        self.exports = json.loads(self.run('--exports'))

    def run(self, *args):
        result = subprocess.run([self.reader, str(self.package), *args], capture_output=True, encoding='utf-8')
        if result.returncode:
            raise ValueError(f'ow-package {args[0]} on {self.package.name} failed: {result.stderr.strip()}')
        return result.stdout

    def records(self, indices):
        with tempfile.TemporaryDirectory() as folder:
            listing = Path(folder) / 'indices.txt'
            listing.write_text('\n'.join(str(index) for index in indices))
            text = self.run('--properties-batch', str(listing), '--property-offset', '4')
        return {record['index']: record for record in map(json.loads, filter(str.strip, text.splitlines()))}

    def find(self, name, klass):
        hits = [e for e in self.exports if e['name'] == name and e['class'].split('.')[-1] == klass]
        return hits[0] if len(hits) == 1 else None


class InstalledFacts:
    """Base-Material defaults and texture address modes, read from the install with the owned reader."""

    def __init__(self, reader, package):
        self.package = PackageReader(reader, package)
        self.engine = PackageReader(reader, Path(package).parent / 'Engine.upk')
        self.cache = {}

    def class_defaults(self):
        values = {}
        for klass, kind in VALUE_PARAMETERS.items():
            cdo = self.engine.find('Default__' + klass, klass)
            if cdo is None:
                raise ValueError(f'No Default__{klass} in Engine.upk')
            props = {p['name']: p for p in self.engine.records([cdo['index']])[cdo['index']]['properties']}
            # A class default that serialises no DefaultValue keeps the property's zero value.
            values[kind] = as_value(kind, props['DefaultValue']['value']) if 'DefaultValue' in props else ZERO[kind]
        return values

    def base_defaults(self, name):
        if name not in self.cache:
            material = self.package.find(name, 'Material')
            self.cache[name] = None
            if material is not None:
                children = [e for e in self.package.exports if e['outer_index'] == material['index']]
                records = self.package.records([e['index'] for e in children])
                self.cache[name] = base_defaults_from(
                    material['path'], [(e['class'], records[e['index']]) for e in children], self.class_defaults())
        return self.cache[name]

    def texture_address(self, name):
        texture = self.package.find(name, 'Texture2D')
        if texture is None:
            return None
        props = {p['name']: p.get('value') for p in self.package.records([texture['index']])[texture['index']]['properties']}
        # Absent means the enum's zero value; the install serialises only TA_Clamp/TA_Mirror, so read as wrap.
        return [props.get('AddressX') or 'TA_Wrap', props.get('AddressY') or 'TA_Wrap']


def is_vector(value):
    return isinstance(value, (tuple, list)) and len(value) == 4 and all(
        isinstance(component, (int, float)) and math.isfinite(component) for component in value)


def decal_layer(params, texture_dir, facts):
    """The chain's p_Decal parameters, and whether the importer can draw them (DECAL_READING)."""
    leaf = params['texture'].get('p_Decal')
    if not leaf:
        return None
    vec, sca = params['vector'], params['scalar']
    layer = {'texture': leaf, 'scale_position': vec.get('p_DecalScalePosition'), 'channel': vec.get('p_DecalChannel'),
             'color': vec.get('p_DecalColor'), 'rotate': sca.get('p_DecalRotate'),
             'full_color': sca.get('p_UseFullColorDecal'), 'replace': sca.get('p_ReplaceDecal'),
             'address': None, 'used': False, 'reading': DECAL_READING}
    path = texture_dir / (leaf + '.png')
    missing = [key for key in ('scale_position', 'channel', 'color') if not is_vector(layer[key])] + [
        key for key in ('rotate', 'full_color', 'replace') if not isinstance(layer[key], (int, float))]
    if missing:
        layer['not_used_because'] = f"no value for {', '.join(missing)} (base defaults need --reader)"
    elif layer['full_color'] != 1:
        layer['not_used_because'] = 'p_UseFullColorDecal is not 1; the single-channel decal is not read'
    elif not path.is_file():
        layer['not_used_because'] = f'missing {path}'
    elif facts is None:
        layer['not_used_because'] = 'texture address modes need --reader'
    else:
        layer['address'] = facts.texture_address(leaf)
        if layer['address'] is None:
            layer['not_used_because'] = f'no unique Texture2D {leaf} in {facts.package.package.name}'
        else:
            layer['used'] = True
    return layer


def prepare(recipe_path, materials, mesh=None, facts=None):
    if not re.fullmatch(r'[A-Za-z0-9_]+', recipe_path.stem):
        raise ValueError('Invalid recipe ID')
    mesh = mesh or f'{MESH_ROOT}Items/SK_{recipe_path.stem}'
    if not re.fullmatch(re.escape(MESH_ROOT) + r'[A-Za-z0-9_/]+', mesh):
        raise ValueError(f'Mesh must be a /Game/OpenWillow/Weapons/ asset path: {mesh}')
    recipe = json.loads(recipe_path.read_text())
    identity = recipe['material']
    resolved = resolve_material(identity.split('.')[-1], [materials], facts.base_defaults if facts else None)
    if resolved is None:
        raise ValueError(f'No local export of {identity}')
    params, texture_dir, chain = resolved
    kind = weapon_class(recipe, params)
    channel = DETAIL_CHANNELS[(params['texture']['p_Diffuse'], kind)]
    weights = params['vector'].get('p_PatternChannelScale')
    pattern_used = bool(params['texture'].get('p_Pattern')) and isinstance(weights, (tuple, list)) and any(
        isinstance(w, (int, float)) and w > 0 for w in weights[:3])
    decal = decal_layer(params, texture_dir, facts)
    decal_used = bool(decal and decal['used'])
    paths = {}
    for name in ['p_Masks', 'p_Diffuse', 'p_NormalScopesEmissive'] + (['p_Pattern'] if pattern_used else []) + (
            ['p_Decal'] if decal_used else []):
        leaf = params['texture'].get(name)
        if not isinstance(leaf, str) or not leaf:
            raise ValueError(f'Missing texture parameter {name}')
        path = texture_dir / (leaf + '.png')
        if not path.is_file():
            raise ValueError(f'Missing {path}')
        paths[name] = str(path.resolve())
    required = [f'p_{zone}Color{tone}' for zone in 'ABC'
                for tone in ['Shadow', 'Midtone', 'Hilight']]
    if pattern_used:
        required += ['p_PatternColor', 'p_PatternChannelScale', 'p_PatternScalePosition']
    for name in required:
        if not is_vector(params['vector'].get(name)):
            raise ValueError(f'Missing or invalid vector parameter {name}')
    return {'recipe_id': recipe_path.stem, 'material_identity': identity,
            'parent_chain': chain, 'weapon_class': kind, 'detail_channel': channel,
            'params': params, 'textures': paths, 'pattern_used': pattern_used,
            'decal': decal, 'mesh': mesh, 'shader_verified': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--recipe', type=Path, nargs='+', required=True)
    parser.add_argument('--materials', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--mesh', help='UE mesh to paint (one recipe only); default /Game/OpenWillow/Weapons/Items/SK_<id>')
    parser.add_argument('--mesh-folder', help='UE folder holding SK_<id> for every recipe, e.g. /Game/OpenWillow/Weapons/SliceItems')
    parser.add_argument('--reader', type=Path, help='ow-package.exe: base-Material defaults and decal address modes')
    parser.add_argument('--package', type=Path, help='installed package holding the MIC chain, e.g. <CookedPCConsole>/Startup.upk')
    args = parser.parse_args()
    if args.mesh and len(args.recipe) != 1:
        parser.error('--mesh takes exactly one --recipe')
    if bool(args.reader) != bool(args.package):
        parser.error('--reader and --package go together')
    local = Path(__file__).resolve().parents[1] / 'local'
    if not args.output.resolve().is_relative_to(local.resolve()):
        parser.error('Output must remain under local/')
    facts = InstalledFacts(args.reader, args.package) if args.reader else None
    folder = args.mesh_folder.rstrip('/') if args.mesh_folder else None
    results = [prepare(recipe, args.materials, args.mesh or (f'{folder}/SK_{recipe.stem}' if folder else None), facts)
               for recipe in args.recipe]
    ids = [result['recipe_id'] for result in results]
    if len(ids) != len(set(ids)):
        parser.error('Duplicate recipe IDs')
    result = results[0] if len(results) == 1 else results
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2))
    for entry in results:
        defaulted = sorted(name for kind in ('scalar', 'vector') for name, origin in entry['params']['source'][kind].items()
                           if origin not in entry['parent_chain'])
        decal = entry['decal']
        print(f"{entry['recipe_id']}: {len(entry['parent_chain'])} MICs, {len(entry['textures'])} textures, "
              f"pattern {'used' if entry['pattern_used'] else 'not used'}, "
              f"decal {'none' if decal is None else 'used' if decal['used'] else 'not used: ' + decal['not_used_because']}, "
              f"{len(defaulted)} base defaults -> {entry['mesh']}; shader UNVERIFIED")
