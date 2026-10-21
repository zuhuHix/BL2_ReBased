"""Observed BL2 tagged aggregate geometry; no PhysX cache decoding."""
import math
from itertools import product


def fields(tags):
    return {t['name']: t['value'] for t in tags if t.get('status') == 'decoded'}


def point(value):
    v = [float(value[k]) for k in ('X', 'Y', 'Z')]
    if not all(math.isfinite(x) for x in v):
        raise ValueError('Nonfinite collision vertex')
    return v


def affine_rows(tm):
    """The X, Y, Z axis rows and the origin row of a UE3 FMatrix (row vectors)."""
    rows = [point(tm[k]) for k in ('XPlane', 'YPlane', 'ZPlane', 'WPlane')]
    if any(abs(tm[k]['W']) > 1e-5 for k in ('XPlane', 'YPlane', 'ZPlane')) or abs(tm['WPlane']['W'] - 1) > 1e-5:
        raise ValueError('Non-affine collision transform')
    return rows


def rigid_rows(tm):
    """Affine rows whose axes are orthonormal: spheres and capsules carry no scale."""
    rows = affine_rows(tm)
    for i in range(3):
        for j in range(3):
            dot = sum(a * b for a, b in zip(rows[i], rows[j]))
            if abs(dot - (i == j)) > 1e-3:
                raise ValueError('Scaled or sheared sphere/capsule transform')
    return rows


def radius(value, allow_zero=False):
    r = float(value)
    if not math.isfinite(r) or r < 0 or (r == 0 and not allow_zero):
        raise ValueError('Invalid collision radius or length')
    return r


def convex_volume_check(vertices):
    if not 4 <= len(vertices) <= 4096:
        raise ValueError('Invalid convex vertex count')
    # A convex physics shape needs volume, not merely four entries.
    edges = [[v[i] - vertices[0][i] for i in range(3)] for v in vertices[1:]]
    edge = next((e for e in edges if sum(x*x for x in e) > 1e-12), None)
    normal = None
    if edge:
        for e in edges:
            n = [edge[1]*e[2]-edge[2]*e[1], edge[2]*e[0]-edge[0]*e[2], edge[0]*e[1]-edge[1]*e[0]]
            length = math.sqrt(sum(x*x for x in n))
            if length > 1e-8:
                normal = [x / length for x in n]
                break
    if normal is None or not any(abs(sum(n*x for n, x in zip(normal, e))) > 1e-5 for e in edges):
        raise ValueError('Degenerate collision hull')


def shapes(properties, key='AggGeom'):
    """Decode a KAggregateGeom property (RB_BodySetup.AggGeom, BrushComponent.BrushAggGeom).

    Returns {'hulls': [{'vertices'}], 'spheres': [{'center', 'radius'}],
    'capsules': [{'center', 'axes', 'radius', 'length'}]} in the owner's local space.
    Capsule 'axes' are the TM's X, Y, Z rows; the capsule's long axis is taken to be
    the TM's Z axis with 'length' between the two hemisphere centres, as in UE5's
    FKSphylElem. That reading of KSphylElem is UNVERIFIED against the running game.
    """
    aggregate_tags = fields(properties).get(key, [])
    for tag in aggregate_tags:
        if tag.get('status') != 'decoded':
            raise ValueError('Undecoded aggregate collision field: ' + tag['name'])
    aggregate = fields(aggregate_tags)
    result = {'hulls': [], 'spheres': [], 'capsules': []}
    for kind, elements in aggregate.items():
        if kind not in ('ConvexElems', 'BoxElems', 'SphereElems', 'SphylElems') and elements:
            raise ValueError('Unsupported collision shape: ' + kind)
        for tags in elements:
            p = fields(tags)
            if p.get('bNoRBCollision', False) or p.get('bPerPolyShape', False):
                raise ValueError('Unsupported collision shape flags')
            if kind == 'ConvexElems':
                vertices = [point(v) for v in p.get('VertexData', [])]
                indices = p.get('FaceTriData', [])
                if len(indices) % 3 or any(type(i) is not int or i < 0 or i >= len(vertices) for i in indices):
                    raise ValueError('Invalid collision triangle indices')
                bounds = p.get('ElemBox')
                if bounds:
                    lo, hi = point(bounds['Min']), point(bounds['Max'])
                    if bounds['IsValid'] != 1 or any(lo[i] > hi[i] for i in range(3)):
                        raise ValueError('Invalid collision bounds')
                    if any(v[i] < lo[i] - .1 or v[i] > hi[i] + .1 for v in vertices for i in range(3)):
                        raise ValueError('Collision vertices outside source bounds')
            elif kind == 'BoxElems':
                sizes = [float(p[k]) for k in ('X', 'Y', 'Z')]
                if not all(math.isfinite(x) and x > 0 for x in sizes):
                    raise ValueError('Invalid collision box dimensions')
                rows = affine_rows(p['TM'])
                vertices = [[rows[3][i] + sum(signs[j] * sizes[j] * .5 * rows[j][i] for j in range(3))
                             for i in range(3)] for signs in product((-1, 1), repeat=3)]
            elif kind == 'SphereElems':
                rows = rigid_rows(p['TM'])
                result['spheres'].append({'center': rows[3], 'radius': radius(p['Radius'])})
                continue
            else:
                rows = rigid_rows(p['TM'])
                result['capsules'].append({'center': rows[3], 'axes': rows[:3], 'radius': radius(p['Radius']),
                                           'length': radius(p.get('Length', 0), allow_zero=True)})
                continue
            convex_volume_check(vertices)
            result['hulls'].append({'vertices': vertices})
    if sum(len(v) for v in result.values()) > 4096:
        raise ValueError('Too many collision hulls')
    return result


def hulls(properties, key='AggGeom'):
    """Convex and box elements only; spheres and capsules stay explicit errors here."""
    result = shapes(properties, key)
    for kind, name in (('spheres', 'SphereElems'), ('capsules', 'SphylElems')):
        if result[kind]:
            raise ValueError('Unsupported collision shape: ' + name)
    return result['hulls']
