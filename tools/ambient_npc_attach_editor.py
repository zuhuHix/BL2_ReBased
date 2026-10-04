"""UE5 editor-Python half of the ambient citizens' hair, hats, gear and head variants (run by tools/seed_ambient_npc_assets.ps1).

AI-assisted (Claude), 2026-10-04. Reads OPENWILLOW_AMBIENT_ATTACH_JOB (local/slice/ambient/attach_job.json, written by
``tools/ambient_npc_assets.py attachments``) and creates, under /Game/OpenWillow/Characters/Ambient/Attachments only:

* ``M_OW_NPC_Tint``     Diffuse and Normal texture parameters, a ``Tint`` colour multiplied into the base colour, matte
                        (specular 0.15, roughness 0.85: the constants ``host/ue5/import_character_menu_look.py`` gives Maya's
                        skin). Hair gets its colour through ``Tint`` at run time. The zone colouring of the original ``Master_NPC``
                        (three colour zones picked by the ``p_Masks`` texture) is NOT reproduced.
* ``M_OW_AmbientOutline`` the inverted-hull ink line of ``import_character_menu_look.py`` (same nodes: unlit near-black, back faces only,
                        pushed out along the vertex normal by ``ThicknessCm``). It is the only ink-line material in the project
                        (Marcus and the dummy have none).
* textures, material instances, one StaticMesh per hair/hat/gear mesh and one head material per head texture set.

It also moves the citizens' own body and head material instances (``Ambient/<Kind>/Materials``) onto ``M_OW_NPC_Tint`` so they
get the same matte constants. Writes ``editor_report_attach.json`` into the job's slice directory.
"""
import json
import os
from pathlib import Path
import unreal

job = json.loads(Path(os.environ['OPENWILLOW_AMBIENT_ATTACH_JOB']).read_text(encoding='utf-8'))
dest = job['dest']
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary
mel = unreal.MaterialEditingLibrary
MP = unreal.MaterialProperty
report = {'meshes': {}, 'materials': {}, 'heads': {}, 'warnings': []}


def log(message):
    unreal.log(f'OW_ATTACH {message}')


def node(material, cls, x, y, **props):
    expression = mel.create_material_expression(material, cls, x, y)
    for key, value in props.items():
        expression.set_editor_property(key, value)
    return expression


def import_one(path, folder, expected):
    task = unreal.AssetImportTask()
    task.filename = str(path)
    task.destination_path = folder
    task.automated = True
    task.replace_existing = True
    task.save = False
    tools.import_asset_tasks([task])
    found = [o for o in task.get_objects() if isinstance(o, expected)]
    if len(found) != 1:
        raise RuntimeError(f'Expected one {expected.__name__} from {Path(path).name}: {task.get_objects()}')
    return found[0]


_textures = {}


def texture(file, parameter):
    if file in _textures:
        return _textures[file]
    asset = import_one(file, f'{dest}/Textures', unreal.Texture2D)
    asset.set_editor_property('srgb', parameter == 'Diffuse')
    if parameter == 'Normal':
        asset.set_editor_property('compression_settings', unreal.TextureCompressionSettings.TC_NORMALMAP)
    eal.save_loaded_asset(asset, only_if_is_dirty=False)
    _textures[file] = asset
    return asset


def tint_master():
    path = f'{dest}/M_OW_NPC_Tint'
    if eal.does_asset_exist(path):
        eal.delete_asset(path)
    material = tools.create_asset('M_OW_NPC_Tint', dest, unreal.Material, unreal.MaterialFactoryNew())
    diffuse = node(material, unreal.MaterialExpressionTextureSampleParameter2D, -700, 0, parameter_name='Diffuse',
                   texture=unreal.load_asset('/Engine/EngineResources/DefaultTexture'))
    tint = node(material, unreal.MaterialExpressionVectorParameter, -700, 250, parameter_name='Tint',
                default_value=unreal.LinearColor(1, 1, 1, 1))
    product = node(material, unreal.MaterialExpressionMultiply, -400, 100)
    mel.connect_material_expressions(diffuse, 'RGB', product, 'A')
    mel.connect_material_expressions(tint, 'RGB', product, 'B')
    mel.connect_material_property(product, '', MP.MP_BASE_COLOR)
    normal = node(material, unreal.MaterialExpressionTextureSampleParameter2D, -700, 450, parameter_name='Normal',
                  sampler_type=unreal.MaterialSamplerType.SAMPLERTYPE_NORMAL,
                  texture=unreal.load_asset('/Engine/EngineMaterials/DefaultNormal'))
    mel.connect_material_property(normal, 'RGB', MP.MP_NORMAL)
    rough = node(material, unreal.MaterialExpressionConstant, -400, 700, r=0.85)
    mel.connect_material_property(rough, '', MP.MP_ROUGHNESS)
    spec = node(material, unreal.MaterialExpressionConstant, -400, 800, r=0.15)
    mel.connect_material_property(spec, '', MP.MP_SPECULAR)
    material.set_editor_property('used_with_skeletal_mesh', True)
    material.set_editor_property('two_sided', True)       # hair cards and caps are single sheets
    mel.recompile_material(material)
    eal.save_loaded_asset(material, only_if_is_dirty=False)
    return material


def outline_master():
    """Same nodes as host/ue5/import_character_menu_look.py outline_material()."""
    path = f'{dest}/M_OW_AmbientOutline'
    if eal.does_asset_exist(path):
        eal.delete_asset(path)
    material = tools.create_asset('M_OW_AmbientOutline', dest, unreal.Material, unreal.MaterialFactoryNew())
    material.set_editor_property('shading_model', unreal.MaterialShadingModel.MSM_UNLIT)
    material.set_editor_property('blend_mode', unreal.BlendMode.BLEND_MASKED)
    material.set_editor_property('two_sided', True)
    material.set_editor_property('used_with_skeletal_mesh', True)
    ink = node(material, unreal.MaterialExpressionConstant3Vector, -400, -200, constant=unreal.LinearColor(0.015, 0.015, 0.02, 1.0))
    mel.connect_material_property(ink, '', MP.MP_EMISSIVE_COLOR)
    sign = node(material, unreal.MaterialExpressionTwoSidedSign, -600, 0)
    flip = node(material, unreal.MaterialExpressionOneMinus, -450, 0)
    mel.connect_material_expressions(sign, '', flip, '')
    half = node(material, unreal.MaterialExpressionMultiply, -300, 0, const_b=0.5)
    mel.connect_material_expressions(flip, '', half, 'A')
    mel.connect_material_property(half, '', MP.MP_OPACITY_MASK)
    normal = node(material, unreal.MaterialExpressionVertexNormalWS, -600, 200)
    thickness = node(material, unreal.MaterialExpressionScalarParameter, -600, 320, parameter_name='ThicknessCm', default_value=0.35)
    offset = node(material, unreal.MaterialExpressionMultiply, -300, 240)
    mel.connect_material_expressions(normal, '', offset, 'A')
    mel.connect_material_expressions(thickness, '', offset, 'B')
    mel.connect_material_property(offset, '', MP.MP_WORLD_POSITION_OFFSET)
    mel.recompile_material(material)
    eal.save_loaded_asset(material, only_if_is_dirty=False)
    return material


def instance(name, folder, parent, files):
    asset = tools.create_asset(name, folder, unreal.MaterialInstanceConstant, unreal.MaterialInstanceConstantFactoryNew())
    mel.set_material_instance_parent(asset, parent)
    for parameter, file in files.items():
        mel.set_material_instance_texture_parameter_value(asset, parameter, texture(file, parameter))
    eal.save_loaded_asset(asset, only_if_is_dirty=False)
    return asset


def main():
    if eal.does_directory_exist(dest):
        eal.delete_directory(dest)
    parent = tint_master()
    outline_master()
    default_white = None
    # Attachment materials the live pawns named (by parent), and the mesh defaults UModel reported.
    for m in job['parent_materials']:
        if not m['textures']:
            continue
        asset = instance(m['id'], f'{dest}/Materials', parent, m['textures'])
        report['materials'][m['id']] = asset.get_path_name().split('.')[0]
    for h in job['heads']:
        if not h['textures'].get('Diffuse'):
            report['warnings'].append(f'head {h["id"]} has no diffuse')
            continue
        asset = instance('MI_Head_' + h['id'], f'{dest}/Materials', parent, {k: v for k, v in h['textures'].items() if v})
        report['heads'][h['id']] = asset.get_path_name().split('.')[0]
    for m in job['meshes']:
        mesh = import_one(m['gltf'], f'{dest}/Meshes', unreal.StaticMesh)
        slots = mesh.get_editor_property('static_materials')
        names = []
        for i in range(len(slots)):
            default = m['default_materials'][i] if i < len(m['default_materials']) else (m['default_materials'][0] if m['default_materials'] else None)
            files = default['textures'] if default else {}
            if files.get('Diffuse'):
                key = f'MI_{m["id"]}_{i}'
                mi = instance(key, f'{dest}/Materials', parent, files)
                mesh.set_material(i, mi)
                report['materials'][key] = mi.get_path_name().split('.')[0]
            else:
                report['warnings'].append(f'{m["id"]} slot {i}: no diffuse texture, default material kept')
            names.append(str(slots[i].get_editor_property('material_slot_name')))
        eal.save_loaded_asset(mesh, only_if_is_dirty=False)
        box = mesh.get_bounds()
        report['meshes'][m['id']] = {'asset': mesh.get_path_name().split('.')[0], 'slots': names, 'bones': m['bones'],
                                     'extent': [round(box.box_extent.x, 2), round(box.box_extent.y, 2), round(box.box_extent.z, 2)],
                                     'origin': [round(box.origin.x, 2), round(box.origin.y, 2), round(box.origin.z, 2)]}
        log(f'{m["id"]} -> {report["meshes"][m["id"]]["asset"]} slots {names}')
    # The citizens' own body and head instances share the matte constants.
    moved = []
    for kind in ('CitizenMale', 'CitizenFemale'):
        folder = f'/Game/OpenWillow/Characters/Ambient/{kind}/Materials'
        for asset_path in eal.list_assets(folder, recursive=False, include_folder=False):
            asset = unreal.load_asset(asset_path)
            if isinstance(asset, unreal.MaterialInstanceConstant):
                mel.set_material_instance_parent(asset, parent)
                eal.save_loaded_asset(asset, only_if_is_dirty=False)
                moved.append(asset.get_path_name().split('.')[0])
    report['reparented'] = moved
    eal.save_directory(dest, only_if_is_dirty=False, recursive=True)


main()
Path(job['slice_dir'], 'editor_report_attach.json').write_text(json.dumps(report, indent=1), encoding='utf-8')
log(f'complete: {len(report["meshes"])} meshes, {len(report["materials"])} materials, {len(report["heads"])} heads, {len(report["warnings"])} warnings')
