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
from collision_geometry import hulls as collision_hulls
from installed_content import PACKAGE_SUFFIXES, cache_directory, content_files


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


DIFFUSE_SUFFIX = re.compile(r'_diff?(?:_\d+)?$', re.IGNORECASE)
# Cooked-resource texture names that are not plausible diffuse sources: normal
# maps, packed composite/specular/emissive channels, masks, gray noise tiles.
AUXILIARY_TEXTURE = re.compile(
    r'(_n|_nm|_nrm|normal|_gray|_grey|_hs|_spec|_emis|_emissive|_alpha|_mask|_comp|_lm|noise|_cube)(?:_\d+)?$',
    re.IGNORECASE)


def cooked_diffuse_candidate(textures, identity, texture_class, blend_mode='BLEND_Opaque'):
    """Pick a diffuse texture from a cooked material's texture list.

    A unique *_Dif/_Diff Texture2D wins. Otherwise, for opaque and masked
    materials only, a unique Texture2D whose name does not mark it as an
    auxiliary channel is used; a translucent material's diffuse alpha would
    become its opacity, and a guessed opacity is worse than the invisible
    fallback. Both are recorded as approximations; the graph, UV mapping and
    tint remain unreconstructed.
    """
    planar = [t for t in textures if texture_class(t) == 'Texture2D']
    named = {t for t in planar if DIFFUSE_SUFFIX.search(identity(t))}
    if len(named) == 1:
        return next(iter(named)), 'sole_cooked_resource_dif_texture'
    if named or blend_mode not in ('BLEND_Opaque', 'BLEND_Masked'):
        return None, None
    plain = {t for t in planar if not AUXILIARY_TEXTURE.search(identity(t))}
    if len(plain) == 1:
        return next(iter(plain)), 'sole_cooked_resource_texture'
    return None, None


def unconnected_diffuse_constant(material_props):
    """Return the constant colour of a Material whose DiffuseColor input was
    never connected, or None when that cannot be established.

    Cooked 832/46 materials strip their expression graphs, so the input's
    Expression reference is always null. The observed distinction is the Mask
    flags: an input that had an expression keeps Mask/MaskR/G/B, an input that
    never had one carries neither. UE3 then evaluates the input as its
    Constant, which defaults to black. Observed on Master_Black; not a general
    format guarantee.
    """
    entries = material_props.get('DiffuseColor')
    if not isinstance(entries, list):
        return None
    fields = {e.get('name'): e.get('value') for e in entries if isinstance(e, dict)}
    expression = fields.get('Expression')
    if isinstance(expression, dict) and expression.get('index'):
        return None
    if any(fields.get(mask) for mask in ('Mask', 'MaskR', 'MaskG', 'MaskB', 'MaskA')):
        return None
    constant = fields.get('Constant')
    if isinstance(constant, dict):
        return [float(constant.get(c, 0)) for c in ('R', 'G', 'B')]
    return [0.0, 0.0, 0.0]


def material_index(records, path):
    matches = [index for index, record in records.items() if record['path'] == path
               and record['class'].rsplit('.', 1)[-1] in
               ('Material', 'MaterialInstanceConstant', 'MaterialInstanceTimeVarying')]
    if len(matches) != 1:
        raise ValueError('Material identity is missing or ambiguous: ' + path)
    return matches[0]


def cooked_texture_references(payload, data):
    """Read the observed 832/46 Material resource prefix, never scan offsets.

    Only empty compile-error/dependency arrays are supported. The remaining
    resource/shader bytes are deliberately opaque. See DECISIONS.md.
    """
    offset = data['property_offset'] + data['consumed_bytes']
    if offset < 0 or offset > len(payload) or len(payload) - offset != data['trailing_bytes']:
        raise ValueError('Cooked material payload/property boundary mismatch')
    tail = memoryview(payload)[offset:]
    if len(tail) < 36:
        raise ValueError('Truncated cooked material resource prefix')
    if struct.unpack_from('<2i', tail) != (0, 0):
        raise ValueError('Unsupported cooked material error/dependency arrays')
    # Two empty arrays, resource int, GUID, resource int, texture-array count.
    count = struct.unpack_from('<i', tail, 32)[0]
    if count < 0 or count > (len(tail) - 36) // 4:
        raise ValueError('Invalid cooked material texture count')
    refs = list(struct.unpack_from(f'<{count}i', tail, 36))
    return refs, len(tail) - 36 - count * 4


class Scene:
    def __init__(self, reader, game, output, include_dlc=False):
        self.reader, self.game, self.output = reader.resolve(), game.resolve(), output.resolve()
        self.cooked = self.game / 'WillowGame/CookedPCConsole'
        self.include_dlc = include_dlc
        self.package_root = self.game if include_dlc else self.cooked
        self.schema = Path(__file__).with_name('level-arrays.schema')
        self.packages = {}
        self.texture_caches = {}
        for path in content_files(self.game, include_dlc):
            if path.suffix.lower() in PACKAGE_SUFFIXES:
                self.packages.setdefault(path.stem.casefold(), []).append(path)
            elif path.suffix.lower() == '.tfc':
                self.texture_caches.setdefault(path.name.casefold(), []).append(path)
        self.records, self.materials, self.meshes, self.textures = {}, {}, {}, {}
        self.resolved, self.imports = {}, {}
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
        if package not in self.imports:
            self.imports[package] = {r['index']: r for r in self.call(package, '--imports')}
        metadata = self.imports[package].get(index, {})
        path = path or metadata.get('path')
        expected_class = metadata.get('class_name')
        if path:
            for loaded, records in self.records.items():
                match = next((r for r in records.values() if r['path'].casefold() == path.casefold()
                              and (not expected_class or r['class'].rsplit('.', 1)[-1] == expected_class)), None)
                if match:
                    self.resolved[package, index] = loaded, match['index']
                    return loaded, match['index']
        # Shared base resources retain the established lookup order in DLC
        # mode. Only an absent target expands the search to the full install.
        try:
            r = self.call(package, '--resolve', index, '--cooked', self.cooked)
        except ValueError as error:
            if not self.include_dlc or 'not found:' not in str(error):
                raise
            r = self.call(package, '--resolve', index, '--cooked', self.package_root)
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

    def glacier_primary_surface(self, key, base, textures):
        """Explicit primary-layer approximation, never a recovered snow shader.

        Restricted to the two inspected Sanctuary family members and the exact
        resource texture set. UV0 is an approximation: native static permutation
        data and the stripped blend graph are not interpreted by this policy.
        """
        base_path = 'Prop_Glacier.Materials.Mat_Glacier'
        if self.identity(base).split(':', 1)[1] != base_path:
            return None
        if self.identity(key).split(':', 1)[1] not in (
                base_path, 'Prop_Glacier.Materials.Mati_Glacier2x'):
            return None
        expected = {'Prop_Glacier.Textures.GlacierFront_Dif',
                    'Prop_Glacier.Textures.GlacierFront_Nrm',
                    'Prop_Terrain.Textures.Snow_Dif',
                    'Prop_Skybox.MoveMe.R2Tex_SnowTempCubeStaticNegY'}
        by_path = {self.identity(t).split(':', 1)[1]: t for t in textures}
        if len(textures) != 4 or set(by_path) != expected:
            return None
        if any(self.load(t[0])[t[1]]['class'].rsplit('.', 1)[-1] != 'Texture2D'
               for t in textures):
            return None
        name = 'p_texscalar_rgmain_basnow'
        scale = None
        for ref in props(self.load(base[0])[base[1]]).get('Expressions', []):
            expression = self.resolve(base[0], ref)
            if expression is None:
                continue
            row = self.load(expression[0])[expression[1]]
            p = props(row)
            if p.get('ParameterName', '').casefold() == name:
                if row['class'].rsplit('.', 1)[-1] != 'MaterialExpressionVectorParameter' or scale is not None:
                    raise ValueError('Ambiguous glacier primary-layer scale')
                scale = p.get('DefaultValue')
        chain, current = [], key
        while current != base:
            if current in chain or len(chain) >= 32:
                raise ValueError('Glacier parent cycle/depth limit')
            chain.append(current)
            current = self.resolve(current[0], props(self.load(current[0])[current[1]]).get('Parent', 0))
            if current is None:
                raise ValueError('Glacier parent does not reach inspected base')
        for instance in reversed(chain):
            overrides = [values(e).get('ParameterValue') for e in
                         props(self.load(instance[0])[instance[1]]).get('VectorParameterValues', [])
                         if values(e).get('ParameterName', '').casefold() == name]
            if len(overrides) > 1:
                raise ValueError('Duplicate glacier scale override')
            if overrides:
                scale = overrides[0]
        if not isinstance(scale, dict) or not all(
                isinstance(scale.get(c), (int, float)) and math.isfinite(scale[c]) for c in 'RGBA'):
            raise ValueError('Missing or invalid glacier primary-layer scale')
        return {'diffuse': by_path['Prop_Glacier.Textures.GlacierFront_Dif'],
                'normal': by_path['Prop_Glacier.Textures.GlacierFront_Nrm'],
                'scale': [float(scale[c]) for c in 'RGBA']}

    def cooked_material_textures(self, key, stack=()):
        if key in stack or len(stack) >= 32:
            raise ValueError('Material parent cycle/depth limit')
        record = self.load(key[0])[key[1]]
        if record['class'].rsplit('.', 1)[-1] != 'Material':
            parent = self.resolve(key[0], props(record).get('Parent', 0))
            return self.cooked_material_textures(parent, (*stack, key)) if parent else None
        refs, opaque = cooked_texture_references(
            bytes(self.call(key[0], '--payload', key[1])), record['data'])
        textures = []
        for ref in refs:
            texture = self.resolve(key[0], ref)
            if texture is None:
                continue
            target = self.load(texture[0]).get(texture[1])
            if target is None or target['class'].rsplit('.', 1)[-1] not in ('Texture2D', 'TextureCube'):
                raise ValueError('Cooked material reference is not a supported texture')
            textures.append(texture)
        return key, textures, opaque

    def texture(self, key, channel):
        if key is None:
            return None
        cache_key = (*key, channel)
        if cache_key not in self.textures:
            filename = self.filename(key, '_' + channel + '.png')
            tfc_root = self.cooked
            if self.include_dlc:
                cache = props(self.load(key[0])[key[1]]).get('TextureFileCacheName', 'None')
                cache_file = cache if cache.lower().endswith('.tfc') else cache + '.tfc'
                tfc_root = cache_directory(self.texture_caches.get(cache_file.casefold(), []),
                                           self.package(key[0]), self.cooked)
            self.call(key[0], '--texture', key[1], '--property-offset', 4,
                      '--output', self.output / filename, '--tfc', tfc_root)
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
                if not any(channel_for_parameter(p) == 'diffuse' for p in parameters):
                    try:
                        cooked = self.cooked_material_textures(key)
                        if cooked is not None:
                            base, textures, opaque = cooked
                            material['cooked_texture_resource'] = {
                                'source': self.identity(base),
                                'textures': [self.identity(t) for t in textures],
                                'opaque_tail_bytes': opaque}
                            inferred, method = cooked_diffuse_candidate(
                                textures, self.identity,
                                lambda t: self.load(t[0])[t[1]]['class'].rsplit('.', 1)[-1],
                                material.get('blend_mode', 'BLEND_Opaque'))
                            glacier = (self.glacier_primary_surface(key, base, textures)
                                       if inferred is None and material.get('blend_mode', 'BLEND_Opaque') == 'BLEND_Opaque'
                                       else None)
                            if glacier is not None:
                                parameters['p_diffuse'] = glacier['diffuse']
                                glacier_channels = ['diffuse']
                                # Preserve any explicit normal parameter, including a null override.
                                if not any(channel_for_parameter(p) == 'normal' for p in parameters):
                                    parameters['p_normal'] = glacier['normal']
                                    glacier_channels.append('normal')
                                material['surface_approximation'] = {
                                    'method': 'glacier_primary_layer_v1',
                                    'status': 'partial_unverified',
                                    'source_scale_parameter': 'P_TexScalar_RGMain_BASnow',
                                    'source_scale': glacier['scale'],
                                    'omitted': ['snow_blend', 'reflection', 'glow'],
                                    'uv_selection': 'UV0 approximation; static permutation not decoded'}
                                material['channel_uv'] = {
                                    channel: {'index': 0, 'scale': glacier['scale'][:2]}
                                    for channel in glacier_channels}
                                self.issue(material['source'], 'Approximation: glacier primary diffuse/normal layer with retained tiling on UV0; snow blend, reflection, glow and static UV selection unverified')
                            if inferred is not None:
                                parameters['p_diffuse'] = inferred
                                material['diffuse_inference'] = self.identity(inferred)
                                material['diffuse_inference_method'] = method
                                self.issue(material['source'], 'Approximation: sole cooked resource _Dif texture used as diffuse; graph, UV mapping and tint not reconstructed'
                                           if method == 'sole_cooked_resource_dif_texture' else
                                           'Approximation: sole non-auxiliary cooked resource texture used as diffuse; graph, UV mapping and tint not reconstructed')
                            elif not textures and material.get('blend_mode', 'BLEND_Opaque') == 'BLEND_Opaque':
                                constant = unconnected_diffuse_constant(props(self.load(base[0])[base[1]]))
                                if constant is not None:
                                    material['constant_diffuse'] = constant
                                    self.issue(material['source'], 'Approximation: unconnected DiffuseColor input rendered as its constant; no cooked textures')
                    except ValueError as error:
                        self.issue(material['source'], error)
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
                if not any(material['channels'].values()) and 'constant_diffuse' not in material:
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
            collision = {'status': 'absent', 'hulls': []}
            body = self.resolve(key[0], data.get('body_setup', 0))
            if body:
                try:
                    record = self.load(body[0])[body[1]]
                    if record['class'] != 'Engine.RB_BodySetup' or 'error' in record:
                        raise ValueError('Invalid collision body record')
                    collision = {'status': 'supported', 'source': self.identity(body),
                                 'hulls': collision_hulls(record['data']['properties'])}
                except (ValueError, KeyError) as error:
                    collision = {'status': 'unsupported', 'hulls': [], 'reason': str(error)}
                    self.issue(self.identity(key) + ':collision', error)
            self.meshes[name] = {'source': self.identity(key), 'sections': sections, 'collision': collision}
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
                                   'transform': pose, 'materials': overrides, 'static': True,
                                   'collision_enabled': p.get('BlockActors', True) and p.get('CollideActors', True)})
                except ValueError as error:
                    self.issue(level + ':' + record['path'], error)
        if not actors:
            raise ValueError('No static mesh placements loaded')
        result = {'schema': 1, 'map': persistent, 'levels': levels, 'actors': actors,
                  'package_scope': 'base_and_dlc' if getattr(self, 'include_dlc', False) else 'base',
                  'meshes': self.meshes, 'materials': self.materials, 'camera': camera,
                  'issues': self.issues, 'dynamic_policy': 'frozen', 'visual_validation': 'pending',
                  'collision_policy': 'observed_convex_and_box_v1'}
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
    parser.add_argument('--include-dlc', action='store_true', help='Index installed DLC packages and named texture caches')
    args = parser.parse_args()
    if not (args.game / 'Binaries/Win32/Borderlands2.exe').is_file():
        parser.error('An installed Borderlands 2 is required')
    if not args.map.endswith('_P'):
        parser.error('Start with a persistent _P package')
    if not args.map.replace('_', '').isascii() or not args.map.replace('_', '').isalnum():
        parser.error('Map names must contain only ASCII letters, digits and underscores')
    if args.output is None:
        args.output = Path('local') / args.map[:-2].lower()
    Scene(args.reader, args.game, args.output, include_dlc=args.include_dlc).build(args.map)


if __name__ == '__main__':
    main()
