"""Import the weapon part library: each gestalt mesh once (one slot per fragment) and every skin as a material instance.

OPENWILLOW_WEAPON_LIBRARY is tools/weapon_library.py's ignored output folder (library.json, <Type>/SK_Gestalt_<Type>.gltf
and fragments.json, skins.json). Assets go under /Game/OpenWillow/Weapons/Library:
  <Type>/SK_Gestalt_<Type>    the whole gestalt; material slot names are the fragment names, so the host
                              (OpenWillowWeaponAssembly.cpp) can show exactly the fragments a gun's parts draw
  M_OW_GunMaster              the paint reading as one parameterised material (import_weapon_paint.build_master)
  Textures/T_<name>           every skin texture once, imported linear (the master decodes sRGB by the installed flag)
  Skins/MI_<MIC leaf>         one MaterialInstanceConstant per weapon skin (Material part MIC)
Writes <library>/import_report.json: per type and per skin imported / failed with the reason. OPENWILLOW_WEAPON_LIBRARY_ONLY
may be "meshes" or "skins" to run half of it.
"""
import hashlib
import json
import os
import sys
from pathlib import Path

import unreal

sys.path.insert(0, str(Path(__file__).resolve().parent))
os.environ.pop('OPENWILLOW_WEAPON_PAINT', None)  # import_weapon_paint only paints when this is set
import import_weapon_paint as paint  # noqa: E402

ROOT = '/Game/OpenWillow/Weapons/Library'
library = Path(os.environ['OPENWILLOW_WEAPON_LIBRARY']).resolve()
only = os.environ.get('OPENWILLOW_WEAPON_LIBRARY_ONLY', '')
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary
mel = unreal.MaterialEditingLibrary
index = json.loads((library / 'library.json').read_text(encoding='utf-8'))
report_path = library / 'import_report.json'
report = json.loads(report_path.read_text(encoding='utf-8')) if report_path.exists() else {}
report.setdefault('types', {})
report.setdefault('skins', {})


def neutral_material():
    path = f'{ROOT}/M_OW_LibraryNeutral'
    if eal.does_asset_exist(path):
        return unreal.load_asset(path)
    material = tools.create_asset('M_OW_LibraryNeutral', ROOT, unreal.Material, unreal.MaterialFactoryNew())
    grey = mel.create_material_expression(material, unreal.MaterialExpressionConstant3Vector, -200, 0)
    grey.set_editor_property('constant', unreal.LinearColor(0.25, 0.25, 0.27, 1.0))
    mel.connect_material_property(grey, '', unreal.MaterialProperty.MP_BASE_COLOR)
    material.set_editor_property('used_with_skeletal_mesh', True)
    mel.recompile_material(material)
    eal.save_loaded_asset(material, only_if_is_dirty=False)
    return material


def import_mesh(kind):
    folder = library / kind
    table = json.loads((folder / 'fragments.json').read_text(encoding='utf-8'))
    destination = f'{ROOT}/{kind}'
    # The whole type folder goes, Anims included (the clips reference this mesh's skeleton, which is re-created);
    # tools/import_weapon_library.ps1 re-imports the recorded reload clips (reloads.json) right after a meshes import.
    if eal.does_directory_exist(destination):
        eal.delete_directory(destination)
    task = unreal.AssetImportTask()
    task.filename = str(folder / f'SK_Gestalt_{kind}.gltf')
    task.destination_path = destination
    task.automated = True
    task.replace_existing = True
    task.save = False
    tools.import_asset_tasks([task])
    meshes = [a for a in task.get_objects() if isinstance(a, unreal.SkeletalMesh)]
    if len(meshes) != 1:
        raise RuntimeError(f'{kind}: expected one skeletal mesh, found {len(meshes)}')
    mesh = meshes[0]
    target = f'{destination}/SK_Gestalt_{kind}'
    if mesh.get_path_name().split('.')[0] != target and not eal.rename_asset(mesh.get_path_name(), target):
        raise RuntimeError(f'could not rename {mesh.get_path_name()} to {target}')
    mesh = unreal.load_asset(target)
    neutral = neutral_material()
    slots = mesh.get_editor_property('materials')
    names = [str(slot.get_editor_property('material_slot_name')) for slot in slots]
    for i in range(len(slots)):
        slot = slots[i]
        slot.set_editor_property('material_interface', neutral)
        slots[i] = slot
    mesh.modify()
    mesh.set_editor_property('materials', slots)
    expected = [f['name'] for f in table['fragments']]
    missing = sorted(set(expected) - set(names))
    extra = sorted(set(names) - set(expected))
    # The glTF import may leave stray material assets (one per fragment) in the folder; they are not used.
    if not eal.save_directory(destination, only_if_is_dirty=False, recursive=True):
        raise RuntimeError(f'could not save {destination}')
    result = {'status': 'imported' if not missing else 'imported_with_missing_slots', 'mesh': table['mesh'],
              'slots': len(names), 'fragments': len(expected), 'missing_slots': missing, 'extra_slots': extra}
    unreal.log(f'OW_LIBRARY_MESH {kind}: {result}')
    return result


texture_assets = {}


def import_texture(source, address=None):
    """One linear texture asset per source PNG; names collide only when two different files share a stem."""
    source = Path(source)
    if str(source) in texture_assets:
        return texture_assets[str(source)]
    stem = source.stem
    digest = hashlib.sha1(source.read_bytes()).hexdigest()[:8]
    name = f'T_{stem}'
    path = f'{ROOT}/Textures/{name}'
    marker = library / 'textures.json'
    known = json.loads(marker.read_text(encoding='utf-8')) if marker.exists() else {}
    if name in known and known[name] != digest:
        name = f'T_{stem}_{digest}'
        path = f'{ROOT}/Textures/{name}'
    if not eal.does_asset_exist(path):
        task = unreal.AssetImportTask()
        task.filename = str(source)
        task.destination_path = f'{ROOT}/Textures'
        task.destination_name = name
        task.automated = True
        task.replace_existing = True
        task.save = False
        tools.import_asset_tasks([task])
        textures = [a for a in task.get_objects() if isinstance(a, unreal.Texture2D)]
        if len(textures) != 1:
            raise RuntimeError(f'expected one texture from {source}')
        asset = textures[0]
        asset.set_editor_property('srgb', False)
        asset.set_editor_property('compression_settings', unreal.TextureCompressionSettings.TC_DEFAULT)
        for prop, mode in zip(('address_x', 'address_y'), address or []):
            asset.set_editor_property(prop, getattr(unreal.TextureAddress, mode.upper()))
        eal.save_loaded_asset(asset, only_if_is_dirty=False)
    known[name] = digest
    marker.write_text(json.dumps(known, indent=1), encoding='utf-8')
    asset = unreal.load_asset(path)
    texture_assets[str(source)] = asset
    return asset


def import_skin(master, data):
    leaf = data['asset'].rsplit('/', 1)[-1].split('.')[0]
    folder = f'{ROOT}/Skins'
    path = f'{folder}/{leaf}'
    instance = unreal.load_asset(path) if eal.does_asset_exist(path) else tools.create_asset(
        leaf, folder, unreal.MaterialInstanceConstant, unreal.MaterialInstanceConstantFactoryNew())
    instance.set_editor_property('parent', master)
    mel.clear_all_material_instance_parameters(instance)
    scalars, vectors, textures = paint.skin_parameters(data)
    for name, value in scalars.items():
        mel.set_material_instance_scalar_parameter_value(instance, name, float(value))
    for name, value in vectors.items():
        v = list(value) + [0.0] * (4 - len(value))
        mel.set_material_instance_vector_parameter_value(instance, name, unreal.LinearColor(*map(float, v[:4])))
    addresses = {'p_Decal': (data.get('decal') or {}).get('address'),
                 'P_SimpleReflect': (data.get('reflection') or {}).get('address')}
    for name, source in textures.items():
        mel.set_material_instance_texture_parameter_value(instance, name, import_texture(source, addresses.get(name)))
    mel.update_material_instance(instance)
    eal.save_loaded_asset(instance, only_if_is_dirty=False)
    return {'status': 'imported', 'asset': f'{path}.{leaf}', 'textures': sorted(textures),
            'pattern': scalars['OW_UsePattern'] > 0, 'decal': scalars['OW_UseDecal'] > 0,
            'reflection': scalars['OW_UseReflect'] > 0}


if only != 'skins':
    for kind in index.get('types', {}):
        try:
            report['types'][kind] = import_mesh(kind)
        except Exception as error:  # recorded per type; the others still import
            report['types'][kind] = {'status': 'failed', 'reason': str(error)}
            unreal.log_error(f'OW_LIBRARY_MESH {kind} failed: {error}')

if only != 'meshes' and (library / 'skins.json').exists():
    master = paint.build_master(f'{ROOT}/M_OW_GunMaster')
    skins = json.loads((library / 'skins.json').read_text(encoding='utf-8'))
    for mic, entry in sorted(skins.items()):
        if entry.get('status') != 'prepared':
            report['skins'][mic] = {'status': entry.get('status', 'not_prepared'), 'reason': entry.get('reason')}
            continue
        try:
            report['skins'][mic] = import_skin(master, entry['paint'])
        except Exception as error:
            report['skins'][mic] = {'status': 'failed', 'reason': str(error)}
            unreal.log_error(f'OW_LIBRARY_SKIN {mic} failed: {error}')
    eal.save_directory(ROOT, only_if_is_dirty=True, recursive=True)

report_path.write_text(json.dumps(report, indent=1), encoding='utf-8')
counts = {}
for group in ('types', 'skins'):
    for entry in report[group].values():
        counts[f"{group}:{entry['status']}"] = counts.get(f"{group}:{entry['status']}", 0) + 1
unreal.log(f'OW_LIBRARY done {counts} -> {report_path}')
