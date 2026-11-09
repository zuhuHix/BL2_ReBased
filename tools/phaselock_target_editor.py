"""UE5 editor-Python step: zone-colour material on imported stock targets (the Phaselock bullymong and psycho, the slice's target dummy).

AI-assisted. Run through tools/import_phaselock_target.ps1 or tools/seed_slice_npc_assets.ps1 (each owns the editor lock),
after tools/slice_npc_editor.py's ``npcs`` mode imported the mesh with the plain diffuse material. Reads that tool's job file
(OPENWILLOW_SLICE_JOB, written by tools/prepare_phaselock_target.py or tools/slice_npc_assets.py). A slot without ``p_Masks``
(Marcus) keeps its plain material.

Parents: the Bullymong's and the TargetDummy's materials have a known ``Master_Creature`` parent (read with our reader; their job entries
name it). The Psycho's slots' parent is unknown: they stay at a light/dark scale of 1.0 and the step logs a scale note for each.
From Master_Creature's compiled shader (read locally; PHASELOCK_STOCK_DATA.md, "Master_Creature's colour model") come only the two
light/dark scales, ``p_HighlightsIntensity`` / ``p_ShadowsIntensity`` with their master default, and the use of the zone vectors as stored
(no gamma step); tools/creature_zone.py picks those values. The rest is our host model, UNVERIFIED: the grey diffuse is coloured by the
zone vectors (p_?Color{Hilight,Midtone,Shadow}, p_ColorD) through ``p_Masks`` (left half light/dark map, right half zone mask), mixed as
the citizens' ``M_OW_NPC_Zone`` (tools/ambient_npc_attach_editor.py) does; this step parents one instance to it. The values are the stock
material instance's own (UModel props). Nothing here is checked in the running game; the dummy's pattern layer is not drawn.
``M_OW_NPC_Zone`` comes from the citizens pipeline; without it this step stops and names that prerequisite, because the attach step
and the target's colours need the instance.
Writes ``<slice_dir>/editor_report_zone.json``; only content under the job's ``dest`` folders is created or changed.
"""
import json
import os
from pathlib import Path
import sys
import unreal

sys.path.insert(0, str(Path(__file__).resolve().parent))
from creature_zone import scale_note, zone_parameters  # noqa: E402

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
        raise RuntimeError(f'{ZONE} is missing. It comes from the citizens pipeline: run powershell -File tools/seed_ambient_npc_assets.ps1 -Steps attach '
                           '(with -Compose <amb_compose json> if the default capture is absent), then this step again.')
    dest = npc['dest']
    mesh = unreal.load_asset(f'{dest}/Meshes/{npc["mesh_name"]}/SkeletalMeshes/{npc["mesh_name"]}')
    slots = mesh.get_editor_property('materials')
    for slot in npc['slots']:
        files = slot['textures']
        if 'p_Masks' not in files or 'p_Diffuse' not in files:
            continue
        name = f'MI_{slot["material"]}_Zone'
        path = f'{dest}/Materials/{name}'
        # An existing instance is updated in place: the attach step's piece meshes reference it, and deleting it would leave them
        # without a material until the attach step ran again. Its overrides are cleared first, so nothing from an older run (a Normal
        # or a vector this job no longer has) survives; then the parent and every value are set again and read back.
        instance = unreal.load_asset(path) if eal.does_asset_exist(path) else None
        if isinstance(instance, unreal.MaterialInstanceConstant):
            mel.clear_all_material_instance_parameters(instance)
        else:
            if instance is not None:
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
        # Master_Creature's colour model (tools/creature_zone.py): the vectors as stored and the two light/dark scales, from the
        # instance or the master's default. OPENWILLOW_ZONE_GAMMA=1 brings back round 23's x^2.2 fit for an A/B capture.
        gamma = os.environ.get('OPENWILLOW_ZONE_GAMMA', '0') == '1'
        values = zone_parameters(slot, master=npc.get('master'), gamma=gamma)
        for parameter, rgba in values['vectors'].items():
            mel.set_material_instance_vector_parameter_value(instance, parameter, unreal.LinearColor(*rgba))
        # UE 5.8's set_material_instance_scalar_parameter_value always returns False (the engine never sets its result), so the
        # parent's parameter list is checked first and every value is read back after it is set.
        known = {str(name) for name in mel.get_scalar_parameter_names(unreal.load_asset(ZONE))}
        for parameter, value in values['scalars'].items():
            if parameter not in known:
                raise RuntimeError(f'{ZONE} has no scalar {parameter}: it predates the light/dark scales. Rebuild it with '
                                   'powershell -File tools/seed_ambient_npc_assets.ps1 -Steps attach, then run this step again.')
            mel.set_material_instance_scalar_parameter_value(instance, parameter, value)
            held = mel.get_material_instance_scalar_parameter_value(instance, parameter)
            if abs(held - value) > 1e-6:
                raise RuntimeError(f'{path}: {parameter} reads {held} after being set to {value}')
        applied = sorted(values['vectors'])
        note = scale_note(slot, npc.get('master'))
        if note:
            log(f'{npc["id"]}: {note}')
            entry.setdefault('scale_notes', []).append(note)
        eal.save_loaded_asset(instance, only_if_is_dirty=False)
        for i in range(len(slots)):
            if str(slots[i].get_editor_property('material_slot_name')) == slot['slot']:
                item = slots[i]
                item.set_editor_property('material_interface', instance)
                slots[i] = item
        entry.update({'status': 'zone material', 'instance': path, 'vectors': applied, 'scalars': values['scalars'], 'gamma_to_linear': gamma})
        log(f'{npc["id"]}: {path} with {len(applied)} zone vectors')
    mesh.modify()
    mesh.set_editor_property('materials', slots)
    eal.save_loaded_asset(mesh, only_if_is_dirty=False)

out = Path(job['slice_dir']) / 'editor_report_zone.json'
out.write_text(json.dumps(report, indent=1), encoding='utf-8')
log(f'zone complete -> {out}')
