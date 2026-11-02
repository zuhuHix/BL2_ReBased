"""UE5 editor-Python step for the Phaselock target: zone-colour material on the imported stock enemy.

AI-assisted. Run through tools/import_phaselock_target.ps1 (which owns the editor lock), after
tools/slice_npc_editor.py's ``npcs`` mode imported the mesh with the plain diffuse material. Reads the same job file
(OPENWILLOW_SLICE_JOB, written by tools/prepare_phaselock_target.py).

The bullymong's grey diffuse gets its colour from the material's zone vectors (p_?Color{Hilight,Midtone,Shadow},
p_ColorD) through ``p_Masks``, whose layout is the citizens' one (left half light/dark map, right half zone mask, checked by
eye on the exported texture). The citizens' ``M_OW_NPC_Zone`` (tools/ambient_npc_attach_editor.py) implements that mix;
this step parents one instance to it. The mixing rule is applied by analogy (UNVERIFIED there, and so here); the vectors
are the stock material instance's own (UModel props). When ``M_OW_NPC_Zone`` is missing the plain material is kept.
Writes ``<slice_dir>/editor_report_zone.json``; only content under the job's ``dest`` folders is created or changed.
"""
import json
import os
from pathlib import Path
import unreal

job = json.loads(Path(os.environ['OPENWILLOW_SLICE_JOB']).read_text(encoding='utf-8'))
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary
mel = unreal.MaterialEditingLibrary
ZONE = '/Game/OpenWillow/Characters/Ambient/Attachments/M_OW_NPC_Zone'
report = {}


def log(message):
    unreal.log(f'OW_SLICE {message}')


def texture(path, folder, srgb):
    task = unreal.AssetImportTask()
    task.filename, task.destination_path, task.automated, task.replace_existing, task.save = str(path), folder, True, True, False
    tools.import_asset_tasks([task])
    asset = next(o for o in task.get_objects() if isinstance(o, unreal.Texture2D))
    asset.set_editor_property('srgb', srgb)
    eal.save_loaded_asset(asset, only_if_is_dirty=False)
    return asset


for npc in job['npcs']:
    entry = report.setdefault(npc['id'], {'status': 'kept plain material'})
    if not eal.does_asset_exist(ZONE):
        entry['reason'] = f'{ZONE} missing (built by tools/ambient_npc_attach_editor.py)'
        log(f'{npc["id"]}: {entry["reason"]}')
        continue
    dest = npc['dest']
    mesh = unreal.load_asset(f'{dest}/Meshes/{npc["mesh_name"]}/SkeletalMeshes/{npc["mesh_name"]}')
    slots = mesh.get_editor_property('materials')
    for slot in npc['slots']:
        files = slot['textures']
        if 'p_Masks' not in files or 'p_Diffuse' not in files:
            continue
        name = f'MI_{slot["material"]}_Zone'
        path = f'{dest}/Materials/{name}'
        if eal.does_asset_exist(path):
            eal.delete_asset(path)
        instance = tools.create_asset(name, f'{dest}/Materials', unreal.MaterialInstanceConstant,
                                      unreal.MaterialInstanceConstantFactoryNew())
        mel.set_material_instance_parent(instance, unreal.load_asset(ZONE))
        mel.set_material_instance_texture_parameter_value(instance, 'Diffuse', texture(files['p_Diffuse'], f'{dest}/Textures', True))
        mel.set_material_instance_texture_parameter_value(instance, 'Masks', texture(files['p_Masks'], f'{dest}/Textures', False))
        if 'p_Normal' in files:
            normal = texture(files['p_Normal'], f'{dest}/Textures', False)
            normal.set_editor_property('compression_settings', unreal.TextureCompressionSettings.TC_NORMALMAP)
            eal.save_loaded_asset(normal, only_if_is_dirty=False)
            mel.set_material_instance_texture_parameter_value(instance, 'Normal', normal)
        applied = []
        for parameter, value in slot.get('vectors', {}).items():
            if parameter.startswith('p_') and parameter.endswith(('Hilight', 'Midtone', 'Shadow', 'ColorD')) and value:
                mel.set_material_instance_vector_parameter_value(instance, parameter, unreal.LinearColor(*[float(v) for v in value]))
                applied.append(parameter)
        eal.save_loaded_asset(instance, only_if_is_dirty=False)
        for i in range(len(slots)):
            if str(slots[i].get_editor_property('material_slot_name')) == slot['slot']:
                item = slots[i]
                item.set_editor_property('material_interface', instance)
                slots[i] = item
        entry.update({'status': 'zone material', 'instance': path, 'vectors': sorted(applied)})
        log(f'{npc["id"]}: {path} with {len(applied)} zone vectors')
    mesh.modify()
    mesh.set_editor_property('materials', slots)
    eal.save_loaded_asset(mesh, only_if_is_dirty=False)

out = Path(job['slice_dir']) / 'editor_report_zone.json'
out.write_text(json.dumps(report, indent=1), encoding='utf-8')
log(f'zone complete -> {out}')
