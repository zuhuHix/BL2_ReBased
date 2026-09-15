"""Reopen and compare saved hull vertices and component collision switches."""
import json
import os
import sys
from pathlib import Path
import unreal
sys.path.insert(0, str(Path(__file__).resolve().parent))
from scene_geometry import actor_label

root = Path(os.environ['OPENWILLOW_SCENE'])
scene = json.loads((root / 'scene.json').read_text())
level = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem)
if not level.load_level('/Game/OpenWillow/' + scene['map'] + '/' + scene['map']):
    raise RuntimeError('Cannot reopen collision map')
actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
by_label = {a.get_actor_label(): a for a in actors.get_all_level_actors()}
checked, enabled, triangle_enabled = set(), 0, 0
for instance in scene['actors']:
    definition = scene['meshes'][instance['mesh']]
    for i, section in enumerate(definition['sections']):
        label = actor_label(instance['level'], instance['source'], section['slot'])
        component = by_label[label].static_mesh_component
        mesh = component.static_mesh
        triangle = definition['collision'].get('status') == 'triangle_mesh'
        if mesh.get_path_name() not in checked:
            expected = definition['collision']['hulls'] if i == 0 else []
            actual = unreal.OpenWillowCollision.get_hulls(mesh)
            if unreal.OpenWillowCollision.has_triangle_collision(mesh) != (triangle and i == 0):
                raise RuntimeError('Saved triangle collision mismatch: ' + label)
            if len(actual) != len(expected):
                raise RuntimeError('Saved hull count mismatch: ' + label)
            for a, e in zip(actual, expected):
                if len(a.vertices) != len(e['vertices']):
                    raise RuntimeError('Saved hull vertex count mismatch: ' + label)
                for v, point in zip(a.vertices, e['vertices']):
                    if max(abs(x-y) for x, y in zip((v.x,v.y,v.z), point)) > .001:
                        raise RuntimeError('Saved collision vertex mismatch: ' + label)
            checked.add(mesh.get_path_name())
        active = i == 0 and instance['collision_enabled'] and (bool(definition['collision']['hulls']) or triangle)
        wanted = unreal.CollisionEnabled.QUERY_AND_PHYSICS if active else unreal.CollisionEnabled.NO_COLLISION
        if component.get_collision_enabled() != wanted:
            raise RuntimeError('Saved collision switch mismatch: ' + label)
        enabled += int(active)
        triangle_enabled += int(active and triangle)
report = {'verified_mesh_sections': len(checked), 'enabled_components': enabled,
          'enabled_triangle_components': triangle_enabled, 'errors': 0}
(root / 'ue-collision-verify.json').write_text(json.dumps(report, indent=2))
unreal.log('OpenWillow collision verified: ' + json.dumps(report))
