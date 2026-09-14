"""Compare saved UE LOD0 UV0 bindings to prepared OBJ corners (editor Python)."""
import json
from collections import Counter
import os
from pathlib import Path
import struct
import unreal

root = Path(os.environ['OPENWILLOW_SCENE']).resolve()
scene = json.loads((root / 'scene.json').read_text(encoding='utf-8'))
report_path = root / 'ue-uv-verify.json'
report_path.unlink(missing_ok=True)
base = '/Game/OpenWillow/' + scene['map'] + '/Assets/'


def f32(value):
    return struct.unpack('<f', struct.pack('<f', float(value)))[0]


def expected_bindings(path):
    vertices, uvs, expected, triangles = [], [], {}, Counter()
    for line in path.read_text().splitlines():
        fields = line.split()
        if not fields:
            continue
        if fields[0] == 'v':
            vertices.append(tuple(f32(v) for v in fields[1:4]))
        elif fields[0] == 'vt':
            # The reader exports 1-sourceV for OBJ; UE imports 1-objV.
            # Therefore this comparison expects the original source UV0.
            uvs.append((float(fields[1]), 1 - float(fields[2])))
        elif fields[0] == 'f':
            assert len(fields) == 4, (path, 'expected exported triangles')
            triangle = []
            for corner in fields[1:]:
                indices = corner.split('/')
                vi, ti = int(indices[0]), int(indices[1])
                assert 1 <= vi <= len(vertices) and 1 <= ti <= len(uvs), (path, corner)
                expected.setdefault(vertices[vi - 1], set()).add(uvs[ti - 1])
                triangle.append(vertices[vi - 1])
            triangles[oriented_triangle(triangle)] += 1
    return expected, triangles


def oriented_triangle(points):
    # Ignore cyclic start vertex and triangle ordering, but retain winding.
    return min(tuple(points[i:] + points[:i]) for i in range(3))


sections, corners, triangle_count = 0, 0, 0
for definition in scene['meshes'].values():
    for section in definition['sections']:
        path = root / section['file']
        expected, expected_triangles = expected_bindings(path)
        mesh = unreal.load_asset(base + path.stem)
        assert isinstance(mesh, unreal.StaticMesh), path
        description = mesh.get_static_mesh_description(0)
        assert description is not None, path
        matched = set()
        for index in range(description.get_vertex_instance_count()):
            instance = unreal.VertexInstanceID(index)
            assert description.is_vertex_instance_valid(instance), (path, index)
            point = description.get_vertex_position(description.get_vertex_instance_vertex(instance))
            position = (point.x, point.y, point.z)
            uv = description.get_vertex_instance_uv(instance, 0)
            candidates = expected.get(position, set())
            matches = [value for value in candidates
                       if abs(uv.x - value[0]) <= 1e-5 and abs(uv.y - value[1]) <= 1e-5]
            assert matches, (path.name, position, (uv.x, uv.y), sorted(candidates))
            matched.update((position, value) for value in matches)
            corners += 1
        required = {(position, uv) for position, values in expected.items() for uv in values}
        assert matched == required, (path.name, 'missing source UV bindings', len(required - matched))
        actual_triangles = Counter()
        for index in range(description.get_triangle_count()):
            triangle = unreal.TriangleID(index)
            assert description.is_triangle_valid(triangle), (path.name, index)
            points = []
            for instance in description.get_triangle_vertex_instances(triangle):
                p = description.get_vertex_position(description.get_vertex_instance_vertex(instance))
                points.append((p.x, p.y, p.z))
            actual_triangles[oriented_triangle(points)] += 1
        assert actual_triangles == expected_triangles, (path.name, 'triangle winding/topology differs from source OBJ')
        triangle_count += description.get_triangle_count()
        sections += 1

report = {'map': scene['map'], 'verified_sections': sections, 'verified_corners': corners,
          'uv_channel': 0, 'uv_tolerance': 1e-5,
          'verified_triangles': triangle_count, 'winding': 'matches source UE index order',
          'result': 'saved UE UV0 bindings match prepared OBJ with inverse OBJ V conversion',
          'material_graph_uvs': 'unverified', 'visual_parity': 'unverified'}
report_path.write_text(json.dumps(report, indent=2) + '\n')
unreal.log('OpenWillow saved UV verification: ' + json.dumps(report))
