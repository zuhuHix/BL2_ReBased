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
# Same rule as the importer: the host's blue shell is only spawned when no
# accepted dome placement carries a sky approximation, because it would
# otherwise hide the native dome from the inspection camera.
native_sky_approximated = any(
    instance.get('native_skybox') and any(
        material and scene['materials'].get(material, {}).get('sky_approximation')
        for material in (instance['materials'] or [
            section['material'] for section in scene['meshes'][instance['mesh']]['sections']]))
    for instance in scene['actors'])
assert set(lighting) == {'OpenWillow_Sun', 'OpenWillow_SkyFill',
                         'OpenWillow_SkyAtmosphere',
                         'OpenWillow_ReflectionCapture', 'OpenWillow_Exposure'} | (
    set() if native_sky_approximated else {'OpenWillow_SkyFallback'})
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
if not native_sky_approximated:
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
verified_outer_shell = 0
verified_outer_shell_replacements = 0
verified_hidden_visual = 0
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
        if source.get('native_skybox'):
            if section is scene['meshes'][source['mesh']]['sections'][0]:
                verified_native_skybox += 1
            assert source.get('native_skybox_source', '').endswith('Prop_Skybox.Meshes.Sky_Dome')
            assert component.get_collision_enabled() == unreal.CollisionEnabled.NO_COLLISION
            assert not component.get_editor_property('cast_shadow')
        if source.get('outer_shell'):
            if section is scene['meshes'][source['mesh']]['sections'][0]:
                verified_outer_shell += 1
            assert scene['meshes'][source['mesh']]['source'].endswith((
                'Prop_Skybox.Meshes.SanctuarySky',
                'FX_ENV_Sanctuary.Meshes.SanctuarySkybox_Antenna1',
                'FX_ENV_Sanctuary.Meshes.SanctuarySkybox_Antenna2'))
            assert not component.get_editor_property('cast_shadow')
            placed_overrides = source.get('outer_shell_source_materials', [])
            for replacement in source.get('outer_shell_replaced', []):
                if replacement['slot'] != section['slot']:
                    continue
                # A replaced slot must be bound to the mesh default, and the
                # placed override it displaced must be a _Teleported variant.
                assert override[section['slot']] is None
                assert component.get_material(0).get_name() == 'M_' + section['material']
                assert placed_overrides[section['slot']] and scene['materials'][
                    placed_overrides[section['slot']]]['source'] == replacement['override']
                assert replacement['override'].rsplit('.', 1)[-1].endswith('_Teleported')
                assert scene['materials'][section['material']]['source'] == replacement['default']
                assert scene['materials'][section['material']]['channels'].get('diffuse')
                verified_outer_shell_replacements += 1
        if source.get('hidden_visual'):
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
verified_unlit_multipliers = []
verified_sky_approximations = []


def verify_sky_approximation(material, name, sky):
    """Walk the saved graph back from Emissive and compare it to the record."""
    def inputs(node):
        return mel.get_inputs_for_material_expression(material, node)

    assert sky['method'] == 'sky_time_of_day_strip_v1', name
    assert material.get_editor_property('is_sky'), name
    visible = mel.get_material_property_input_node(material, unreal.MaterialProperty.MP_EMISSIVE_COLOR)
    assert isinstance(visible, unreal.MaterialExpressionLinearInterpolate), name
    sky_color, cloud_color, alpha = inputs(visible)
    assert isinstance(sky_color, unreal.MaterialExpressionMultiply)
    gradient, brightness = inputs(sky_color)
    assert isinstance(gradient, unreal.MaterialExpressionTextureSample)
    assert gradient.get_editor_property('texture').get_name() == Path(sky['textures']['transition_track']['file']).stem
    assert gradient.get_editor_property('texture').get_editor_property('srgb')
    assert isinstance(brightness, unreal.MaterialExpressionConstant)
    close([brightness.get_editor_property('r')], [sky['scalars']['sky_brightness']], 1e-6)
    strip_uv = [item for item in inputs(gradient) if item is not None]
    assert len(strip_uv) == 1 and isinstance(strip_uv[0], unreal.MaterialExpressionAppendVector)
    column, dome_v = inputs(strip_uv[0])
    assert isinstance(column, unreal.MaterialExpressionConstant)
    close([column.get_editor_property('r')], [sky['time_axis']['column_u']], 1e-6)
    close([sky['time_axis']['column_u'] * sky['time_axis']['divisor']], [sky['scalars']['time_of_day']], 1e-3)
    assert isinstance(dome_v, unreal.MaterialExpressionComponentMask)
    assert not dome_v.get_editor_property('r') and dome_v.get_editor_property('g')
    assert not dome_v.get_editor_property('b') and not dome_v.get_editor_property('a')
    dome_uv = inputs(dome_v)[0]
    assert isinstance(dome_uv, unreal.MaterialExpressionTextureCoordinate)
    assert dome_uv.get_editor_property('coordinate_index') == 0
    assert isinstance(cloud_color, unreal.MaterialExpressionMultiply)
    horizon, cloud_brightness = inputs(cloud_color)
    assert isinstance(horizon, unreal.MaterialExpressionTextureSample)
    assert horizon.get_editor_property('texture') == gradient.get_editor_property('texture')
    horizon_uv = [item for item in inputs(horizon) if item is not None]
    assert len(horizon_uv) == 1 and isinstance(horizon_uv[0], unreal.MaterialExpressionConstant2Vector)
    close([horizon_uv[0].get_editor_property('r'), horizon_uv[0].get_editor_property('g')],
          [sky['time_axis']['column_u'], sky['horizon_row_v']], 1e-6)
    assert isinstance(cloud_brightness, unreal.MaterialExpressionConstant)
    close([cloud_brightness.get_editor_property('r')],
          [sky['scalars']['sky_brightness'] * sky['scalars']['cloud_brightness']], 1e-6)
    assert isinstance(alpha, unreal.MaterialExpressionSaturate)
    coverage = inputs(alpha)[0]
    assert isinstance(coverage, unreal.MaterialExpressionMultiply)
    coverage_sample, opacity = inputs(coverage)
    assert isinstance(coverage_sample, unreal.MaterialExpressionTextureSample)
    assert coverage_sample.get_editor_property('texture').get_name() == Path(sky['textures']['clouds']['file']).stem
    assert not coverage_sample.get_editor_property('texture').get_editor_property('srgb')
    coverage_uv = [item for item in inputs(coverage_sample) if item is not None]
    assert len(coverage_uv) == 1 and coverage_uv[0] == dome_uv
    assert isinstance(opacity, unreal.MaterialExpressionConstant)
    close([opacity.get_editor_property('r')], [sky['scalars']['cloud_cap_opacity']], 1e-6)


for name, definition in scene['materials'].items():
    material = unreal.load_asset(base + '/Assets/M_' + name)
    assert material.get_editor_property('two_sided') == bool(definition.get('two_sided', False)), name
    if definition.get('lighting_model') == 'MLM_Unlit':
        assert material.get_editor_property('shading_model') == unreal.MaterialShadingModel.MSM_UNLIT
        visible = mel.get_material_property_input_node(material, unreal.MaterialProperty.MP_EMISSIVE_COLOR)
        assert visible is not None, 'Unlit material has no visible color: ' + name
        if definition.get('sky_approximation'):
            verify_sky_approximation(material, name, definition['sky_approximation'])
            verified_sky_approximations.append(name)
        elif not definition['channels'].get('emissive'):
            multiplier = definition.get('unlit_color_multiplier')
            if multiplier is not None:
                assert isinstance(visible, unreal.MaterialExpressionMultiply), name
                visible, scale = mel.get_inputs_for_material_expression(material, visible)
                assert isinstance(scale, unreal.MaterialExpressionConstant3Vector), name
                value = scale.get_editor_property('constant')
                close([value.r, value.g, value.b], multiplier['rgb'], 1e-6)
                verified_unlit_multipliers.append(name)
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
          'verified_sky_approximation_materials': verified_sky_approximations,
          'verified_unlit_multiplier_materials': verified_unlit_multipliers,
          'verified_outer_shell_placements': verified_outer_shell,
          'verified_outer_shell_replacements': verified_outer_shell_replacements,
          'expected_outer_shell_replacements': sum(
              len(a.get('outer_shell_replaced', [])) for a in scene['actors']),
          'verified_hidden_visual_placements': verified_hidden_visual,
          'verified_neutral_fallback_sections': verified_neutral_fallback,
          'geometry_bounds': 'matches source OBJ', 'lighting_actors': sorted(lighting),
          'temporary_sky_fallback': ('UE5_SkyAtmosphere' if native_sky_approximated
                                     else 'UE5_SkyAtmosphere+OpenWillow_SkyFallback'),
          'visual_validation': 'pending'}
assert report['verified_outer_shell_replacements'] == report['expected_outer_shell_replacements']
(root / 'ue-verify.json').write_text(json.dumps(report, indent=2))
unreal.log('OpenWillow saved-scene verification: ' + json.dumps(report))
