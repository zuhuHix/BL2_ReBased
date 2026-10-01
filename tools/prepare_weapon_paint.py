"""Prepare one local UE paint input from an existing recipe/UModel export.

Uses the same explicitly approximate Master_Gun interpretation as the local
thumbnail renderer. Output is game-derived and must remain under local/.
As in the renderer, a MIC chain without p_Pattern (or with no pattern weight)
gets the zone colours only (`pattern_used` false). `mesh` names the UE
skeletal mesh the importer paints.

With --reader/--package the owned reader adds installed facts UModel's MIC
export lacks: the base Material's own parameter defaults (its surviving
MaterialExpression*Parameter objects; params["source"] says where each value
came from), the decal texture's address modes, and the leaf MIC's static
parameters (tools/material_static_parameters.py), which pick the detail,
pattern and decal channels. The colour model the importer draws is the
UNVERIFIED reading in tools/weapon_paint_model.py (PAINT_READING), recovered
from the compiled Master_Gun shaders; the graph itself is stripped.
"""
import argparse
import json
import math
from pathlib import Path
import re
import subprocess
import tempfile
from render_weapon_previews import resolve_material, weapon_class, DETAIL_CHANNELS
from material_static_parameters import decode_tail, mask_channels
from weapon_paint_model import PAINT_PARAMETERS, PAINT_READING


MESH_ROOT = '/Game/OpenWillow/Weapons/'
VALUE_PARAMETERS = {'MaterialExpressionScalarParameter': 'scalar', 'MaterialExpressionVectorParameter': 'vector'}
ZERO = {'scalar': 0.0, 'vector': (0.0, 0.0, 0.0, 0.0)}
DECAL_READING = ('UNVERIFIED: p_Decal at UV1 shifted by zw of p_DecalScalePosition, rotated about (0.5, 0.5) by '
                 'p_DecalRotate * pi and scaled by xy about (0.5, 0.5), with the installed texture address modes; '
                 'tint and amount as in tools/weapon_paint_model.py.')
STATIC_DEFAULTS = {'p_WeapClassSelect': None, 'p_PatternChannel': 'RGB', 'p_DecalChannel': 'RGB'}


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

    def names(self):
        if not hasattr(self, '_names'):
            self._names = json.loads(self.run('--names'))
        return self._names

    def trailing_bytes(self, index):
        """Bytes after one export's tagged properties."""
        record = self.records([index])[index]
        payload = bytes(json.loads(self.run('--payload', str(index))))
        return payload[record['property_offset'] + record['consumed_bytes']:]

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

    def static_channels(self, name):
        """Channels the MIC's static parameter set selects, by parameter name, or None.

        A cooked MIC with a static permutation stores its resolved set (inherited values included),
        so the leaf MIC is enough. decode_tail refuses unless the trailing bytes are consumed exactly."""
        mic = self.package.find(name, 'MaterialInstanceConstant')
        if mic is None:
            return None
        tail = self.package.trailing_bytes(mic['index'])
        if not tail:
            return None
        static = decode_tail(tail, self.package.names())['static']
        return {entry['name']: mask_channels(entry) for entry in static['component_masks']}

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


def static_choices(chain, params, kind, facts):
    """(detail channel index, pattern channels, decal channels, where they came from)."""
    found = facts.static_channels(chain[0]) if facts is not None and hasattr(facts, 'static_channels') else None
    if found:
        channels = {name: found.get(name, default) for name, default in STATIC_DEFAULTS.items()}
        detail = channels['p_WeapClassSelect']
        if detail is None or len(detail) != 1 or detail not in 'RGB':
            raise ValueError(f'{chain[0]}: p_WeapClassSelect selects {detail!r}, not one colour channel')
        return ('RGB'.index(detail), channels['p_PatternChannel'], channels['p_DecalChannel'],
                f'{chain[0]} static parameters')
    guess = DETAIL_CHANNELS[(params['texture']['p_Diffuse'], kind)]
    return guess, 'RGB', 'RGB', 'DETAIL_CHANNELS guess (static parameters need --reader)'


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
    channel, pattern_channels, decal_channels, channel_source = static_choices(chain, params, kind, facts)
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
    required = PAINT_PARAMETERS['vector'][1:] + PAINT_PARAMETERS['vector'][:1]  # zone colours first
    scalars = list(PAINT_PARAMETERS['scalar'])
    if pattern_used:
        required += PAINT_PARAMETERS['pattern_vector']
        scalars += PAINT_PARAMETERS['pattern_scalar']
    for name in required:
        if not is_vector(params['vector'].get(name)):
            raise ValueError(f'Missing or invalid vector parameter {name} (base defaults need --reader)')
    for name in scalars:
        if not isinstance(params['scalar'].get(name), (int, float)):
            raise ValueError(f'Missing scalar parameter {name} (base defaults need --reader)')
    return {'recipe_id': recipe_path.stem, 'material_identity': identity,
            'parent_chain': chain, 'weapon_class': kind, 'detail_channel': channel,
            'pattern_channels': pattern_channels, 'decal_channels': decal_channels,
            'channel_source': channel_source, 'params': params, 'textures': paths, 'pattern_used': pattern_used,
            'decal': decal, 'mesh': mesh, 'reading': PAINT_READING, 'shader_verified': False}


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
              f"detail {'RGB'[entry['detail_channel']]} from {entry['channel_source']}, "
              f"{len(defaulted)} base defaults -> {entry['mesh']}; shader UNVERIFIED")
