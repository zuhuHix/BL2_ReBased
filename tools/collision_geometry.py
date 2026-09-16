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


def hulls(properties):
    aggregate_tags = fields(properties).get('AggGeom', [])
    for tag in aggregate_tags:
        if tag.get('status') != 'decoded':
            raise ValueError('Undecoded aggregate collision field: ' + tag['name'])
    aggregate = fields(aggregate_tags)
    result = []
    for kind, elements in aggregate.items():
        if kind not in ('ConvexElems', 'BoxElems') and elements:
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
                tm = p['TM']
                rows = [point(tm[k]) for k in ('XPlane', 'YPlane', 'ZPlane', 'WPlane')]
                if any(abs(tm[k]['W']) > 1e-5 for k in ('XPlane', 'YPlane', 'ZPlane')) or abs(tm['WPlane']['W'] - 1) > 1e-5:
                    raise ValueError('Non-affine collision box transform')
                vertices = [[rows[3][i] + sum(signs[j] * sizes[j] * .5 * rows[j][i] for j in range(3))
                             for i in range(3)] for signs in product((-1, 1), repeat=3)]
            else:
                continue
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
            result.append({'vertices': vertices})
    if len(result) > 4096:
        raise ValueError('Too many collision hulls')
    return result
