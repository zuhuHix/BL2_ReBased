"""Cross-check a prepared scene's meshes, textures and material picks against umodel exports.

UE Viewer (umodel) is run beforehand as an independent oracle (``-export -gltf -png``;
see docs/TOOLING.md) and its output tree is read here, never its code. For every
mesh in ``scene.json`` the matching ``<Level>/<Outer>/<Group>/<Name>.gltf`` is
compared with our per-section OBJ files: section count, triangles per section,
referenced vertex count, vertex positions (after an empirically chosen axis
mapping, reported in the output), texture coordinates (identity or V-flipped)
and the section's material name. For every material channel the decoded PNG is
compared with umodel's PNG: dimensions and per-channel mean absolute difference.
umodel's ``.mat`` summaries are compared with our channel picks as a third,
informational check.

Agreement means two independent decoders produce the same geometry and pixels
from the same bytes. It says nothing about how the original renders them.
Game-derived output stays under ``local/``.
"""
import argparse
from collections import Counter
import hashlib
import itertools
import json
import math
from pathlib import Path
import struct
import subprocess
import sys

COMPONENT = {5120: ('b', 1), 5121: ('B', 1), 5122: ('h', 2), 5123: ('H', 2), 5125: ('I', 4), 5126: ('f', 4)}
WIDTH = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3, 'VEC4': 4}
MAT_CHANNELS = {'Diffuse': 'diffuse', 'Normal': 'normal', 'Emissive': 'emissive', 'Specular': 'specular'}
# Candidate mappings from umodel glTF metres to UE centimetres: axis permutation and signs.
CANDIDATES = [(perm, signs) for perm in itertools.permutations(range(3)) for signs in itertools.product((1, -1), repeat=3)]


def read_accessor(gltf, binary, index):
    accessor = gltf['accessors'][index]
    view = gltf['bufferViews'][accessor['bufferView']]
    fmt, size = COMPONENT[accessor['componentType']]
    width = WIDTH[accessor['type']]
    start = view.get('byteOffset', 0) + accessor.get('byteOffset', 0)
    stride = view.get('byteStride', size * width)
    out = []
    for i in range(accessor['count']):
        at = start + i * stride
        out.append(struct.unpack_from('<' + fmt * width, binary, at))
    return out


def load_gltf(path):
    """Return a list of primitives: dict(vertices, triangles, positions, uvs, material)."""
    gltf = json.loads(path.read_text(encoding='utf-8'))
    binary = (path.parent / gltf['buffers'][0]['uri']).read_bytes()
    primitives = []
    for mesh in gltf['meshes']:
        for primitive in mesh['primitives']:
            attributes = primitive['attributes']
            indices = read_accessor(gltf, binary, primitive['indices']) if 'indices' in primitive else None
            positions = read_accessor(gltf, binary, attributes['POSITION'])
            uvs = read_accessor(gltf, binary, attributes['TEXCOORD_0']) if 'TEXCOORD_0' in attributes else []
            material = gltf['materials'][primitive['material']].get('name') if 'material' in primitive else None
            primitives.append(dict(vertices=len(positions), triangles=len(indices) // 3 if indices else len(positions) // 3,
                                   positions=positions, uvs=uvs, material=material))
    return primitives


def load_obj(path):
    """Return dict(positions, uvs, faces) from one of our OBJ files; faces are index triples (0-based)."""
    positions, uvs, faces, material = [], [], [], None
    for line in path.read_text(encoding='utf-8').splitlines():
        parts = line.split()
        if not parts:
            continue
        if parts[0] == 'v':
            positions.append(tuple(float(x) for x in parts[1:4]))
        elif parts[0] == 'vt':
            uvs.append(tuple(float(x) for x in parts[1:3]))
        elif parts[0] == 'f':
            faces.append(tuple(int(p.split('/')[0]) - 1 for p in parts[1:4]))
        elif parts[0] == '#' and len(parts) > 2 and parts[1] == 'material':
            material = parts[2]
    return dict(positions=positions, uvs=uvs, faces=faces, material=material)


def map_position(point, mapping):
    perm, signs = mapping
    return tuple(point[perm[i]] * signs[i] * 100.0 for i in range(3))


def bbox(points):
    return [min(p[i] for p in points) for i in range(3)], [max(p[i] for p in points) for i in range(3)]


def choose_mapping(pairs):
    """Pick the axis mapping whose mapped umodel bounding boxes best match ours over (ours, umodel) point-list pairs."""
    best, best_error = None, None
    for mapping in CANDIDATES:
        error = 0.0
        for ours, theirs in pairs:
            lo, hi = bbox(ours)
            tlo, thi = bbox([map_position(p, mapping) for p in theirs])
            error += sum(abs(a - b) for a, b in zip(lo + hi, tlo + thi))
        if best_error is None or error < best_error:
            best, best_error = mapping, error
    return best, best_error


def key_set(points, scale):
    return {tuple(int(round(c * scale)) for c in p) for p in points}


def match_fraction(ours, theirs, scale):
    """Fraction of ``theirs`` found in ``ours`` after rounding to 1/scale, tolerant of rounding-boundary straddles."""
    if not theirs:
        return None
    keys = key_set(ours, scale)
    hits = 0
    for p in theirs:
        cells = [(math.floor(c * scale), math.floor(c * scale) + 1) for c in p]
        if any(candidate in keys for candidate in itertools.product(*cells)):
            hits += 1
    return hits / len(theirs)


def position_scale(points):
    """Cells of 0.01 cm for ordinary props; 0.1 cm once float32 metres lose that precision (|coord| > 200 m)."""
    largest = max((abs(c) for p in points for c in p), default=0.0)
    return 10.0 if largest > 20000.0 else 100.0


def compare_mesh(sections, primitives, mapping, uv_scale=1000.0):
    """Compare our per-section OBJ dicts (in slot order) with umodel primitives.

    umodel names a section material ``dummy_material_N`` when the material lives in a package it did
    not load; that is treated as no oracle, not a disagreement.
    """
    report = dict(sections=len(sections), umodel_sections=len(primitives), section_reports=[], mismatches=[])
    if len(sections) != len(primitives):
        report['mismatches'].append('section_count')
    for i, (section, primitive) in enumerate(zip(sections, primitives)):
        obj = section['obj']
        referenced = sorted({v for face in obj['faces'] for v in face})
        ours_positions = [obj['positions'][v] for v in referenced]
        ours_uvs = [obj['uvs'][v] for v in referenced] if obj['uvs'] else []
        theirs_positions = [map_position(p, mapping) for p in primitive['positions']]
        scale = position_scale(ours_positions)
        umodel_material = primitive['material']
        if umodel_material and umodel_material.startswith('dummy_material_'):
            umodel_material = None
        entry = dict(slot=section['slot'], triangles=len(obj['faces']), umodel_triangles=primitive['triangles'],
                     vertices=len(referenced), umodel_vertices=primitive['vertices'],
                     position_match=match_fraction(ours_positions, theirs_positions, scale), position_cell_cm=1.0 / scale,
                     material=section.get('material_name'), umodel_material=umodel_material)
        if ours_uvs and primitive['uvs']:
            entry['uv_match_identity'] = match_fraction(ours_uvs, primitive['uvs'], uv_scale)
            entry['uv_match_vflip'] = match_fraction(ours_uvs, [(u, 1.0 - v) for u, v in primitive['uvs']], uv_scale)
        if entry['triangles'] != entry['umodel_triangles']:
            report['mismatches'].append(f'triangles[{i}]')
        if entry['vertices'] != entry['umodel_vertices']:
            report['mismatches'].append(f'vertices[{i}]')
        if entry['position_match'] is not None and entry['position_match'] < 1.0:
            report['mismatches'].append(f'positions[{i}]')
        if entry['material'] and umodel_material and entry['material'] != umodel_material:
            report['mismatches'].append(f'material[{i}]')
        report['section_reports'].append(entry)
    return report


def parse_mat(text):
    """umodel .mat summary: Key=Value lines; Other[N] entries are unclassified textures."""
    out = {}
    for line in text.splitlines():
        if '=' in line:
            key, value = line.split('=', 1)
            out[key.strip()] = value.strip()
    return out


def compare_material(channels, mat):
    """Compare our channel texture short names with umodel's .mat picks."""
    result = {}
    for mat_key, channel in MAT_CHANNELS.items():
        ours, theirs = channels.get(channel), mat.get(mat_key)
        if ours is None and theirs is None:
            continue
        if ours is None:
            status = 'umodel_only'
        elif theirs is None:
            others = [v for k, v in mat.items() if k.startswith('Other[')]
            status = 'ours_in_umodel_other' if ours in others else 'ours_only'
        else:
            status = 'agree' if ours == theirs else 'differ'
        result[channel] = dict(status=status, ours=ours, umodel=theirs)
    return result


def compare_textures(ours_path, theirs_path):
    from PIL import Image
    import numpy as np
    ours = Image.open(ours_path)
    theirs = Image.open(theirs_path)
    report = dict(size=list(ours.size), umodel_size=list(theirs.size), mode=ours.mode, umodel_mode=theirs.mode)
    if ours.size != theirs.size:
        report['mismatch'] = 'size'
        return report
    a = np.asarray(ours.convert('RGBA'), dtype=np.int16)
    b = np.asarray(theirs.convert('RGBA'), dtype=np.int16)
    diff = np.abs(a - b)
    report['mad'] = [round(float(x), 3) for x in diff.reshape(-1, 4).mean(axis=0)]
    report['max'] = [int(x) for x in diff.reshape(-1, 4).max(axis=0)]
    return report


def identity_hash(package, path):
    return hashlib.sha256(f'{package}:{path}'.encode()).hexdigest()[:24]


def texture_index(reader, cooked, packages):
    """Map identity hash -> (package, path) for every Texture2D export in the given packages."""
    index = {}
    for package in packages:
        candidates = [cooked / f'{package}.upk']
        upk = next((c for c in candidates if c.exists()), None)
        if upk is None:
            continue
        result = subprocess.run([str(reader), str(upk), '--exports'], capture_output=True, text=True, encoding='utf-8', errors='replace')
        if result.returncode != 0:
            continue
        for record in json.loads(result.stdout):
            if record['class'].rsplit('.', 1)[-1] == 'Texture2D':
                index[identity_hash(package, record['path'])] = (package, record['path'])
    return index


def oracle_path(root, source, suffix):
    package, path = source.split(':', 1)
    return root / package / Path(*path.split('.')).with_suffix(suffix)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--scene', type=Path, required=True, help='prepared scene.json (its directory holds the OBJ/PNG files)')
    parser.add_argument('--umodel-exports', type=Path, required=True, help='root of umodel -export output, one folder per level package')
    parser.add_argument('--reader', type=Path, required=True, help='ow-package executable (used to invert texture identity hashes)')
    parser.add_argument('--game', type=Path, required=True, help='Borderlands 2 install root')
    parser.add_argument('--extra-packages', nargs='*', default=[], help='additional packages to index for texture identities')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()

    scene = json.loads(args.scene.read_text(encoding='utf-8'))
    scene_dir = args.scene.parent
    cooked = args.game / 'WillowGame' / 'CookedPCConsole'
    materials = scene['materials']

    # Meshes: load everything first so the axis mapping is chosen over the whole set.
    loaded = {}
    for mesh_id, mesh in scene['meshes'].items():
        gltf = oracle_path(args.umodel_exports, mesh['source'], '.gltf')
        if not gltf.exists():
            loaded[mesh_id] = dict(source=mesh['source'], status='oracle_missing')
            continue
        sections = []
        for section in mesh['sections']:
            obj = load_obj(scene_dir / section['file'])
            material = materials.get(section.get('material') or '', {}).get('source')
            sections.append(dict(slot=section['slot'], obj=obj, material_name=material.rsplit('.', 1)[-1] if material else None))
        loaded[mesh_id] = dict(source=mesh['source'], sections=sections, primitives=load_gltf(gltf))

    pairs = []
    for item in loaded.values():
        if 'sections' in item and len(item['sections']) == len(item['primitives']):
            for section, primitive in zip(item['sections'], item['primitives']):
                referenced = sorted({v for face in section['obj']['faces'] for v in face})
                if referenced and primitive['positions']:
                    pairs.append(([section['obj']['positions'][v] for v in referenced], primitive['positions']))
    mapping, mapping_error = choose_mapping(pairs[:200]) if pairs else (((0, 1, 2), (1, 1, 1)), None)

    mesh_reports = {}
    for mesh_id, item in loaded.items():
        if item.get('status') == 'oracle_missing':
            mesh_reports[mesh_id] = item
            continue
        report = compare_mesh(item['sections'], item['primitives'], mapping)
        report['source'] = item['source']
        report['status'] = 'agree' if not report['mismatches'] else 'differ'
        mesh_reports[mesh_id] = report

    # Textures: invert identity hashes through the reader's export tables.
    packages = list(dict.fromkeys(scene['levels'] + [m['source'].split(':')[0] for m in materials.values()] + args.extra_packages))
    index = texture_index(args.reader, cooked, packages)
    texture_reports = {}
    material_reports = {}
    for material_id, material in materials.items():
        short_names = {}
        for channel, filename in material.get('channels', {}).items():
            if not filename:
                continue
            digest = filename.split('_')[0]
            identity = index.get(digest)
            if identity is None:
                texture_reports[filename] = dict(status='identity_unresolved')
                continue
            source = f'{identity[0]}:{identity[1]}'
            short_names[channel] = identity[1].rsplit('.', 1)[-1]
            if filename in texture_reports:
                continue
            oracle = oracle_path(args.umodel_exports, source, '.png')
            ours = scene_dir / filename
            if not oracle.exists():
                texture_reports[filename] = dict(source=source, status='oracle_missing')
                continue
            report = compare_textures(ours, oracle)
            report.update(source=source, channel=channel)
            # Independent DXT decoders round differently; a maximum per-channel difference of 1 is decoder noise.
            largest = max(report['max']) if 'max' in report else None
            report['status'] = ('differ' if report.get('mismatch') else 'agree' if largest == 0
                                else 'agree_within_1' if largest <= 1 else 'pixels_differ')
            texture_reports[filename] = report
        mat = oracle_path(args.umodel_exports, material['source'], '.mat')
        if mat.exists():
            material_reports[material_id] = dict(source=material['source'],
                                                 channels=compare_material(short_names, parse_mat(mat.read_text(encoding='latin-1'))))
        else:
            material_reports[material_id] = dict(source=material['source'], status='oracle_missing')

    mesh_status = Counter(r.get('status') for r in mesh_reports.values())
    texture_status = Counter(r.get('status') for r in texture_reports.values())
    material_status = Counter(c['status'] for r in material_reports.values() for c in r.get('channels', {}).values())
    mesh_mismatch_kinds = Counter(m.split('[')[0] for r in mesh_reports.values() for m in r.get('mismatches', []))
    uv_identity = [s['uv_match_identity'] for r in mesh_reports.values() for s in r.get('section_reports', []) if 'uv_match_identity' in s]
    uv_vflip = [s['uv_match_vflip'] for r in mesh_reports.values() for s in r.get('section_reports', []) if 'uv_match_vflip' in s]
    summary = dict(
        meshes=len(mesh_reports), mesh_status=dict(mesh_status), mesh_mismatch_kinds=dict(mesh_mismatch_kinds),
        axis_mapping=dict(permutation=list(mapping[0]), signs=list(mapping[1]), scale=100.0, bbox_error_cm=mapping_error,
                          note='umodel glTF (x,y,z) in metres -> UE centimetres: ue[i] = gltf[permutation[i]] * signs[i] * 100'),
        uv_sections=len(uv_identity),
        uv_identity_exact=sum(1 for f in uv_identity if f == 1.0), uv_vflip_exact=sum(1 for f in uv_vflip if f == 1.0),
        textures=len(texture_reports), texture_status=dict(texture_status),
        materials_with_oracle=sum(1 for r in material_reports.values() if 'channels' in r),
        material_channel_status=dict(material_status),
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(dict(summary=summary, meshes=mesh_reports, textures=texture_reports,
                                           materials=material_reports), indent=1), encoding='utf-8')
    print(json.dumps(summary, indent=1))
    for mesh_id, report in mesh_reports.items():
        if report.get('status') == 'differ':
            print(f"  mesh {report['source']}: {report['mismatches']}")
    hard = mesh_status.get('differ', 0) + texture_status.get('differ', 0) + texture_status.get('pixels_differ', 0)
    return 1 if hard else 0


if __name__ == '__main__':
    sys.exit(main())
