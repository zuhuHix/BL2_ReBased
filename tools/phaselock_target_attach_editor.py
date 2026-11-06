"""UE5 editor-Python step for the Phaselock target: the static meshes its body composition wears on its bones.

AI-assisted. Run through tools/import_phaselock_target.ps1 (which owns the editor lock), after the zone step. Reads the job file
(OPENWILLOW_SLICE_JOB, written by tools/prepare_phaselock_target.py) and imports each attachment glTF into <dest>/Attachments, using
the zone-colour instance of the same stock material on every slot. A missing instance, a slot that does not keep it or a failed save
stops the step: the pieces would show the wrong colour or be lost with no sign of it. Only after every piece passed does it write
<slice_dir>/editor_report_attach.json ({npc id: {mesh id: asset path}}), which the tool's attachments step reads. Only content under the
job's dest folders is created or changed.
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
            raise RuntimeError(f'{piece["id"]}: {material_path} is missing (the zone step makes it; it stops and names its own prerequisite if it cannot)')
        material = unreal.load_asset(material_path)
        if material is None:
            raise RuntimeError(f'{piece["id"]}: {material_path} exists but did not load')
        task = unreal.AssetImportTask()
        task.filename, task.destination_path = piece['gltf'], f'{npc["dest"]}/Attachments'
        task.automated, task.replace_existing, task.save = True, True, False
        tools.import_asset_tasks([task])
        found = [o for o in task.get_objects() if isinstance(o, unreal.StaticMesh)]
        if len(found) != 1:
            raise RuntimeError(f'Expected one StaticMesh from {piece["gltf"]}: {task.get_objects()}')
        mesh = found[0]
        slots = len(mesh.get_editor_property('static_materials'))
        if slots == 0:
            raise RuntimeError(f'{piece["id"]}: the imported mesh has no material slots')
        for slot in range(slots):
            mesh.set_material(slot, material)
            kept = mesh.get_material(slot)
            if kept is None or kept.get_path_name() != material.get_path_name():
                raise RuntimeError(f'{piece["id"]}: slot {slot} did not keep {material_path}')
        if not eal.save_loaded_asset(mesh, only_if_is_dirty=False):
            raise RuntimeError(f'{piece["id"]}: saving {mesh.get_path_name()} failed')
        path = mesh.get_path_name()
        entry[piece['id']] = path
        unreal.log(f'OW_SLICE {piece["id"]} -> {path} with {material_path}')

out = Path(job['slice_dir']) / 'editor_report_attach.json'
out.write_text(json.dumps(report, indent=1), encoding='utf-8')
unreal.log(f'OW_SLICE attach complete -> {out}')
