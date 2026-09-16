"""Run in UE editor Python after import; verifies saved assets, not rendering."""
import json
import math
import os
import sys
from pathlib import Path
import unreal
sys.path.insert(0, str(Path(__file__).resolve().parent))
from scene_geometry import actor_label

root = Path(os.environ['OPENWILLOW_SCENE']).resolve()
scene = json.loads((root / 'scene.json').read_text(encoding='utf-8'))
(root / 'ue-verify.json').unlink(missing_ok=True)
base = '/Game/OpenWillow/' + scene['map']
level = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem)
assert level.load_level(base + '/' + scene['map'])
actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem).get_all_level_actors()
lighting_labels = {'OpenWillow_Sun', 'OpenWillow_SkyFill', 'OpenWillow_SkyAtmosphere',
                   'OpenWillow_SkyFallback', 'OpenWillow_ReflectionCapture', 'OpenWillow_Exposure'}
placed = {a.get_actor_label(): a for a in actors if isinstance(a, unreal.StaticMeshActor)
          and a.get_actor_label().startswith('OpenWillow_')
          and a.get_actor_label() not in lighting_labels}
expected_count = sum(len(scene['meshes'][a['mesh']]['sections']) for a in scene['actors'])
assert len(placed) == expected_count, (len(placed), expected_count)

lighting = {a.get_actor_label(): a for a in actors if a.get_actor_label().startswith('OpenWillow_')
            and a.get_actor_label() in lighting_labels}
assert set(lighting) == {'OpenWillow_Sun', 'OpenWillow_SkyFill',
                         'OpenWillow_SkyAtmosphere',
                         'OpenWillow_SkyFallback',
                         'OpenWillow_ReflectionCapture', 'OpenWillow_Exposure'}
sun_component = lighting['OpenWillow_Sun'].get_component_by_class(unreal.DirectionalLightComponent)
assert sun_component.get_editor_property('mobility') == unreal.ComponentMobility.MOVABLE
assert abs(sun_component.get_editor_property('intensity') - 1.0) < .01
sky_component = lighting['OpenWillow_SkyFill'].get_component_by_class(unreal.SkyLightComponent)
assert sky_component.get_editor_property('mobility') == unreal.ComponentMobility.MOVABLE
assert sky_component.get_editor_property('source_type') == unreal.SkyLightSourceType.SLS_SPECIFIED_CUBEMAP
assert sky_component.get_editor_property('cubemap').get_path_name() == '/Engine/EngineResources/GrayLightTextureCube.GrayLightTextureCube'
assert abs(sky_component.get_editor_property('intensity') - 0.5) < .01
atmosphere_component = lighting['OpenWillow_SkyAtmosphere'].get_component_by_class(
    unreal.SkyAtmosphereComponent)
assert atmosphere_component is not None
sky_fallback_component = lighting['OpenWillow_SkyFallback'].static_mesh_component
assert sky_fallback_component.get_editor_property('static_mesh').get_path_name() == (
    '/Engine/BasicShapes/Sphere.Sphere')
assert sky_fallback_component.get_material(0).get_name() == 'M_OpenWillowSkyFallback'
assert sky_fallback_component.get_collision_enabled() == unreal.CollisionEnabled.NO_COLLISION
assert sky_fallback_component.get_editor_property('reverse_culling')
try:
    assert not sky_fallback_component.get_editor_property('cast_shadow')
except Exception:
    pass
capture_component = lighting['OpenWillow_ReflectionCapture'].get_component_by_class(
    unreal.SphereReflectionCaptureComponent)
assert capture_component.get_editor_property('influence_radius') >= 1000.0
try:
    assert capture_component.get_editor_property('runtime_capture')
except Exception:
    pass
post = lighting['OpenWillow_Exposure']
assert post.get_editor_property('unbound')
assert abs(post.get_editor_property('blend_weight') - 1.0) < .01
post_settings = post.get_editor_property('settings')
for name in ('override_auto_exposure_method', 'override_auto_exposure_min_brightness',
             'override_auto_exposure_max_brightness', 'override_auto_exposure_bias'):
    try:
        value = post_settings.get_editor_property(name)
    except Exception:
        # Older UE5 settings structs may expose the override bit with a b_ prefix.
        try:
            value = post_settings.get_editor_property('b_' + name)
        except Exception:
            continue
    assert not value, name


def xyz(v):
    return [v.x, v.y, v.z]


def close(actual, expected, tolerance=.05):
    assert all(abs(a - b) <= tolerance for a, b in zip(actual, expected)), (actual, expected)


# Independently compare collection world translation and axis lengths to the
# serialized data; no reuse of the importer's placement function.
verified_native_skybox = 0
verified_hidden_visual = 0
verified_unsupported_translucent = 0
verified_neutral_fallback = 0
for source in scene['actors']:
    for section in scene['meshes'][source['mesh']]['sections']:
        label = actor_label(source['level'], source['source'], section['slot'])
        actor = placed[label]
        component = actor.static_mesh_component
        assert component.get_editor_property('mobility') == unreal.ComponentMobility.STATIC
        assert not component.is_simulating_physics()
        if 'matrix' in source['transform']:
            matrix = source['transform']['matrix']
            close(xyz(actor.get_actor_location()), matrix[12:15])
            close(xyz(actor.get_actor_scale3d()), source['transform']['scale'])
            close(xyz(actor.get_actor_forward_vector()), matrix[0:3], .0001)
            close(xyz(actor.get_actor_right_vector()), matrix[4:7], .0001)
            close(xyz(actor.get_actor_up_vector()), matrix[8:11], .0001)
        override = source['materials']
        material = override[section['slot']] if section['slot'] < len(override) and override[section['slot']] else section['material']
        if material:
            assert component.get_material(0).get_name() == 'M_' + material
        else:
            # Unresolved preparer materials must carry the labeled host
            # neutral fallback, never UE's default WorldGridMaterial.
            assert component.get_material(0).get_name() == 'M_OpenWillowNeutralFallback'
            verified_neutral_fallback += 1
        material_definition = scene['materials'].get(material) if material else None
        unsupported_translucent = bool(material_definition and
            material_definition.get('blend_mode', 'BLEND_Opaque') != 'BLEND_Opaque' and
            not any(material_definition.get('channels', {}).values()) and
            'constant_diffuse' not in material_definition and
            'surface_approximation' not in material_definition)
        if unsupported_translucent:
            assert actor.actor_has_tag('OpenWillow_UnsupportedTranslucent')
        if unsupported_translucent and not source.get('hidden_visual'):
            assert not component.get_editor_property('visible')
            verified_unsupported_translucent += 1
        if source.get('native_skybox'):
            if section is scene['meshes'][source['mesh']]['sections'][0]:
                verified_native_skybox += 1
            assert source.get('native_skybox_source', '').endswith('Prop_Skybox.Meshes.Sky_Dome')
            assert component.get_collision_enabled() == unreal.CollisionEnabled.NO_COLLISION
            assert not component.get_editor_property('cast_shadow')
        if source.get('hidden_visual'):
            assert actor.actor_has_tag('OpenWillow_HiddenVisual')
            if section is scene['meshes'][source['mesh']]['sections'][0]:
                verified_hidden_visual += 1
            hidden_source = scene['meshes'][source['mesh']]['source']
            assert hidden_source.endswith((
                'Common_Meshes.Blocking.Blocking_Cube',
                'Common_Meshes.CollisionCube',
                'Common_Meshes.Blocking.Blocking_Plane',
                'Common_Meshes.BasePlane_256x128',
                'Prop_Garbage.Meshes.BoxLrg'))
            if hidden_source.endswith('Prop_Garbage.Meshes.BoxLrg'):
                assert source['source'].endswith(
                    'TheWorld.PersistentLevel.InterpActor_34.StaticMeshComponent_20')
            assert not component.get_editor_property('visible')
            definition = scene['meshes'][source['mesh']]
            expected_collision = (unreal.CollisionEnabled.QUERY_AND_PHYSICS
                                  if source.get('collision_enabled', False)
                                  and section is definition['sections'][0]
                                  and bool(definition.get('collision', {}).get('hulls', []))
                                  else unreal.CollisionEnabled.NO_COLLISION)
            assert component.get_collision_enabled() == expected_collision

# Check each imported section's geometry bounds against the OBJ's referenced
# vertices. This catches silent OBJ axis/unit conversion by the host importer.
for definition in scene['meshes'].values():
    for section in definition['sections']:
        filename = root / section['file']
        vertices, used = [], set()
        for line in filename.read_text().splitlines():
            if line.startswith('v '):
                vertices.append(list(map(float, line.split()[1:4])))
            elif line.startswith('f '):
                used.update(int(v.split('/')[0]) - 1 for v in line.split()[1:])
        if not used:
            continue
        minimum = [min(vertices[i][k] for i in used) for k in range(3)]
        maximum = [max(vertices[i][k] for i in used) for k in range(3)]
        mesh = unreal.load_asset(base + '/Assets/' + filename.stem)
        bounds = mesh.get_bounds()
        close(xyz(bounds.origin), [(a + b) * .5 for a, b in zip(minimum, maximum)])
        close(xyz(bounds.box_extent), [(b - a) * .5 for a, b in zip(minimum, maximum)])

mel = unreal.MaterialEditingLibrary
channels = {'diffuse': unreal.MaterialProperty.MP_BASE_COLOR, 'normal': unreal.MaterialProperty.MP_NORMAL,
            'specular': unreal.MaterialProperty.MP_SPECULAR, 'emissive': unreal.MaterialProperty.MP_EMISSIVE_COLOR}
verified_channels = set()
verified_unlit_materials = []
for name, definition in scene['materials'].items():
    material = unreal.load_asset(base + '/Assets/M_' + name)
    assert material.get_editor_property('two_sided') == bool(definition.get('two_sided', False)), name
    if definition.get('lighting_model') == 'MLM_Unlit':
        assert material.get_editor_property('shading_model') == unreal.MaterialShadingModel.MSM_UNLIT
        visible = mel.get_material_property_input_node(material, unreal.MaterialProperty.MP_EMISSIVE_COLOR)
        assert visible is not None, 'Unlit material has no visible color: ' + name
        if not definition['channels'].get('emissive'):
            if definition['channels'].get('diffuse'):
                assert isinstance(visible, unreal.MaterialExpressionTextureSample)
                assert visible.get_editor_property('texture').get_name() == Path(definition['channels']['diffuse']).stem
            else:
                assert isinstance(visible, unreal.MaterialExpressionConstant3Vector)
                value = visible.get_editor_property('constant')
                close([value.r, value.g, value.b], definition.get('constant_diffuse') or [0.5, 0.5, 0.5], 1e-6)
        verified_unlit_materials.append(name)
    for channel, filename in definition['channels'].items():
        if not filename:
            continue
        node = mel.get_material_property_input_node(material, channels[channel])
        if channel == 'emissive':
            assert isinstance(node, unreal.MaterialExpressionMultiply)
            node = mel.get_inputs_for_material_expression(material, node)[0]
        assert isinstance(node, unreal.MaterialExpressionTextureSample)
        uv = definition.get('channel_uv', {}).get(channel)
        if uv is not None:
            inputs = [item for item in mel.get_inputs_for_material_expression(material, node) if item is not None]
            assert len(inputs) == 1 and isinstance(inputs[0], unreal.MaterialExpressionTextureCoordinate)
            coordinates = inputs[0]
            assert coordinates.get_editor_property('coordinate_index') == uv['index']
            close([coordinates.get_editor_property('u_tiling'), coordinates.get_editor_property('v_tiling')], uv['scale'], 1e-6)
        texture = node.get_editor_property('texture')
        assert texture.get_name() == Path(filename).stem
        assert texture.get_editor_property('srgb') == (channel in ('diffuse', 'emissive'))
        if channel == 'normal':
            assert texture.get_editor_property('compression_settings') == unreal.TextureCompressionSettings.TC_NORMALMAP
        verified_channels.add(channel)

if scene['map'] == 'MaterialV1Smoke':
    assert verified_channels == set(channels)
    ordinary = placed[actor_label('Synthetic_P', 'Synthetic.Mesh', 0)]
    close(xyz(ordinary.get_actor_location()), [100, 210, 300])
    close(xyz(ordinary.get_actor_scale3d()), [2, 3, 4])
report = {'verified_section_actors': len(placed), 'verified_channels': sorted(verified_channels),
          'verified_unlit_materials': verified_unlit_materials,
          'verified_native_skybox_placements': verified_native_skybox,
          'verified_hidden_visual_placements': verified_hidden_visual,
          'verified_unsupported_translucent_sections': verified_unsupported_translucent,
          'verified_neutral_fallback_sections': verified_neutral_fallback,
          'geometry_bounds': 'matches source OBJ', 'lighting_actors': sorted(lighting),
          'temporary_sky_fallback': 'UE5_SkyAtmosphere+OpenWillow_SkyFallback',
          'visual_validation': 'pending'}
(root / 'ue-verify.json').write_text(json.dumps(report, indent=2))
unreal.log('OpenWillow saved-scene verification: ' + json.dumps(report))
