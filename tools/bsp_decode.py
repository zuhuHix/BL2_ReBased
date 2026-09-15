"""Observed root Model/ModelComponent polygons; no native BSP solver or lightmaps.

Original implementation from installed-package observations. Model nodes,
vertex-pool point references, surface planes and component membership must
agree. Unused native fields are retained as opaque hashes, never guessed.
"""
import hashlib
import math
import struct

from terrain_decode import native_offset

MAX_ITEMS = 1_000_000
PLANE_TOLERANCE = 0.02  # cm; observed float roundoff below 0.011 in both maps


class Cursor:
    def __init__(self, payload, offset):
        self.payload, self.offset = payload, offset

    def take(self, size):
        if size < 0 or size > len(self.payload) - self.offset:
            raise ValueError('Truncated BSP payload')
        start = self.offset
        self.offset += size
        return self.payload[start:self.offset]

    def read(self, fmt):
        return struct.unpack('<' + fmt, self.take(struct.calcsize('<' + fmt)))

    def array(self, stride, bulk=False):
        if bulk and self.read('I')[0] != stride:
            raise ValueError('Unsupported BSP array stride')
        count = self.read('I')[0]
        if count > MAX_ITEMS or count > (len(self.payload) - self.offset) // stride:
            raise ValueError('Invalid BSP array count')
        return [self.take(stride) for _ in range(count)]


def digest(data):
    return {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(), 'status': 'UNVERIFIED'}


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def sub(a, b):
    return tuple(x - y for x, y in zip(a, b))


def cross(a, b):
    return (a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0])


def finite(values):
    if not all(math.isfinite(v) for v in values):
        raise ValueError('Nonfinite BSP geometry')


def require_root(record, records, class_name):
    owner = records.get(record['outer'], {})
    if (record['class'] != class_name or owner.get('class') != 'Engine.Level'
            or owner.get('path') != 'TheWorld.PersistentLevel'):
        raise ValueError('BSP object is not directly owned by the persistent level')


def polygon_triangles(points, plane):
    """Validate a convex, planar, ordered polygon before constructing its fan.

    Collinear boundary points remain in the polygon; zero-area fan triangles
    are omitted. Ordering agrees with the native plane, then is reversed only
    on OBJ export to match the host adapter's established index convention.
    """
    if len(points) < 3 or len(set(points)) != len(points):
        raise ValueError('Degenerate BSP polygon')
    finite(plane)
    normal = plane[:3]
    if abs(dot(normal, normal) - 1) > 0.001:
        raise ValueError('Nonunit BSP plane')
    for p in points:
        finite(p)
        if abs(dot(p, normal) - plane[3]) > PLANE_TOLERANCE:
            raise ValueError('BSP point does not lie on its plane')
    # All points must lie on the inward side of every directed boundary edge.
    for i, a in enumerate(points):
        edge = sub(points[(i + 1) % len(points)], a)
        length = math.sqrt(dot(edge, edge))
        if length < 1e-6:
            raise ValueError('Zero-length BSP edge')
        if any(dot(cross(edge, sub(p, a)), normal) < -PLANE_TOLERANCE * length for p in points):
            raise ValueError('Nonconvex or reversed BSP polygon')
    triangles = []
    for i in range(1, len(points) - 1):
        edge = sub(points[i], points[0])
        area2 = dot(cross(edge, sub(points[i+1], points[0])), normal)
        # Bound collinearity in centimeters, not square centimeters: long
        # serialized edges amplify sub-millimeter float rounding in area.
        tolerance = PLANE_TOLERANCE * math.sqrt(dot(edge, edge))
        if area2 > tolerance:
            triangles.append((0, i, i+1))
        elif area2 < -tolerance:
            raise ValueError('Reversed BSP fan triangle')
    if not triangles:
        raise ValueError('Zero-area BSP polygon')
    return triangles


def decode_model(payload, record, records):
    require_root(record, records, 'Engine.Model')
    start = native_offset(payload, record['data'], 4)
    c = Cursor(payload, start)
    if c.take(28) != bytes(28):
        raise ValueError('Unsupported root Model prefix')
    vectors = [struct.unpack('<3f', b) for b in c.array(12, True)]
    points = [struct.unpack('<3f', b) for b in c.array(12, True)]
    nodes = [struct.unpack('<4f12i', b) for b in c.array(64, True)]
    if c.read('i')[0] != record['index']:
        raise ValueError('Model self reference mismatch')
    surface_bytes = c.array(60)
    surfaces = [struct.unpack('<8i4f3i', b) for b in surface_bytes]
    vertex_bytes = c.array(24, True)
    vertices = [struct.unpack('<2i4f', b) for b in vertex_bytes]
    for v in vectors + points:
        finite(v)
    polygons = []
    for i, n in enumerate(nodes):
        first, surface = n[4:6]
        count = (n[13] >> 16) & 255
        if count < 3 or first < 0 or first + count > len(vertices) or not 0 <= surface < len(surfaces):
            raise ValueError('Invalid BSP polygon range or surface')
        s = surfaces[surface]
        normal_index = s[3]
        if not 0 <= normal_index < len(vectors):
            raise ValueError('Invalid BSP surface normal index')
        if max(abs(a-b) for a,b in zip(vectors[normal_index], s[8:11])) > 0.001:
            raise ValueError('BSP surface vector and normal disagree')
        indices = [v[0] for v in vertices[first:first+count]]
        if any(not 0 <= p < len(points) for p in indices):
            raise ValueError('Invalid BSP point reference')
        polygon = [points[p] for p in indices]
        triangles = polygon_triangles(polygon, n[:4])
        finite(s[8:12])
        if (dot(n[:3], s[8:11]) < 0.999 or
                any(abs(dot(p, s[8:11])-s[11]) > PLANE_TOLERANCE for p in polygon)):
            raise ValueError('BSP node and surface planes disagree')
        polygons.append({'node': i, 'surface': surface, 'material': s[0],
                         'component': n[7] & 65535, 'component_node': (n[7] >> 16) & 65535,
                         'points': polygon, 'plane': n[:4], 'triangles': triangles})
    return {'index': record['index'], 'path': record['path'], 'polygons': polygons,
            'opaque_remainder': digest(payload[c.offset:]),
            'opaque_surface_records': digest(b''.join(surface_bytes)),
            'opaque_vertex_records': digest(b''.join(vertex_bytes)),
            'consumed_native_bytes': c.offset - start}


def decode_component(payload, record, records, model):
    require_root(record, records, 'Engine.ModelComponent')
    c = Cursor(payload, native_offset(payload, record['data'], 8))
    if c.read('i')[0] != model['index'] or c.read('I')[0] != 1:
        raise ValueError('Unsupported ModelComponent header')
    count = c.read('I')[0]
    if not 0 < count <= len(model['polygons']):
        raise ValueError('Invalid BSP element count')
    elements = []
    for _ in range(count):
        opaque_start = c.offset
        kind = c.read('I')[0]
        if kind == 2:
            c.array(16)  # opaque identities
            c.take(64)   # observed fixed lighting block; semantics not interpreted
        elif kind != 0:
            raise ValueError('Unsupported BSP lighting block')
        lighting = digest(payload[opaque_start:c.offset])
        if c.read('i')[0] != record['index']:
            raise ValueError('BSP element self reference mismatch')
        material = c.read('i')[0]
        node_ids = [struct.unpack('<H', b)[0] for b in c.array(2)]
        auxiliary = c.array(4)
        identities = c.array(16)
        if not node_ids or len(node_ids) != len(set(node_ids)):
            raise ValueError('Empty or duplicate BSP element membership')
        for i in node_ids:
            if not 0 <= i < len(model['polygons']) or model['polygons'][i]['material'] != material:
                raise ValueError('BSP element material or node mismatch')
        elements.append({'material': material, 'nodes': node_ids, 'lighting': lighting,
                         'opaque_auxiliary': digest(b''.join(auxiliary + identities))})
    ordinal = c.read('H')[0]
    node_ids = [struct.unpack('<H', b)[0] for b in c.array(2)]
    if c.offset != len(payload):
        raise ValueError('Unexpected ModelComponent remainder')
    if (len(set(node_ids)) != len(node_ids) or
            sorted(node_ids) != sorted(i for e in elements for i in e['nodes'])):
        raise ValueError('BSP component and element memberships disagree')
    for position, i in enumerate(node_ids):
        p = model['polygons'][i]
        if p['component'] != ordinal or p['component_node'] != position:
            raise ValueError('BSP node backreference mismatch')
    return {'index': record['index'], 'path': record['path'], 'ordinal': ordinal,
            'nodes': node_ids, 'elements': elements}


def validate_coverage(model, components):
    ids = [i for c in components for i in c['nodes']]
    if sorted(ids) != list(range(len(model['polygons']))):
        raise ValueError('BSP components do not cover every Model node exactly once')
    if len({c['ordinal'] for c in components}) != len(components):
        raise ValueError('Duplicate BSP component ordinal')
