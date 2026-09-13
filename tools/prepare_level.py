"""Prepare Ash_P and its referenced sublevels for the UE5 editor host.

All game-derived output stays in local/. This is an offline scene loader v1,
not gameplay streaming or a general UE3 material-graph interpreter.
"""
import argparse
import hashlib
import json
import math
import re
from pathlib import Path
import struct
import subprocess


def values(tags):
    return {p['name']: p['value'] for p in tags if p.get('status') == 'decoded'}


def props(record):
    if 'error' in record:
        raise ValueError(record['error'])
    return values(record.get('data', {}).get('properties', []))


def vector(value, default=0):
    return [value.get(k, default) for k in ('X', 'Y', 'Z')]


def transform(p, component=False):
    rot = p.get('Rotation', {})
    return {'location': vector(p.get('Translation' if component else 'Location', {})),
            'rotation': [rot.get(k, 0) * 360 / 65536 for k in ('Pitch', 'Yaw', 'Roll')],
            'scale': [v * p.get('Scale' if component else 'DrawScale', 1)
                      for v in vector(p.get('Scale3D' if component else 'DrawScale3D', {}), 1)]}


def collection_transforms(payload, data, count):
    """Observed Ash layout: rotation/translation matrix, Scale3D, scale, int.

    Require the exact tail size and affine finite matrices; no offset scanning.
    The final int is retained without assigning unverified semantics.
    """
    offset = data['property_offset'] + data['consumed_bytes']
    if len(payload) - offset != count * 84:
        raise ValueError('Unsupported collection transform layout')
    result = []
    for i in range(count):
        row = struct.unpack_from('<20fi', payload, offset + i * 84)
        if not all(math.isfinite(v) for v in row[:20]):
            raise ValueError('Nonfinite collection transform')
        if any(abs(row[k]) > 1e-5 for k in (3, 7, 11)) or abs(row[15] - 1) > 1e-5:
            raise ValueError('Non-affine collection matrix')
        result.append({'matrix': list(row[:16]), 'scale': [v * row[19] for v in row[16:19]],
                       'source_flags': row[20]})
    return result


# UE3 material graphs use a small set of exact names for the ordinary
# channels, but environment masters also use stable aliases. Keep this
# allow-list narrow: treating every mask/noise/cubemap parameter as albedo
# makes sky and FX planes become opaque white geometry in the host.
CHANNELS = {'p_diffuse': 'diffuse', 'diffuse': 'diffuse',
            'p_normal': 'normal', 'normal': 'normal',
            'p_specular': 'specular', 'specular': 'specular',
            'p_emissive': 'emissive', 'emissive': 'emissive'}
ALIASES = {
    'p_dif': 'diffuse', 'dif': 'diffuse', 'ad_diff': 'diffuse',
    'tex_diff': 'diffuse', 'diff_texture': 'diffuse',
    'p_snowdiffuse': 'diffuse', 'diffuse_a': 'diffuse',
    'diffuse_b': 'diffuse', 'diffuse_c': 'diffuse',
    'p_nrm': 'normal', 'nrm': 'normal', 'normalmap': 'normal',
    'tex_spec': 'specular',
    'emissive_gp': 'emissive', 'emissiveopacityinput': 'emissive',
}


def channel_for_parameter(parameter):
    name = str(parameter).casefold()
    return CHANNELS.get(name) or ALIASES.get(name)


def parameter_priority(parameter):
    """Prefer a direct diffuse override over a shared master default."""
    name = str(parameter).casefold()
    if name in ('p_diffuse', 'diffuse'):
        return 100
    if name in ('diffuse_a', 'diffuse_b', 'diffuse_c', 'p_dif', 'dif', 'ad_diff', 'tex_diff'):
        return 90
    if name == 'p_snowdiffuse':
        return 50
    return 80


def unnamed_diffuse_candidate(parameters, identity):
    """Conservative Material v1 approximation for stripped cooked graphs.

    Sanctuary's Mat_SancBuild_Colorized retains a single unnamed sample pointing
    to SancBuild1a_Dif_04. Require that exact naming pattern and uniqueness;
    never replace a named diffuse channel (including an explicit null override).
    """
    if any(channel_for_parameter(name) == 'diffuse' for name in parameters):
        return None
    candidates = {key for name, key in parameters.items() if key is not None
                  and re.fullmatch(r'materialexpressiontexturesampleparameter2d_\d+', name)
                  and re.search(r'_dif(?:_\d+)?$', identity(key), re.IGNORECASE)}
    return next(iter(candidates)) if len(candidates) == 1 else None


def material_index(records, path):
    matches = [index for index, record in records.items() if record['path'] == path
               and record['class'].rsplit('.', 1)[-1] in
               ('Material', 'MaterialInstanceConstant', 'MaterialInstanceTimeVarying')]
    if len(matches) != 1:
        raise ValueError('Material identity is missing or ambiguous: ' + path)
    return matches[0]


class Scene:
    def __init__(self, reader, game, output):
        self.reader, self.game, self.output = reader.resolve(), game.resolve(), output.resolve()
        self.cooked = self.game / 'WillowGame/CookedPCConsole'
        self.schema = Path(__file__).with_name('level-arrays.schema')
        self.packages = {}
        for path in sorted(self.cooked.rglob('*')):
            if path.suffix.lower() in ('.upk', '.umap', '.u'):
                self.packages.setdefault(path.stem.casefold(), []).append(path)
        self.records, self.materials, self.meshes, self.textures = {}, {}, {}, {}
        self.resolved = {}
        self.issues = []
        self.output.mkdir(parents=True, exist_ok=True)

    def package(self, name):
        paths = self.packages.get(name.casefold(), [])
        if len(paths) != 1:
            raise ValueError(f'Expected unique package {name}, found {len(paths)}')
        return paths[0]

    def call(self, package, *args):
        run = subprocess.run([str(self.reader), str(self.package(package)), *map(str, args)],
                             capture_output=True, text=True, encoding='utf-8')
        if run.returncode:
            raise ValueError(run.stderr.strip())
        return json.loads(run.stdout)

    def load(self, package):
        if package not in self.records:
            self.records[package] = {r['index']: r for r in self.call(package, '--scene-records', self.schema)}
        return self.records[package]

    def resolve(self, package, ref):
        index = ref.get('index', 0) if isinstance(ref, dict) else ref
        if not index:
            return None
        if index > 0:
            return package, index
        if (package, index) in self.resolved:
            return self.resolved[package, index]
        path = ref.get('path') if isinstance(ref, dict) else None
        if path:
            for loaded, records in self.records.items():
                match = next((r for r in records.values() if r['path'].casefold() == path.casefold()), None)
                if match:
                    self.resolved[package, index] = loaded, match['index']
                    return loaded, match['index']
        r = self.call(package, '--resolve', index, '--cooked', self.cooked)
        self.resolved[package, index] = r['resolved_package'], r['resolved_index']
        return self.resolved[package, index]

    def identity(self, key):
        return key[0] + ':' + self.load(key[0])[key[1]]['path']

    def filename(self, key, suffix):
        return hashlib.sha256(self.identity(key).encode()).hexdigest()[:24] + suffix

    def issue(self, context, error):
        self.issues.append({'object': context, 'error': str(error)})

    def material_parameters(self, key, stack=()):
        if key in stack or len(stack) >= 32:
            raise ValueError('Material parent cycle/depth limit')
        p = props(self.load(key[0])[key[1]])
        parent = self.resolve(key[0], p.get('Parent', 0))
        parameters = self.material_parameters(parent, (*stack, key)) if parent else {}
        # Base-material named texture expressions supply defaults.
        for ref in p.get('Expressions', []):
            expr = self.resolve(key[0], ref)
            if not expr:
                continue
            expression = self.load(expr[0])[expr[1]]
            if 'TextureSampleParameter' not in expression.get('class', ''):
                continue
            e = props(expression)
            name = e.get('ParameterName', '').casefold()
            if (channel_for_parameter(name) or re.fullmatch(r'materialexpressiontexturesampleparameter2d_\d+', name)) and e.get('Texture', {}).get('index'):
                parameters[name] = self.resolve(expr[0], e['Texture'])
        for entry in p.get('TextureParameterValues', []):
            e = values(entry)
            name = e.get('ParameterName', '').casefold()
            if channel_for_parameter(name) or re.fullmatch(r'materialexpressiontexturesampleparameter2d_\d+', name):
                parameters[name] = self.resolve(key[0], e.get('ParameterValue', 0))
        return parameters

    def material_metadata(self, key, stack=()):
        """Carry safe UE3 blend/shading flags without importing full graphs."""
        if key in stack or len(stack) >= 32:
            raise ValueError('Material parent cycle/depth limit')
        p = props(self.load(key[0])[key[1]])
        parent = self.resolve(key[0], p.get('Parent', 0))
        metadata = self.material_metadata(parent, (*stack, key)) if parent else {}
        if p.get('BlendMode'):
            metadata['blend_mode'] = p['BlendMode']
        if p.get('LightingModel'):
            metadata['lighting_model'] = p['LightingModel']
        if 'TwoSided' in p:
            metadata['two_sided'] = bool(p['TwoSided'])
        # A translucent/masked base graph has an opacity input even when its
        # supported texture channels are outside Material v1.
        opacity = p.get('Opacity')
        if isinstance(opacity, list):
            metadata['has_opacity'] = any(
                item.get('name') == 'Expression'
                and isinstance(item.get('value'), dict)
                and item['value'].get('index')
                for item in opacity if isinstance(item, dict))
        return metadata

    def texture(self, key, channel):
        if key is None:
            return None
        cache_key = (*key, channel)
        if cache_key not in self.textures:
            filename = self.filename(key, '_' + channel + '.png')
            self.call(key[0], '--texture', key[1], '--property-offset', 4,
                      '--output', self.output / filename, '--tfc', self.cooked)
            self.textures[cache_key] = filename
        return self.textures[cache_key]

    def material(self, key):
        if key is None:
            return None
        name = self.filename(key, '')
        if name not in self.materials:
            material = {'source': self.identity(key), 'channels': {}}
            self.materials[name] = material
            try:
                material.update(self.material_metadata(key))
                parameters = self.material_parameters(key)
                inferred = unnamed_diffuse_candidate(parameters, self.identity)
                if inferred is not None:
                    parameters['p_diffuse'] = inferred
                    material['diffuse_inference'] = self.identity(inferred)
                    self.issue(material['source'], 'Approximation: sole unnamed _Dif texture used as diffuse; cooked graph and tint not reconstructed')
                for parameter, texture in parameters.items():
                    channel = channel_for_parameter(parameter)
                    if not channel or texture is None:
                        continue
                    try:
                        filename = self.texture(texture, channel)
                        current = material['channels'].get(channel)
                        # Parent defaults often point at StubGray. A concrete
                        # child override wins, while an alias never replaces a
                        # stronger exact parameter that was already selected.
                        priority = parameter_priority(parameter)
                        current_priority = material.setdefault('_channel_priority', {}).get(channel, -1)
                        is_stub = 'common_textures.stub.' in self.identity(texture).casefold()
                        current_is_stub = material.setdefault('_channel_stub', {}).get(channel, False)
                        if (current is None or (current_is_stub and not is_stub)
                                or (current_is_stub == is_stub and priority > current_priority)):
                            material['channels'][channel] = filename
                            material['_channel_priority'][channel] = priority
                            material['_channel_stub'][channel] = is_stub
                    except ValueError as error:
                        self.issue(material['source'] + ':' + channel, error)
                material.pop('_channel_priority', None)
                material.pop('_channel_stub', None)
                if not any(material['channels'].values()):
                    self.issue(material['source'], 'No supported named Material v1 texture parameters; neutral fallback')
            except ValueError as error:
                self.issue(material['source'], error)
        return name

    def mesh(self, key):
        name = self.filename(key, '')
        if name not in self.meshes:
            filename = name + '.obj'
            data = self.call(key[0], '--mesh', key[1], '--property-offset', 4, '--output', self.output / filename)
            # One OBJ per section preserves material slots without relying on FBX
            # importer's OBJ group merging or generated material-name order.
            lines = (self.output / filename).read_text().splitlines(keepends=True)
            header, groups = [], []
            for line in lines:
                if line.startswith('g '):
                    groups.append([])
                elif groups:
                    groups[-1].append(line)
                else:
                    header.append(line)
            if len(groups) != len(data['sections']):
                raise ValueError('Mesh section count differs from OBJ')
            sections = []
            for i, (group, section) in enumerate(zip(groups, data['sections'])):
                if not section['triangles']:
                    continue
                file = f'{name}_s{i}.obj'
                (self.output / file).write_text(''.join(header + group))
                sections.append({'slot': i, 'file': file,
                                 'material': self.material(self.resolve(key[0], section['material_index']))})
            self.meshes[name] = {'source': self.identity(key), 'sections': sections}
        return name

    def build(self, persistent):
        self.load('Startup')
        levels, pending, seen = [], [persistent], set()
        while pending:
            name = pending.pop(0)
            if name.casefold() in seen:
                continue
            seen.add(name.casefold())
            records = self.load(name)
            levels.append(name)
            for record in records.values():
                if record['class'].startswith('Engine.LevelStreaming'):
                    p = props(record)
                    sublevel = p.get('PackageName')
                    if sublevel and sublevel != 'None':
                        self.package(sublevel)  # missing dependencies are fatal
                        pending.append(sublevel)
        actors, camera = [], None
        for level in levels:
            print(f'Preparing {level}', flush=True)
            records = self.load(level)
            placements = {}
            for record in records.values():
                if record['class'] == 'Engine.StaticMeshCollectionActor':
                    p = props(record)
                    refs = p.get('StaticMeshComponents', [])
                    payload = bytes(self.call(level, '--payload', record['index']))
                    transforms = collection_transforms(payload, record['data'], len(refs))
                    for ref, pose in zip(refs, transforms):
                        if ref['index'] in placements:
                            raise ValueError('Duplicate collection component')
                        placements[ref['index']] = pose
                elif 'PlayerStart' in record['class'] and camera is None:
                    camera = transform(props(record))
                    camera['location'][2] += 100
            for record in records.values():
                if record['class'] != 'Engine.StaticMeshComponent' or not record['path'].startswith('TheWorld.PersistentLevel.'):
                    continue
                try:
                    p = props(record)
                    key = self.resolve(level, p.get('StaticMesh', 0))
                    if not key:
                        continue
                    pose = placements.get(record['index'])
                    if pose is None:
                        owner = records.get(record['outer'])
                        if not owner or owner['class'] not in ('Engine.StaticMeshActor', 'Engine.InterpActor'):
                            self.issue(level + ':' + record['path'], 'Unsupported component owner')
                            continue
                        pose = {'actor': transform(props(owner)), 'component': transform(p, True)}
                    overrides = [self.material(self.resolve(level, ref)) for ref in p.get('Materials', [])]
                    actors.append({'source': record['path'], 'level': level, 'mesh': self.mesh(key),
                                   'transform': pose, 'materials': overrides, 'static': True})
                except ValueError as error:
                    self.issue(level + ':' + record['path'], error)
        if not actors:
            raise ValueError('No static mesh placements loaded')
        result = {'schema': 1, 'map': persistent, 'levels': levels, 'actors': actors,
                  'meshes': self.meshes, 'materials': self.materials, 'camera': camera,
                  'issues': self.issues, 'dynamic_policy': 'frozen', 'visual_validation': 'pending'}
        temporary = self.output / 'scene.json.tmp'
        temporary.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
        temporary.replace(self.output / 'scene.json')
        print(json.dumps({'levels': levels, 'placements': len(actors), 'meshes': len(self.meshes),
                          'materials': len(self.materials), 'issues': len(self.issues)}), flush=True)
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--map', default='Ash_P')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if not (args.game / 'Binaries/Win32/Borderlands2.exe').is_file():
        parser.error('An installed Borderlands 2 is required')
    if not args.map.endswith('_P'):
        parser.error('Start with a persistent _P package')
    if not args.map.replace('_', '').isascii() or not args.map.replace('_', '').isalnum():
        parser.error('Map names must contain only ASCII letters, digits and underscores')
    if args.output is None:
        args.output = Path('local') / args.map[:-2].lower()
    Scene(args.reader, args.game, args.output).build(args.map)


if __name__ == '__main__':
    main()
