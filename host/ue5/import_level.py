"""Import a prepared Material v1 scene into the UE5 host (editor Python)."""
import json
import os
import sys
from pathlib import Path
import unreal
sys.path.insert(0, str(Path(__file__).resolve().parent))
from scene_geometry import actor_label, host_obj

root = Path(os.environ['OPENWILLOW_SCENE']).resolve()
if not (Path(os.environ['OPENWILLOW_BL2']) / 'Binaries/Win32/Borderlands2.exe').is_file():
    raise RuntimeError('An installed Borderlands 2 is required')
scene = json.loads((root / 'scene.json').read_text(encoding='utf-8'))
(root / 'ue-import.json').unlink(missing_ok=True)
if scene['schema'] != 1 or scene['dynamic_policy'] != 'frozen':
    raise RuntimeError('Unsupported scene schema/policy')
destination = '/Game/OpenWillow/' + scene['map']
tools = unreal.AssetToolsHelpers.get_asset_tools()
mel = unreal.MaterialEditingLibrary


def imported(filename, expected):
    task = unreal.AssetImportTask()
    task.filename = str(root / filename)
    task.destination_path = destination + '/Assets'
    task.automated = True
    task.replace_existing = True
    task.save = False
    if filename.endswith('.obj'):
        converted = root / 'ue-obj' / Path(filename).name
        converted.parent.mkdir(exist_ok=True)
        converted.write_text(host_obj((root / filename).read_text()))
        task.filename = str(converted)
        options = unreal.FbxImportUI()
        options.import_mesh = True
        options.import_materials = False
        options.import_textures = False
        options.import_as_skeletal = False
        options.mesh_type_to_import = unreal.FBXImportType.FBXIT_STATIC_MESH
        data = options.static_mesh_import_data
        data.set_editor_property('combine_meshes', True)
        # Units stay centimeters. OBJ handedness is adapted above because
        # the Interchange OBJ importer reflects Y independently of this flag.
        data.set_editor_property('convert_scene', False)
        data.set_editor_property('convert_scene_unit', False)
        options.set_editor_property('static_mesh_import_data', data)
        task.options = options
    tools.import_asset_tasks([task])
    objects = [o for o in task.get_objects() if isinstance(o, expected)]
    if len(objects) != 1:
        raise RuntimeError(f'Expected one {expected} from {filename}: {objects}')
    return objects[0]


materials = {}
textures = {}


def sky_texture(item, srgb):
    """Import one recorded sky input texture once, as color or linear data."""
    texture = textures.get(item['file'])
    if texture is None:
        texture = imported(item['file'], unreal.Texture2D)
        textures[item['file']] = texture
    texture.set_editor_property('srgb', srgb)
    texture.set_editor_property('compression_settings', unreal.TextureCompressionSettings.TC_DEFAULT)
    unreal.EditorAssetLibrary.save_loaded_asset(texture)
    return texture


def connect(material, source, source_pin, target, target_pin):
    if not mel.connect_material_expressions(source, source_pin, target, target_pin):
        raise RuntimeError(f'Cannot connect sky graph {source_pin!r} -> {target_pin!r}: ' + material.get_name())


def build_sky_approximation(material, name, sky):
    """Host graph for `sky_time_of_day_strip_v1`; see NATIVE_SKY_APPROXIMATION.md.

    The preparer recorded the instance's named inputs; the master graph that
    combined them is stripped. This graph is the recorded approximation:
    visible = lerp(strip(column_u, dome V) * sky_brightness,
                   strip(column_u, horizon_row) * sky_brightness * cloud_brightness,
                   saturate(clouds.R * cloud_cap_opacity)).
    Every relationship here is UNVERIFIED against the original shader.
    """
    if sky.get('method') != 'sky_time_of_day_strip_v1':
        raise RuntimeError('Unsupported sky approximation method: ' + str(sky.get('method')))
    # The dome sits 20,000 km from the camera; without this flag the host
    # atmosphere's aerial perspective fogs it to a brown field. UE's sky flag
    # exempts Unlit opaque materials from fog and aerial perspective.
    try:
        material.set_editor_property('is_sky', True)
    except Exception as error:
        raise RuntimeError('Sky approximation requires the material Is Sky flag') from error
    scalars = sky['scalars']
    strip = sky_texture(sky['textures']['transition_track'], True)
    clouds = sky_texture(sky['textures']['clouds'], False)
    new = lambda cls: mel.create_material_expression(material, cls)
    dome_uv = new(unreal.MaterialExpressionTextureCoordinate)
    dome_uv.set_editor_property('coordinate_index', 0)
    dome_v = new(unreal.MaterialExpressionComponentMask)
    dome_v.set_editor_property('r', False)
    dome_v.set_editor_property('g', True)
    connect(material, dome_uv, '', dome_v, '')
    column = new(unreal.MaterialExpressionConstant)
    column.set_editor_property('r', float(sky['time_axis']['column_u']))
    strip_uv = new(unreal.MaterialExpressionAppendVector)
    connect(material, column, '', strip_uv, 'A')
    connect(material, dome_v, '', strip_uv, 'B')
    gradient = new(unreal.MaterialExpressionTextureSample)
    gradient.set_editor_property('texture', strip)
    gradient.set_editor_property('sampler_type', unreal.MaterialSamplerType.SAMPLERTYPE_COLOR)
    connect(material, strip_uv, '', gradient, 'UVs')
    brightness = new(unreal.MaterialExpressionConstant)
    brightness.set_editor_property('r', float(scalars['sky_brightness']))
    sky_color = new(unreal.MaterialExpressionMultiply)
    connect(material, gradient, 'RGB', sky_color, 'A')
    connect(material, brightness, '', sky_color, 'B')
    horizon_uv = new(unreal.MaterialExpressionConstant2Vector)
    horizon_uv.set_editor_property('r', float(sky['time_axis']['column_u']))
    horizon_uv.set_editor_property('g', float(sky['horizon_row_v']))
    horizon = new(unreal.MaterialExpressionTextureSample)
    horizon.set_editor_property('texture', strip)
    horizon.set_editor_property('sampler_type', unreal.MaterialSamplerType.SAMPLERTYPE_COLOR)
    connect(material, horizon_uv, '', horizon, 'UVs')
    cloud_brightness = new(unreal.MaterialExpressionConstant)
    cloud_brightness.set_editor_property('r', float(scalars['sky_brightness'] * scalars['cloud_brightness']))
    cloud_color = new(unreal.MaterialExpressionMultiply)
    connect(material, horizon, 'RGB', cloud_color, 'A')
    connect(material, cloud_brightness, '', cloud_color, 'B')
    coverage_sample = new(unreal.MaterialExpressionTextureSample)
    coverage_sample.set_editor_property('texture', clouds)
    coverage_sample.set_editor_property('sampler_type', unreal.MaterialSamplerType.SAMPLERTYPE_LINEAR_COLOR)
    connect(material, dome_uv, '', coverage_sample, 'UVs')
    opacity = new(unreal.MaterialExpressionConstant)
    opacity.set_editor_property('r', float(scalars['cloud_cap_opacity']))
    coverage = new(unreal.MaterialExpressionMultiply)
    connect(material, coverage_sample, sky['cloud_channel'], coverage, 'A')
    connect(material, opacity, '', coverage, 'B')
    saturated = new(unreal.MaterialExpressionSaturate)
    connect(material, coverage, '', saturated, '')
    visible = new(unreal.MaterialExpressionLinearInterpolate)
    connect(material, sky_color, '', visible, 'A')
    connect(material, cloud_color, '', visible, 'B')
    connect(material, saturated, '', visible, 'Alpha')
    if not mel.connect_material_property(visible, '', unreal.MaterialProperty.MP_EMISSIVE_COLOR):
        raise RuntimeError('Cannot connect sky approximation visible color: ' + name)


def terrain_texture(filename, srgb, grayscale=False):
    """Import one terrain input and preserve the recorded color-space policy."""
    texture = textures.get(filename)
    if texture is None:
        texture = imported(filename, unreal.Texture2D)
        textures[filename] = texture
    texture.set_editor_property('srgb', srgb)
    if grayscale:
        compression = getattr(unreal.TextureCompressionSettings, 'TC_GRAYSCALE',
                              unreal.TextureCompressionSettings.TC_DEFAULT)
    else:
        compression = unreal.TextureCompressionSettings.TC_DEFAULT
    texture.set_editor_property('compression_settings', compression)
    unreal.EditorAssetLibrary.save_loaded_asset(texture)
    return texture


def terrain_uv(material, mapping):
    """Create the explicit patch-to-texture coordinate transform from evidence."""
    mapping = mapping or {}
    scale = mapping.get('scale', [1.0, 1.0])
    offset = mapping.get('offset', [0.0, 0.0])
    if (not isinstance(scale, list) or len(scale) != 2 or
            not isinstance(offset, list) or len(offset) != 2):
        raise RuntimeError('Terrain material has incomplete sampling coordinates')
    coordinates = mel.create_material_expression(material, unreal.MaterialExpressionTextureCoordinate)
    coordinates.set_editor_property('coordinate_index', int(mapping.get('coordinate_index', 0)))
    coordinates.set_editor_property('u_tiling', float(scale[0]))
    coordinates.set_editor_property('v_tiling', float(scale[1]))
    if offset != [0, 0] and offset != [0.0, 0.0]:
        translate = mel.create_material_expression(material, unreal.MaterialExpressionConstant2Vector)
        translate.set_editor_property('r', float(offset[0]))
        translate.set_editor_property('g', float(offset[1]))
        add = mel.create_material_expression(material, unreal.MaterialExpressionAdd)
        if not (mel.connect_material_expressions(coordinates, '', add, 'A') and
                mel.connect_material_expressions(translate, '', add, 'B')):
            raise RuntimeError('Cannot connect terrain sampling offset')
        return add
    return coordinates


def build_terrain_blend(material, name, definition):
    """Build the dedicated weighted terrain graph from validated manifest rows.

    The graph is deliberately a host weighted sum.  The manifest records the
    validated map/layer sampling evidence and keeps native normalization,
    slope/noise filters and lightmaps explicitly UNVERIFIED.
    """
    if definition.get('method') != 'terrain_weighted_sum_v2':
        raise RuntimeError('Unsupported terrain material method: ' + str(definition.get('method')))
    if not definition.get('layers'):
        raise RuntimeError('Terrain weighted material has no layers')
    accumulated = None
    for layer in definition['layers']:
        weight_file = layer.get('weightmap_file')
        if not weight_file:
            raise RuntimeError('Terrain layer has no imported weightmap: ' + str(layer.get('layer_index')))
        weight = terrain_texture(weight_file, False, grayscale=True)
        expected = layer.get('weightmap_dimensions')
        if expected:
            try:
                actual = [weight.get_size_x(), weight.get_size_y()]
                if actual != expected:
                    raise RuntimeError('Terrain weightmap dimensions changed during import: ' +
                                       f'{actual} != {expected}')
            except AttributeError:
                # Older UE Python bindings do not expose size accessors; the
                # PNG manifest still carries the source dimensions and hash.
                pass
        weight_sample = mel.create_material_expression(material, unreal.MaterialExpressionTextureSample)
        weight_sample.set_editor_property('texture', weight)
        # TC_GRAYSCALE textures must be sampled as Linear Grayscale or the material fails to compile.
        weight_sample.set_editor_property('sampler_type', unreal.MaterialSamplerType.SAMPLERTYPE_LINEAR_GRAYSCALE)
        weight_uv = terrain_uv(material, layer.get('mapping'))
        if not mel.connect_material_expressions(weight_uv, '', weight_sample, 'UVs'):
            raise RuntimeError('Cannot connect terrain weightmap UVs: ' + name)

        diffuse_file = layer.get('diffuse')
        if diffuse_file:
            diffuse = terrain_texture(diffuse_file, True)
            color = mel.create_material_expression(material, unreal.MaterialExpressionTextureSample)
            color.set_editor_property('texture', diffuse)
            color.set_editor_property('sampler_type', unreal.MaterialSamplerType.SAMPLERTYPE_COLOR)
            color_uv = terrain_uv(material, layer.get('diffuse_mapping') or layer.get('mapping'))
            if not mel.connect_material_expressions(color_uv, '', color, 'UVs'):
                raise RuntimeError('Cannot connect terrain diffuse UVs: ' + name)
            color_pin = 'RGB'
        else:
            fallback = layer.get('fallback_color')
            if not (isinstance(fallback, list) and len(fallback) == 3):
                raise RuntimeError('Terrain layer has no diffuse or explicit fallback color')
            color = mel.create_material_expression(material, unreal.MaterialExpressionConstant3Vector)
            color.set_editor_property('constant', unreal.LinearColor(*fallback, 1.0))
            color_pin = ''
        weighted = mel.create_material_expression(material, unreal.MaterialExpressionMultiply)
        if not (mel.connect_material_expressions(color, color_pin, weighted, 'A') and
                mel.connect_material_expressions(weight_sample, 'R', weighted, 'B')):
            raise RuntimeError('Cannot connect terrain layer weight: ' + name)
        if accumulated is None:
            accumulated = weighted
        else:
            added = mel.create_material_expression(material, unreal.MaterialExpressionAdd)
            if not (mel.connect_material_expressions(accumulated, '', added, 'A') and
                    mel.connect_material_expressions(weighted, '', added, 'B')):
                raise RuntimeError('Cannot connect terrain layer sum: ' + name)
            accumulated = added
    if not mel.connect_material_property(accumulated, '', unreal.MaterialProperty.MP_BASE_COLOR):
        raise RuntimeError('Cannot connect terrain weighted Base Color: ' + name)
    roughness = mel.create_material_expression(material, unreal.MaterialExpressionConstant)
    roughness.set_editor_property('r', 0.65)
    if not mel.connect_material_property(roughness, '', unreal.MaterialProperty.MP_ROUGHNESS):
        raise RuntimeError('Cannot connect terrain weighted roughness: ' + name)


for name, definition in scene['materials'].items():
    material_path = destination + '/Assets/M_' + name
    material = unreal.load_asset(material_path) if unreal.EditorAssetLibrary.does_asset_exist(material_path) else None
    if material is None:
        material = tools.create_asset('M_' + name, destination + '/Assets', unreal.Material, unreal.MaterialFactoryNew())
    mel.delete_all_material_expressions(material)
    blend_modes = {
        'BLEND_Opaque': unreal.BlendMode.BLEND_OPAQUE,
        'BLEND_Masked': unreal.BlendMode.BLEND_MASKED,
        'BLEND_Translucent': unreal.BlendMode.BLEND_TRANSLUCENT,
        'BLEND_Additive': unreal.BlendMode.BLEND_ADDITIVE,
        'BLEND_Modulate': unreal.BlendMode.BLEND_MODULATE,
        'BLEND_AlphaComposite': getattr(unreal.BlendMode, 'BLEND_ALPHA_COMPOSITE', unreal.BlendMode.BLEND_TRANSLUCENT),
        'BLEND_AlphaHoldout': getattr(unreal.BlendMode, 'BLEND_ALPHA_HOLDOUT', unreal.BlendMode.BLEND_TRANSLUCENT),
    }
    shading_models = {
        'MLM_Unlit': unreal.MaterialShadingModel.MSM_UNLIT,
        'MLM_DefaultLit': unreal.MaterialShadingModel.MSM_DEFAULT_LIT,
    }
    blend_name = definition.get('blend_mode', 'BLEND_Opaque')
    material.set_editor_property('blend_mode', blend_modes.get(blend_name, unreal.BlendMode.BLEND_OPAQUE))
    material.set_editor_property('shading_model', shading_models.get(
        definition.get('lighting_model', 'MLM_DefaultLit'), unreal.MaterialShadingModel.MSM_DEFAULT_LIT))
    try:
        material.set_editor_property('two_sided', bool(definition.get('two_sided', False)))
    except Exception:
        pass
    terrain_blend = definition.get('terrain_blend')
    if terrain_blend is not None:
        build_terrain_blend(material, name, terrain_blend)
        mel.recompile_material(material)
        unreal.EditorAssetLibrary.save_loaded_asset(material)
        materials[name] = material
        continue
    fallback = None
    if not definition['channels'].get('diffuse'):
        # A recorded constant comes from an unconnected UE3 DiffuseColor input
        # (see prepare_level.unconnected_diffuse_constant); otherwise neutral.
        color = definition.get('constant_diffuse') or [0.5, 0.5, 0.5]
        fallback = mel.create_material_expression(material, unreal.MaterialExpressionConstant3Vector)
        fallback.set_editor_property('constant', unreal.LinearColor(color[0], color[1], color[2], 1))
        mel.connect_material_property(fallback, '', unreal.MaterialProperty.MP_BASE_COLOR)
    outputs = {'diffuse': unreal.MaterialProperty.MP_BASE_COLOR,
               'normal': unreal.MaterialProperty.MP_NORMAL,
               'specular': unreal.MaterialProperty.MP_SPECULAR,
               'emissive': unreal.MaterialProperty.MP_EMISSIVE_COLOR}
    diffuse_sample = None
    emissive_sample = None
    for channel, filename in definition['channels'].items():
        if filename is None:
            continue
        texture = textures.get(filename)
        if texture is None:
            texture = imported(filename, unreal.Texture2D)
            textures[filename] = texture
        texture.set_editor_property('srgb', channel in ('diffuse', 'emissive'))
        texture.set_editor_property('compression_settings', unreal.TextureCompressionSettings.TC_NORMALMAP
                                    if channel == 'normal' else unreal.TextureCompressionSettings.TC_DEFAULT)
        sample = mel.create_material_expression(material, unreal.MaterialExpressionTextureSample)
        sample.set_editor_property('texture', texture)
        sample.set_editor_property('sampler_type', unreal.MaterialSamplerType.SAMPLERTYPE_NORMAL if channel == 'normal'
                                   else unreal.MaterialSamplerType.SAMPLERTYPE_COLOR if channel in ('diffuse', 'emissive')
                                   else unreal.MaterialSamplerType.SAMPLERTYPE_LINEAR_COLOR)
        uv = definition.get('channel_uv', {}).get(channel)
        if uv is not None:
            coordinates = mel.create_material_expression(material, unreal.MaterialExpressionTextureCoordinate)
            coordinates.set_editor_property('coordinate_index', uv['index'])
            coordinates.set_editor_property('u_tiling', uv['scale'][0])
            coordinates.set_editor_property('v_tiling', uv['scale'][1])
            if not mel.connect_material_expressions(coordinates, '', sample, 'UVs'):
                raise RuntimeError('Cannot connect material UV coordinates: ' +
                                   str(mel.get_material_expression_input_names(sample)))
        if channel == 'diffuse':
            diffuse_sample = sample
        if channel == 'emissive':
            emissive_sample = sample
        output_node, output_pin = sample, 'R' if channel == 'specular' else 'RGB'
        if channel == 'emissive':
            # V1 uses alpha as an emissive mask. In particular the observed
            # white/zero-alpha default must not make ordinary surfaces glow.
            output_node = mel.create_material_expression(material, unreal.MaterialExpressionMultiply)
            mel.connect_material_expressions(sample, 'RGB', output_node, 'A')
            mel.connect_material_expressions(sample, 'A', output_node, 'B')
            output_pin = ''
        if not mel.connect_material_property(output_node, output_pin, outputs[channel]):
            raise RuntimeError(f'Cannot connect {channel}')
        unreal.EditorAssetLibrary.save_loaded_asset(texture)
    # Unlit uses Emissive Color for visible color. Retain the recovered diffuse
    # (or constant fallback) there when no explicit emissive channel exists.
    # This is Material v1 host policy, not reconstruction of the UE3 sky graph.
    sky = definition.get('sky_approximation')
    if definition.get('lighting_model') == 'MLM_Unlit' and sky is not None:
        # The preparer's diffuse inference stays on Base Color as the
        # recorded fallback; the visible Unlit color is the sky approximation.
        build_sky_approximation(material, name, sky)
    elif definition.get('lighting_model') == 'MLM_Unlit' and emissive_sample is None:
        color_node = diffuse_sample if diffuse_sample is not None else fallback
        color_pin = 'RGB' if diffuse_sample is not None else ''
        multiplier = definition.get('unlit_color_multiplier')
        if color_node is not None and multiplier is not None:
            # An inspected named constant scales the visible color (see
            # prepare_level.INSPECTED_UNLIT_COLOR_MULTIPLIERS); recorded, not decoded.
            scale = mel.create_material_expression(material, unreal.MaterialExpressionConstant3Vector)
            rgb = multiplier['rgb']
            scale.set_editor_property('constant', unreal.LinearColor(rgb[0], rgb[1], rgb[2], 1))
            scaled = mel.create_material_expression(material, unreal.MaterialExpressionMultiply)
            if not (mel.connect_material_expressions(color_node, color_pin, scaled, 'A')
                    and mel.connect_material_expressions(scale, '', scaled, 'B')):
                raise RuntimeError('Cannot connect unlit color multiplier: ' + name)
            color_node, color_pin = scaled, ''
        if color_node is None or not mel.connect_material_property(
                color_node, color_pin, unreal.MaterialProperty.MP_EMISSIVE_COLOR):
            raise RuntimeError('Cannot connect unlit visible color: ' + name)
    if blend_name != 'BLEND_Opaque':
        opacity_node = diffuse_sample or emissive_sample
        opacity_property = (unreal.MaterialProperty.MP_OPACITY_MASK
                            if blend_name == 'BLEND_Masked'
                            else unreal.MaterialProperty.MP_OPACITY)
        if opacity_node is not None:
            mel.connect_material_property(opacity_node, 'A', opacity_property)
        else:
            # Unsupported translucent/masked graphs must not become opaque
            # blockers. Their full UE3 opacity graph is outside Material v1.
            opacity = mel.create_material_expression(material, unreal.MaterialExpressionConstant)
            opacity.set_editor_property('r', 0.0)
            mel.connect_material_property(opacity, '', opacity_property)
    # UE3 specular RGB is approximated by its red channel; roughness is a v1 constant.
    roughness = mel.create_material_expression(material, unreal.MaterialExpressionConstant)
    roughness.set_editor_property('r', 0.65)
    mel.connect_material_property(roughness, '', unreal.MaterialProperty.MP_ROUGHNESS)
    mel.recompile_material(material)
    unreal.EditorAssetLibrary.save_loaded_asset(material)
    materials[name] = material

# Sections whose preparer recorded no material (for example terrain whose
# alpha maps did not decode, labeled 'neutral_constant') must not fall through
# to UE's WorldGridMaterial checkerboard. Bind an explicit lit neutral gray so
# the inspection view stays readable and the gap stays labeled, not hidden.
neutral_fallback_path = destination + '/Assets/M_OpenWillowNeutralFallback'
neutral_fallback = (unreal.load_asset(neutral_fallback_path)
                    if unreal.EditorAssetLibrary.does_asset_exist(neutral_fallback_path)
                    else tools.create_asset('M_OpenWillowNeutralFallback', destination + '/Assets',
                                            unreal.Material, unreal.MaterialFactoryNew()))
mel.delete_all_material_expressions(neutral_fallback)
neutral_fallback.set_editor_property('blend_mode', unreal.BlendMode.BLEND_OPAQUE)
neutral_fallback.set_editor_property('shading_model', unreal.MaterialShadingModel.MSM_DEFAULT_LIT)
neutral_color = mel.create_material_expression(neutral_fallback, unreal.MaterialExpressionConstant3Vector)
neutral_color.set_editor_property('constant', unreal.LinearColor(0.5, 0.5, 0.5, 1.0))
neutral_roughness = mel.create_material_expression(neutral_fallback, unreal.MaterialExpressionConstant)
neutral_roughness.set_editor_property('r', 0.65)
if not (mel.connect_material_property(neutral_color, '', unreal.MaterialProperty.MP_BASE_COLOR)
        and mel.connect_material_property(neutral_roughness, '', unreal.MaterialProperty.MP_ROUGHNESS)):
    raise RuntimeError('Cannot connect neutral fallback material')
mel.recompile_material(neutral_fallback)
unreal.EditorAssetLibrary.save_loaded_asset(neutral_fallback)
neutral_fallback_sections = 0

meshes = {}
for name, definition in scene['meshes'].items():
    for section in definition['sections']:
        mesh = imported(section['file'], unreal.StaticMesh)
        if section['material']:
            mesh.set_material(0, materials[section['material']])
        else:
            mesh.set_material(0, neutral_fallback)
            neutral_fallback_sections += 1
        hulls = []
        collision = definition.get('collision', {})
        if section is definition['sections'][0]:
            for item in collision.get('hulls', []):
                hull = unreal.OpenWillowHull()
                hull.vertices = [unreal.Vector(*v) for v in item['vertices']]
                hulls.append(hull)
        if collision.get('status') == 'triangle_mesh':
            # Each terrain/BSP section carries its own collision triangles.
            if hulls or not unreal.OpenWillowCollision.set_triangle_collision(mesh):
                raise RuntimeError('Invalid triangle collision for ' + name)
        elif not unreal.OpenWillowCollision.set_hulls(mesh, hulls):
            raise RuntimeError('Invalid collision hulls for ' + name)
        unreal.EditorAssetLibrary.save_loaded_asset(mesh)
        meshes[name, section['slot']] = mesh

level = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem)
actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
map_path = destination + '/' + scene['map']
if unreal.EditorAssetLibrary.does_asset_exist(map_path):
    if not level.load_level(map_path):
        raise RuntimeError('Cannot load generated map')
    for actor in actors.get_all_level_actors():
        if actor.get_actor_label().startswith('OpenWillow_'):
            actors.destroy_actor(actor)
else:
    if not level.new_level(map_path):
        raise RuntimeError('Cannot create generated map')


def pose(value):
    pitch, yaw, roll = value['rotation']
    return unreal.Transform(location=unreal.Vector(*value['location']),
                            rotation=unreal.Rotator(pitch=pitch, yaw=yaw, roll=roll), scale=unreal.Vector(*value['scale']))


def collection_rotation(value):
    m = value['matrix']
    return unreal.MathLibrary.make_rotation_from_axes(unreal.Vector(*m[0:3]),
               unreal.Vector(*m[4:7]), unreal.Vector(*m[8:11]))


def placement(value):
    if 'matrix' in value:
        m = value['matrix']
        rotation = collection_rotation(value)
        return unreal.Transform(location=unreal.Vector(*m[12:15]), rotation=rotation,
                                scale=unreal.Vector(*value['scale']))
    return unreal.MathLibrary.compose_transforms(pose(value['component']), pose(value['actor']))


count = 0
scene_bounds_min = unreal.Vector(float('inf'), float('inf'), float('inf'))
scene_bounds_max = unreal.Vector(float('-inf'), float('-inf'), float('-inf'))
for instance in scene['actors']:
    transform = placement(instance['transform'])
    is_native_skybox = bool(instance.get('native_skybox'))
    is_outer_shell = bool(instance.get('outer_shell'))
    hidden_visual = bool(instance.get('hidden_visual'))
    for section in scene['meshes'][instance['mesh']]['sections']:
        actor = actors.spawn_actor_from_class(unreal.StaticMeshActor, unreal.Vector())
        actor.set_actor_label(actor_label(instance['level'], instance['source'], section['slot']))
        actor.set_editor_property('tags', [unreal.Name(instance['source'])])
        actor.set_folder_path(('NativeSkybox/' if is_native_skybox else 'OuterShell/' if is_outer_shell else '')
                              + instance['level'])
        component = actor.static_mesh_component
        component.set_mobility(unreal.ComponentMobility.MOVABLE)
        component.set_static_mesh(meshes[instance['mesh'], section['slot']])
        actor.set_actor_transform(transform, False, False)
        if 'matrix' in instance['transform']:
            # SetActorTransform round-trips through a quaternion and snaps very
            # near-vertical pitch to 90 degrees. Preserve the matrix-derived
            # Euler rotation on the unattached root for saving and reopening.
            component.set_editor_property('relative_rotation', collection_rotation(instance['transform']))
        overrides = instance['materials']
        if section['slot'] < len(overrides) and overrides[section['slot']]:
            component.set_material(0, materials[overrides[section['slot']]])
        component.set_simulate_physics(False)
        component.set_collision_profile_name('BlockAll')
        component.set_collision_enabled(unreal.CollisionEnabled.NO_COLLISION
            if is_native_skybox else
            unreal.CollisionEnabled.QUERY_AND_PHYSICS
            if instance.get('collision_enabled', False)
            and (scene['meshes'][instance['mesh']].get('collision', {}).get('status') == 'triangle_mesh'
                 or (section is scene['meshes'][instance['mesh']]['sections'][0]
                     and bool(scene['meshes'][instance['mesh']].get('collision', {}).get('hulls', []))))
            else unreal.CollisionEnabled.NO_COLLISION)
        if is_native_skybox or is_outer_shell:
            # The dome is a visual shell. It must not block the player or cast
            # a giant shadow over Sanctuary; its source collision is absent.
            # The outer hull's source component records CastShadow=False.
            try:
                component.set_editor_property('cast_shadow', False)
            except Exception:
                pass
        if hidden_visual:
            # These observed helper assets have no recoverable host-side
            # visual. Keep source collision state, but do not draw the helper.
            component.set_visibility(False)
            try:
                actor.set_actor_hidden_in_game(True)
            except Exception:
                pass
        component.set_mobility(unreal.ComponentMobility.STATIC)
        origin, extent = actor.get_actor_bounds(False)
        scene_bounds_min = unreal.Vector(min(scene_bounds_min.x, origin.x - extent.x),
                                         min(scene_bounds_min.y, origin.y - extent.y),
                                         min(scene_bounds_min.z, origin.z - extent.z))
        scene_bounds_max = unreal.Vector(max(scene_bounds_max.x, origin.x + extent.x),
                                         max(scene_bounds_max.y, origin.y + extent.y),
                                         max(scene_bounds_max.z, origin.z + extent.z))
        count += 1

camera_pose = scene['camera'] or {'location': [0, 0, 1000], 'rotation': [0, 0, 0], 'scale': [1, 1, 1]}
position = unreal.Vector(*camera_pose['location'])
pitch, yaw, roll = camera_pose['rotation']
rotation = unreal.Rotator(pitch=pitch, yaw=yaw, roll=roll)
start = actors.spawn_actor_from_class(unreal.PlayerStart, position, rotation)
start.set_actor_label('OpenWillow_PlayerStart')
start.set_editor_property('tags', [unreal.Name('OpenWillow_PlayerStart')])

# Keep a deterministic starting pose in the map. Runtime game mode code copies
# this pose to the possessed spectator pawn after spawning it.
inspection_camera = actors.spawn_actor_from_class(unreal.CameraActor, position, rotation)
inspection_camera.set_actor_label('OpenWillow_InspectionCamera')
inspection_camera.set_editor_property('tags', [unreal.Name('OpenWillow_InspectionCamera')])
inspection_camera.set_folder_path('Inspection')
camera_component = inspection_camera.get_component_by_class(unreal.CameraComponent)
if camera_component:
    camera_component.set_editor_property('field_of_view', 75.0)
unreal.get_editor_subsystem(unreal.UnrealEditorSubsystem).set_level_viewport_camera_info(position, rotation)
world = unreal.get_editor_subsystem(unreal.UnrealEditorSubsystem).get_editor_world()
world.get_world_settings().set_editor_property('default_game_mode', unreal.load_class(None, '/Script/OpenWillow.OpenWillowGameMode'))
# Material v1 uses an explicit inspection rig until UE3 lightmaps are loaded:
# warm sun, cool ambient fill, one reflection capture and automatic exposure.
# Some UE3 sky/environment placements carry intentionally huge scales. Their
# bounds are useful for geometry validation but are not a useful light origin;
# keep the rig at the imported start camera so the local scene is illuminated.
scene_extent = (scene_bounds_max - scene_bounds_min) * 0.5
lighting_center = position
sun = actors.spawn_actor_from_class(unreal.DirectionalLight, lighting_center,
                                    unreal.Rotator(pitch=-45, yaw=-35, roll=0))
sun.set_actor_label('OpenWillow_Sun')
sun.set_folder_path('Lighting')
sun_component = sun.get_component_by_class(unreal.DirectionalLightComponent)
sun_component.set_editor_property('mobility', unreal.ComponentMobility.MOVABLE)
sun_component.set_editor_property('intensity', 1.0)
sun_component.set_light_color(unreal.LinearColor(1.0, 0.94, 0.82, 1.0))
sun_component.set_editor_property('light_source_angle', 0.5357)
sun_component.set_editor_property('dynamic_shadow_distance_movable_light',
                                   min(max(scene_extent.x, scene_extent.y, scene_extent.z) * 2.0, 50000.0))
sun_component.set_editor_property('dynamic_shadow_cascades', 4)
# The temporary UE5 atmosphere needs an explicit atmosphere light. This is a
# host-only fallback; it does not claim to be Sanctuary's native sky setup.
try:
    sun_component.set_atmosphere_sun_light(True)
except Exception:
    try:
        sun_component.set_editor_property('atmosphere_sun_light', True)
    except Exception:
        pass
try:
    sun_component.set_atmosphere_sun_light_index(0)
except Exception:
    try:
        sun_component.set_editor_property('atmosphere_sun_light_index', 0)
    except Exception:
        pass

sky = actors.spawn_actor_from_class(unreal.SkyLight, lighting_center)
sky.set_actor_label('OpenWillow_SkyFill')
sky.set_folder_path('Lighting')
sky_component = sky.get_component_by_class(unreal.SkyLightComponent)
sky_component.set_editor_property('mobility', unreal.ComponentMobility.MOVABLE)
# A captured-scene skylight is black in this generated map because there is no
# sky atmosphere or world background yet. Use UE's neutral gray light cube so
# Lit mode has ambient fill before native UE3 sky/light actors are translated.
sky_component.set_editor_property('source_type', unreal.SkyLightSourceType.SLS_SPECIFIED_CUBEMAP)
sky_component.set_editor_property('cubemap', unreal.load_asset('/Engine/EngineResources/GrayLightTextureCube'))
sky_component.set_real_time_capture(False)
sky_component.set_editor_property('lower_hemisphere_is_black', False)
sky_component.set_editor_property('lower_hemisphere_color', unreal.LinearColor(0.08, 0.10, 0.14, 1.0))
sky_component.set_intensity(0.5)
sky_component.set_light_color(unreal.LinearColor(0.72, 0.82, 1.0, 1.0))

# The UE3 sky graph is still unresolved and the temporary atmosphere can
# collapse to a brown/black field when its sun direction is outside the
# recovered setup. Keep that atmosphere for ambient lighting, but add a
# deterministic visual shell so unfilled parts of the inspection view remain
# a cool blue. This is a host fallback, not a native Sanctuary sky claim.
# The shell sits inside the native dome and would hide it, so it is only
# spawned when no accepted dome placement carries a sky approximation.
native_sky_approximated = any(
    instance.get('native_skybox') and any(
        material and scene['materials'].get(material, {}).get('sky_approximation')
        for material in (instance['materials'] or [
            section['material'] for section in scene['meshes'][instance['mesh']]['sections']]))
    for instance in scene['actors'])
sky_fallback_path = destination + '/Assets/M_OpenWillowSkyFallback'
sky_fallback_material = (unreal.load_asset(sky_fallback_path)
                          if unreal.EditorAssetLibrary.does_asset_exist(sky_fallback_path)
                          else tools.create_asset('M_OpenWillowSkyFallback', destination + '/Assets',
                                                  unreal.Material, unreal.MaterialFactoryNew()))
mel.delete_all_material_expressions(sky_fallback_material)
sky_fallback_material.set_editor_property('blend_mode', unreal.BlendMode.BLEND_OPAQUE)
sky_fallback_material.set_editor_property('shading_model', unreal.MaterialShadingModel.MSM_UNLIT)
sky_fallback_material.set_editor_property('two_sided', True)
sky_color = mel.create_material_expression(
    sky_fallback_material, unreal.MaterialExpressionConstant3Vector)
sky_color.set_editor_property('constant', unreal.LinearColor(0.018, 0.055, 0.20, 1.0))
if not mel.connect_material_property(sky_color, '', unreal.MaterialProperty.MP_EMISSIVE_COLOR):
    raise RuntimeError('Cannot connect sky fallback color')
mel.recompile_material(sky_fallback_material)
unreal.EditorAssetLibrary.save_loaded_asset(sky_fallback_material)
if not native_sky_approximated:
    sky_fallback = actors.spawn_actor_from_class(unreal.StaticMeshActor, lighting_center)
    sky_fallback.set_actor_label('OpenWillow_SkyFallback')
    sky_fallback.set_folder_path('Lighting')
    sky_fallback_component = sky_fallback.static_mesh_component
    sky_fallback_component.set_static_mesh(unreal.load_asset('/Engine/BasicShapes/Sphere.Sphere'))
    sky_fallback_component.set_material(0, sky_fallback_material)
    sky_fallback_component.set_collision_profile_name('NoCollision')
    sky_fallback_component.set_collision_enabled(unreal.CollisionEnabled.NO_COLLISION)
    try:
        # The camera is inside the source sphere; reverse culling keeps the
        # two-sided shell visible on UE5's saved static mesh component.
        sky_fallback_component.set_editor_property('reverse_culling', True)
    except Exception as error:
        raise RuntimeError('Sky fallback requires reverse culling support') from error
    try:
        sky_fallback_component.set_editor_property('cast_shadow', False)
    except Exception:
        pass
    sky_fallback.set_actor_scale3d(unreal.Vector(10000.0, 10000.0, 10000.0))
    sky_fallback_component.set_mobility(unreal.ComponentMobility.STATIC)

# The observed native dome is imported above with its Material v1 approximation.
# Keep the atmosphere as a temporary fill for maps or sky layers whose native
# graph/activation state remains unresolved.
sky_atmosphere = actors.spawn_actor_from_class(unreal.SkyAtmosphere, lighting_center)
sky_atmosphere.set_actor_label('OpenWillow_SkyAtmosphere')
sky_atmosphere.set_folder_path('Lighting')
sky_atmosphere_component = sky_atmosphere.get_component_by_class(unreal.SkyAtmosphereComponent)
if sky_atmosphere_component is None:
    raise RuntimeError('UE5 SkyAtmosphere actor has no SkyAtmosphereComponent')
for property_name, value in (
        ('rayleigh_scattering_scale', 0.55),
        ('mie_scattering_scale', 0.35),
        ('mie_absorption_scale', 0.08),
        ('mie_anisotropy', 0.78),
        ('multi_scattering_factor', 0.8),
        ('sky_luminance_factor', unreal.LinearColor(0.72, 0.80, 1.0, 1.0)),
        ('sky_and_aerial_perspective_luminance_factor', unreal.LinearColor(0.72, 0.80, 1.0, 1.0)),
        ('height_fog_contribution', 0.25)):
    try:
        sky_atmosphere_component.set_editor_property(property_name, value)
    except Exception:
        # Keep the fallback portable across UE5 minor versions where an
        # atmosphere tuning property may not be exposed to editor Python.
        pass

capture_radius = min(max(scene_extent.x, scene_extent.y, scene_extent.z) * 1.15, 16384.0)
capture = actors.spawn_actor_from_class(unreal.SphereReflectionCapture, lighting_center)
capture.set_actor_label('OpenWillow_ReflectionCapture')
capture.set_folder_path('Lighting')
capture_component = capture.get_component_by_class(unreal.SphereReflectionCaptureComponent)
capture_component.set_editor_property('influence_radius', max(capture_radius, 1000.0))
try:
    # Runtime capture avoids requiring a baked light build for this inspection
    # map. It captures once when the world first renders.
    capture_component.set_editor_property('runtime_capture', True)
except Exception:
    # Older UE5 minor versions keep this as a project-level setting.
    pass

post = actors.spawn_actor_from_class(unreal.PostProcessVolume, lighting_center)
post.set_actor_label('OpenWillow_Exposure')
post.set_folder_path('Lighting')
post.set_editor_property('unbound', True)
post.set_editor_property('priority', 100.0)
post.set_editor_property('blend_weight', 1.0)
post_settings = post.get_editor_property('settings')
for property_name, value in (
        # Leave exposure automatic: the imported map has large sparse areas
        # and a fixed manual override turns the Lit preview black at this
        # scale. AO remains a small, deterministic inspection aid.
        ('override_ambient_occlusion_intensity', True), ('ambient_occlusion_intensity', 0.35),
        ('override_ambient_occlusion_radius', True), ('ambient_occlusion_radius', 200.0)):
    try:
        post_settings.set_editor_property(property_name, value)
    except Exception:
        # Older UE5 minor releases exposed the override bit with a `b_`
        # prefix; retain that fallback while preferring the UE5.8 spelling.
        if property_name.startswith('override_'):
            try:
                post_settings.set_editor_property('b_' + property_name, value)
            except Exception:
                pass
post.set_editor_property('settings', post_settings)
try:
    sky_component.recapture_sky()
except Exception:
    # Null-RHI commandlets cannot render a capture. The editor will recapture
    # it when the map is opened.
    pass
terrain_placements = sum(1 for item in scene['actors'] if item.get('terrain'))
native_skybox_placements = sum(1 for item in scene['actors'] if item.get('native_skybox'))
outer_shell_placements = sum(1 for item in scene['actors'] if item.get('outer_shell'))
outer_shell_replaced = sum(len(item.get('outer_shell_replaced', [])) for item in scene['actors'])
sky_approximations = [m['source'] for m in scene['materials'].values() if m.get('sky_approximation')]
hidden_visual_placements = sum(1 for item in scene['actors'] if item.get('hidden_visual'))
level.save_current_level()
unreal.EditorAssetLibrary.save_directory(destination)
(root / 'ue-import.json').write_text(json.dumps({'imported': True, 'map': map_path, 'section_actors': count,
    'source_placements': len(scene['actors']), 'issues': len(scene['issues']),
    'neutral_fallback_sections': neutral_fallback_sections,
    'native_skybox': {'placements': native_skybox_placements,
                      'mesh': 'Prop_Skybox.Meshes.Sky_Dome',
                      'policy': ('sky_time_of_day_strip_v1' if native_sky_approximated
                                 else 'observed_sky_dome_material_v1'),
                      'sky_approximation_materials': sky_approximations,
                      'graph_status': 'partial_unverified',
                      'two_sided_interior_policy': True,
                      'host_sky_shell_spawned': not native_sky_approximated},
    'outer_shell': {'placements': outer_shell_placements,
                    'replaced_overrides': outer_shell_replaced,
                    'policy': scene.get('outer_shell_policy', 'placed_overrides')},
    'hidden_visual': {'placements': hidden_visual_placements,
                      'meshes': ['Common_Meshes.Blocking.Blocking_Cube',
                                 'Common_Meshes.CollisionCube',
                                 'Common_Meshes.Blocking.Blocking_Plane',
                                 'Prop_Garbage.Meshes.BoxLrg'],
                      'materials': ['Sanctuary_P:Env_Ice.Materials.Mat_CloudLayer_Light',
                                    'Sanctuary_Light:Env_Ice.Materials.Mat_CloudLayer_01'],
                      'sources': [
                          'TheWorld.PersistentLevel.InterpActor_26.StaticMeshComponent_20',
                          'TheWorld.PersistentLevel.InterpActor_33.StaticMeshComponent_20',
                          'TheWorld.PersistentLevel.InterpActor_34.StaticMeshComponent_20'],
                      'policy': 'hide_unrecovered_visual_preserve_source_collision'},
    'terrain': {'placements': terrain_placements,
                'policy': scene.get('terrain_policy'),
                'collision': 'triangle_mesh_complex_as_simple' if any(
                    m.get('collision', {}).get('status') == 'triangle_mesh' for m in scene['meshes'].values()) else 'none'},
    'lighting': {'sun_intensity': 1.0, 'sky_intensity': 0.5,
                 'reflection_capture_radius': max(capture_radius, 1000.0),
                 'exposure': 'auto', 'ambient_occlusion': 0.35,
                 'temporary_sky_fallback': ('UE5_SkyAtmosphere' if native_sky_approximated
                                            else 'UE5_SkyAtmosphere+OpenWillow_SkyFallback')},
    'visual_validation': 'pending'}, indent=2))
unreal.log(f'OpenWillow: imported {count} mesh sections, including {native_skybox_placements} native skybox placements and {hidden_visual_placements} hidden collision helpers, with lighting rig and temporary UE5 sky fallback. Play: WASD + mouse, E/Q vertical flight.')
