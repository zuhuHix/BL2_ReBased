"""Extract the Phase 0 asset pair by object path from the user's BL2 installation."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--reader', type=Path, required=True)
parser.add_argument('--game', type=Path, required=True)
parser.add_argument('--output', type=Path, default=Path('local/probe'))
args = parser.parse_args()
game, output, reader = args.game.resolve(), args.output.resolve(), args.reader.resolve()
if not (game / 'Binaries/Win32/Borderlands2.exe').is_file():
    parser.error('An installed Borderlands 2 is required')
cooked = game / 'WillowGame/CookedPCConsole'
package = cooked / 'Ash_P.upk'
def call(*options):
    return json.loads(subprocess.check_output([str(reader), str(package), *map(str, options)], text=True, encoding='utf-8'))
records = {r['path']: r for r in call('--exports')}
mesh = records['Env_Ash.Mesh.Ash_Road01']
texture = records['Prop_Roads.Textures.MetalRoadConcrete_Dif']
output.mkdir(parents=True, exist_ok=True)
with package.open('rb') as source:
    source_hash = hashlib.file_digest(source, 'sha256').hexdigest()
manifest = {'schema': 1, 'package': 'WillowGame/CookedPCConsole/Ash_P.upk', 'package_sha256': source_hash}
manifest['mesh'] = call('--mesh', mesh['index'], '--property-offset', 4, '--output', output/'mesh.obj')
manifest['texture'] = call('--texture', texture['index'], '--property-offset', 4, '--output', output/'texture.png', '--tfc', cooked)
# Verify the selected texture is the mesh material's actual diffuse parameter.
material_index = manifest['mesh']['sections'][0]['material_index']
schema = Path(__file__).with_name('phase0-arrays.schema')
material = call('--properties', material_index, '--property-offset', 4, '--array-schema', schema)
parameters = next(p['value'] for p in material['properties'] if p['name'] == 'TextureParameterValues')
diffuse = [dict((p['name'], p['value']) for p in entry) for entry in parameters]
if not any(p.get('ParameterName') == 'p_Diffuse' and p['ParameterValue']['index'] == texture['index'] for p in diffuse):
    raise RuntimeError('Selected texture is not the mesh material diffuse parameter')
manifest['material_verified'] = True
for filename in ['mesh.obj', 'texture.png']:
    with (output/filename).open('rb') as source:
        manifest[filename+'_sha256'] = hashlib.file_digest(source,'sha256').hexdigest()
(output/'probe.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
print(json.dumps(manifest,indent=2))
