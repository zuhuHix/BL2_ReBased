"""Generate an entirely synthetic four-channel UE host fixture under local/."""
import json
import math
from pathlib import Path
import struct
import zlib

root = Path('local/material-smoke')
root.mkdir(parents=True, exist_ok=True)

def png(name, pixel, pixels=None):
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    if pixels is None:
        pixels = [pixel] * 16
    raw = b''.join(b'\0' + b''.join(bytes(p) for p in pixels[y * 4:y * 4 + 4]) for y in range(4))
    (root / name).write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>2I5B', 4, 4, 8, 6, 0, 0, 0))
                              + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))

channels = {'diffuse': [200, 90, 40, 255], 'normal': [128, 128, 255, 255],
            'specular': [80, 80, 80, 255], 'emissive': [200, 80, 20, 128]}
for name, pixel in channels.items():
    png(name + '.png', pixel)
# Distinct corner colors reveal U/V flips; a uniform texture cannot do this.
png('diffuse.png', None, [[255, 0, 0, 255] if x < 2 and y < 2 else
                        [0, 255, 0, 255] if y < 2 else
                        [0, 0, 255, 255] if x < 2 else [255, 255, 0, 255]
                        for y in range(4) for x in range(4)])
(root / 'triangle.obj').write_text('v 10 20 30\nv 110 20 30\nv 10 220 330\n'
    'vt 0 0\nvt 1 0\nvt 0 1\nvn 0 -0.83205 0.55470\n'
    'f 3/3/1 2/2/1 1/1/1\n')
scene = {'schema': 1, 'map': 'MaterialV1Smoke', 'dynamic_policy': 'frozen',
         'levels': ['Synthetic_P'], 'camera': None, 'issues': [],
         'materials': {'Synthetic': {'source': 'Synthetic', 'channels': {n: n + '.png' for n in channels}}},
         'meshes': {'Triangle': {'source': 'Synthetic', 'sections': [{'slot': 0, 'file': 'triangle.obj', 'material': 'Synthetic'}]}},
         'actors': [{'source': 'Synthetic.Mesh', 'level': 'Synthetic_P', 'mesh': 'Triangle', 'materials': [],
                     'transform': {'actor': {'location': [100, 200, 300], 'rotation': [0, 90, 0], 'scale': [2, 3, 4]},
                                   'component': {'location': [5, 0, 0], 'rotation': [0, 0, 0], 'scale': [1, 1, 1]}}}]}
# A synthetic near-vertical collection catches quaternion-to-Euler snapping
# in the host's SetActorTransform path. The saved verifier compares all axes.
angle = math.radians(89.96)
c, s = math.cos(angle), math.sin(angle)
scene['actors'].append({'source': 'Synthetic.NearVertical', 'level': 'Synthetic_P',
    'mesh': 'Triangle', 'materials': [],
    'transform': {'matrix': [c, 0, s, 0, 0, 1, 0, 0, -s, 0, c, 0, 400, 500, 600, 1],
                  'scale': [-2, 3, 4]}})
(root / 'scene.json').write_text(json.dumps(scene, indent=2))
print(root / 'scene.json')
