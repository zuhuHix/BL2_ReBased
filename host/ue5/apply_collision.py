"""Update collision only on an already imported map, preserving its art."""
import json
import os
import sys
from pathlib import Path
import unreal
sys.path.insert(0, str(Path(__file__).resolve().parent))
from scene_geometry import actor_label

root = Path(os.environ['OPENWILLOW_SCENE'])
scene = json.loads((root / 'scene.json').read_text())
if scene.get('collision_policy') != 'observed_convex_and_box_v1':
    raise RuntimeError('Refresh collision metadata first')
level = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem)
if not level.load_level('/Game/OpenWillow/' + scene['map'] + '/' + scene['map']):
    raise RuntimeError('Cannot load imported map')
actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
by_label = {a.get_actor_label(): a for a in actors.get_all_level_actors()}
prepared, changed = set(), []
enabled = 0
for instance in scene['actors']:
    definition = scene['meshes'][instance['mesh']]
    for i, section in enumerate(definition['sections']):
        label = actor_label(instance['level'], instance['source'], section['slot'])
        actor = by_label[label]
        component = actor.static_mesh_component
        mesh = component.static_mesh
        triangle = definition['collision'].get('status') == 'triangle_mesh'
        if mesh.get_path_name() not in prepared:
            hulls = []
            if i == 0:
                for h in definition['collision']['hulls']:
                    hull = unreal.OpenWillowHull()
                    hull.vertices = [unreal.Vector(*v) for v in h['vertices']]
                    hulls.append(hull)
            if triangle and i == 0:
                if hulls or not unreal.OpenWillowCollision.set_triangle_collision(mesh):
                    raise RuntimeError('Triangle collision cooking failed: ' + label)
            elif not unreal.OpenWillowCollision.set_hulls(mesh, hulls):
                raise RuntimeError('Collision cooking failed: ' + label)
            prepared.add(mesh.get_path_name())
            changed.append(mesh)
        active = i == 0 and instance['collision_enabled'] and (bool(definition['collision']['hulls']) or triangle)
        component.set_collision_profile_name('BlockAll')
        component.set_collision_enabled(unreal.CollisionEnabled.QUERY_AND_PHYSICS if active else unreal.CollisionEnabled.NO_COLLISION)
        enabled += int(active)
for mesh in changed:
    if not unreal.EditorAssetLibrary.save_loaded_asset(mesh):
        raise RuntimeError('Cannot save collision mesh')
if not level.save_current_level():
    raise RuntimeError('Cannot save collision map')
report = {'mesh_sections': len(changed), 'enabled_components': enabled}
(root / 'ue-collision.json').write_text(json.dumps(report, indent=2))
unreal.log('OpenWillow collision applied: ' + json.dumps(report))
