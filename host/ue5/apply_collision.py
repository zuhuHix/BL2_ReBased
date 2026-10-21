"""Update collision only on an already imported map, preserving its art."""
import json
import os
import sys
from pathlib import Path
import unreal
sys.path.insert(0, str(Path(__file__).resolve().parent))
from scene_geometry import actor_label



def has_simple(collision):
    return any(collision.get(kind) for kind in ('hulls', 'spheres', 'capsules'))


def shape_lists(collision):
    hulls, spheres, capsules = [], [], []
    for h in collision.get('hulls', []):
        hull = unreal.OpenWillowHull()
        hull.vertices = [unreal.Vector(*v) for v in h['vertices']]
        hulls.append(hull)
    for s in collision.get('spheres', []):
        sphere = unreal.OpenWillowSphere()
        sphere.center, sphere.radius = unreal.Vector(*s['center']), s['radius']
        spheres.append(sphere)
    for c in collision.get('capsules', []):
        capsule = unreal.OpenWillowCapsule()
        capsule.center = unreal.Vector(*c['center'])
        capsule.axis_x, capsule.axis_y, capsule.axis_z = (unreal.Vector(*a) for a in c['axes'])
        capsule.radius, capsule.length = c['radius'], c['length']
        capsules.append(capsule)
    return hulls, spheres, capsules


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
            hulls, spheres, capsules = shape_lists(definition['collision']) if i == 0 else ([], [], [])
            if triangle:
                if hulls or spheres or capsules or not unreal.OpenWillowCollision.set_triangle_collision(mesh):
                    raise RuntimeError('Triangle collision cooking failed: ' + label)
            elif not unreal.OpenWillowCollision.set_shapes(mesh, hulls, spheres, capsules):
                raise RuntimeError('Collision cooking failed: ' + label)
            prepared.add(mesh.get_path_name())
            changed.append(mesh)
        active = instance['collision_enabled'] and (triangle or (i == 0 and has_simple(definition['collision'])))
        component.set_collision_profile_name('BlockAll')
        component.set_collision_enabled(unreal.CollisionEnabled.QUERY_AND_PHYSICS if active else unreal.CollisionEnabled.NO_COLLISION)
        enabled += int(active)
# Blocking volumes are built at play time from volumes-runtime.json by one
# OpenWillowVolumes actor (it reads $OPENWILLOW_SCENE, so the map stays portable).
volume_actor = by_label.get('OpenWillow_Volumes')
if scene.get('volume_policy') == 'observed_blocking_volumes_v1' and volume_actor is None:
    volume_actor = actors.spawn_actor_from_class(unreal.OpenWillowVolumes, unreal.Vector(0, 0, 0))
    volume_actor.set_actor_label('OpenWillow_Volumes')
for mesh in changed:
    if not unreal.EditorAssetLibrary.save_loaded_asset(mesh):
        raise RuntimeError('Cannot save collision mesh')
if not level.save_current_level():
    raise RuntimeError('Cannot save collision map')
report = {'mesh_sections': len(changed), 'enabled_components': enabled,
          'blocking_volumes': sum(v['status'] == 'blocking' for v in scene.get('volumes', [])),
          'volume_actor': volume_actor is not None}
(root / 'ue-collision.json').write_text(json.dumps(report, indent=2))
unreal.log('OpenWillow collision applied: ' + json.dumps(report))
