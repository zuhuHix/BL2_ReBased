"""Report a map's native sky placement chain without changing any policy.

For a persistent map and its streamed sublevels this lists every placed
StaticMesh that is sky-named (under `Prop_Skybox` or with `sky` in its object
name), the component and owner that place it, the observed transform, the
effective material of each section (component override, else mesh default),
each material's parent chain, blend flags, named texture parameters and cooked
texture resource list, and each referenced Texture2D's format, size and cache.

Every material also records what the existing Material v1 diffuse policy in
`prepare_level.py` would select and why, so gaps are explained rather than
resolved here. The report is manifest evidence only: it does not interpret
stripped material graphs, Kismet streaming state, or lighting, and it makes no
claim about in-game appearance. Output stays under the ignored `local/`.
"""
import argparse
import json
from pathlib import Path
import re

from prepare_level import (Scene, channel_for_parameter, collection_transforms,
                           cooked_diffuse_candidate, parameter_priority, props, transform, values)

SKY_NAME = re.compile(r'sky', re.IGNORECASE)
MATERIAL_CLASSES = ('Material', 'MaterialInstanceConstant', 'MaterialInstanceTimeVarying')


def short_class(record):
    return record.get('class', '').rsplit('.', 1)[-1]


def sky_named(path):
    """A `Prop_Skybox` package member or an object whose own name mentions sky."""
    return path.startswith('Prop_Skybox.') or bool(SKY_NAME.search(path.rsplit('.', 1)[-1]))


def diffuse_policy(parameters, cooked_textures, identity, texture_class, blend_mode, target_identity):
    """Restate the Material v1 diffuse outcome for one material.

    Mirrors `Scene.material`: a named diffuse parameter wins; otherwise the
    cooked-resource candidate rules apply. The returned reason names the
    blocking condition so a later policy change can be targeted precisely.
    """
    named = [(name, texture) for name, texture in parameters.items()
             if channel_for_parameter(name) == 'diffuse']
    if named:
        # Same precedence as Scene.material: a concrete texture beats a stub,
        # then the stronger exact parameter wins.
        def rank(item):
            name, texture = item
            stub = texture is not None and 'common_textures.stub.' in identity(texture).casefold()
            return (texture is not None, not stub, parameter_priority(name))
        name, texture = max(named, key=rank)
        return {'selected': identity(texture) if texture else None, 'method': 'named_parameter',
                'parameter': name, 'reason': 'named diffuse parameter present'
                if texture else 'named diffuse parameter explicitly null'}
    if cooked_textures is None:
        return {'selected': None, 'method': None, 'reason': 'no cooked base material'}
    candidate, method = cooked_diffuse_candidate(cooked_textures, identity, texture_class, blend_mode)
    if candidate is not None:
        return {'selected': identity(candidate), 'method': method,
                'reason': 'unique cooked resource candidate'}
    planar = [t for t in cooked_textures if texture_class(t) == 'Texture2D']
    dif = [identity(t) for t in planar if re.search(r'_diff?(?:_\d+)?$', identity(t), re.IGNORECASE)]
    if len(dif) > 1:
        reason = 'ambiguous: several cooked _Dif/_Diff textures'
    elif not planar:
        reason = 'no cooked Texture2D resources'
    elif blend_mode not in ('BLEND_Opaque', 'BLEND_Masked'):
        reason = 'non-opaque blend mode blocks the non-auxiliary fallback'
    else:
        reason = 'ambiguous or auxiliary-only non-_Dif cooked textures'
    if target_identity in dif:
        reason += '; a sky-named _Dif texture is among the candidates'
    elif target_identity is not None:
        reason += '; a sky-named texture is present but not a _Dif candidate'
    return {'selected': None, 'method': None, 'reason': reason, 'dif_candidates': dif}


class SkyCensus:
    def __init__(self, scene):
        self.scene = scene
        self.materials = {}
        self.textures = {}

    def identity(self, key):
        return self.scene.identity(key)

    def texture_class(self, key):
        return short_class(self.scene.load(key[0])[key[1]])

    def texture(self, key):
        identity = self.identity(key)
        if identity not in self.textures:
            record = self.scene.load(key[0])[key[1]]
            entry = {'class': short_class(record)}
            try:
                # Texture2D properties start after the same 4-byte prefix the
                # --texture extractor already uses; nothing new is assumed.
                p = values(self.scene.call(key[0], '--properties', key[1],
                                           '--property-offset', 4)['properties'])
                entry.update({k: p.get(k) for k in ('Format', 'SizeX', 'SizeY',
                                                     'TextureFileCacheName', 'MipTailBaseIdx')})
            except ValueError as error:
                entry['error'] = str(error)
            self.textures[identity] = entry
        return identity

    def parent_chain(self, key):
        chain, current = [], key
        while current is not None and current not in chain and len(chain) < 32:
            chain.append(current)
            record = self.scene.load(current[0])[current[1]]
            current = self.scene.resolve(current[0], props(record).get('Parent', 0))
        return chain

    def expression_parameters(self, chain_keys):
        """Every named sampler/scalar/vector parameter along a material chain.

        Unlike the Material v1 channel allow-list this keeps all names, so a
        sky master's inputs are visible even when none maps to a channel.
        """
        samplers, scalars, vectors, undecoded = {}, {}, {}, 0
        for key in reversed(chain_keys):
            p = props(self.scene.load(key[0])[key[1]])
            for ref in p.get('Expressions', []):
                expr = self.scene.resolve(key[0], ref)
                if not expr:
                    continue
                record = self.scene.load(expr[0])[expr[1]]
                cls = short_class(record)
                if 'Parameter' not in cls:
                    continue
                if 'error' in record:
                    undecoded += 1
                    continue
                e = props(record)
                name = e.get('ParameterName')
                if name is None:
                    continue
                if 'TextureSampleParameter' in cls:
                    texture = self.scene.resolve(expr[0], e.get('Texture', 0))
                    samplers[name] = self.texture(texture) if texture else None
                elif cls == 'MaterialExpressionScalarParameter':
                    scalars[name] = e.get('DefaultValue')
                elif cls == 'MaterialExpressionVectorParameter':
                    vectors[name] = e.get('DefaultValue')
            for entry in p.get('TextureParameterValues', []):
                e = values(entry)
                texture = self.scene.resolve(key[0], e.get('ParameterValue', 0))
                samplers[e.get('ParameterName')] = self.texture(texture) if texture else None
            for entry in p.get('ScalarParameterValues', []):
                e = values(entry)
                scalars[e.get('ParameterName')] = e.get('ParameterValue')
            for entry in p.get('VectorParameterValues', []):
                e = values(entry)
                vectors[e.get('ParameterName')] = e.get('ParameterValue')
        return {'samplers': samplers, 'scalars': scalars, 'vectors': vectors,
                'undecoded_parameter_expressions': undecoded}

    def material(self, key):
        if key is None:
            return None
        identity = self.identity(key)
        if identity in self.materials:
            return identity
        record = self.scene.load(key[0])[key[1]]
        entry = {'class': short_class(record), 'sky_named': sky_named(record['path'])}
        self.materials[identity] = entry
        try:
            chain = self.parent_chain(key)
            entry['chain'] = [self.identity(k) for k in chain]
            entry.update(self.scene.material_metadata(key))
            entry['parameters'] = self.expression_parameters(chain)
            parameters = self.scene.material_parameters(key)
            entry['named_parameters'] = {name: self.texture(texture) if texture else None
                                         for name, texture in parameters.items()}
            cooked = self.scene.cooked_material_textures(key)
            textures = None
            if cooked is not None:
                base, textures, opaque = cooked
                entry['cooked_resource'] = {'base': self.identity(base),
                                            'textures': [self.texture(t) for t in textures],
                                            'opaque_tail_bytes': opaque}
            # Hint only when the texture's own name says sky; a Prop_Skybox
            # package member such as SlateGravel_Dif is not a sky image.
            sky_dif = next((self.identity(t) for t in textures or []
                            if SKY_NAME.search(self.identity(t).rsplit('.', 1)[-1])), None)
            entry['diffuse_policy'] = diffuse_policy(
                parameters, textures, self.identity, self.texture_class,
                entry.get('blend_mode', 'BLEND_Opaque'), sky_dif)
        except ValueError as error:
            entry['error'] = str(error)
        return identity

    def mesh(self, key):
        data = self.scene.call(key[0], '--mesh', key[1], '--property-offset', 4,
                               '--output', self.scene.output / 'probe.obj')
        (self.scene.output / 'probe.obj').unlink(missing_ok=True)
        sections = []
        for i, section in enumerate(data['sections']):
            default = self.scene.resolve(key[0], section['material_index'])
            sections.append({'slot': i, 'triangles': section['triangles'],
                             'default_material': self.material(default)})
        return {'lods': data['lods'], 'vertices': data['vertices'], 'triangles': data['triangles'],
                'uv_sets': data['uv_sets'], 'sections': sections}

    def placements(self, level):
        """Sky-named StaticMesh placements in one level, with observed poses."""
        records = self.scene.load(level)
        collections = {}
        found = []
        for record in records.values():
            if record['class'] != 'Engine.StaticMeshComponent' \
                    or not record['path'].startswith('TheWorld.PersistentLevel.'):
                continue
            try:
                p = props(record)
                key = self.scene.resolve(level, p.get('StaticMesh', 0))
                if not key:
                    continue
                path = self.scene.load(key[0])[key[1]]['path']
                if not sky_named(path):
                    continue
                owner = records.get(record['outer'])
                owner_class = owner['class'] if owner else None
                pose = None
                if owner_class == 'Engine.StaticMeshCollectionActor':
                    if record['outer'] not in collections:
                        refs = props(owner).get('StaticMeshComponents', [])
                        payload = bytes(self.scene.call(level, '--payload', owner['index']))
                        poses = collection_transforms(payload, owner['data'], len(refs))
                        collections[record['outer']] = {ref['index']: pose for ref, pose in zip(refs, poses)}
                    pose = collections[record['outer']].get(record['index'])
                elif owner_class in ('Engine.StaticMeshActor', 'Engine.InterpActor'):
                    pose = {'actor': transform(props(owner)), 'component': transform(p, True)}
                overrides = [self.material(self.scene.resolve(level, ref)) for ref in p.get('Materials', [])]
                found.append({'level': level, 'component': record['path'], 'owner_class': owner_class,
                              'owner_supported': pose is not None,
                              'mesh': self.identity(key), 'transform': pose,
                              'material_overrides': overrides,
                              'max_draw_distance': p.get('CachedMaxDrawDistance'),
                              'cast_shadow': p.get('CastShadow')})
            except ValueError as error:
                found.append({'level': level, 'component': record['path'], 'error': str(error)})
        return found

    def sky_named_materials(self, level):
        """Sky-named material exports in a level that no sky placement uses."""
        records = self.scene.load(level)
        names = []
        for record in records.values():
            if short_class(record) in MATERIAL_CLASSES and sky_named(record['path']):
                identity = level + ':' + record['path']
                if identity not in self.materials:
                    names.append(identity)
        return names

    def report(self, persistent):
        levels = self.scene.levels(persistent)
        placements = [entry for level in levels for entry in self.placements(level)]
        meshes = {}
        for entry in placements:
            mesh = entry.get('mesh')
            if mesh and mesh not in meshes:
                package, path = mesh.split(':', 1)
                index = next(i for i, r in self.scene.load(package).items() if r['path'] == path)
                try:
                    meshes[mesh] = self.mesh((package, index))
                except ValueError as error:
                    meshes[mesh] = {'error': str(error)}
        for entry in placements:
            mesh = meshes.get(entry.get('mesh'), {})
            overrides = entry.get('material_overrides', [])
            entry['effective_materials'] = [
                overrides[s['slot']] if s['slot'] < len(overrides) and overrides[s['slot']]
                else s['default_material'] for s in mesh.get('sections', [])]
        unplaced = {level: self.sky_named_materials(level) for level in levels}
        return {'schema': 1, 'map': persistent, 'levels': levels,
                'placements': placements, 'meshes': meshes, 'materials': self.materials,
                'textures': self.textures,
                'unplaced_sky_named_materials': {k: v for k, v in unplaced.items() if v},
                'issues': self.scene.issues,
                'note': 'Placement and material chains are decoded manifest data. Kismet streaming '
                        'state, stripped material graphs, lighting and in-game appearance are not '
                        'interpreted; nothing here is visual-parity evidence.'}


def extract(census, report, output):
    """Write each placed sky mesh as OBJ and every referenced Texture2D as PNG."""
    written = []
    for mesh in report['meshes']:
        package, path = mesh.split(':', 1)
        index = next(i for i, r in census.scene.load(package).items() if r['path'] == path)
        target = output / (path + '.obj')
        census.scene.call(package, '--mesh', index, '--property-offset', 4, '--output', target)
        written.append(target.name)
    for identity, texture in report['textures'].items():
        if texture.get('class') != 'Texture2D':
            continue
        package, path = identity.split(':', 1)
        index = next(i for i, r in census.scene.load(package).items() if r['path'] == path)
        target = output / (path + '.png')
        try:
            census.scene.call(package, '--texture', index, '--property-offset', 4,
                              '--output', target, '--tfc', census.scene.cooked)
            written.append(target.name)
        except ValueError as error:
            texture['extract_error'] = str(error)
    return written


def summary(report):
    lines = [f"{report['map']}: {len(report['placements'])} sky placements across {len(report['levels'])} levels"]
    for entry in report['placements']:
        if 'error' in entry:
            lines.append(f"  {entry['level']} | {entry['component']} | ERROR {entry['error']}")
            continue
        lines.append(f"  {entry['level']} | {entry['component']} | {entry['owner_class']} | {entry['mesh']}")
        for slot, material in enumerate(entry['effective_materials']):
            policy = report['materials'].get(material, {}).get('diffuse_policy', {})
            lines.append(f"      slot {slot}: {material} -> diffuse {policy.get('selected')} ({policy.get('reason')})")
    for level, names in report['unplaced_sky_named_materials'].items():
        lines.append(f"  unplaced sky-named materials in {level}: {len(names)}")
    return '\n'.join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--map', default='Sanctuary_P')
    parser.add_argument('--output', type=Path, help='Report directory; default local/sky/<map>')
    parser.add_argument('--extract', action='store_true',
                        help='Also write sky meshes as OBJ and referenced Texture2D as PNG')
    args = parser.parse_args()
    if not (args.game / 'Binaries/Win32/Borderlands2.exe').is_file():
        parser.error('An installed Borderlands 2 is required')
    if not args.map.endswith('_P') or not args.map.replace('_', '').isalnum():
        parser.error('Start with a persistent _P package name')
    output = args.output or Path('local') / 'sky' / args.map.lower()
    scene = Scene(args.reader, args.game, output)
    census = SkyCensus(scene)
    report = census.report(args.map)
    if args.extract:
        report['extracted'] = extract(census, report, output)
    (output / 'sky_census.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(summary(report))
    print(json.dumps({'placements': len(report['placements']), 'meshes': len(report['meshes']),
                      'materials': len(report['materials']), 'textures': len(report['textures']),
                      'issues': len(report['issues'])}))


if __name__ == '__main__':
    main()
