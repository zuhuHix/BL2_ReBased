"""Run in UE editor Python after import; verifies saved assets, not rendering."""
import json
import math
import os
import sys
from pathlib import Path
import unreal
sys.path.insert(0, str(Path(__file__).resolve().parent))
from scene_geometry import actor_label
from scene_import_rules import temporary_sky_policy, ue3_color_linear


def xyz(v):
    return [v.x, v.y, v.z]


def close(actual, expected, tolerance=.05):
    assert all(abs(a - b) <= tolerance for a, b in zip(actual, expected)), (actual, expected)


root = Path(os.environ['OPENWILLOW_SCENE']).resolve()
scene = json.loads((root / 'scene.json').read_text(encoding='utf-8'))
(root / 'ue-verify.json').unlink(missing_ok=True)
base = '/Game/OpenWillow/' + scene['map']
level = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem)
assert level.load_level(base + '/' + scene['map'])
actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem).get_all_level_actors()
sky_policy = temporary_sky_policy(scene)
source_fog_expected = scene.get('lighting', {}).get('height_fog') is not None
lighting_labels = {'OpenWillow_Sun', 'OpenWillow_SkyFill', 'OpenWillow_SkyAtmosphere',
                   'OpenWillow_SkyFallback', 'OpenWillow_SourceHeightFog',
                   'OpenWillow_ReflectionCapture', 'OpenWillow_Exposure'}
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
native_sky_approximated = sky_policy['native_sky_approximated']
expected_lighting = {'OpenWillow_Sun', 'OpenWillow_SkyFill',
                     'OpenWillow_ReflectionCapture', 'OpenWillow_Exposure'}
if sky_policy['spawn_sky_atmosphere']:
    expected_lighting.add('OpenWillow_SkyAtmosphere')
if sky_policy['spawn_sky_fallback']:
    expected_lighting.add('OpenWillow_SkyFallback')
if source_fog_expected:
    expected_lighting.add('OpenWillow_SourceHeightFog')
assert set(lighting) == expected_lighting
sun_component = lighting['OpenWillow_Sun'].get_component_by_class(unreal.DirectionalLightComponent)
assert sun_component.get_editor_property('mobility') == unreal.ComponentMobility.MOVABLE
assert abs(sun_component.get_editor_property('intensity') - 1.0) < .01
source_directional = scene.get('lighting', {}).get('directional_light')
if source_directional is not None:
    # UE5 normalizes rotator angles, so compare them modulo 360 degrees.
    rotation = lighting['OpenWillow_Sun'].get_actor_rotation()
    for actual, expected in zip([rotation.pitch, rotation.yaw, rotation.roll], source_directional['rotation']):
        assert abs((actual - expected + 180) % 360 - 180) <= 1e-2, (rotation, source_directional['rotation'])
sky_component = lighting['OpenWillow_SkyFill'].get_component_by_class(unreal.SkyLightComponent)
assert sky_component.get_editor_property('mobility') == unreal.ComponentMobility.MOVABLE
assert sky_component.get_editor_property('source_type') == unreal.SkyLightSourceType.SLS_SPECIFIED_CUBEMAP
assert sky_component.get_editor_property('cubemap').get_path_name() == '/Engine/EngineResources/GrayLightTextureCube.GrayLightTextureCube'
source_sky = scene.get('lighting', {}).get('sky_light')
expected_sky_intensity = 1.0 if source_sky is not None else 0.5
assert abs(sky_component.get_editor_property('intensity') - expected_sky_intensity) < .01
if source_sky is not None:
    # light_color is stored as an sRGB FColor; compare bytes against the source FColor.
    color = sky_component.get_editor_property('light_color')
    close([color.r, color.g, color.b], [c * 255 for c in source_sky['light_color_rgb']], 3)
    lower = sky_component.get_editor_property('lower_hemisphere_color')
    close([lower.r, lower.g, lower.b],
          [c * source_sky['lower_brightness'] for c in ue3_color_linear(source_sky['lower_color_rgb'])], 1e-4)
if sky_policy['spawn_sky_atmosphere']:
    atmosphere_component = lighting['OpenWillow_SkyAtmosphere'].get_component_by_class(
        unreal.SkyAtmosphereComponent)
    assert atmosphere_component is not None
if source_fog_expected:
    fog = scene['lighting']['height_fog']
    fog_actor = lighting['OpenWillow_SourceHeightFog']
    close(xyz(fog_actor.get_actor_location()), fog['location'])
    component = fog_actor.get_component_by_class(unreal.ExponentialHeightFogComponent)
    assert abs(component.get_editor_property('fog_density') - fog['density']) < 1e-9
    assert abs(component.get_editor_property('start_distance') - fog['start_distance']) < 1e-3
    color = component.get_editor_property('fog_inscattering_luminance')
    close([color.r, color.g, color.b], [v * fog['light_brightness'] for v in ue3_color_linear(fog['light_color_rgb'])], 1e-4)
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
            # A static placement the source hides (HiddenGame / owner bHidden)
            # may carry any mesh; everything else must be a known helper.
            source_hidden_static = (source.get('source_hidden')
                                    and '.InterpActor_' not in source['source'])
            assert source_hidden_static or hidden_source.endswith((
                'Common_Meshes.Blocking.Blocking_Cube',
                'Common_Meshes.CollisionCube',
                'Common_Meshes.Blocking.Blocking_Plane',
                'Common_Meshes.BasePlane_256x128',
                'Prop_Garbage.Meshes.BoxLrg'))
            if hidden_source.endswith('Prop_Garbage.Meshes.BoxLrg') and not source_hidden_static:
                assert source['source'] in {
                    'TheWorld.PersistentLevel.InterpActor_26.StaticMeshComponent_20',
                    'TheWorld.PersistentLevel.InterpActor_33.StaticMeshComponent_20',
                    'TheWorld.PersistentLevel.InterpActor_34.StaticMeshComponent_20',
                }
            assert not component.get_editor_property('visible')
            definition = scene['meshes'][source['mesh']]
            expected_collision = (unreal.CollisionEnabled.QUERY_AND_PHYSICS
                                  if source.get('collision_enabled', False)
                                  and (definition.get('collision', {}).get('status') == 'triangle_mesh'
                                       or (section is definition['sections'][0]
                                           and bool(definition.get('collision', {}).get('hulls', []))))
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
verified_terrain_blends = []


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


def verify_terrain_blend(material, name, definition):
    """Verify each recorded weight/diffuse input reaches Base Color."""
    assert definition['method'] == 'terrain_weighted_sum_v2', name
    assert definition['mapping_status'] == 'validated', name
    root_node = mel.get_material_property_input_node(material, unreal.MaterialProperty.MP_BASE_COLOR)
    assert root_node is not None, name
    nodes, texture_names = [], []
    pending, seen = [root_node], set()
    while pending:
        node = pending.pop()
        if node is None or id(node) in seen:
            continue
        seen.add(id(node))
        nodes.append(node)
        if isinstance(node, unreal.MaterialExpressionTextureSample):
            texture = node.get_editor_property('texture')
            if texture is not None:
                texture_names.append(texture.get_name())
        pending.extend(item for item in mel.get_inputs_for_material_expression(material, node)
                       if item is not None)
    expected_weights = {Path(layer['weightmap_file']).stem for layer in definition['layers']}
    expected_diffuse = {Path(layer['diffuse']).stem for layer in definition['layers']
                        if layer.get('diffuse')}
    assert expected_weights <= set(texture_names), (name, expected_weights, texture_names)
    assert expected_diffuse <= set(texture_names), (name, expected_diffuse, texture_names)
    assert sum(isinstance(node, unreal.MaterialExpressionMultiply) for node in nodes) >= len(definition['layers'])
    # Layer sums plus one Add per non-zero UV offset (weight texel centring).
    assert sum(isinstance(node, unreal.MaterialExpressionAdd) for node in nodes) >= max(0, len(definition['layers']) - 1)
    for layer in definition['layers']:
        if layer.get('diffuse'):
            continue
        color = layer.get('fallback_color')
        assert isinstance(color, list) and len(color) == 3, name
        assert any(isinstance(node, unreal.MaterialExpressionConstant3Vector) and
                   abs(node.get_editor_property('constant').r - color[0]) < 1e-6 and
                   abs(node.get_editor_property('constant').g - color[1]) < 1e-6 and
                   abs(node.get_editor_property('constant').b - color[2]) < 1e-6
                   for node in nodes), (name, color)
    return {'layers': len(definition['layers']), 'weightmaps': len(expected_weights),
            'diffuse_textures': len(expected_diffuse), 'graph': 'weighted_sum_reaches_base_color'}


def expression_key(node):
    return node.get_path_name()


def material_graph(material, material_property):
    """Return the saved expression graph connected to one material property."""
    root_node = mel.get_material_property_input_node(material, material_property)
    nodes, inputs = {}, {}
    pending = [root_node]
    while pending:
        node = pending.pop()
        if node is None:
            continue
        key = expression_key(node)
        if key in nodes:
            continue
        nodes[key] = node
        inputs[key] = [item for item in mel.get_inputs_for_material_expression(material, node)
                       if item is not None]
        pending.extend(inputs[key])
    return root_node, nodes, inputs


def has_emissive_alpha_mask(nodes, graph_inputs, sample):
    sample_key = expression_key(sample)
    return any(isinstance(node, unreal.MaterialExpressionMultiply)
               and len(graph_inputs[expression_key(node)]) == 2
               and all(expression_key(input_node) == sample_key
                       for input_node in graph_inputs[expression_key(node)])
               for node in nodes.values())


def verify_texture_layers(material, name, layers):
    """Check each recorded layer's sampled channel, color, and visible connection."""
    if not layers:
        return 0
    root_node, nodes, graph_inputs = material_graph(material, unreal.MaterialProperty.MP_EMISSIVE_COLOR)
    assert root_node is not None, name
    samples = {}
    for node in nodes.values():
        if isinstance(node, unreal.MaterialExpressionTextureSample):
            texture = node.get_editor_property('texture')
            if texture is not None:
                samples.setdefault(texture.get_name(), []).append(node)

    used_multipliers = set()
    for layer in layers:
        stem = Path(layer['file']).stem
        matching_samples = [sample for sample in samples.get(stem, [])
                            if sample.get_editor_property('texture').get_editor_property('srgb')
                            == bool(layer['srgb'])]
        assert matching_samples, (name, stem, sorted(samples))
        expected_color = layer['color']
        assert len(expected_color) == 3, (name, stem, expected_color)
        expected_channel = layer['channel']
        matches = []
        for multiplier in nodes.values():
            if not isinstance(multiplier, unreal.MaterialExpressionMultiply):
                continue
            pair = graph_inputs[expression_key(multiplier)]
            if len(pair) != 2 or not isinstance(pair[1], unreal.MaterialExpressionConstant3Vector):
                continue
            color = pair[1].get_editor_property('constant')
            if any(abs(actual - expected) > 1e-6 for actual, expected in
                   zip([color.r, color.g, color.b], expected_color)):
                continue
            source = pair[0]
            for sample in matching_samples:
                sample_key = expression_key(sample)
                if expected_channel == 'RGB':
                    channel_matches = expression_key(source) == sample_key
                else:
                    channel_matches = (isinstance(source, unreal.MaterialExpressionComponentMask)
                                       and graph_inputs[expression_key(source)]
                                       and expression_key(graph_inputs[expression_key(source)][0]) == sample_key
                                       and all(bool(source.get_editor_property(component.lower()))
                                               == (component == expected_channel)
                                               for component in 'RGBA'))
                if channel_matches:
                    matches.append(multiplier)
                    break
        available = [node for node in matches if expression_key(node) not in used_multipliers]
        assert available, (name, stem, expected_channel, expected_color)
        used_multipliers.add(expression_key(available[0]))
    return len(layers)


def verify_emissive_multiplier(material, name, definition):
    """Require the recorded scale on the connected, alpha-masked emissive input."""
    filename = definition['channels'].get('emissive')
    assert filename, (name, 'emissive multiplier has no emissive source texture')
    _, nodes, graph_inputs = material_graph(material, unreal.MaterialProperty.MP_EMISSIVE_COLOR)
    stem = Path(filename).stem
    expected = definition['emissive_multiplier']['rgb']
    matches = []
    for multiplier in nodes.values():
        if not isinstance(multiplier, unreal.MaterialExpressionMultiply):
            continue
        pair = graph_inputs[expression_key(multiplier)]
        if len(pair) != 2 or not isinstance(pair[1], unreal.MaterialExpressionConstant3Vector):
            continue
        scale = pair[1].get_editor_property('constant')
        if any(abs(actual - wanted) > 1e-6 for actual, wanted in
               zip([scale.r, scale.g, scale.b], expected)):
            continue
        source_nodes = []
        pending, seen = [pair[0]], set()
        while pending:
            node = pending.pop()
            if node is None or expression_key(node) in seen:
                continue
            key = expression_key(node)
            seen.add(key)
            source_nodes.append(node)
            pending.extend(graph_inputs.get(key, []))
        source_samples = [node for node in source_nodes
                          if isinstance(node, unreal.MaterialExpressionTextureSample)
                          and node.get_editor_property('texture').get_name() == stem]
        if any(has_emissive_alpha_mask({expression_key(node): node for node in source_nodes},
                                       graph_inputs, sample) for sample in source_samples):
            matches.append(multiplier)
    assert len(matches) == 1, (name, stem, expected, len(matches))
    return True


verified_scene_recipe_layers = 0
verified_scene_recipe_materials = set()
verified_emissive_multipliers = []
verified_unlit_constants = []
for name, definition in scene['materials'].items():
    material = unreal.load_asset(base + '/Assets/M_' + name)
    assert material.get_editor_property('two_sided') == bool(definition.get('two_sided', False)), name
    recipe = definition.get('scene_recipe', {})
    host_shading_name = recipe.get('host_lighting_model',
                                   'MLM_Unlit' if definition.get('unlit_constant_color') else
                                   definition.get('lighting_model', 'MLM_DefaultLit'))
    host_blend_name = definition.get('host_blend_mode',
                                     'BLEND_Opaque' if definition.get('unlit_constant_color') else
                                     definition.get('blend_mode', 'BLEND_Opaque'))
    if definition.get('unlit_constant_color'):
        assert material.get_editor_property('blend_mode') == unreal.BlendMode.BLEND_OPAQUE, name
    if definition.get('emissive_multiplier') is not None:
        verify_emissive_multiplier(material, name, definition)
        verified_emissive_multipliers.append(name)
    if recipe.get('emissive_layers'):
        verified_scene_recipe_layers += verify_texture_layers(material, name, recipe['emissive_layers'])
        verified_scene_recipe_materials.add(name)
    if recipe.get('unlit_color_layers'):
        verified_scene_recipe_layers += verify_texture_layers(material, name, recipe['unlit_color_layers'])
        verified_scene_recipe_materials.add(name)
    if recipe.get('diffuse'):
        assert definition['channels'].get('diffuse') == recipe['diffuse'], name
    if definition.get('terrain_blend'):
        verified_terrain_blends.append({'material': name,
                                       **verify_terrain_blend(material, name, definition['terrain_blend'])})
    if host_shading_name == 'MLM_Unlit':
        assert material.get_editor_property('shading_model') == unreal.MaterialShadingModel.MSM_UNLIT
        visible = mel.get_material_property_input_node(material, unreal.MaterialProperty.MP_EMISSIVE_COLOR)
        assert visible is not None, 'Unlit material has no visible color: ' + name
        if definition.get('sky_approximation'):
            verify_sky_approximation(material, name, definition['sky_approximation'])
            verified_sky_approximations.append(name)
        elif definition.get('unlit_constant_color') is not None:
            expected_color = definition['unlit_constant_color']['rgb']
            assert isinstance(visible, unreal.MaterialExpressionConstant3Vector), name
            value = visible.get_editor_property('constant')
            close([value.r, value.g, value.b], expected_color, 1e-6)
            verified_unlit_constants.append(name)
        elif recipe.get('unlit_color_layers'):
            pass
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
                # Same neutral as import_level.py: white (no change) for Modulate.
                neutral = ([1.0, 1.0, 1.0] if host_blend_name == 'BLEND_Modulate'
                           else [0.5, 0.5, 0.5])
                close([value.r, value.g, value.b], definition.get('constant_diffuse') or neutral, 1e-6)
        verified_unlit_materials.append(name)
    for channel, filename in definition['channels'].items():
        if not filename:
            continue
        if channel == 'emissive':
            _, graph_nodes, graph_inputs = material_graph(material, channels[channel])
            emissive_samples = [node for node in graph_nodes.values()
                                if isinstance(node, unreal.MaterialExpressionTextureSample)
                                and node.get_editor_property('texture').get_name() == Path(filename).stem]
            assert emissive_samples, (name, filename)
            assert any(has_emissive_alpha_mask(graph_nodes, graph_inputs, sample)
                       for sample in emissive_samples), (name, filename, 'missing RGB/alpha emissive mask')
            node = emissive_samples[0]
        else:
            node = mel.get_material_property_input_node(material, channels[channel])
            assert isinstance(node, unreal.MaterialExpressionTextureSample), (name, channel)
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
          'verified_terrain_blend_materials': verified_terrain_blends,
          'verified_unlit_multiplier_materials': verified_unlit_multipliers,
          'verified_emissive_multiplier_materials': verified_emissive_multipliers,
          'verified_unlit_constant_materials': verified_unlit_constants,
          'verified_scene_recipe_material_count': len(verified_scene_recipe_materials),
          'verified_scene_recipe_layer_count': verified_scene_recipe_layers,
          'verified_outer_shell_placements': verified_outer_shell,
          'verified_outer_shell_replacements': verified_outer_shell_replacements,
          'expected_outer_shell_replacements': sum(
              len(a.get('outer_shell_replaced', [])) for a in scene['actors']),
          'verified_hidden_visual_placements': verified_hidden_visual,
          'verified_neutral_fallback_sections': verified_neutral_fallback,
          'geometry_bounds': 'matches source OBJ', 'lighting_actors': sorted(lighting),
          'temporary_sky_fallback': ('none_native_dome_approximation' if native_sky_approximated
                                     else 'UE5_SkyAtmosphere+OpenWillow_SkyFallback'),
          'visual_validation': 'pending'}
assert report['verified_outer_shell_replacements'] == report['expected_outer_shell_replacements']
(root / 'ue-verify.json').write_text(json.dumps(report, indent=2))
unreal.log('OpenWillow saved-scene verification: ' + json.dumps(report))
