"""Synthetic test for tools/slice_npc_assets.candidate_sections_gltf: a fragment name with several ranges keeps all of them."""
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

try:
    import numpy  # noqa: F401  (tools/slice_npc_assets.py imports it at module level)
except ImportError:
    numpy = None
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))


def write_gltf(folder, indices):
    """One mesh, one LOD section (material 0) with the given 16-bit index list; the vertex data is not read."""
    blob = struct.pack(f'<{len(indices)}H', *indices)
    gltf = {'buffers': [{'uri': 'src.bin', 'byteLength': len(blob)}],
            'bufferViews': [{'buffer': 0, 'byteOffset': 0, 'byteLength': len(blob)}],
            'accessors': [{'bufferView': 0, 'componentType': 5123, 'count': len(indices), 'type': 'SCALAR'}],
            'materials': [{'name': 'M0'}],
            'meshes': [{'primitives': [{'attributes': {}, 'indices': 0, 'material': 0}]}]}
    (folder / 'src.bin').write_bytes(blob)
    (folder / 'src.gltf').write_text(json.dumps(gltf), encoding='utf-8')
    return folder / 'src.gltf'


def table(*entries):
    """Part table tiling the index buffer in order: entries are (fragment name, triangles); FirstIndex counts indices."""
    parts, first = [], 0
    for name, triangles in entries:
        parts.append({'SkeletalMeshFragmentName': name, 'MaterialIndex': 0, 'FirstIndex': first, 'NumPrimitives': triangles})
        first += 3 * triangles
    return parts


@unittest.skipIf(numpy is None, 'numpy is not installed for this interpreter')
class CandidateSectionsTest(unittest.TestCase):
    def run_case(self, parts, fragments):
        import slice_npc_assets
        total = 3 * sum(p['NumPrimitives'] for p in parts)
        with tempfile.TemporaryDirectory() as name:
            folder = Path(name)
            source = write_gltf(folder, list(range(total)))  # index value i at position i: positions identify the range
            report = slice_npc_assets.candidate_sections_gltf(source, parts, fragments, folder / 'out.gltf')
            out = json.loads((folder / 'out.gltf').read_text(encoding='utf-8'))
            blob = (folder / 'out.bin').read_bytes()
            sections = []
            for primitive in out['meshes'][0]['primitives']:
                accessor = out['accessors'][primitive['indices']]
                view = out['bufferViews'][accessor['bufferView']]
                sections.append(list(struct.unpack_from(f'<{accessor["count"]}H', blob, view['byteOffset'])))
            return report, sections, out

    def test_duplicate_name_keeps_every_range(self):
        # 'Acc' owns three separate ranges (2, 1 and 3 triangles) between single-range fragments; the index value at each
        # position equals the position, so the output lists show which input ranges were kept
        parts = table(('Body', 2), ('Acc', 2), ('Barrel', 1), ('Acc', 1), ('Grip', 1), ('Acc', 3))
        report, sections, out = self.run_case(parts, ['Acc', 'Barrel'])
        self.assertEqual(sections[0], list(range(6, 12)) + list(range(15, 18)) + list(range(21, 30)))
        self.assertEqual(sections[1], list(range(12, 15)))
        self.assertEqual([r['triangles'] for r in report], [6, 1])
        self.assertEqual([r['ranges'] for r in report], [3, 1])
        self.assertEqual([m['name'] for m in out['materials']], ['Frag_Acc', 'Frag_Barrel'])

    def test_single_range_fragment_is_unchanged(self):
        parts = table(('A', 2), ('B', 3))
        report, sections, _ = self.run_case(parts, ['B', 'A'])
        self.assertEqual(sections, [list(range(6, 15)), list(range(0, 6))])
        self.assertEqual([r['ranges'] for r in report], [1, 1])


if __name__ == '__main__':
    unittest.main()
