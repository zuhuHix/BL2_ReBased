"""Generate an asymmetric, entirely synthetic UV-orientation scene."""
import json
from pathlib import Path
import struct
import zlib

root = Path('local/uv-smoke')
root.mkdir(parents=True, exist_ok=True)


def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))


size = 64
colors = ((255, 0, 0, 255), (0, 255, 0, 255), (0, 0, 255, 255), (255, 255, 0, 255))
raw = b''.join(b'\0' + b''.join(bytes(colors[(2 if y >= size // 2 else 0) +
                                         (1 if x >= size // 2 else 0)])
                              for x in range(size)) for y in range(size))
(root / 'corners.png').write_bytes(b'\x89PNG\r\n\x1a\n' +
    chunk(b'IHDR', struct.pack('>2I5B', size, size, 8, 6, 0, 0, 0)) +
    chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))

# Camera faces +X, with +Y to screen right and +Z up. OBJ V is 1-sourceV.
# Match game index order: face cross products oppose outward stored normals.
(root / 'quad.obj').write_text(
    'v 500 -200 200\nv 500 200 200\nv 500 -200 -200\nv 500 200 -200\n'
    'vt 0 1\nvt 1 1\nvt 0 0\nvt 1 0\nvn -1 0 0\n'
    'f 3/3/1 2/2/1 1/1/1\nf 3/3/1 4/4/1 2/2/1\n')
identity = {'location': [0, 0, 0], 'rotation': [0, 0, 0], 'scale': [1, 1, 1]}
scene = {'schema': 1, 'map': 'UVSmoke_P', 'dynamic_policy': 'frozen',
         'levels': ['Synthetic_P'], 'camera': identity, 'issues': [],
         'materials': {'Corners': {'source': 'Synthetic.Corners',
                                  'channels': {'diffuse': 'corners.png', 'emissive': 'corners.png'}}},
         'meshes': {'Quad': {'source': 'Synthetic.Quad',
                            'sections': [{'slot': 0, 'file': 'quad.obj', 'material': 'Corners'}]}},
         'actors': [{'source': 'Synthetic.Quad', 'level': 'Synthetic_P', 'mesh': 'Quad',
                     'materials': [], 'transform': {'actor': identity, 'component': identity}}]}
(root / 'scene.json').write_text(json.dumps(scene, indent=2))
print(root / 'scene.json')
