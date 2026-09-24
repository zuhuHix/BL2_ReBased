"""Create AnimSequences on Maya's arms skeleton from converted bone tracks (editor Python).

OPENWILLOW_CHARACTER_ANIMS lists `set=path.json` pairs written by
tools/prepare_character_anims.py. Run after import_character.py, which
recreates the arms skeletal mesh. Also dumps the skeleton's reference pose to
OPENWILLOW_CHARACTER_REFERENCE when that is set (input to the converter).
"""
import json
import os
from pathlib import Path
import unreal

destination = '/Game/OpenWillow/Characters/Maya'
mesh = unreal.load_asset(f'{destination}/Meshes/Hands_Siren/SkeletalMeshes/Hands_Siren')
if mesh is None:
    raise RuntimeError('Import Hands_Siren with import_character.py first')
skeleton = mesh.get_editor_property('skeleton')
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary

reference_path = os.environ.get('OPENWILLOW_CHARACTER_REFERENCE')
if reference_path:
    pose = unreal.AnimPoseExtensions.get_reference_pose(skeleton)
    bones = []
    for name in unreal.AnimPoseExtensions.get_bone_names(pose):
        t = unreal.AnimPoseExtensions.get_bone_pose(pose, name, unreal.AnimPoseSpaces.WORLD)
        bones.append({'name': str(name), 'loc': [t.translation.x, t.translation.y, t.translation.z],
                      'quat': [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w]})
    Path(reference_path).write_text(json.dumps(bones, indent=1), encoding='utf-8')
    unreal.log(f'OW_ANIM reference pose {len(bones)} bones -> {reference_path}')

for pair in filter(None, os.environ.get('OPENWILLOW_CHARACTER_ANIMS', '').split(';')):
    label, path = pair.split('=', 1)
    for clip, data in json.loads(Path(path).read_text(encoding='utf-8')).items():
        name = f'Anim_{label}_{clip}'
        asset_path = f'{destination}/FirstPerson/{name}'
        if eal.does_asset_exist(asset_path):
            eal.delete_asset(asset_path)
        factory = unreal.AnimSequenceFactory()
        factory.set_editor_property('target_skeleton', skeleton)
        factory.set_editor_property('preview_skeletal_mesh', mesh)
        sequence = tools.create_asset(name, f'{destination}/FirstPerson', unreal.AnimSequence, factory)
        controller = sequence.controller
        controller.open_bracket(unreal.Text('OpenWillow MD5 import'))
        controller.set_frame_rate(unreal.FrameRate(int(round(data['rate'])), 1))
        # UE counts frames as intervals: N keys span N-1 frames.
        controller.set_number_of_frames(unreal.FrameNumber(max(1, data['frames'] - 1)))
        for bone, track in data['tracks'].items():
            controller.add_bone_track(bone)
            positions = [unreal.Vector(*p) for p in track['pos']]
            rotations = [unreal.Quat(*q) for q in track['rot']]
            scales = [unreal.Vector(1, 1, 1)] * len(positions)
            controller.set_bone_track_keys(bone, positions, rotations, scales)
        controller.close_bracket()
        eal.save_loaded_asset(sequence, only_if_is_dirty=False)
        unreal.log(f'OW_ANIM {asset_path} frames={data["frames"]}')
