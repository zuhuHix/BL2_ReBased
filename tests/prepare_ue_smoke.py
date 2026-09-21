"""Generate an entirely synthetic four-channel UE host fixture under local/."""
import json
import math
from pathlib import Path
import struct
import sys
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
# Exercise Unlit color routing with a texture, constant, neutral fallback,
# null emissive entry, and explicit emissive taking precedence in either order.
for name, definition in {
    'UnlitTexture': {'channels': {'diffuse': 'diffuse.png'}},
    'UnlitNullEmissive': {'channels': {'diffuse': 'diffuse.png', 'emissive': None}},
    'UnlitConstant': {'channels': {}, 'constant_diffuse': [0.2, 0.4, 0.6]},
    'UnlitNeutral': {'channels': {}},
    'UnlitEmissiveFirst': {'channels': {'emissive': 'emissive.png', 'diffuse': 'diffuse.png'}},
    'UnlitEmissiveLast': {'channels': {'diffuse': 'diffuse.png', 'emissive': 'emissive.png'}},
}.items():
    scene['materials'][name] = dict(definition, source='Synthetic.' + name, lighting_model='MLM_Unlit')

# A synthetic sky approximation exercises the host's time-of-day strip graph
# and the saved-graph verifier without game data. The strip has a distinct
# column so a wrong column lookup would be visible; the cloud mask is linear.
png('sky_strip.png', None, [[40, 60, 200, 255] if x == 2 else [10, 10, 10, 255]
                            for y in range(4) for x in range(4)])
png('sky_clouds.png', [128, 0, 0, 255])
scene['materials']['UnlitSky'] = {
    'source': 'Synthetic.UnlitSky', 'lighting_model': 'MLM_Unlit', 'two_sided': True,
    'channels': {'diffuse': 'sky_strip.png'},
    'sky_approximation': {
        'method': 'sky_time_of_day_strip_v1', 'status': 'partial_unverified',
        'master': 'Synthetic.Mat_SkyTimeOfDay_Master',
        'textures': {'transition_track': {'source': 'Synthetic.Strip', 'file': 'sky_strip.png'},
                     'clouds': {'source': 'Synthetic.Clouds', 'file': 'sky_clouds.png'},
                     'masks': {'source': 'Synthetic.Masks', 'file': 'sky_clouds.png'}},
        'scalars': {'time_of_day': 160.0, 'sky_brightness': 1.5, 'sun_spot_brightness': 6.0,
                    'cloud_cap_opacity': 0.75, 'cloud_brightness': 0.5},
        'vectors': {'horizon_track_color_multiplier': [0.2, 0.2, 0.2, 1.0]},
        'time_axis': {'divisor': 256.0, 'column_u': 160.0 / 256.0,
                      'note': 'UNVERIFIED: Time_of_Day read as a strip pixel column'},
        'horizon_row_v': 0.95, 'cloud_channel': 'R', 'omitted': ['synthetic']}}

# A synthetic outer hull placement exercises the opt-in override policy: the
# placed _Teleported override is displaced by the mesh default that has a
# diffuse, and the verifier checks the saved binding against the record.
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from prepare_level import apply_outer_shell_policy
scene['materials']['HullDefault'] = {'source': 'Synthetic.Prop_Skybox.Materials.Mat_SancSkyNew',
                                     'channels': {'diffuse': 'diffuse.png'}}
scene['materials']['HullTeleported'] = {'source': 'Synthetic.FX.Mat_Sanctuary_Teleported',
                                        'blend_mode': 'BLEND_Masked', 'two_sided': True, 'channels': {}}
# Its own OBJ: sections importing the same file share one host asset.
(root / 'hull.obj').write_text('v 0 0 0\nv 100 0 0\nv 0 0 100\n'
    'vt 0 0\nvt 1 0\nvt 0 1\nvn 0 -1 0\n'
    'f 3/3/1 2/2/1 1/1/1\n')
scene['meshes']['Hull'] = {'source': 'Synthetic:Prop_Skybox.Meshes.SanctuarySky',
                           'sections': [{'slot': 0, 'file': 'hull.obj', 'material': 'HullDefault'}]}
hull = {'source': 'Synthetic.Hull', 'level': 'Synthetic_P', 'mesh': 'Hull', 'materials': ['HullTeleported'],
        'transform': {'actor': {'location': [0, 0, 0], 'rotation': [0, 90, 0], 'scale': [1, 1, 1]},
                      'component': {'location': [0, 0, 0], 'rotation': [0, 0, 0], 'scale': [1, 1, 1]}}}
apply_outer_shell_policy(hull, scene['meshes']['Hull']['source'], scene['meshes']['Hull']['sections'],
                         scene['materials'], True)
assert hull['materials'] == [None] and len(hull['outer_shell_replaced']) == 1
scene['actors'].append(hull)
scene['outer_shell_policy'] = 'mesh_default_for_teleported_overrides_v1'

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
