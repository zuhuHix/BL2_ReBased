"""Run in UE editor Python after import; verifies saved assets, not rendering."""
import json
import math
import os
import sys
from pathlib import Path
import unreal
sys.path.insert(0, str(Path(__file__).resolve().parent))
mel = unreal.MaterialEditingLibrary
from scene_geometry import actor_label
from scene_import_rules import (SANCTUARY_GRAFITTO_INK_COLOR,
                                SANCTUARY_GRAFITTO_INK_LUMINANCE_CEILING,
                                SANCTUARY_GRAFITTO_INK_OPACITY_GAIN,
                                SANCTUARY_LIGHT_CELL_SIZE,
                                SANCTUARY_LIGHT_BRIGHTNESS_CEILING,
                                SANCTUARY_ORANGE_POINT_LIGHT_INTENSITY_SCALE,
                                SANCTUARY_LIGHT_MAX_IMPORTED,
                                SANCTUARY_LIGHT_MAX_PER_CELL,
                                SANCTUARY_LIGHT_MIN_BRIGHTNESS,
                                SANCTUARY_LIGHT_MIN_RADIUS,
                                SANCTUARY_POINT_LIGHT_CANDELA_PER_SOURCE_BRIGHTNESS,
                                SANCTUARY_SPOT_LIGHT_CANDELA_PER_SOURCE_BRIGHTNESS,
                                SANCTUARY_SKY_INTENSITY_CALIBRATION,
                                temporary_sky_policy, ue3_color_linear,
                                ue3_grafitto_dimensions_to_ue5,
                                ue3_grafitto_projection_to_ue5,
                                ue3_directional_brightness_to_ue5,
                                sanctuary_light_calibration,
                                ue3_sky_light_color_to_ue5,
                                ue3_world_post_process_to_ue5,
                                sanctuary_opt_ins, sanctuary_profile,
                                sanctuary_source_lighting, effective_scene_recipe,
                                SANCTUARY_OPT_IN_ENV, SANCTUARY_XMAS_BULB_METHOD)


def xyz(v):
    return [v.x, v.y, v.z]


def close(actual, expected, tolerance=.05):
    assert all(abs(a - b) <= tolerance for a, b in zip(actual, expected)), (actual, expected)


def linear_to_srgb_byte(value):
    value = max(0.0, min(1.0, float(value)))
    encoded = (12.92 * value if value <= 0.0031308
               else 1.055 * value ** (1.0 / 2.4) - 0.055)
    return round(encoded * 255.0)


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


def verify_world_map_recipe(material, name, recipe):
    """Check the saved pale-cyan backing and three exact mask-to-palette lerps."""
    root_node, nodes, graph_inputs = material_graph(
        material, unreal.MaterialProperty.MP_EMISSIVE_COLOR)
    assert isinstance(root_node, unreal.MaterialExpressionLinearInterpolate), name
    expected_layers = [('markers', 'B'), ('grid', 'G'), ('land', 'R')]
    current = root_node
    seen_samples = set()
    for palette_name, channel in expected_layers:
        assert isinstance(current, unreal.MaterialExpressionLinearInterpolate), (name, palette_name)
        inputs = graph_inputs[expression_key(current)]
        assert len(inputs) == 3, (name, palette_name, len(inputs))
        prior, color_node, alpha = inputs
        assert isinstance(color_node, unreal.MaterialExpressionConstant3Vector), (name, palette_name)
        expected_color = recipe['palette'][palette_name]
        color = color_node.get_editor_property('constant')
        close([color.r, color.g, color.b], expected_color, 1e-6)
        assert isinstance(alpha, unreal.MaterialExpressionComponentMask), (name, palette_name)
        assert all(bool(alpha.get_editor_property(component.lower())) == (component == channel)
                   for component in 'RGBA'), (name, palette_name, channel)
        mask_inputs = graph_inputs[expression_key(alpha)]
        assert len(mask_inputs) == 1 and isinstance(mask_inputs[0], unreal.MaterialExpressionTextureSample)
        sample = mask_inputs[0]
        seen_samples.add(expression_key(sample))
        assert sample.get_editor_property('texture').get_name() == Path(
            recipe['mask_texture_file']).stem, name
        assert not sample.get_editor_property('texture').get_editor_property('srgb'), name
        current = prior
    assert isinstance(current, unreal.MaterialExpressionConstant3Vector), name
    background = current.get_editor_property('constant')
    close([background.r, background.g, background.b], recipe['palette']['background'], 1e-6)
    assert len(seen_samples) == 1 and len(nodes) >= 10, (name, len(seen_samples), len(nodes))
    return {'material': name, 'channels': ['R', 'G', 'B'],
            'mask_texture': recipe['mask_texture_source'], 'graph': 'cyan_backing_mask_lerps'}


root = Path(os.environ['OPENWILLOW_SCENE']).resolve()
scene = json.loads((root / 'scene.json').read_text(encoding='utf-8'))
# Same opt-in rule as import_level.py: each round-4 flag is checked on or off below.
opt_ins = sanctuary_opt_ins(os.environ.get(SANCTUARY_OPT_IN_ENV, ''), scene)
source_lighting = sanctuary_source_lighting(scene)
import_report = json.loads((root / 'ue-import.json').read_text(encoding='utf-8'))
(root / 'ue-verify.json').unlink(missing_ok=True)
base = '/Game/OpenWillow/' + scene['map']
level = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem)
assert level.load_level(base + '/' + scene['map'])
actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem).get_all_level_actors()
sky_policy = temporary_sky_policy(scene)
source_fog_expected = source_lighting.get('height_fog') is not None
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
source_directional = source_lighting.get('directional_light')
expected_sun_intensity = (ue3_directional_brightness_to_ue5(source_directional['light_brightness'])
                          if source_directional is not None
                          and 'light_brightness' in source_directional else 1.0)
assert abs(sun_component.get_editor_property('intensity') - expected_sun_intensity) < .01
if source_directional is not None:
    # UE5 normalizes rotator angles, so compare them modulo 360 degrees.
    rotation = lighting['OpenWillow_Sun'].get_actor_rotation()
    for actual, expected in zip([rotation.pitch, rotation.yaw, rotation.roll], source_directional['rotation']):
        assert abs((actual - expected + 180) % 360 - 180) <= 1e-2, (rotation, source_directional['rotation'])
    if 'light_color_rgb' in source_directional:
        color = sun_component.get_editor_property('light_color')
        close([color.r, color.g, color.b], [c * 255 for c in source_directional['light_color_rgb']], 3)
sky_component = lighting['OpenWillow_SkyFill'].get_component_by_class(unreal.SkyLightComponent)
assert sky_component.get_editor_property('mobility') == unreal.ComponentMobility.MOVABLE
assert sky_component.get_editor_property('source_type') == unreal.SkyLightSourceType.SLS_SPECIFIED_CUBEMAP
assert sky_component.get_editor_property('cubemap').get_path_name() == '/Engine/EngineResources/GrayLightTextureCube.GrayLightTextureCube'
source_sky = source_lighting.get('sky_light')
expected_sky_intensity = SANCTUARY_SKY_INTENSITY_CALIBRATION if source_sky is not None else 0.5
assert abs(sky_component.get_editor_property('intensity') - expected_sky_intensity) < .01
if source_sky is not None:
    # The calibrated linear fill is stored back as sRGB FColor channels.
    color = sky_component.get_editor_property('light_color')
    calibrated_sky = ue3_sky_light_color_to_ue5(source_sky['light_color_rgb'])
    close([color.r, color.g, color.b],
          [value ** (1.0 / 2.2) * 255 for value in calibrated_sky], 3)
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
             'override_auto_exposure_max_brightness'):
    try:
        value = post_settings.get_editor_property(name)
    except Exception:
        # Older UE5 settings structs may expose the override bit with a b_ prefix.
        try:
            value = post_settings.get_editor_property('b_' + name)
        except Exception:
            continue
    assert not value, name
world_info = next((item for item in source_lighting.get('world_info', [])
                   if item.get('default_post_process_settings')), None)
source_post_process = (ue3_world_post_process_to_ue5(world_info['default_post_process_settings'])
                       if world_info is not None else None)
if source_post_process is not None:
    for override_name, value_name in (
            ('override_auto_exposure_bias', 'auto_exposure_bias'),
            ('override_color_gain', 'color_gain'),
            ('override_bloom_intensity', 'bloom_intensity')):
        assert post_settings.get_editor_property(override_name), override_name
        actual = post_settings.get_editor_property(value_name)
        expected = source_post_process[value_name]
        if value_name == 'color_gain':
            close([actual.x, actual.y, actual.z], expected, 1e-5)
        else:
            assert abs(float(actual) - float(expected)) < 1e-5, (value_name, actual, expected)
grade_on = 'grade' in opt_ins
if grade_on:
    # Round-4 host grade: the expected values are written out here, not imported,
    # so a silent edit to scene_import_rules.py fails this check.
    for override_name in ('override_white_temp', 'override_white_tint',
                          'override_color_saturation', 'override_color_contrast',
                          'override_color_offset'):
        assert post_settings.get_editor_property(override_name), override_name
    assert abs(float(post_settings.get_editor_property('white_temp')) - 8200.0) < 1e-3
    assert abs(float(post_settings.get_editor_property('white_tint')) - 0.0) < 1e-6
    for value_name, expected in (('color_saturation', [1.12, 1.02, 0.92, 1.0]),
                                 ('color_contrast', [1.10, 1.10, 1.10, 1.0])):
        actual = post_settings.get_editor_property(value_name)
        close([actual.x, actual.y, actual.z, actual.w], expected, 1e-5)
    # Source offset (zero when the WorldInfo grade is not decoded) plus the literal host lift.
    source_offset = (source_post_process['color_offset'] if source_post_process is not None
                     else [0.0, 0.0, 0.0])
    actual_offset = post_settings.get_editor_property('color_offset')
    close([actual_offset.x, actual_offset.y, actual_offset.z],
          [source_offset[0] + 0.03, source_offset[1] + 0.03, source_offset[2] + 0.035], 1e-5)
else:
    # Off: no round-4 grade overrides; only the decoded source offset remains.
    for override_name in ('override_white_temp', 'override_white_tint',
                          'override_color_saturation', 'override_color_contrast'):
        assert not post_settings.get_editor_property(override_name), override_name
    off_offset = (source_post_process['color_offset'] if source_post_process is not None
                  else [0.0, 0.0, 0.0])
    off_actual = post_settings.get_editor_property('color_offset')
    close([off_actual.x, off_actual.y, off_actual.z], off_offset, 1e-5)
source_volume_records = source_lighting.get('post_process_volumes', [])
host_post_process_volumes = [actor for actor in actors
                             if isinstance(actor, unreal.PostProcessVolume)]
assert [actor.get_actor_label() for actor in host_post_process_volumes] == ['OpenWillow_Exposure']
assert all(isinstance(item.get('settings'), dict) and item.get('bounds_status')
           == 'brush bounds not decoded' for item in source_volume_records)

# Independently match only the default-allowlisted graffiti to a saved UE5
# decal; all other source decal materials must stay excluded.
decal_manifest = scene.get('decals', {})
source_decals = decal_manifest.get('actors', [])
supported_decals = [item for item in source_decals if item.get('status') == 'partial_unverified']
excluded_decals = [item for item in source_decals if item.get('status') == 'excluded_default_deny']
decal_allowlist = {'Sanctuary_P:Prop_Decals.DMat_GrafittoBurrito'}
decal_actors = {actor.get_actor_label(): actor for actor in actors
                if isinstance(actor, unreal.DecalActor)}
assert len(source_decals) == decal_manifest.get('census', {}).get('actors', len(source_decals))
assert all(item.get('material_source') in decal_allowlist for item in supported_decals)
assert all(item.get('exclusion_reason') and item.get('material_source') not in decal_allowlist
           for item in excluded_decals)
graffiti_on = 'graffiti_decals' in opt_ins
if not graffiti_on:
    # Off: the decals stay recorded in scene.json and no DecalActor is spawned.
    assert not any(isinstance(actor, unreal.DecalActor) for actor in actors)
    assert not decal_actors
else:
    assert len(decal_actors) == len(supported_decals), (len(decal_actors), len(supported_decals))
expected_excluded = decal_manifest.get('census', {}).get('excluded_import_candidates', 0)
assert len(excluded_decals) == expected_excluded
if graffiti_on:
    for source in supported_decals:
        label = actor_label(source['level'], source['source'], 0)
        actor = decal_actors[label]
        basis = source.get('projection_basis', {})
        normal = basis.get('HitNormal', [0.0, 0.0, 0.0])
        expected_location, expected_forward = ue3_grafitto_projection_to_ue5(
            source['transform']['location'], normal, source['dimensions'])
        close(xyz(actor.get_actor_location()), expected_location, .01)
        close(xyz(actor.get_actor_forward_vector()), expected_forward, .001)
        component = actor.get_component_by_class(unreal.DecalComponent)
        assert component is not None
        size = component.get_editor_property('decal_size')
        close(xyz(size), ue3_grafitto_dimensions_to_ue5(source['dimensions']), .001)
        material = component.get_editor_property('decal_material')
        assert material is not None and material.get_name() == 'M_OpenWillowDecal_' + source['material']
        assert material.get_editor_property('material_domain') == unreal.MaterialDomain.MD_DEFERRED_DECAL
        assert material.get_editor_property('blend_mode') == unreal.BlendMode.BLEND_TRANSLUCENT
        source_material = decal_manifest.get('materials', {}).get(source['material'], {})
        diffuse = mel.get_material_property_input_node(material, unreal.MaterialProperty.MP_BASE_COLOR)
        opacity = mel.get_material_property_input_node(material, unreal.MaterialProperty.MP_OPACITY)
        assert source_material.get('method') == 'grafitto_dark_ink_deferred_decal_v1'
        assert source_material.get('opacity_source') == 'grafitto_dark_ink_mask_v1'
        assert source_material.get('ink_color_linear') == list(SANCTUARY_GRAFITTO_INK_COLOR)
        assert source_material.get('opacity_luminance_ceiling') == SANCTUARY_GRAFITTO_INK_LUMINANCE_CEILING
        assert source_material.get('opacity_gain') == SANCTUARY_GRAFITTO_INK_OPACITY_GAIN
        assert isinstance(diffuse, unreal.MaterialExpressionConstant3Vector)
        ink = diffuse.get_editor_property('constant')
        close([ink.r, ink.g, ink.b], SANCTUARY_GRAFITTO_INK_COLOR, 1e-6)
        assert isinstance(opacity, unreal.MaterialExpressionSaturate)
        _, opacity_nodes, opacity_inputs = material_graph(material, unreal.MaterialProperty.MP_OPACITY)
        assert any(isinstance(node, unreal.MaterialExpressionDotProduct)
                   and len(opacity_inputs[expression_key(node)]) == 2
                   and isinstance(opacity_inputs[expression_key(node)][1], unreal.MaterialExpressionConstant3Vector)
                   and all(abs(actual - expected) < 1e-6 for actual, expected in zip(
                       [opacity_inputs[expression_key(node)][1].get_editor_property('constant').r,
                        opacity_inputs[expression_key(node)][1].get_editor_property('constant').g,
                        opacity_inputs[expression_key(node)][1].get_editor_property('constant').b],
                        [0.2126, 0.7152, 0.0722]))
                   for node in opacity_nodes.values())
        assert any(isinstance(node, unreal.MaterialExpressionConstant)
                   and abs(float(node.get_editor_property('r'))
                           - SANCTUARY_GRAFITTO_INK_LUMINANCE_CEILING) < 1e-6
                   for node in opacity_nodes.values())
        assert any(isinstance(node, unreal.MaterialExpressionConstant)
                   and abs(float(node.get_editor_property('r')) - SANCTUARY_GRAFITTO_INK_OPACITY_GAIN) < 1e-6
                   for node in opacity_nodes.values())
        assert any(isinstance(node, unreal.MaterialExpressionTextureSample)
                   and node.get_editor_property('texture').get_name()
                   == Path(source_material['texture_file']).stem for node in opacity_nodes.values())
        tags = [str(tag) for tag in actor.get_editor_property('tags')]
        assert source['source'] in tags
        # Read the axes as values: a bound method of a UE actor left in a module
        # global crashed python311.dll when the commandlet finalized Python (exit 3).
        for vector_name, host_axis in (('HitTangent', actor.get_actor_right_vector()),
                                       ('HitBinormal', actor.get_actor_up_vector())):
            expected_axis = source.get('projection_basis', {}).get(vector_name, [0.0, 0.0, 0.0])
            magnitude = math.sqrt(sum(float(value) ** 2 for value in expected_axis))
            if magnitude > 1e-5:
                close(xyz(host_axis), [float(value) / magnitude for value in expected_axis], .002)

static_light_manifest = source_lighting.get('static_light_import', {})
static_light_records = static_light_manifest.get('components', [])
selection_policy = static_light_manifest.get('selection_policy', {})
assert selection_policy.get('min_radius') == SANCTUARY_LIGHT_MIN_RADIUS
assert selection_policy.get('min_brightness') == SANCTUARY_LIGHT_MIN_BRIGHTNESS
assert selection_policy.get('cell_size') == SANCTUARY_LIGHT_CELL_SIZE
assert selection_policy.get('max_per_cell') == SANCTUARY_LIGHT_MAX_PER_CELL
assert selection_policy.get('max_imported') == SANCTUARY_LIGHT_MAX_IMPORTED
candidate_records = []
for item in static_light_records:
    if item.get('enabled') is False or item.get('status') == 'unsupported':
        continue
    radius, brightness, position = item.get('radius'), item.get('brightness'), item.get('position')
    valid = (item.get('kind') in ('Point', 'Spot')
             and isinstance(radius, (int, float)) and not isinstance(radius, bool)
             and math.isfinite(radius) and isinstance(brightness, (int, float))
             and not isinstance(brightness, bool) and math.isfinite(brightness)
             and isinstance(position, list) and len(position) == 3
             and all(isinstance(value, (int, float)) and not isinstance(value, bool)
                     and math.isfinite(value) for value in position)
             and isinstance(item.get('light_color_rgb'), list)
             and len(item['light_color_rgb']) == 3
             and all(isinstance(value, (int, float)) and 0 <= value <= 1
                     for value in item['light_color_rgb']))
    if not valid or radius < SANCTUARY_LIGHT_MIN_RADIUS or brightness < SANCTUARY_LIGHT_MIN_BRIGHTNESS:
        continue
    item['_verify_score'] = float(radius) * float(brightness)
    item['_verify_cell'] = tuple(math.floor(float(value) / SANCTUARY_LIGHT_CELL_SIZE)
                                 for value in position)
    candidate_records.append(item)
candidate_buckets = {}
for item in candidate_records:
    candidate_buckets.setdefault(item['_verify_cell'], []).append(item)
quota_winners = []
for cell in sorted(candidate_buckets):
    ranked = sorted(candidate_buckets[cell], key=lambda item: (-item['_verify_score'], item['source']))
    quota_winners.extend(ranked[:SANCTUARY_LIGHT_MAX_PER_CELL])
expected_light_sources = {item['source'] for item in sorted(
    quota_winners, key=lambda item: (-item['_verify_score'], item['source']))[:SANCTUARY_LIGHT_MAX_IMPORTED]}
for item in static_light_records:
    expected_selected = item.get('source') in expected_light_sources
    assert (item.get('status') == 'selected') == expected_selected, item.get('source')
    assert bool(item.get('selected')) == expected_selected, item.get('source')
for item in static_light_records:
    item.pop('_verify_score', None)
    item.pop('_verify_cell', None)
source_light_actors = [actor for actor in actors
                       if 'OpenWillow_SourceStaticLight' in
                       [str(tag) for tag in actor.get_editor_property('tags')]]
imported_light_sources = expected_light_sources if 'static_lights' in opt_ins else set()
if 'static_lights' not in opt_ins:
    # Off: no source point/spot light actor may exist; the import reports zero imported.
    assert not any(isinstance(actor, (unreal.PointLight, unreal.SpotLight)) for actor in actors)
reported_static_lights = import_report.get('lighting', {}).get('static_source_lights', {})
expected_orange_scaled = sum(
    1 for item in static_light_records if item.get('source') in imported_light_sources
    and item.get('kind') == 'Point'
    and sanctuary_light_calibration('Point', item['brightness'], item['light_color_rgb'])
    < sanctuary_light_calibration('Point', item['brightness']))
assert reported_static_lights.get('source_components') == len(static_light_records)
assert reported_static_lights.get('imported') == len(imported_light_sources)
assert reported_static_lights.get('skipped') == len(static_light_records) - len(imported_light_sources)
assert reported_static_lights.get('source_brightness_ceiling') == SANCTUARY_LIGHT_BRIGHTNESS_CEILING
assert reported_static_lights.get('orange_point_light_intensity_scale') == SANCTUARY_ORANGE_POINT_LIGHT_INTENSITY_SCALE
assert reported_static_lights.get('orange_point_lights_scaled') == expected_orange_scaled
assert len(source_light_actors) == len(imported_light_sources)
source_actor_by_source = {}
for actor in source_light_actors:
    tags = [str(tag) for tag in actor.get_editor_property('tags')]
    matching = [tag for tag in tags if tag in imported_light_sources]
    assert len(matching) == 1, (actor.get_actor_label(), matching)
    source_actor_by_source[matching[0]] = actor
assert set(source_actor_by_source) == imported_light_sources
for source in static_light_records:
    if source['source'] not in imported_light_sources:
        continue
    actor = source_actor_by_source[source['source']]
    close(xyz(actor.get_actor_location()), source['position'], .01)
    kind = source['kind']
    if kind == 'Point':
        assert isinstance(actor, unreal.PointLight)
        component = actor.get_component_by_class(unreal.PointLightComponent)
        scale = SANCTUARY_POINT_LIGHT_CANDELA_PER_SOURCE_BRIGHTNESS
    else:
        assert isinstance(actor, unreal.SpotLight)
        component = actor.get_component_by_class(unreal.SpotLightComponent)
        scale = SANCTUARY_SPOT_LIGHT_CANDELA_PER_SOURCE_BRIGHTNESS
        direction = source['forward']
        magnitude = math.sqrt(sum(float(value) ** 2 for value in direction))
        assert magnitude > 1e-5
        close(xyz(actor.get_actor_forward_vector()),
              [float(value) / magnitude for value in direction], .002)
        close([component.get_editor_property('inner_cone_angle'),
               component.get_editor_property('outer_cone_angle')],
              [source['inner_cone_angle'], source['outer_cone_angle']], .001)
    assert component is not None
    assert component.get_editor_property('mobility') == unreal.ComponentMobility.MOVABLE
    assert component.get_editor_property('intensity_units') == unreal.LightUnits.CANDELAS
    expected_intensity = sanctuary_light_calibration(kind, source['brightness'], source['light_color_rgb'])
    assert abs(float(component.get_editor_property('intensity')) - expected_intensity) < .01
    assert abs(float(component.get_editor_property('attenuation_radius')) - source['radius']) < .01
    assert not component.get_editor_property('cast_shadows')
    color = component.get_editor_property('light_color')
    expected_color = [linear_to_srgb_byte(value) for value in
                      ue3_color_linear(source['light_color_rgb'])]
    close([color.r, color.g, color.b], expected_color, 1.0)
static_mesh_candidates = decal_manifest.get('static_mesh_candidates', [])
verified_static_mesh_candidate_placements = 0
for candidate in static_mesh_candidates:
    instances = [item for item in scene['actors'] if item['mesh'] == candidate['mesh']]
    assert len(instances) == candidate['placements'], (candidate['source'], len(instances))
    for instance in instances:
        for section in scene['meshes'][instance['mesh']]['sections']:
            assert actor_label(instance['level'], instance['source'], section['slot']) in placed
        verified_static_mesh_candidate_placements += 1
assert verified_static_mesh_candidate_placements == decal_manifest.get('census', {}).get(
    'static_mesh_candidate_placements', 0)


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
    # Round-4 host sky calibration: literal 0.14 keeps this independent of the importer.
    close([opacity.get_editor_property('r')], [0.14 * sky['scalars']['cloud_cap_opacity']], 1e-6)


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


def verify_diffuse_tint(material, name, definition):
    """Require Base Color = Multiply(diffuse TextureSample, recorded p_diffmod rgb)."""
    root = mel.get_material_property_input_node(material, unreal.MaterialProperty.MP_BASE_COLOR)
    assert isinstance(root, unreal.MaterialExpressionMultiply), (name, 'diffuse tint is not a Multiply')
    pair = mel.get_inputs_for_material_expression(material, root)
    assert len(pair) == 2, (name, len(pair))
    sample, tint = pair
    assert isinstance(sample, unreal.MaterialExpressionTextureSample), (name, 'tint input A')
    assert sample.get_editor_property('texture').get_name() == Path(definition['channels']['diffuse']).stem, name
    assert isinstance(tint, unreal.MaterialExpressionConstant3Vector), (name, 'tint input B')
    value = tint.get_editor_property('constant')
    close([value.r, value.g, value.b], definition['diffuse_tint']['rgb'], 1e-6)
    return sample


verified_scene_recipe_layers = 0
verified_scene_recipe_materials = set()
verified_world_map_recipe = []
verified_emissive_multipliers = []
verified_unlit_constants = []
verified_diffuse_tints = []
for name, definition in scene['materials'].items():
    material = unreal.load_asset(base + '/Assets/M_' + name)
    assert material.get_editor_property('two_sided') == bool(definition.get('two_sided', False)), name
    recipe = effective_scene_recipe(definition, opt_ins)
    if (definition.get('scene_recipe', {}).get('method') == SANCTUARY_XMAS_BULB_METHOD
            and 'xmas_bulbs' not in opt_ins):
        bulb_name = definition['scene_recipe']['emissive_layers'][0]['texture'].split('.')[-1]
        _, emissive_nodes, _ = material_graph(material, unreal.MaterialProperty.MP_EMISSIVE_COLOR)
        assert not any(isinstance(node, unreal.MaterialExpressionTextureSample)
                       and node.get_editor_property('texture') is not None
                       and node.get_editor_property('texture').get_name() == bulb_name
                       for node in emissive_nodes.values()), (name, 'bulb emissive while opt-in is off')
    if recipe.get('method') == 'world_map_mask_colorization_v2':
        verified_world_map_recipe.append(verify_world_map_recipe(material, name, recipe))
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
        elif recipe.get('unlit_color_layers') or recipe.get('emissive_layers'):
            # The recipe layers (not the plain diffuse) are the visible color;
            # verify_texture_layers checked them above.
            pass
        elif recipe.get('method') == 'world_map_mask_colorization_v2':
            # The custom R/G/B mask graph was independently checked above.
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
        elif (channel == 'diffuse' and 'diffuse_tint' in opt_ins and definition.get('diffuse_tint') is not None
              and host_shading_name != 'MLM_Unlit' and not definition.get('terrain_blend')):
            node = verify_diffuse_tint(material, name, definition)
            verified_diffuse_tints.append(name)
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
report = {'sanctuary_host': {'profile': sanctuary_profile(scene), 'opt_ins': sorted(opt_ins)},
          'verified_section_actors': len(placed), 'verified_channels': sorted(verified_channels),
          'verified_unlit_materials': verified_unlit_materials,
          'verified_native_skybox_placements': verified_native_skybox,
          'verified_sky_approximation_materials': verified_sky_approximations,
          'verified_terrain_blend_materials': verified_terrain_blends,
          'verified_unlit_multiplier_materials': verified_unlit_multipliers,
          'verified_emissive_multiplier_materials': verified_emissive_multipliers,
          'verified_unlit_constant_materials': verified_unlit_constants,
          'verified_diffuse_tint_materials': len(verified_diffuse_tints),
          'expected_diffuse_tint_materials': sum(
              1 for d in scene['materials'].values() if 'diffuse_tint' in opt_ins and d.get('diffuse_tint') is not None
              and d['channels'].get('diffuse') and not d.get('terrain_blend')
              and d.get('scene_recipe', {}).get('host_lighting_model',
                                                'MLM_Unlit' if d.get('unlit_constant_color') else
                                                d.get('lighting_model', 'MLM_DefaultLit')) != 'MLM_Unlit'),
          'verified_scene_recipe_material_count': len(verified_scene_recipe_materials),
          'verified_scene_recipe_layer_count': verified_scene_recipe_layers,
          'verified_world_map_recipes': verified_world_map_recipe,
          'verified_outer_shell_placements': verified_outer_shell,
          'verified_outer_shell_replacements': verified_outer_shell_replacements,
          'expected_outer_shell_replacements': sum(
              len(a.get('outer_shell_replaced', [])) for a in scene['actors']),
          'verified_hidden_visual_placements': verified_hidden_visual,
          'verified_neutral_fallback_sections': verified_neutral_fallback,
          'geometry_bounds': 'matches source OBJ', 'lighting_actors': sorted(lighting),
          'verified_decals': len(supported_decals),
          'excluded_default_deny_decals': len(excluded_decals),
          'unsupported_decals': len(source_decals) - len(supported_decals) - len(excluded_decals),
          'verified_source_lights': len(expected_light_sources),
          'skipped_source_lights': len(static_light_records) - len(expected_light_sources),
          'source_decal_components': decal_manifest.get('census', {}).get('components', 0),
          'source_decal_materials': decal_manifest.get('census', {}).get('unique_materials', 0),
          'verified_decal_like_mesh_placements': verified_static_mesh_candidate_placements,
          'source_sky_light_brightness_status': (source_sky or {}).get('light_brightness_status'),
          'verified_world_info_post_process': source_post_process is not None,
          'verified_source_post_process_volume_records': len(source_volume_records),
          'host_post_process_volumes': len(host_post_process_volumes),
          'temporary_sky_fallback': ('none_native_dome_approximation' if native_sky_approximated
                                     else 'UE5_SkyAtmosphere+OpenWillow_SkyFallback'),
          'visual_validation': 'pending'}
assert report['verified_outer_shell_replacements'] == report['expected_outer_shell_replacements']
assert report['verified_diffuse_tint_materials'] == report['expected_diffuse_tint_materials']
(root / 'ue-verify.json').write_text(json.dumps(report, indent=2))
unreal.log('OpenWillow saved-scene verification: ' + json.dumps(report))
