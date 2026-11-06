"""UE5 editor-Python step for the Phaselock target: the static meshes its body composition wears on its bones.

AI-assisted. Run through tools/import_phaselock_target.ps1 (which owns the editor lock), after the zone step. Reads the job file
(OPENWILLOW_SLICE_JOB, written by tools/prepare_phaselock_target.py) and imports each attachment glTF into <dest>/Attachments, using
the zone-colour instance of the same stock material on every slot. A missing instance stops the step: the pieces would show the wrong
colour with no sign of it. Writes <slice_dir>/editor_report_attach.json ({npc id: {mesh id: asset path}}), which the tool's attachments
step reads. Only content under the job's dest folders is created or changed.
"""
import json
import os
from pathlib import Path
import unreal

job = json.loads(Path(os.environ['OPENWILLOW_SLICE_JOB']).read_text(encoding='utf-8'))
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary
report = {}

for npc in job['npcs']:
    entry = report.setdefault(npc['id'], {})
    for piece in npc['attachment_meshes']:
        material_path = f'{npc["dest"]}/Materials/MI_{piece["material"]}_Zone'
        if not eal.does_asset_exist(material_path):
            raise RuntimeError(f'{piece["id"]}: {material_path} is missing (run the zone step first)')
        material = unreal.load_asset(material_path)
        task = unreal.AssetImportTask()
        task.filename, task.destination_path = piece['gltf'], f'{npc["dest"]}/Attachments'
        task.automated, task.replace_existing, task.save = True, True, False
        tools.import_asset_tasks([task])
        found = [o for o in task.get_objects() if isinstance(o, unreal.StaticMesh)]
        if len(found) != 1:
            raise RuntimeError(f'Expected one StaticMesh from {piece["gltf"]}: {task.get_objects()}')
        mesh = found[0]
        for slot in range(len(mesh.get_editor_property('static_materials'))):
            mesh.set_material(slot, material)
        eal.save_loaded_asset(mesh, only_if_is_dirty=False)
        path = mesh.get_path_name()
        entry[piece['id']] = path
        unreal.log(f'OW_SLICE {piece["id"]} -> {path} with {material_path}')

out = Path(job['slice_dir']) / 'editor_report_attach.json'
out.write_text(json.dumps(report, indent=1), encoding='utf-8')
unreal.log(f'OW_SLICE attach complete -> {out}')
