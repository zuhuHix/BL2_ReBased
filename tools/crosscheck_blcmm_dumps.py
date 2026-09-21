"""Cross-check a prepared scene against OpenBLCMM's Borderlands 2 object dumps.

The dumps are the output of the game's own ``obj dump`` console command: what the
running engine reports for an object after it has loaded and cooked it. They are
an oracle for *placement and property* questions our decoder answers from the
package bytes, in the same spirit as running umodel as a geometry oracle.

Four comparisons, each reported separately:

``terrain``   Per ``TerrainComponent``: our section base/size against
              ``SectionBaseX/Y`` and ``SectionSizeX/Y``, and our decoded heights
              against ``Bounds`` -- our component-local vertices are mapped
              through the dump's ``_LocalToWorld`` and the resulting box is
              compared with the reported one.
``bsp``       Per ``ModelComponent``: node and element counts and the owning
              ``Model``. The dumps print ``Nodes(N)=``/``Elements(N)=`` with
              empty values, so only the counts are available, not the contents.
``actors``    Our actor and component location/rotation/scale composed into a
              matrix against the component's ``_LocalToWorld``.
``materials`` Our per-channel texture pick against the material's effective
              ``TextureParameterValues`` (walking the ``Parent`` chain).

Nothing here reads game packages except through our own reader, and no dump text
is written to the repository: the report goes under ``local/``.
"""
import argparse
from collections import Counter
import json
import math
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from blcmm_dumps import Dumps, level_object
from crosscheck_umodel_assets import texture_index

# Texture parameter names seen in Sanctuary's materials, mapped to our channels with
# a rank: the canonical name a master material exposes wins over a variant such as
# ``p_DiffuseVertexPaint`` when a material carries both. Anything unlisted is
# reported as an unrecognised parameter rather than guessed at.
PARAMETERS = {
    'p_diffuse': ('diffuse', 0), 'diffuse': ('diffuse', 1), 'diff': ('diffuse', 2), 'tex_diff': ('diffuse', 2),
    'p_diffusevertexpaint': ('diffuse', 3), 'p_diffusefade': ('diffuse', 3), 'p_snowdiffuse': ('diffuse', 3),
    'p_normal': ('normal', 0), 'normal': ('normal', 1), 'tex_norm': ('normal', 2),
    'p_specular': ('specular', 0), 'specular': ('specular', 1), 'tex_spec': ('specular', 2),
    'p_emissive': ('emissive', 0), 'emissive': ('emissive', 1),
}
# Unreal expands a component's reported bounds by one unit per axis; seen on every
# Sanctuary terrain component, so it is subtracted before extents are compared.
BOUNDS_EXPANSION = 1.0
POSITION_TOLERANCE = 0.05       # centimetres
ROTATION_TOLERANCE = 1e-3       # unitless matrix entries
PARENT_CHAIN_LIMIT = 8
# An InterpActor is matinee-driven, so the dump reports wherever it had moved to
# when the dump was taken, not its cooked placement. Those are reported as
# ``mover`` rather than counted as disagreements; the ones that do match are
# still informative, so the count of each is kept.
MOVER_CLASSES = ('InterpActor',)


def rotation_matrix(pitch, yaw, roll):
    """Unreal rotator in degrees -> 3x3 rows, matching ``FRotationMatrix``."""
    p, y, r = (math.radians(v) for v in (pitch, yaw, roll))
    sp, sy, sr, cp, cy, cr = math.sin(p), math.sin(y), math.sin(r), math.cos(p), math.cos(y), math.cos(r)
    return [[cp * cy, cp * sy, sp],
            [sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp],
            [-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp]]


def transform_matrix(location, rotation, scale):
    """location/rotation/scale -> dict(rows, translation); scale multiplies each row."""
    rows = rotation_matrix(*rotation)
    return dict(rows=[[rows[i][j] * scale[i] for j in range(3)] for i in range(3)],
                translation=list(location))


def compose(inner, outer):
    """Apply ``inner`` first, then ``outer`` (component relative to actor)."""
    rows = [[sum(inner['rows'][i][k] * outer['rows'][k][j] for k in range(3)) for j in range(3)] for i in range(3)]
    translation = [sum(inner['translation'][k] * outer['rows'][k][j] for k in range(3)) + outer['translation'][j]
                   for j in range(3)]
    return dict(rows=rows, translation=translation)


def dump_matrix(planes):
    """A dump's ``_LocalToWorld`` struct -> the same dict(rows, translation) shape."""
    order = ('XPlane', 'YPlane', 'ZPlane')
    return dict(rows=[[planes[name][axis] for axis in 'XYZ'] for name in order],
                translation=[planes['WPlane'][axis] for axis in 'XYZ'])


def transform_point(matrix, point):
    return tuple(sum(point[k] * matrix['rows'][k][i] for k in range(3)) + matrix['translation'][i] for i in range(3))


def matrix_difference(ours, theirs):
    rotation = max(abs(ours['rows'][i][j] - theirs['rows'][i][j]) for i in range(3) for j in range(3))
    translation = max(abs(ours['translation'][i] - theirs['translation'][i]) for i in range(3))
    return rotation, translation


def bounds_of(points):
    low = [min(p[i] for p in points) for i in range(3)]
    high = [max(p[i] for p in points) for i in range(3)]
    return ([(low[i] + high[i]) / 2 for i in range(3)], [(high[i] - low[i]) / 2 for i in range(3)])


def dump_bounds(bounds):
    return ([bounds['Origin'][axis] for axis in 'XYZ'], [bounds['BoxExtent'][axis] for axis in 'XYZ'])


def obj_positions(path):
    points = []
    for line in path.read_text(encoding='utf-8').splitlines():
        if line.startswith('v '):
            points.append(tuple(float(v) for v in line.split()[1:4]))
    return points


def compare_terrain(mesh, properties, points):
    """Section base/size and, through ``_LocalToWorld``, the decoded height range."""
    section = mesh['terrain']['section']
    theirs = [properties.get('SectionBaseX'), properties.get('SectionBaseY'),
              properties.get('SectionSizeX'), properties.get('SectionSizeY')]
    report = dict(source=mesh['source'], section=section, dump_section=theirs, mismatches=[])
    if section != theirs:
        report['mismatches'].append('section')
    matrix = dump_matrix(properties['_LocalToWorld'])
    # Each component's matrix already carries its own section base, so our
    # terrain-local vertices are shifted back to component-local before mapping.
    base = (section[0], section[1], 0)
    world = [transform_point(matrix, tuple(p[i] - base[i] for i in range(3))) for p in points]
    origin, extent = bounds_of(world)
    their_origin, their_extent = dump_bounds(properties['Bounds'])
    report['origin_delta'] = [their_origin[i] - origin[i] for i in range(3)]
    report['extent_delta'] = [their_extent[i] - extent[i] - BOUNDS_EXPANSION for i in range(3)]
    if max(abs(v) for v in report['origin_delta']) > POSITION_TOLERANCE:
        report['mismatches'].append('bounds_origin')
    if max(abs(v) for v in report['extent_delta']) > POSITION_TOLERANCE:
        report['mismatches'].append('bounds_extent')
    report['status'] = 'differ' if report['mismatches'] else 'agree'
    return report


def compare_bsp(mesh, properties):
    """Node and element counts and the owning model; the dumps carry no node contents."""
    report = dict(source=mesh['source'], mismatches=[],
                  nodes=len(mesh['bsp']['nodes']), dump_nodes=len(properties.get('Nodes') or []),
                  sections=len(mesh['sections']), dump_elements=len(properties.get('Elements') or []),
                  model=mesh['bsp']['model'], dump_model=(properties.get('Model') or {}).get('path'),
                  zone_index=properties.get('ZoneIndex'), component_index=properties.get('ComponentIndex'))
    if report['nodes'] != report['dump_nodes']:
        report['mismatches'].append('nodes')
    if report['sections'] != report['dump_elements']:
        report['mismatches'].append('elements')
    package, path = mesh['bsp']['model'].split(':', 1)
    if report['dump_model'] != level_object(package, path):
        report['mismatches'].append('model')
    report['status'] = 'differ' if report['mismatches'] else 'agree'
    return report


def actor_matrix(transform):
    """A scene actor's transform in either recorded shape -> dict(rows, translation).

    ``StaticMeshCollectionActor`` entries carry the cooked 4x4 row-major matrix
    with the draw scale kept apart; everything else carries actor and component
    location/rotation/scale that have to be composed.
    """
    if 'matrix' in transform:
        matrix, scale = transform['matrix'], transform['scale']
        return dict(rows=[[matrix[i * 4 + j] * scale[i] for j in range(3)] for i in range(3)],
                    translation=[matrix[12 + j] for j in range(3)])
    return compose(transform_matrix(**transform['component']), transform_matrix(**transform['actor']))


def is_mover(source):
    return any(f'.{name}_' in source for name in MOVER_CLASSES)


def compare_actor(actor, properties):
    """Our recorded placement against the dump's ``_LocalToWorld``."""
    source = f"{actor['level']}:{actor['source']}"
    ours = actor_matrix(actor['transform'])
    theirs = dump_matrix(properties['_LocalToWorld'])
    rotation, translation = matrix_difference(ours, theirs)
    report = dict(source=source, rotation_delta=rotation, translation_delta=translation, mismatches=[])
    if rotation > ROTATION_TOLERANCE:
        report['mismatches'].append('rotation')
    if translation > POSITION_TOLERANCE:
        report['mismatches'].append('translation')
    if not report['mismatches']:
        report['status'] = 'agree'
    else:
        report['status'] = 'mover' if is_mover(source) else 'differ'
    return report


def channel_for(parameter):
    ranked = PARAMETERS.get((parameter or '').lower())
    return None if ranked is None else ranked[0]


def channel_rank(parameter):
    return PARAMETERS[(parameter or '').lower()][1]


def effective_parameters(dumps, name, limit=PARENT_CHAIN_LIMIT):
    """Texture parameter name -> texture path, with a child's overrides beating its parent's."""
    parameters, seen = {}, set()
    while name and name not in seen and len(seen) < limit:
        seen.add(name)
        dump = dumps.dump(name)
        if dump is None:
            break
        for value in dump['properties'].get('TextureParameterValues') or []:
            if not isinstance(value, dict):
                continue
            texture = value.get('ParameterValue')
            key = value.get('ParameterName')
            if key is not None and key not in parameters:
                parameters[key] = texture.get('path') if isinstance(texture, dict) else None
        parent = dump['properties'].get('Parent')
        name = parent.get('path') if isinstance(parent, dict) else None
    return parameters


def compare_material_parameters(channels, parameters):
    """Per channel, one of:

    ``agree``            the parameter the game reports for this channel is our texture
    ``differ``           it reports a different texture
    ``other_parameter``  our texture is in the material's parameter set, under another name
    ``unparameterised``  the material has texture parameters but none for this channel
    ``no_parameters``    the dump lists no texture parameters at all
    """
    mapped = {}
    for key, path in parameters.items():
        channel = channel_for(key)
        if channel is None:
            continue
        rank = channel_rank(key)
        if channel not in mapped or rank < mapped[channel][1]:
            mapped[channel] = (path, rank)
    paths = {path for path in parameters.values() if path}
    report = {}
    for channel, ours in channels.items():
        theirs = mapped.get(channel, (None, None))[0]
        if theirs is not None and theirs == ours:
            status = 'agree'
        elif ours in paths:
            status = 'other_parameter'
        elif theirs is not None:
            status = 'differ'
        else:
            status = 'no_parameters' if not parameters else 'unparameterised'
        report[channel] = dict(status=status, ours=ours, dump=theirs)
    return report


def scene_channels(material, textures):
    """Resolve a scene material's PNG file names back to the texture object paths they came from."""
    channels = {}
    for channel, filename in (material.get('channels') or {}).items():
        located = textures.get(filename.split('_', 1)[0])
        if located is not None:
            channels[channel] = located[1]
    return channels


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--scene', type=Path, required=True, help='prepared scene.json')
    parser.add_argument('--output', type=Path, help='write the full report as JSON')
    parser.add_argument('--db', type=Path, default=None, help='OpenBLCMM data.db (default: the installed one)')
    parser.add_argument('--jar', type=Path, default=None, help='OpenBLCMM data jar (default: the newest installed)')
    parser.add_argument('--reader', type=Path, help='ow-package executable, to resolve texture PNG names back to paths')
    parser.add_argument('--game', type=Path, help='Borderlands 2 install root, used with --reader')
    parser.add_argument('--actor-limit', type=int, default=0, help='compare only the first N actors (0 = all)')
    args = parser.parse_args()

    scene = json.loads(args.scene.read_text(encoding='utf-8'))
    meshes = args.scene.parent
    dumps = Dumps(**({} if args.db is None else dict(db=args.db)), jar=args.jar)

    def properties_for(source):
        package, path = source.split(':', 1)
        dump = dumps.dump(level_object(package, path))
        return None if dump is None else dump['properties']

    report = dict(terrain=[], bsp=[], actors=[], materials={}, missing=[])

    for mesh in scene['meshes'].values():
        if 'terrain' not in mesh and 'bsp' not in mesh:
            continue
        properties = properties_for(mesh['source'])
        if properties is None:
            report['missing'].append(mesh['source'])
            continue
        if 'terrain' in mesh:
            points = [p for section in mesh['sections'] for p in obj_positions(meshes / section['file'])]
            report['terrain'].append(compare_terrain(mesh, properties, points))
        else:
            report['bsp'].append(compare_bsp(mesh, properties))

    actors = scene['actors'][:args.actor_limit] if args.actor_limit else scene['actors']
    for actor in actors:
        # A terrain component's own matrix carries its section base, which the actor
        # entry does not; the terrain comparison above handles that offset properly.
        if '.TerrainComponent_' in actor['source']:
            continue
        properties = properties_for(f"{actor['level']}:{actor['source']}")
        if properties is None or '_LocalToWorld' not in properties:
            report['missing'].append(f"{actor['level']}:{actor['source']}")
            continue
        report['actors'].append(compare_actor(actor, properties))

    textures = {}
    if args.reader and args.game:
        packages = sorted({m['source'].split(':', 1)[0] for m in scene['materials'].values()} |
                          {m['source'].split(':', 1)[0] for m in scene['meshes'].values()})
        textures = texture_index(args.reader, Path(args.game) / 'WillowGame' / 'CookedPCConsole', packages)
    unrecognised = Counter()
    for identity, material in scene['materials'].items():
        package, path = material['source'].split(':', 1)
        parameters = effective_parameters(dumps, level_object(package, path))
        channels = scene_channels(material, textures) if textures else {}
        for key in parameters:
            if channel_for(key) is None:
                unrecognised[key] += 1
        report['materials'][identity] = dict(source=material['source'], parameters=len(parameters),
                                             channels=compare_material_parameters(channels, parameters))

    summary = dict(
        terrain=len(report['terrain']),
        terrain_status=dict(Counter(r['status'] for r in report['terrain'])),
        terrain_mismatch_kinds=dict(Counter(k for r in report['terrain'] for k in r['mismatches'])),
        bsp=len(report['bsp']),
        bsp_status=dict(Counter(r['status'] for r in report['bsp'])),
        bsp_mismatch_kinds=dict(Counter(k for r in report['bsp'] for k in r['mismatches'])),
        actors=len(report['actors']),
        actor_status=dict(Counter(r['status'] for r in report['actors'])),
        actor_mismatch_kinds=dict(Counter(k for r in report['actors'] for k in r['mismatches'])),
        materials=len(report['materials']),
        material_channel_status=dict(Counter(c['status'] for r in report['materials'].values()
                                             for c in r['channels'].values())),
        unrecognised_parameters=dict(unrecognised.most_common(20)),
        missing_dumps=len(report['missing']))
    report['summary'] = summary
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=1), encoding='utf-8')
    print(json.dumps(summary))
    disagreements = (summary['terrain_status'].get('differ', 0) + summary['bsp_status'].get('differ', 0)
                     + summary['actor_status'].get('differ', 0) + summary['material_channel_status'].get('differ', 0))
    return 1 if disagreements else 0


if __name__ == '__main__':
    sys.exit(main())
