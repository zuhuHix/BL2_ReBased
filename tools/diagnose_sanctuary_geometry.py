"""Trace Sanctuary scene ownership and camera surfaces from a host manifest.

This is a read-only diagnostic for generated host evidence.  It uses only the
scene manifest and its local OBJ files; it never reads an original package and
never changes the prepared scene.  Bounds and ray hits identify which imported
host actor is in a camera view.  They do not establish original-game parity,
native BSP semantics, or native material/lightmap behaviour.
"""
import argparse
from collections import Counter, defaultdict
import json
import math
from pathlib import Path

from crosscheck_blcmm_dumps import actor_matrix, transform_point


MAX_OBJ_VERTICES = 4_000_000
MAX_OBJ_FACES = 8_000_000
MAX_RAY_HITS = 32
DEFAULT_ASPECT = 16.0 / 9.0


def _finite(value):
    return isinstance(value, (int, float)) and math.isfinite(value)


def _vector(value):
    if not isinstance(value, (list, tuple)) or len(value) != 3 or not all(_finite(x) for x in value):
        raise ValueError('Expected a finite three-vector')
    return tuple(float(x) for x in value)


def _add(a, b):
    return tuple(x + y for x, y in zip(a, b))


def _scale(a, scalar):
    return tuple(x * scalar for x in a)


def _dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def _cross(a, b):
    return (a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0])


def _normalise(value):
    length = math.sqrt(_dot(value, value))
    if length <= 1e-12:
        raise ValueError('Zero camera direction')
    return _scale(value, 1.0 / length)


def actor_scope(actor):
    """Classify an actor from explicit manifest ownership markers."""
    if actor.get('terrain'):
        return 'terrain'
    if actor.get('bsp'):
        return 'bsp'
    return 'static_or_prop'


def effective_material(actor, mesh, section, materials):
    """Resolve a section's actor override exactly as the importer does."""
    slot = section['slot']
    overrides = actor.get('materials') or []
    name = overrides[slot] if slot < len(overrides) and overrides[slot] else section.get('material')
    return name, materials.get(name, {}) if name is not None else None


def parse_obj(path):
    """Read bounded positions/UVs/faces from one generated OBJ section."""
    vertices, uvs, faces = [], [], []
    with path.open(encoding='utf-8', errors='replace') as stream:
        for line in stream:
            fields = line.split()
            if not fields:
                continue
            if fields[0] == 'v':
                if len(fields) < 4 or len(vertices) >= MAX_OBJ_VERTICES:
                    raise ValueError(f'Invalid or oversized OBJ vertex stream: {path.name}')
                try:
                    point = _vector([float(value) for value in fields[1:4]])
                except ValueError as error:
                    raise ValueError(f'Invalid OBJ vertex: {path.name}') from error
                vertices.append(point)
            elif fields[0] == 'vt':
                if len(fields) < 3:
                    raise ValueError(f'Invalid OBJ UV: {path.name}')
                try:
                    uv = (float(fields[1]), float(fields[2]))
                except ValueError as error:
                    raise ValueError(f'Invalid OBJ UV: {path.name}') from error
                if not all(_finite(value) for value in uv):
                    raise ValueError(f'Nonfinite OBJ UV: {path.name}')
                uvs.append(uv)
            elif fields[0] == 'f':
                if len(fields) < 4 or len(faces) + len(fields) - 3 > MAX_OBJ_FACES:
                    raise ValueError(f'Invalid or oversized OBJ face stream: {path.name}')
                indices = []
                for token in fields[1:]:
                    raw = token.split('/', 1)[0]
                    try:
                        index = int(raw)
                    except ValueError as error:
                        raise ValueError(f'Invalid OBJ face index: {path.name}') from error
                    index = index - 1 if index > 0 else len(vertices) + index
                    if not 0 <= index < len(vertices):
                        raise ValueError(f'OBJ face index out of range: {path.name}')
                    indices.append(index)
                faces.extend((indices[0], indices[i], indices[i + 1])
                             for i in range(1, len(indices) - 1))
    return {'vertices': vertices, 'uvs': uvs, 'faces': faces}


def _local_bounds(obj):
    points = obj['vertices']
    if not points:
        raise ValueError('OBJ has no vertices')
    return (tuple(min(point[i] for point in points) for i in range(3)),
            tuple(max(point[i] for point in points) for i in range(3)))


def _box_corners(bounds):
    low, high = bounds
    return [(x, y, z) for x in (low[0], high[0])
            for y in (low[1], high[1])
            for z in (low[2], high[2])]


def _world_bounds(matrix, local_bounds):
    points = [transform_point(matrix, point) for point in _box_corners(local_bounds)]
    return (tuple(min(point[i] for point in points) for i in range(3)),
            tuple(max(point[i] for point in points) for i in range(3)))


def _distance_to_box(point, bounds):
    low, high = bounds
    return math.sqrt(sum((low[i] - point[i] if point[i] < low[i]
                          else point[i] - high[i] if point[i] > high[i] else 0.0) ** 2
                         for i in range(3)))


def _contains(point, bounds):
    low, high = bounds
    return all(low[i] <= point[i] <= high[i] for i in range(3))


def _ray_box(origin, direction, bounds):
    low, high = bounds
    start, end = 0.0, float('inf')
    for axis in range(3):
        if abs(direction[axis]) < 1e-12:
            if origin[axis] < low[axis] or origin[axis] > high[axis]:
                return None
            continue
        first = (low[axis] - origin[axis]) / direction[axis]
        last = (high[axis] - origin[axis]) / direction[axis]
        if first > last:
            first, last = last, first
        start, end = max(start, first), min(end, last)
        if start > end:
            return None
    return start, end


def _ray_triangle(origin, direction, a, b, c):
    edge_a, edge_b = _add(b, _scale(a, -1.0)), _add(c, _scale(a, -1.0))
    h = _cross(direction, edge_b)
    determinant = _dot(edge_a, h)
    if abs(determinant) < 1e-9:
        return None
    inverse = 1.0 / determinant
    offset = _add(origin, _scale(a, -1.0))
    bary_u = inverse * _dot(offset, h)
    if bary_u < 0.0 or bary_u > 1.0:
        return None
    q = _cross(offset, edge_a)
    bary_v = inverse * _dot(direction, q)
    if bary_v < 0.0 or bary_u + bary_v > 1.0:
        return None
    distance = inverse * _dot(edge_b, q)
    if distance <= 1e-4:
        return None
    return distance, bary_u, bary_v


def _section_record(scene_dir, actor, mesh, section, materials, obj_cache):
    filename = section.get('file')
    if not filename:
        return None
    path = scene_dir / filename
    if not path.is_file():
        return None
    key = str(path.resolve())
    obj = obj_cache.get(key)
    if obj is None:
        obj = parse_obj(path)
        obj_cache[key] = obj
    matrix = actor_matrix(actor['transform'])
    world_vertices = [transform_point(matrix, point) for point in obj['vertices']]
    name, material = effective_material(actor, mesh, section, materials)
    return {'actor': actor, 'mesh': mesh, 'section': section, 'material_name': name,
            'material': material, 'obj': obj, 'world_vertices': world_vertices,
            'bounds': _world_bounds(matrix, _local_bounds(obj))}


def index_scene(scene, scene_dir):
    """Build bounded actor/section records and world bounds for a manifest."""
    obj_cache, actors = {}, []
    for actor in scene.get('actors', []):
        mesh = scene['meshes'][actor['mesh']]
        sections = []
        all_points = []
        for section in mesh.get('sections', []):
            record = _section_record(scene_dir, actor, mesh, section, scene.get('materials', {}), obj_cache)
            if record is None:
                continue
            sections.append(record)
            all_points.extend(record['world_vertices'])
        if not all_points:
            continue
        bounds = (tuple(min(point[i] for point in all_points) for i in range(3)),
                  tuple(max(point[i] for point in all_points) for i in range(3)))
        actors.append({'actor': actor, 'mesh': mesh, 'scope': actor_scope(actor),
                       'bounds': bounds, 'sections': sections})
    return actors


def _camera_ray(camera, pixel, width=1280, height=720, aspect=DEFAULT_ASPECT):
    location = _vector(camera['location'])
    rotation = _vector(camera.get('rotation', [0.0, 0.0, 0.0]))
    fov = float(camera.get('fov', 75.0))
    if not _finite(fov) or not 1.0 <= fov < 180.0:
        raise ValueError('Camera FOV is outside the bounded diagnostic range')
    x, y = pixel
    if not (0 <= x < width and 0 <= y < height):
        raise ValueError(f'Pixel outside {width}x{height}: {pixel}')
    half_horizontal = math.radians(fov / 2.0)
    half_vertical = math.atan(math.tan(half_horizontal) / aspect)
    local = (1.0,
             ((x + 0.5) / width - 0.5) * 2.0 * math.tan(half_horizontal),
             (0.5 - (y + 0.5) / height) * 2.0 * math.tan(half_vertical))
    # actor_matrix uses the same row convention as the host's transform check.
    from crosscheck_blcmm_dumps import rotation_matrix
    rotation_rows = rotation_matrix(*rotation)
    direction = tuple(sum(local[k] * rotation_rows[k][axis] for k in range(3))
                      for axis in range(3))
    return location, _normalise(direction)


def _ray_hits(indexed, origin, direction, include_hidden=True):
    hits = []
    for item in indexed:
        box_hit = _ray_box(origin, direction, item['bounds'])
        if box_hit is None:
            continue
        actor = item['actor']
        if not include_hidden and actor.get('hidden_visual'):
            continue
        for section in item['sections']:
            if _ray_box(origin, direction, section['bounds']) is None:
                continue
            best = None
            vertices, faces = section['world_vertices'], section['obj']['faces']
            for face_index, (a, b, c) in enumerate(faces):
                result = _ray_triangle(origin, direction, vertices[a], vertices[b], vertices[c])
                if result is not None and (best is None or result[0] < best[0]):
                    best = (result[0], face_index, result[1], result[2])
            if best is None:
                continue
            point = _add(origin, _scale(direction, best[0]))
            material = section['material']
            hits.append({'distance': best[0], 'point': list(point), 'face': best[1],
                         'barycentric': [best[2], best[3]], 'scope': item['scope'],
                         'level': actor['level'], 'actor': actor['source'],
                         'mesh': item['mesh']['source'], 'slot': section['section']['slot'],
                         'material': material.get('source') if material else None,
                         'material_id': section['material_name'],
                         'hidden_visual': bool(actor.get('hidden_visual')),
                         'collision_enabled': bool(actor.get('collision_enabled')),
                         'collision_status': item['mesh'].get('collision', {}).get('status')})
            if material and 'terrain_blend' in material:
                hits[-1]['terrain_layers'] = [
                    {'layer_index': layer.get('layer_index'),
                     'source_material': layer.get('source_material'),
                     'diffuse': layer.get('diffuse'),
                     'weightmap': layer.get('weightmap'),
                     'diffuse_status': layer.get('diffuse_status')}
                    for layer in material['terrain_blend'].get('layers', [])]
    return sorted(hits, key=lambda hit: hit['distance'])[:MAX_RAY_HITS]


def _view_camera(inspection, name):
    if not inspection:
        return None
    for view in inspection.get('views', []):
        if view.get('name') == name:
            return view
    raise ValueError(f'Inspection view not found: {name}')


def _coverage(indexed, point):
    point = _vector(point)
    rows = []
    for item in indexed:
        if item['scope'] not in ('terrain', 'bsp'):
            continue
        rows.append({'scope': item['scope'], 'level': item['actor']['level'],
                     'actor': item['actor']['source'], 'mesh': item['mesh']['source'],
                     'bounds': [list(x) for x in item['bounds']],
                     'contains_point': _contains(point, item['bounds']),
                     'distance': _distance_to_box(point, item['bounds'])})
    return sorted(rows, key=lambda row: row['distance'])


def _opening_gap(coverage, point):
    """Report adjacent X intervals at a point's Y/Z slice.

    This is a coverage diagnostic only.  Static props and opaque BSP meanings
    are deliberately excluded from the interval list.
    """
    point = _vector(point)
    intervals = []
    for row in coverage:
        low, high = row['bounds']
        if low[1] <= point[1] <= high[1] and low[2] <= point[2] <= high[2]:
            intervals.append((low[0], high[0], row))
    intervals.sort(key=lambda row: (row[0], row[1]))
    gaps = []
    for first, second in zip(intervals, intervals[1:]):
        if second[0] > first[1]:
            gaps.append({'from_x': first[1], 'to_x': second[0],
                         'width': second[0] - first[1],
                         'left_actor': first[2]['actor'], 'right_actor': second[2]['actor']})
    return {'slice': {'y': point[1], 'z': point[2]},
            'intervals': [{'from_x': a, 'to_x': b, 'actor': row['actor'],
                           'scope': row['scope']} for a, b, row in intervals],
            'gaps': gaps}


def _unassigned(scene):
    groups = defaultdict(list)
    for actor in scene.get('actors', []):
        mesh = scene['meshes'][actor['mesh']]
        for section in mesh.get('sections', []):
            name, _ = effective_material(actor, mesh, section, scene.get('materials', {}))
            if name is None:
                groups[mesh['source']].append({'level': actor['level'],
                                               'actor': actor['source'],
                                               'slot': section['slot'],
                                               'file': section.get('file')})
    return {'count': sum(map(len, groups.values())),
            'by_mesh': {name: rows for name, rows in sorted(groups.items())}}


def _policy_audit(scene):
    status = Counter()
    for mesh in scene.get('meshes', {}).values():
        status[mesh.get('collision', {}).get('status', 'missing')] += 1
    terrain = scene.get('terrain_policy', {})
    bsp = scene.get('bsp_policy', {})
    observed_fields = sorted({key for mesh in scene.get('meshes', {}).values()
                              for key in mesh.get('collision', {})
                              if any(token in key.casefold() for token in ('flag', 'light', 'poly'))})
    return {'collision_mesh_status_counts': dict(sorted(status.items())),
            'terrain_collision_policy': terrain.get('collision'),
            'bsp_collision_enabled': bsp.get('collision'),
            'observed_flag_or_lightmap_fields': observed_fields,
            'unverified_terrain_fields': terrain.get('unverified', []),
            'interpretation': 'No lightmap or native BSP PolyFlags serialization is present in this manifest; no host flag change is justified.'}


def analyse(scene, scene_dir, inspection=None, view_name=None, pixels=(), width=1280, height=720):
    if scene.get('schema') != 1 or scene.get('map') != 'Sanctuary_P':
        raise ValueError('Expected schema 1 Sanctuary_P scene')
    indexed = index_scene(scene, scene_dir)
    camera = _view_camera(inspection, view_name) if view_name else scene.get('camera')
    report = {'schema': 1, 'map': scene['map'], 'indexed_actors': len(indexed),
              'unassigned': _unassigned(scene), 'policy_audit': _policy_audit(scene),
              'limitations': ['Host manifest/OBJ provenance only; original-game parity is UNVERIFIED.',
                              'Terrain endpoint assertions do not prove an original hole.',
                              'No native lightmap or BSP PolyFlags semantics are inferred.']}
    if camera:
        coverage = _coverage(indexed, camera['location'])
        report['camera'] = {'name': view_name or 'scene.camera', 'location': camera['location'],
                           'coverage': coverage[:32],
                           'opening_coverage_gap': _opening_gap(coverage, camera['location'])}
        rays = []
        for pixel in pixels:
            origin, direction = _camera_ray(camera, pixel, width, height)
            rays.append({'pixel': list(pixel), 'direction': list(direction),
                         'hits': _ray_hits(indexed, origin, direction, include_hidden=True),
                         'visible_hits': _ray_hits(indexed, origin, direction, include_hidden=False)})
        if rays:
            report['camera']['rays'] = rays
    report['summary'] = {'terrain_actors': sum(item['scope'] == 'terrain' for item in indexed),
                         'bsp_actors': sum(item['scope'] == 'bsp' for item in indexed),
                         'static_or_prop_actors': sum(item['scope'] == 'static_or_prop' for item in indexed),
                         'unassigned_sections': report['unassigned']['count']}
    return report


def _parse_pixel(value):
    fields = value.split(',')
    if len(fields) != 2:
        raise argparse.ArgumentTypeError('Pixel must be X,Y')
    try:
        return int(fields[0]), int(fields[1])
    except ValueError as error:
        raise argparse.ArgumentTypeError('Pixel must be integer X,Y') from error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scene', type=Path, required=True,
                        help='Directory containing scene.json and generated OBJ files')
    parser.add_argument('--inspection', type=Path,
                        help='Optional inspection-views.json for a named camera')
    parser.add_argument('--view', help='Inspection view name; defaults to scene.camera')
    parser.add_argument('--pixel', type=_parse_pixel, action='append', default=[],
                        help='Trace one 1280x720 camera pixel as X,Y; repeatable')
    parser.add_argument('--output', type=Path, help='Write the JSON report under local/')
    args = parser.parse_args()
    manifest = args.scene / 'scene.json'
    if not manifest.is_file():
        parser.error(f'Missing scene manifest: {manifest}')
    inspection = json.loads(args.inspection.read_text(encoding='utf-8')) if args.inspection else None
    report = analyse(json.loads(manifest.read_text(encoding='utf-8')), args.scene,
                     inspection, args.view, args.pixel)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'summary': report['summary'],
                      'camera': report.get('camera', {}).get('name'),
                      'unassigned_by_mesh': {k: len(v) for k, v in report['unassigned']['by_mesh'].items()},
                      'limitations': report['limitations']}, indent=2))


if __name__ == '__main__':
    main()
