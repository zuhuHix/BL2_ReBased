"""Prepare Ash_P and its referenced sublevels for the UE5 editor host.

All game-derived output stays in local/. This is an offline scene loader v1,
not gameplay streaming or a general UE3 material-graph interpreter.
"""
import argparse
import hashlib
import json
import math
import re
from collections import Counter
from pathlib import Path
import struct
import subprocess
from collision_geometry import hulls as collision_hulls
from installed_content import PACKAGE_SUFFIXES, cache_directory, content_files


SCOOTER_FRONTAGE_TRIANGLE_SOURCES = frozenset({
    'Sanctuary_P:Env_Sanctuary.Meshes.SancScooterStairs',
    'Sanctuary_P:Prop_SanctuaryRoad.Mesh.SanctuarySidewalk_ParkingLot_Low',
})
ARCHIVES_FLOOR_TRIANGLE_SOURCES = frozenset({
    'Sanctuary_P:Env_Sanctuary.Meshes.RolandsArchivesFloors',
    'Sanctuary_P:Env_Sanctuary.Meshes.RolandsArchivesCrushRoom',
})


def scooter_frontage_collision(source, collision):
    """Use render triangles only for observed missing Scooter floor shapes."""
    if source in SCOOTER_FRONTAGE_TRIANGLE_SOURCES and collision['status'] == 'absent':
        return {'status': 'triangle_mesh', 'hulls': [],
                'method': 'scooter_frontage_render_triangles_v1',
                'source_collision': 'absent'}
    return collision


def archives_floor_collision(source, collision):
    """Use render triangles for the two observed Archives floor meshes."""
    if source in ARCHIVES_FLOOR_TRIANGLE_SOURCES and collision['status'] == 'absent':
        return {'status': 'triangle_mesh', 'hulls': [],
                'method': 'archives_floor_render_triangles_v1',
                'source_collision': 'absent'}
    return collision


def values(tags):
    return {p['name']: p['value'] for p in tags if p.get('status') == 'decoded'}


def props(record):
    if 'error' in record:
        raise ValueError(record['error'])
    return values(record.get('data', {}).get('properties', []))


def decoded_texture_properties(record):
    """Read decoded Texture2D UProperties from the package reader result.

    Scene records intentionally omit Texture2D properties. Offset 8 is the
    serialized UObject property start; texture bulk payload follows the
    property stream, so trailing bytes are expected and are not treated as an
    incomplete property decode.
    """
    if 'error' in record:
        raise ValueError(record['error'])
    if record.get('property_offset') != 8 or not isinstance(record.get('properties'), list):
        raise ValueError('Texture2D property decode did not use the expected property stream')
    return values(record['properties'])


def vector(value, default=0):
    return [value.get(k, default) for k in ('X', 'Y', 'Z')]


def byte_color_rgb(value):
    """Convert a decoded UE3 FColor to normalized RGB without guessing channels."""
    if not isinstance(value, dict) or not all(
            isinstance(value.get(c), int) and not isinstance(value.get(c), bool)
            and 0 <= value[c] <= 255 for c in 'RGB'):
        raise ValueError('Expected a decoded byte color')
    return [value[c] / 255.0 for c in 'RGB']


SANCTUARY_LIGHT_SELECTION = {
    'min_radius': 128.0,
    'min_brightness': 0.1,
    'cell_size': 1500.0,
    'max_per_cell': 2,
    'max_imported': 128,
    'score': 'source brightness multiplied by source radius',
    'status': 'UNVERIFIED host performance cap; shadows disabled on imported lights',
}


def sanctuary_lighting_enabled(map_name):
    """Only the Sanctuary persistent map gets the Sanctuary lighting policy."""
    return map_name == 'Sanctuary_P'


def select_sanctuary_light_components(components):
    """Apply deterministic thresholds, spatial quota and total light cap."""
    records = [dict(item) for item in components]
    candidates, counts = [], {'below_threshold': 0, 'cell_quota': 0,
                              'global_cap': 0, 'unsupported': 0}
    for item in records:
        if item.get('status') != 'candidate':
            item['selected'] = False
            item.setdefault('skip_reason', 'unsupported_source_data')
            counts['unsupported'] += 1
            continue
        radius, brightness = item.get('radius'), item.get('brightness')
        position, color = item.get('position'), item.get('light_color_rgb')
        if (not isinstance(radius, (int, float)) or isinstance(radius, bool)
                or not math.isfinite(radius) or not isinstance(brightness, (int, float))
                or isinstance(brightness, bool) or not math.isfinite(brightness)
                or not isinstance(position, list) or len(position) != 3
                or any(not isinstance(value, (int, float)) or isinstance(value, bool)
                       or not math.isfinite(value) for value in position)
                or not isinstance(color, list) or len(color) != 3
                or any(not isinstance(value, (int, float)) or isinstance(value, bool)
                       or not math.isfinite(value) or value < 0 or value > 1 for value in color)):
            item.update({'status': 'skipped', 'selected': False,
                         'skip_reason': 'invalid_source_light_values'})
            counts['unsupported'] += 1
            continue
        if radius < SANCTUARY_LIGHT_SELECTION['min_radius'] or brightness < SANCTUARY_LIGHT_SELECTION['min_brightness']:
            item.update({'status': 'skipped', 'selected': False,
                         'skip_reason': 'below_radius_or_brightness_threshold'})
            counts['below_threshold'] += 1
            continue
        item['selection_score'] = float(radius) * float(brightness)
        cell = tuple(math.floor(float(value) / SANCTUARY_LIGHT_SELECTION['cell_size']
                                ) for value in position)
        item['_selection_cell'] = cell
        candidates.append(item)

    buckets = {}
    for item in candidates:
        buckets.setdefault(item['_selection_cell'], []).append(item)
    cell_winners, cell_skipped = [], []
    for cell in sorted(buckets):
        ranked = sorted(buckets[cell], key=lambda item: (-item['selection_score'], item['source']))
        quota = SANCTUARY_LIGHT_SELECTION['max_per_cell']
        cell_winners.extend(ranked[:quota])
        cell_skipped.extend(ranked[quota:])
    for item in cell_skipped:
        item.update({'status': 'skipped', 'selected': False, 'skip_reason': 'spatial_cell_quota'})
        counts['cell_quota'] += 1

    winners = sorted(cell_winners, key=lambda item: (-item['selection_score'], item['source']))
    selected = winners[:SANCTUARY_LIGHT_SELECTION['max_imported']]
    global_skipped = winners[SANCTUARY_LIGHT_SELECTION['max_imported']:]
    for item in global_skipped:
        item.update({'status': 'skipped', 'selected': False, 'skip_reason': 'global_light_cap'})
        counts['global_cap'] += 1
    for item in selected:
        item.update({'status': 'selected', 'selected': True})
    for item in records:
        item.pop('_selection_cell', None)
    return records, {'source_components': len(records),
                     'eligible_before_spatial_quota': len(candidates),
                     'eligible_after_spatial_quota': len(cell_winners),
                     'imported': len(selected),
                     'skipped': len(records) - len(selected),
                     'skip_reasons': counts}


def decoded_settings(value):
    """Keep only decoded name/value fields from a serialized settings array."""
    if not isinstance(value, list):
        return {}
    return {item['name']: item['value'] for item in value
            if isinstance(item, dict) and item.get('status') == 'decoded'
            and isinstance(item.get('name'), str) and 'value' in item}


def property_name_offsets(payload, names, property_name):
    """Find package-name-table anchors in an export payload.

    Callers still validate the complete property stream with the package
    reader. This helper only returns byte positions carrying the exact FName
    requested by the caller; it does not infer arbitrary serialization sizes.
    """
    if not isinstance(payload, (bytes, bytearray, memoryview)) or not isinstance(names, list):
        raise ValueError('Property anchor requires payload bytes and a package name table')
    indices = [index for index, name in enumerate(names) if name == property_name]
    offsets = []
    for index in indices:
        marker = struct.pack('<ii', index, 0)
        offset = payload.find(marker)
        while offset >= 0:
            offsets.append(offset)
            offset = payload.find(marker, offset + 1)
    return sorted(set(offsets))


def transform(p, component=False):
    rot = p.get('Rotation', {})
    return {'location': vector(p.get('Translation' if component else 'Location', {})),
            'rotation': [rot.get(k, 0) * 360 / 65536 for k in ('Pitch', 'Yaw', 'Roll')],
            'scale': [v * p.get('Scale' if component else 'DrawScale', 1)
                      for v in vector(p.get('Scale3D' if component else 'DrawScale3D', {}), 1)]}


def collection_transforms(payload, data, count):
    """Observed Ash layout: rotation/translation matrix, Scale3D, scale, int.

    Require the exact tail size and affine finite matrices; no offset scanning.
    The final int is retained without assigning unverified semantics.
    """
    offset = data['property_offset'] + data['consumed_bytes']
    if len(payload) - offset != count * 84:
        raise ValueError('Unsupported collection transform layout')
    result = []
    for i in range(count):
        row = struct.unpack_from('<20fi', payload, offset + i * 84)
        if not all(math.isfinite(v) for v in row[:20]):
            raise ValueError('Nonfinite collection transform')
        if any(abs(row[k]) > 1e-5 for k in (3, 7, 11)) or abs(row[15] - 1) > 1e-5:
            raise ValueError('Non-affine collection matrix')
        result.append({'matrix': list(row[:16]), 'scale': [v * row[19] for v in row[16:19]],
                       'source_flags': row[20]})
    return result


def collection_scale(p, cooked):
    """A collection entry's scale, preferring the component's own Scale3D/Scale.

    The cooked per-entry tail carries a copy of the component's scale, and for
    2036 of the 2037 Sanctuary_P collection components that declare one the two
    agree exactly. The exception is StaticMeshActor_SMC_1802, whose component
    property says Scale3D=(0.97,1,1) while the tail says (1,1,1); the running
    game applies the 0.97 (its object dump reports a _LocalToWorld determinant
    of 0.97), so the property is treated as authoritative where it exists.
    """
    if 'Scale3D' not in p and 'Scale' not in p:
        return cooked
    return [v * p.get('Scale', 1) for v in vector(p.get('Scale3D', {}), 1)]


# UE3 material graphs use a small set of exact names for the ordinary
# channels, but environment masters also use stable aliases. Keep this
# allow-list narrow: treating every mask/noise/cubemap parameter as albedo
# makes sky and FX planes become opaque white geometry in the host.
CHANNELS = {'p_diffuse': 'diffuse', 'diffuse': 'diffuse',
            'p_normal': 'normal', 'normal': 'normal',
            'p_specular': 'specular', 'specular': 'specular',
            'p_emissive': 'emissive', 'emissive': 'emissive'}

# Native sky import is intentionally limited to the observed dome mesh. Other
# sky-named meshes retain ordinary Material v1 handling until their activation
# and material chains are understood.
NATIVE_SKYBOX_MESH = 'Prop_Skybox.Meshes.Sky_Dome'
# The dome's master graph is stripped from the cooked package. Only its named
# inputs survive; the host combines them under a recorded approximation
# (docs/verification/NATIVE_SKY_APPROXIMATION.md). Every mapping here is a
# guess labeled UNVERIFIED, not a decoded graph.
NATIVE_SKY_MASTER = 'Common_Materials.Sky.Mat_SkyTimeOfDay_Master'
SKY_SAMPLERS = {'transition_track': 'transition_track', 'clouds': 'clouds', 'masks': 'masks'}
SKY_SCALARS = {'time_of_day': 'time_of_day', 'sky_brightness': 'sky_brightness',
               'sun_spot_brightness': 'sun_spot_brightness',
               'cloud_cap_opacity': 'cloud_cap_opacity', 'p_couldbrightness': 'cloud_brightness'}
SKY_VECTORS = {'horizion_track_color_multiplier': 'horizon_track_color_multiplier'}
# Column lookup divisor for Time_of_Day on the 256-wide transition strip. The
# alternative reading (degrees, /360) lands in a sun column and produces dusk
# hues; the observed value 170 read as a pixel column gives the blue daytime
# gradient. Neither is decoded from the graph.
SKY_TIME_AXIS = 256.0
SKY_HORIZON_ROW = 0.95
# The `_Outer` city hull and its antennas. Their placed overrides are the
# masked `_Teleported` phase-in variants whose graphs Material v1 cannot
# recover, so they import invisible. Opt-in, the placement falls back to the
# mesh-default materials, which resolve a diffuse. Which of `_Land` and
# `_Outer` the running game shows is Kismet state and is not interpreted.
OUTER_SHELL_MESHES = {'Prop_Skybox.Meshes.SanctuarySky',
                      'FX_ENV_Sanctuary.Meshes.SanctuarySkybox_Antenna1',
                      'FX_ENV_Sanctuary.Meshes.SanctuarySkybox_Antenna2'}
OUTER_SHELL_OVERRIDE_SUFFIX = '_Teleported'
HIDDEN_VISUAL_MESH = 'Common_Meshes.Blocking.Blocking_Cube'
HIDDEN_COLLISION_MESH = 'Common_Meshes.CollisionCube'
HIDDEN_COLLISION_MATERIAL = 'Common_Meshes.Collision.Mat_Collision'
HIDDEN_CLOUD_MESH = 'Common_Meshes.Blocking.Blocking_Plane'

# Cheap, opt-in Matinee placement experiment for the Sanctuary hull. This is
# intentionally a source-path match, not a general SeqAct_Interp decoder.
MATINEE_FIRST_KEY_COMPONENT = (
    'TheWorld.PersistentLevel.InterpActor_29.StaticMeshComponent_393')
MATINEE_FIRST_KEY_TRANSLATION = [16551, -171794, -164]
MATINEE_FIRST_KEY_ROTATION = [0, 78.75, 0]


# Actor classes whose StaticMeshComponents are placed like a StaticMeshActor's:
# actor pose times component pose, with the same hidden-flag rules. Classes
# other than the two engine ones are not in the bulk scene records; their
# properties are read per owner (see Scene.owner_properties).
PLACED_OWNER_CLASSES = ('Engine.StaticMeshActor', 'Engine.InterpActor',
                        # One generated cable mesh in Sanctuary_P (a WiringMesh,
                        # read as a StaticMesh; see DECISIONS 2026-10-21).
                        'GearboxFramework.WiringActor',
                        # Interactive objects (vault symbols, piano, bounty board,
                        # customization machine...). In the running game at Plan B
                        # each of these components was attached and not hidden, with
                        # the package's mesh and relative pose. Their visibility is
                        # mission/challenge state that is not interpreted here.
                        'WillowGame.WillowInteractiveObject')
# Owner classes whose actors may serialize the same component twice. The five
# Sanctuary vault-symbol actors each carry two identical planes, and the running
# game attaches one; the second exact copy is listed, not placed. A copy only
# counts as a duplicate when it also matches on visibility and collision, so a
# hidden and a visible component are never merged (the owner's bHidden is
# shared by both and needs no entry here).
DEDUPLICATED_OWNER_CLASSES = ('WillowGame.WillowInteractiveObject',)
DUPLICATE_SIGNATURE_PROPERTIES = ('HiddenGame', 'CastShadow', 'CollideActors', 'BlockActors',
                                  'BlockRigidBody', 'BlockZeroExtent', 'BlockNonZeroExtent')
# Owners whose StaticMeshComponent is not what the game draws. They are listed
# in the manifest's excluded_components with the reason, not placed.
EXCLUDED_OWNER_CLASSES = {
    # Each spawner and failsafe serializes its component as `PreviewMesh` next
    # to a MissionItemDef: an editor preview of the mission item, which the
    # game spawns as a pickup only when the mission state asks for it.
    'WillowGame.WillowMissionPickupSpawner': 'mission_item_preview_mesh',
    'WillowGame.WillowMissionPickupFailsafe': 'mission_item_preview_mesh',
}


def owner_disposition(owner_class, owner_props, component_path):
    """Return 'placed', an exclusion reason, or None for an unsupported owner."""
    if owner_class in PLACED_OWNER_CLASSES:
        return 'placed'
    reason = EXCLUDED_OWNER_CLASSES.get(owner_class)
    if reason == 'mission_item_preview_mesh':
        preview = owner_props.get('PreviewMesh')
        if not (isinstance(preview, dict) and preview.get('path') == component_path):
            return None
    return reason


def placement_collision(level, source, component_props):
    """Return whether a placed component blocks the host walker."""
    if (level, source) in LANDED_INACTIVE_COLLISION:
        return False
    return bool(component_props.get('BlockActors', True) and component_props.get('CollideActors', True))


def apply_matinee_first_key_pose(source, pose):
    """Apply the observed first RelativeToInitial key to one hull pose.

    The serialized actor pose remains the source of the base transform. The
    translation and yaw are only an opt-in visual experiment; this function
    does not claim to decode SeqAct_Interp start-position or Kismet state.
    """
    if source != MATINEE_FIRST_KEY_COMPONENT:
        return pose, False
    if 'matrix' in pose or 'actor' not in pose:
        raise ValueError('Matinee first-key target is not an actor/component pose')
    actor = dict(pose['actor'])
    actor['location'] = [a + offset for a, offset in
                         zip(actor['location'], MATINEE_FIRST_KEY_TRANSLATION)]
    actor['rotation'] = list(MATINEE_FIRST_KEY_ROTATION)
    return dict(pose, actor=actor), True
# Both observed cloud-layer instances: `_Light` in Sanctuary_P (four planes)
# and `_01` in Sanctuary_Light (two planes on the horizon, whose sole cooked
# texture is a dust sprite that tiled as yellow/black stripes).
HIDDEN_CLOUD_MATERIALS = {'Sanctuary_P:Env_Ice.Materials.Mat_CloudLayer_Light',
                          'Sanctuary_Light:Env_Ice.Materials.Mat_CloudLayer_01'}
HIDDEN_TRANSITION_MESH = 'Common_Meshes.BasePlane_256x128'
HIDDEN_TRANSITION_MATERIAL = 'Common_Materials.Environment.WorldTransition'
# These three oversized BoxLrg placements are environmental blocking volumes,
# not authored props. Preserve their source collision while hiding only their
# visuals; InterpActor_26 and _33 were confirmed in the UE viewport.
HIDDEN_BOX_LRG_MESH = 'Prop_Garbage.Meshes.BoxLrg'
HIDDEN_BOX_LRG_SOURCES = {
    'TheWorld.PersistentLevel.InterpActor_26.StaticMeshComponent_20',
    'TheWorld.PersistentLevel.InterpActor_33.StaticMeshComponent_20',
    'TheWorld.PersistentLevel.InterpActor_34.StaticMeshComponent_20',
}
# Sanctuary_Outer InterpActor_34's box (z 2503-2963) encloses the game's own
# WillowCoopPlayerStart_0 (655.6, -6348.9, 2800), so it cannot be blocking in
# landed play; which of _Land/_Outer is active is Kismet state we do not run.
# Only this placement has that evidence; _26 and _33 keep their collision.
LANDED_INACTIVE_COLLISION = {
    ('Sanctuary_Outer', 'TheWorld.PersistentLevel.InterpActor_34.StaticMeshComponent_20'),
}
# These adjacent Sanctuary_P collision boxes were surfaced with the concrete
# tile override in the authored level. Their roughly 10 x 9 x 4 m bounds make
# them collision helpers, not rooftop render meshes; keep their source
# collision while hiding only these exact visual placements.
HIDDEN_COLLISION_PLACEMENTS = {
    ('Sanctuary_P:Common_Meshes.CollisionCube',
     'TheWorld.PersistentLevel.StaticMeshActor_372.StaticMeshComponent_2968'),
    ('Sanctuary_P:Common_Meshes.CollisionCube',
     'TheWorld.PersistentLevel.StaticMeshActor_690.StaticMeshComponent_2968'),
    ('Sanctuary_P:Common_Meshes.CollisionCube',
     'TheWorld.PersistentLevel.StaticMeshCollectionActor_27.StaticMeshActor_SMC_544'),
}
# This optimized Sanctuary material is an HLS master: its Color and Luminosity
# texture parameters are combined by a stripped static permutation resource.
# Feeding the luminosity atlas directly to Base Color produces the observed
# neon green/magenta walkways.  The ordinary same-package instance has a
# recoverable diffuse atlas with the compatible material family and is a
# bounded visual fallback until that HLS graph is decoded.
REGULAR_DIFFUSE_FALLBACKS = {
    'Sanctuary_P:Prop_SancBuildings.Optimization.Mati_SancBuild4a':
        'Sanctuary_P:Prop_SancBuildings.Material.Mati_SancBuild4a',
}
# Inspected opaque materials whose stripped graphs defeat the generic _Dif
# heuristic: their cooked lists carry several _Dif overlays, so the sole-_Dif
# rule either picks a blend layer or gives up. Each entry names the texture
# that carries the surface's own color, and a native normal (and emissive)
# only when one survives in the cooked list. An entry keyed by a placed
# instance must name the inspected 'base' it was read against; the instance
# is only accepted when it adds no texture parameter of its own, which the
# caller already guarantees. Membership never recovers the layered graph.
INSPECTED_COLOR_FALLBACKS = {
    'Sanctuary_Land:Prop_Glacier.Materials.Mat_FrozenLake': {
        'method': 'frozen_lake_color_fallback_v1', 'label': 'Frozen-lake', 'noun': 'ice',
        'color': 'Sanctuary_Land:Prop_Glacier.Textures.FrozenLake',
        'normal': 'Sanctuary_Land:Prop_Skybox.MoveMe.Ice_Nrm',
        'omitted': ['vertex-painted snow/noise blend', 'reflection',
                    'native normal graph', 'glow', 'UV modulation'],
        'issue': 'Approximation: inspected FrozenLake texture used as ice color; layered graph and UV modulation not reconstructed'},
    'Sanctuary_Land:Env_Sanctuary.Materials.Mat_IceRoadSanctuary': {
        'method': 'ice_road_color_fallback_v1', 'label': 'Ice-road', 'noun': 'road',
        'color': 'Sanctuary_Land:Env_Ice.Textures.BrokenRoad_Dif',
        'normal': None,
        'omitted': ['BrokenRoad_Alpha snow/rock layer blend', 'noise and splatter overlays',
                    'stripped p_Normal texture', 'UV modulation'],
        'issue': 'Approximation: inspected BrokenRoad_Dif texture used as road color; snow/rock layer blend and stripped normal not reconstructed'},
    # The Hyperion station hanging in Sanctuary's sky. The placed instance
    # overrides nothing; its base keeps only tint/fog/rim/emissive-multiplier
    # constants, and the cooked list is the hull's own _Dif/_Nrm/_Emis plus a
    # Tiling_SmokePanner2_Dif overlay, so the sole-_Dif rule gave up and the
    # station rendered as the neutral gray (read as black against the sky).
    'Sanctuary_Light:Prop_MoonBase.Materials.Mati_MoonBase_02a': {
        'method': 'moon_base_color_fallback_v1', 'label': 'Moon-base', 'noun': 'hull',
        'base': 'Sanctuary_Light:Prop_MoonBase.Materials.Mat_MoonBase_02a',
        'color': 'Sanctuary_Light:Prop_MoonBase.Textures.MoonBase02a_Dif',
        'normal': 'Sanctuary_Light:Prop_MoonBase.Textures.MoonBase02a_Nrm',
        'emissive': 'Sanctuary_Light:Prop_MoonBase.Textures.MoonBase02a_Emis',
        'omitted': ['MoonBase_Color tint', 'Emissive_Mult scalar', 'Fog/Fog_Intensity blend',
                    'RimLight_Color', 'Tiling_SmokePanner2_Dif overlay', 'UV modulation'],
        'issue': 'Approximation: inspected MoonBase02a _Dif/_Nrm/_Emis textures used as hull color, normal and emissive; tint, fog, rim light, emissive multiplier and smoke overlay not reconstructed'},
    # Sanctuary's snow terrains. Both cooked lists carry SnowDrift_01_Dif and
    # DirtySnow_Dif (PatchySnow adds GrasslandsRock_Dif and the
    # HighContrastPatches_Gray mask), so the sole-_Dif rule gave up and the
    # ground rendered as the neutral white. The inspected textures were read
    # side by side: SnowDrift is near-uniform clean snow, DirtySnow is snow
    # with dark dirt patches, the mask is a blob pattern. One color per
    # material is a stand-in; the masked layer blend is not reconstructed.
    'Sanctuary_P:Env_Ice.Materials.Mat_PatchySnow': {
        'method': 'patchy_snow_color_fallback_v1', 'label': 'Patchy-snow', 'noun': 'snow',
        'color': 'Sanctuary_P:Prop_Terrain.Textures.DirtySnow_Dif',
        'normal': 'Sanctuary_P:Prop_Rocks_02.Textures.WasteLandRock_Nrm',
        'omitted': ['HighContrastPatches_Gray masked blend of SnowDrift_01_Dif / DirtySnow_Dif / GrasslandsRock_Dif',
                    'UV modulation'],
        'issue': 'Approximation: inspected DirtySnow_Dif texture used as snow color; masked snow/dirt/rock blend not reconstructed'},
    'Sanctuary_Land:Prop_Skybox.MoveMe.Mat_SolidSnow': {
        'method': 'solid_snow_color_fallback_v1', 'label': 'Solid-snow', 'noun': 'snow',
        'color': 'Sanctuary_P:Prop_Aliens.Textures.SnowDrift_01_Dif',
        'normal': 'Sanctuary_P:Prop_Rocks_02.Textures.WasteLandRock_Nrm',
        'omitted': ['SnowDrift_01_Dif / DirtySnow_Dif blend', 'UV modulation'],
        'issue': 'Approximation: inspected SnowDrift_01_Dif texture used as snow color; snow blend not reconstructed'},
    # Remaining Sanctuary terrain layers, read the same way once the cooked
    # weight blend exposed them: sand with tyre tracks under a gray macro
    # variation mask; dark lichen ground under a pale green macro tint and a
    # rock overlay; dirty snow next to the clean drift.
    'Sanctuary_P:Prop_Terrain.Materials.Mat_InterludeSandTracks': {
        'method': 'sand_tracks_color_fallback_v1', 'label': 'Sand-tracks', 'noun': 'sand',
        'color': 'Sanctuary_P:Prop_Terrain.Textures.InterludeSandTracks_Dif',
        'normal': 'Sanctuary_P:Prop_Terrain.Textures.InterludeSandTracks_Nrm',
        'omitted': ['InterludeSandMacro_Dif macro variation', 'UV modulation'],
        'issue': 'Approximation: inspected InterludeSandTracks_Dif/_Nrm used as sand color and normal; macro variation not reconstructed'},
    'Sanctuary_P:Env_Ice.Materials.Mat_ColdGrass': {
        'method': 'cold_grass_color_fallback_v1', 'label': 'Cold-grass', 'noun': 'ground',
        'color': 'Sanctuary_P:Prop_Terrain.Textures.LichenTerrian_Dif',
        'normal': None,
        'omitted': ['FoliageGreensB macro tint', 'HighContrastPatches_Gray masked GrasslandsRock_Dif overlay', 'UV modulation'],
        'issue': 'Approximation: inspected LichenTerrian_Dif texture used as ground color; tint and masked rock overlay not reconstructed'},
    'Sanctuary_P:Env_Ice.Materials.Mat_DirtySnow': {
        'method': 'dirty_snow_color_fallback_v1', 'label': 'Dirty-snow', 'noun': 'snow',
        'color': 'Sanctuary_P:Prop_Terrain.Textures.DirtySnow_Dif',
        'normal': 'Sanctuary_P:Prop_Rocks_02.Textures.WasteLandRock_Nrm',
        'omitted': ['SnowDrift_01_Dif blend', 'UV modulation'],
        'issue': 'Approximation: inspected DirtySnow_Dif texture used as snow color; snow blend not reconstructed'},
    # Sanctuary_Land's remaining opaque gaps (2026-10-21), read the same way.
    # The icicles' Ice_Dif is not their color: it is the named
    # P_InternalGlowTexture (tinted by P_InternalGlowColor) and the cube is
    # P_SimpleReflect, so the one remaining unnamed _Dif, SnowDrift_01_Dif,
    # is used as the body.
    'Sanctuary_Land:Prop_Icicles.Materials.Mat_Icicles': {
        'method': 'icicle_color_fallback_v1', 'label': 'Icicle', 'noun': 'ice',
        'color': 'Sanctuary_P:Prop_Aliens.Textures.SnowDrift_01_Dif',
        'normal': 'Sanctuary_Land:Prop_Icicles.Textures.IcicleMultiple_Nrm',
        'omitted': ['P_InternalGlowTexture (Ice_Dif) tinted by P_InternalGlowColor',
                    'P_SimpleReflect cube reflection', 'UV modulation'],
        'issue': 'Approximation: unnamed SnowDrift_01_Dif used as icicle color; named internal glow (Ice_Dif) and reflection not reconstructed'},
    # Distant snow fields: the same DirtySnow_Dif reading as Mat_PatchySnow;
    # both carry C1/C2 tint vectors and a fresnel colour that are omitted.
    'Sanctuary_Land:Env_TundraExpress.Materials.Mat_PatchySnow_Skybox01': {
        'method': 'distant_patchy_snow_color_fallback_v1', 'label': 'Distant-patchy-snow', 'noun': 'snow',
        'color': 'Sanctuary_P:Prop_Terrain.Textures.DirtySnow_Dif',
        'normal': 'Sanctuary_P:Prop_Rocks_02.Textures.WasteLandRock_Nrm',
        'omitted': ['WaterHqNoise_Gray masked blend with SnowDrift_01_Dif / SlateGravel_Dif',
                    'C1/C2 tint vectors', 'P_FresnelColor', 'UV modulation'],
        'issue': 'Approximation: inspected DirtySnow_Dif texture used as distant snow color; masked snow/gravel blend and tints not reconstructed'},
    'Sanctuary_Land:Env_Ice.Materials.Mat_DirtySnowDistant': {
        'method': 'distant_dirty_snow_color_fallback_v1', 'label': 'Distant-dirty-snow', 'noun': 'snow',
        'color': 'Sanctuary_P:Prop_Terrain.Textures.DirtySnow_Dif',
        'normal': 'Sanctuary_P:Prop_Rocks_02.Textures.WasteLandRock_Nrm',
        'omitted': ['SnowDrift_01_Dif blend', 'C1/C2 tint vectors', 'P_FresnelColor', 'UV modulation'],
        'issue': 'Approximation: inspected DirtySnow_Dif texture used as distant snow color; snow blend and tints not reconstructed'},
    # Skybox mountains: no named samplers survive. SkyboxMountain_Dif is a
    # rock-and-snow photo; SkyboxMountainSnow_Dif is mostly white and reads
    # as the snow layer whose blend is not recovered.
    'Sanctuary_Land:Prop_Glacier.Materials.Mat_MountainDistant': {
        'method': 'distant_mountain_snow_color_fallback_v1', 'label': 'Distant-mountain', 'noun': 'mountain',
        'color': 'Sanctuary_Land:Prop_Skybox.Textures.SkyboxMountainSnow_Dif',
        'normal': None,
        'omitted': ['SkyboxMountain_Dif rock layer blend', 'UV modulation'],
        'issue': 'Approximation: inspected mostly snowy SkyboxMountainSnow_Dif texture used as mountain color; rock/snow layer blend not reconstructed'},
    # Roland's HQ chemical tank: the tank's own atlas next to a smoke overlay
    # _Dif; the cube is the named P_SimpleReflect.
    'Sanctuary_P:Prop_Tanks.Materials.Mat_ChemTank02': {
        'method': 'chem_tank_color_fallback_v1', 'label': 'Chem-tank', 'noun': 'tank',
        'color': 'Sanctuary_P:Prop_Tanks.Textures.ChemTank02_Dif',
        'normal': None,
        'omitted': ['ChemTank02_Comp packed mask', 'Transmission/Fresnel/Tube/Spec/EmissColorA/B vectors',
                    'P_SimpleReflect cube reflection', 'Smoke2_GP_Dif overlay'],
        'issue': 'Approximation: inspected ChemTank02_Dif texture used as tank color; packed mask, tints, reflection and smoke overlay not reconstructed'},
}

# Small material recipes for the five matched-viewpoint gaps. These are
# source-identity and cooked-resource scoped; each mask/color relationship is
# an explicit host approximation, not a recovered UE3 graph. Textures are
# decoded to the ignored local scene only.
INSPECTED_SCENE_MATERIAL_RECIPES = {
    'Sanctuary_P:Prop_CustomizationMachine.Materials.Master_CharacterCustomization': {
        'method': 'customization_banner_swirl_v1',
        'base': 'Sanctuary_P:Prop_CustomizationMachine.Materials.Master_CharacterCustomization',
        'diffuse': 'Sanctuary_P:Prop_CustomizationMachine.Textures.CM_MachineBanner',
        # UNVERIFIED host calibration against lane A's matched round-2 shots;
        # this is a brighter host display, not a decoded UE3 scalar.
        'emissive_calibration_scale': 3.0,
        'emissive_calibration_evidence': 'round2 matched camera 01: cyan display and holo are still subdued',
        'emissive_layers': [
            {'texture': 'Sanctuary_P:FX_Shared_Energy.Textures.EnergySwirls_Dif_Tex',
             'channel': 'RGB', 'color': [0.08, 0.72, 1.0], 'srgb': True}],
        'omitted': ['Aliencenterfade_Dif gradient overlay', 'native panel mask and UV animation'],
        'issue': 'Approximation: CM_MachineBanner used as the panel color and EnergySwirls_Dif_Tex as cyan emissive; graph and animated overlay not reconstructed'},
    'Sanctuary_P:Prop_Signs_02.Materials.Mat_SanctuaryMoxxis': {
        'method': 'moxxi_neon_masks_v1',
        'base': 'Sanctuary_P:Prop_Signs_02.Materials.Mat_SanctuaryMoxxis',
        # UNVERIFIED host calibration against lane A's matched round-2 shots;
        # the sign glow is too dim in the host and this scale is not decoded.
        'emissive_calibration_scale': 2.8,
        'emissive_calibration_evidence': 'round2 matched camera 05: Moxxi sign glow is dimmer than reference',
        'emissive_layers': [
            {'texture': 'Sanctuary_P:Prop_Signs_02.Textures.SanctuaryMoxxis_Mask',
             'channel': 'R', 'color': [2.2, 0.08, 0.14], 'srgb': False},
            {'texture': 'Sanctuary_P:Prop_Signs_02.Textures.SanctuaryMoxxis_Mask_02',
             'channel': 'R', 'color': [1.8, 0.10, 0.85], 'srgb': False},
            {'texture': 'Sanctuary_P:Prop_Signs_02.Textures.SanctuaryMoxxis_Mask_02',
             'channel': 'G', 'color': [1.8, 0.35, 0.10], 'srgb': False}],
        'omitted': ['native sign mask blend and palette', 'night-time light contribution'],
        'issue': 'Approximation: sparse R/R/G channels in the two inspected Moxxi mask atlases drive pink, violet and warm emissive layers; native palette and glow are not reconstructed'},
    'Sanctuary_P:Prop_Carnival.Materials.Mati_LightStrips_01': {
        'method': 'light_strip_component_emissive_v1',
        'base': 'Sanctuary_P:Prop_Carnival.Materials.Mat_LightStrips_01',
        # UNVERIFIED host calibration against matched round-2 cameras 05/07;
        # the bulb colors are channel masks, and this scale is not decoded.
        'emissive_calibration_scale': 4.0,
        'emissive_calibration_evidence': 'round2 matched cameras 05/07: string bulbs are not visible in the host',
        'emissive_layers': [
            {'texture': 'Sanctuary_P:Prop_Carnival.LightStrip_Comp',
             'channel': 'RGB', 'color': [2.0, 2.0, 2.0], 'srgb': True}],
        'omitted': ['native bulb palette and bloom'],
        'issue': 'Approximation: inspected colored LightStrip_Comp atlas drives emissive RGB; native bulb shader and bloom are not reconstructed'},
    # Christmas-light bulbs on WiringMesh_14 (cable section 0). The sole cooked
    # _Dif texture is the bulb atlas: its bright texels are the lit bulbs and
    # the rest is dark wire. The bulbs are emissive in the game, so the warm
    # tint below is applied to that atlas's own RGB, scaled like LightStrip.
    'Sanctuary_P:Prop_Wiretech.MAT_WireXMasWhite': {
        'method': 'xmas_bulb_emissive_v1',
        'base': 'Sanctuary_P:Prop_Wiretech.MAT_WireXMasWhite',
        # UNVERIFIED host calibration; no host capture here shows the bulbs lit.
        'emissive_calibration_scale': 4.0,
        'emissive_calibration_evidence': 'round2 matched cameras 05/07: string bulbs not visible in the host',
        'emissive_layers': [
            {'texture': 'Sanctuary_P:Prop_Wiretech.XmasLightBulbs_Dif',
             'channel': 'RGB', 'color': [1.0, 0.62, 0.28], 'srgb': True}],
        'omitted': ['bulb glow and bloom', 'native bulb palette'],
        'issue': 'UNVERIFIED approximation: XmasLightBulbs_Dif atlas drives a warm emissive on the bulbs; the diffuse stays the same atlas; glow and bloom not reconstructed'},
    'Sanctuary_P:Prop_Electronics.Materials.Mat_WorldMap': {
        'method': 'world_map_mask_colorization_v2',
        'base': 'Sanctuary_P:Prop_Electronics.Materials.Mat_WorldMap',
        # UNVERIFIED palette calibration against the matched camera 03/08 crops.
        # R land artwork, G grid and B markers were re-read from WorldMap_Comp;
        # zero-mask regions use a cyan backing so they do not render black.
        'host_lighting_model': 'MLM_Unlit',
        'mask_texture': 'Sanctuary_P:Prop_Electronics.Textures.WorldMap_Comp',
        'mask_channels': {'land': 'R', 'grid': 'G', 'markers': 'B'},
        'palette': {'background': [0.11, 0.38, 0.50],
                    'land': [0.30, 0.72, 0.82],
                    'grid': [0.004, 0.020, 0.028],
                    'markers': [0.006, 0.030, 0.045]},
        'omitted': ['RolandsControlRoom02_Cube reflection', 'native map palette and animated markers'],
        'issue': 'UNVERIFIED calibration: WorldMap_Comp R carries land/contour artwork, G carries the grid, and B carries markers; a pale-cyan backing fills zero-mask areas and the G/B masks select darker teal; native palette and animated markers remain unreconstructed'},
}

SANCTUARY_DECAL_MATERIAL_ALLOWLIST = {
    'Sanctuary_P:Prop_Decals.DMat_GrafittoBurrito',
}
DECAL_DEFAULT_DENY_REASON = (
    'Excluded by the round-3 default-deny rule: this single-texture material has no '
    'channel/blend recipe validated against the matched Sanctuary reference; round-2 '
    'imports produced rainbow noise or magenta/violet projected quads.')

# The cooked Engine package serializes Height, FieldOfView and FarPlane on
# Default__DecalComponent. The remaining fallback values below follow the
# public UE3 DecalComponent class reference; Width/Thickness/NearPlane are not
# serialized in this local class default object and remain UNVERIFIED here.
UE3_DECAL_COMPONENT_DEFAULTS_UNVERIFIED = {
    'Width': 200.0, 'Height': 200.0, 'Thickness': 10.0,
    'NearPlane': 0.0, 'FarPlane': 300.0, 'FieldOfView': 80.0,
}


def decal_opacity_source(blend_mode, texture_properties=None):
    """Choose a bounded opacity input for the supported single-texture decal path."""
    texture_properties = texture_properties if isinstance(texture_properties, dict) else {}
    if blend_mode == 'BLEND_Modulate' and texture_properties.get('CompressionNoAlpha') is not True:
        return 'inverse_rgb_luminance_v1'
    if blend_mode == 'BLEND_Masked':
        return 'Texture2D alpha channel'
    return None

DECAL_LIKE_STATIC_MESH_IDENTITIES = {
    'Startup:FX_Shared_Mesh.Meshes.Planes.DecalPlane_LiquidDev',
}

# Instance parameters are read from the cooked parent chain; only the exact
# material identities below may use them. Values are never copied into code.
INSPECTED_EMISSIVE_MULTIPLIERS = {
    'Sanctuary_P:Prop_CustomizationMachine.Materials.CM_Mat': {
        'base': 'WillowGame:Common_Materials.Environment.Master_World',
        'parameter': 'p_EmissiveScale',
        'method': 'customization_emissive_scale_v1',
        'omitted': ['emissive mask graph and animated panel overlays'],
    },
}

INSPECTED_UNLIT_CONSTANT_COLORS = {
    'Sanctuary_P:Common_Materials.Hyperion.Master_HyperionEye_INST': {
        'base': 'Sanctuary_P:Common_Materials.Hyperion.Master_HyperionEye',
        'parameter': 'p_EyeColorA',
        'method': 'hyperion_eye_unlit_color_v1',
        'omitted': ['other eye color vectors', 'masked eye graph and texture animation'],
    },
    'Sanctuary_P:Char_ClapTrap.Materials.Mati_ClapTrapLensTeal': {
        'base': 'Sanctuary_P:Common_Materials.Hyperion.Master_HyperionEye',
        'parameter': 'p_EyeColorA',
        'method': 'hyperion_eye_unlit_color_v1',
        'omitted': ['other eye color vectors', 'masked eye graph and texture animation'],
    },
}

# Inspected Unlit materials whose visible color is scaled by a named vector
# constant. The host multiplies the recovered Unlit color by the recorded
# value; everything else in the stripped graph stays omitted. Keyed by the
# placed material; the chain must end at the named base.
INSPECTED_UNLIT_COLOR_MULTIPLIERS = {
    # Elpis. Mat_Moon is additive Unlit with p_moonColor 4.02 gray; without
    # it the additive moon washes out against the dome. The H-shaped
    # MoonBase02_GRP mask, p_moonTimeBaseShadow, Moon_Comp relief and the
    # orange p_Basecolor2 are not placed by anything the package retains.
    'Sanctuary_Light:Prop_MoonBase.Materials.Mati_Moon': {
        'method': 'unlit_color_multiplier_v1',
        'base': 'Sanctuary_Light:Prop_MoonBase.Materials.Mat_Moon',
        'parameter': 'p_moonColor',
        'omitted': ['MoonBase02_GRP station shadow mask', 'p_moonTimeBaseShadow',
                    'Moon_Comp crater relief', 'p_Basecolor2 secondary color',
                    'p_DarkColor', 'Transition_Track time-of-day tint',
                    'p_moonTime/p_moonRotation UV motion']},
}

# Lit instances whose diffuse is multiplied by a named vector on a shared base
# master. Keyed by (base master, parameter); only instances whose identity
# starts with the scope are considered. Sanctuary buildings override p_diffmod
# per instance (plain grey, rust orange, green), and scene.json otherwise
# carries no tint. The host multiplies Base Color by the recorded rgb.
INSPECTED_DIFFUSE_TINTS = {
    ('WillowGame:Common_Materials.Environment.Master_World', 'p_diffmod'): {
        'method': 'diffuse_tint_v1',
        'scope': 'Sanctuary_P:Prop_SancBuildings.',
        'omitted': ['Master_World colour ramps, vertex paint and snow vectors',
                    'p_vertexnoisemod / p_highlightsintensity / p_shadowsintensity',
                    'p_speccolor_specpow specular and the glow channel'],
    },
}


def native_skybox_mesh(identity):
    """Return whether an object identity is the observed native sky dome."""
    return identity.rsplit(':', 1)[-1] == NATIVE_SKYBOX_MESH


def native_skybox_placement(identity, effective_materials, materials):
    """Accept only the dome with entirely Unlit effective materials."""
    return (native_skybox_mesh(identity) and bool(effective_materials)
            and all(materials.get(name, {}).get('lighting_model') == 'MLM_Unlit'
                    for name in effective_materials))


def outer_shell_mesh(identity):
    """Return whether an object identity is one of the observed outer hull meshes."""
    return identity.rsplit(':', 1)[-1] in OUTER_SHELL_MESHES


def outer_shell_overrides(overrides, sections, materials):
    """Drop only `_Teleported` overrides whose mesh default recovered a diffuse.

    Returns the adjusted override list and a record of every replacement.
    Any other override, and any slot whose default has no diffuse, is kept as
    placed so the policy cannot turn an unrelated material into hull plating.
    """
    kept, replaced = list(overrides), []
    defaults = {section['slot']: section['material'] for section in sections}
    for slot, name in enumerate(overrides):
        if not name:
            continue
        source = materials.get(name, {}).get('source', '')
        if not source.rsplit('.', 1)[-1].endswith(OUTER_SHELL_OVERRIDE_SUFFIX):
            continue
        default = defaults.get(slot)
        if not default or not materials.get(default, {}).get('channels', {}).get('diffuse'):
            continue
        kept[slot] = None
        replaced.append({'slot': slot, 'override': source,
                         'default': materials[default]['source']})
    return kept, replaced


def apply_outer_shell_policy(actor, mesh_identity, sections, materials, enabled):
    """Set an actor's outer-shell fields from its placed overrides.

    `outer_shell_source_materials` always keeps the placed overrides so the
    policy can be re-applied or withdrawn by a later refresh.
    """
    if not outer_shell_mesh(mesh_identity):
        return
    placed = actor.get('outer_shell_source_materials', actor['materials'])
    actor['outer_shell_source_materials'] = list(placed)
    actor['outer_shell'] = bool(enabled)
    if enabled:
        actor['materials'], actor['outer_shell_replaced'] = outer_shell_overrides(
            placed, sections, materials)
    else:
        actor['materials'] = list(placed)
        actor.pop('outer_shell_replaced', None)


def hidden_visual_mesh(identity, effective_materials=None, materials=None, source=None,
                       source_hidden=None):
    """Return whether an observed helper has no recoverable host-side visual."""
    mesh = identity.rsplit(':', 1)[-1]
    if mesh == HIDDEN_BOX_LRG_MESH and source in HIDDEN_BOX_LRG_SOURCES:
        # These exact observed BoxLrg placements are blocking volumes despite
        # carrying a prop mesh; keep their collision, but do not render them.
        return True
    if (identity, source) in HIDDEN_COLLISION_PLACEMENTS:
        return True
    if source_hidden and '.InterpActor_' not in (source or ''):
        # A static placement whose component serializes HiddenGame (or whose
        # owner serializes bHidden) is never drawn in game. Sanctuary_Px's
        # merged building shells and the _Low sidewalk/parking-lot proxies
        # are such placements; rendered, they lay roof atlases over the street
        # in front of Scooter's. An InterpActor's flag is only its saved state
        # (Kismet or Matinee can toggle it), so movers keep the rules below.
        return True
    if mesh == HIDDEN_COLLISION_MESH:
        # CollisionCube is usually hidden collision geometry, but not always:
        # six Sanctuary_Land placements tile the Mati_FloorConcrete01 street
        # in front of Scooter's garage and serialize no hidden flag. The three
        # exact oversized Sanctuary_P boxes above are hidden despite their
        # concrete material override. For all others, obey source hidden flags
        # or render only a resolved material other than Mat_Collision.
        if source_hidden:
            return True
        sources = [(materials or {}).get(name, {}).get('source', '').rsplit(':', 1)[-1]
                   for name in effective_materials or ()]
        return not any(s and s != HIDDEN_COLLISION_MATERIAL for s in sources)
    if mesh == HIDDEN_VISUAL_MESH:
        # Blocking_Cube is an observed placement helper even when an
        # unreliable diffuse override was attached to it.
        return True
    if mesh == HIDDEN_CLOUD_MESH and effective_materials and materials:
        # The cloud material is translucent in UE3 but its opacity graph is not
        # recovered; drawing its diffuse alone produces the observed yellow /
        # black blocking planes. Hide only the exact observed cloud instances.
        return all(materials.get(name, {}).get('source') in HIDDEN_CLOUD_MATERIALS
                   for name in effective_materials)
    if mesh == HIDDEN_TRANSITION_MESH and effective_materials and materials:
        # WorldTransition is a translucent loading/boundary plane. Its graph
        # is not recovered; feeding the undecoded material into a host mesh
        # renders an opaque helper plane. The separate lower IcePlate surfaces
        # are legitimate geometry, not these transition helpers. Hide only
        # this exact helper mesh/material pair and keep its source collision
        # state independent.
        return all((materials.get(name, {}).get('source') or '').rsplit(':', 1)[-1]
                   == HIDDEN_TRANSITION_MATERIAL
                   for name in effective_materials)
    return False
ALIASES = {
    'p_dif': 'diffuse', 'dif': 'diffuse', 'ad_diff': 'diffuse',
    'tex_diff': 'diffuse', 'diff_texture': 'diffuse',
    'p_snowdiffuse': 'diffuse', 'diffuse_a': 'diffuse',
    'diffuse_b': 'diffuse', 'diffuse_c': 'diffuse',
    'p_nrm': 'normal', 'nrm': 'normal', 'normalmap': 'normal',
    'tex_spec': 'specular',
    'emissive_gp': 'emissive', 'emissiveopacityinput': 'emissive',
}


def channel_for_parameter(parameter):
    name = str(parameter).casefold()
    return CHANNELS.get(name) or ALIASES.get(name)


def parameter_priority(parameter):
    """Prefer a direct diffuse override over a shared master default."""
    name = str(parameter).casefold()
    if name in ('p_diffuse', 'diffuse'):
        return 100
    if name in ('diffuse_a', 'diffuse_b', 'diffuse_c', 'p_dif', 'dif', 'ad_diff', 'tex_diff'):
        return 90
    if name == 'p_snowdiffuse':
        return 50
    return 80


def unnamed_diffuse_candidate(parameters, identity):
    """Conservative Material v1 approximation for stripped cooked graphs.

    Sanctuary's Mat_SancBuild_Colorized retains a single unnamed sample pointing
    to SancBuild1a_Dif_04. Require that exact naming pattern and uniqueness;
    never replace a named diffuse channel (including an explicit null override).
    """
    if any(channel_for_parameter(name) == 'diffuse' for name in parameters):
        return None
    candidates = {key for name, key in parameters.items() if key is not None
                  and re.fullmatch(r'materialexpressiontexturesampleparameter2d_\d+', name)
                  and re.search(r'_dif(?:_\d+)?$', identity(key), re.IGNORECASE)}
    return next(iter(candidates)) if len(candidates) == 1 else None


DIFFUSE_SUFFIX = re.compile(r'_diff?(?:_\d+)?$', re.IGNORECASE)
# Cooked-resource texture names that are not plausible diffuse sources: normal
# maps, packed composite/specular/emissive channels, masks, gray noise tiles,
# and detail/roughness/height/opacity-style utility maps that would otherwise
# become a wrong BaseColor with view-dependent shading.
AUXILIARY_TEXTURE = re.compile(
    r'(_n|_nm|_nrm|normal|_gray|_grey|_hs|_spec|_emis|_emissive|_alpha|_mask|_comp|_lm|noise|_cube'
    r'|_detail|_rough(?:ness)?|_height|_bump|_opacity|_illum|_lightmap|_gloss|_metal(?:lic)?'
    r'|_ao|_cavity|_disp(?:lacement)?|_refl(?:ection)?|_env)(?:_\d+)?$',
    re.IGNORECASE)


def cooked_diffuse_candidate(textures, identity, texture_class, blend_mode='BLEND_Opaque'):
    """Pick a diffuse texture from a cooked material's texture list.

    A unique *_Dif/_Diff Texture2D wins. Otherwise, for opaque and masked
    materials only, a unique Texture2D whose name does not mark it as an
    auxiliary channel is used; a translucent material's diffuse alpha would
    become its opacity, and a guessed opacity is worse than the invisible
    fallback. Both are recorded as approximations; the graph, UV mapping and
    tint remain unreconstructed.
    """
    planar = [t for t in textures if texture_class(t) == 'Texture2D']
    named = {t for t in planar if DIFFUSE_SUFFIX.search(identity(t))}
    if len(named) == 1:
        return next(iter(named)), 'sole_cooked_resource_dif_texture'
    if named or blend_mode not in ('BLEND_Opaque', 'BLEND_Masked'):
        return None, None
    plain = {t for t in planar if not AUXILIARY_TEXTURE.search(identity(t))}
    if len(plain) == 1:
        return next(iter(plain)), 'sole_cooked_resource_texture'
    return None, None


def unconnected_diffuse_constant(material_props):
    """Return the constant colour of a Material whose DiffuseColor input was
    never connected, or None when that cannot be established.

    Cooked 832/46 materials strip their expression graphs, so the input's
    Expression reference is always null. The observed distinction is the Mask
    flags: an input that had an expression keeps Mask/MaskR/G/B, an input that
    never had one carries neither. UE3 then evaluates the input as its
    Constant, which defaults to black. Observed on Master_Black; not a general
    format guarantee.
    """
    entries = material_props.get('DiffuseColor')
    if not isinstance(entries, list):
        return None
    fields = {e.get('name'): e.get('value') for e in entries if isinstance(e, dict)}
    expression = fields.get('Expression')
    if isinstance(expression, dict) and expression.get('index'):
        return None
    if any(fields.get(mask) for mask in ('Mask', 'MaskR', 'MaskG', 'MaskB', 'MaskA')):
        return None
    constant = fields.get('Constant')
    if isinstance(constant, dict):
        return [float(constant.get(c, 0)) for c in ('R', 'G', 'B')]
    return [0.0, 0.0, 0.0]


def material_index(records, path):
    matches = [index for index, record in records.items() if record['path'] == path
               and record['class'].rsplit('.', 1)[-1] in
               ('Material', 'MaterialInstanceConstant', 'MaterialInstanceTimeVarying')]
    if len(matches) != 1:
        raise ValueError('Material identity is missing or ambiguous: ' + path)
    return matches[0]


# Four source mesh families have an observed material index of zero (None) in
# the installed payload. Keep those observations explicit instead of letting
# the host silently use its default material. The two real-material
# rules are narrow and only cover the exact section/mesh pairs below; they are
# not a general "nearest material" heuristic.
SECTION_MATERIAL_POLICY_VERSION = 'sanctuary_unassigned_sections_v1'
RESISTANCE_BANNER_STATIC_MATERIAL = (
    'Prop_RolandsResistance.Materials.Mati_ResistanceBanners_Static')


def _section_for_slot(sections, slot):
    matches = [section for section in sections if section.get('slot') == slot]
    if len(matches) != 1:
        raise ValueError(f'Expected exactly one mesh section slot {slot}')
    return matches[0]


def apply_section_material_policies(mesh_identity, sections, material_for_path=None,
                                    material_source_for_id=None):
    """Apply the four observed Sanctuary section dispositions.

    ``material_for_path`` is supplied by the package-backed preparer and lets
    the banner rule resolve its explicitly observed companion material.  The
    refresh path supplies the same callback so older manifests are upgraded
    deterministically.  Null source indices stay recorded in
    ``material_policy.native_material_index``; a policy is not evidence that
    the original shader graph or final in-game appearance has been recovered.
    """
    # These are Sanctuary-specific observations. Do not apply them to an
    # unrelated map that happens to reuse a common package object name.
    supported = {
        'Sanctuary_P:Prop_RolandsResistance.Mesh.ResistanceBanner_03',
        'Sanctuary_Px:Env_Sanctuary.Meshes.SancBuild1_Trim',
        'Sanctuary_P:prop_signs.VendingIcon',
        'Sanctuary_P:Common_Meshes.Blocking.Blocking_Cube'}
    if mesh_identity not in supported:
        return sections
    short_name = mesh_identity.rsplit(':', 1)[-1]

    if short_name == 'Prop_RolandsResistance.Mesh.ResistanceBanner_03':
        source = _section_for_slot(sections, 0)
        target = _section_for_slot(sections, 1)
        observed = source.get('material')
        if not observed:
            raise ValueError('ResistanceBanner_03 observed source material is missing')
        expected = (material_for_path(RESISTANCE_BANNER_STATIC_MATERIAL)
                    if material_for_path is not None else target.get('material'))
        if not expected:
            raise ValueError('ResistanceBanner_03 static companion material is unavailable')
        existing_policy = target.get('material_policy')
        if target.get('material') not in (None, expected) or (
                target.get('material') is not None and
                (not isinstance(existing_policy, dict) or
                 existing_policy.get('method') != 'observed_companion_material_v1')):
            raise ValueError('ResistanceBanner_03 section 1 has an unexpected material')
        target['material'] = expected
        target['material_policy'] = {
            'version': SECTION_MATERIAL_POLICY_VERSION,
            'method': 'observed_companion_material_v1',
            'status': 'partial_unverified',
            'native_material_index': 0,
            'material_source': RESISTANCE_BANNER_STATIC_MATERIAL,
            'evidence': ('Section 1 is eight source triangles on the two banner-end '
                         'strips; ResistanceBannerFrame_02 observes the same static '
                         'banner material.')}
        return sections

    if short_name == 'Env_Sanctuary.Meshes.SancBuild1_Trim':
        source = _section_for_slot(sections, 0)
        target = _section_for_slot(sections, 1)
        observed = source.get('material')
        if not observed:
            raise ValueError('SancBuild1_Trim observed source material is missing')
        expected_source = 'Sanctuary_P:Prop_SancBuildings.Material.Mati_SancBuild1a_04'
        if material_source_for_id is not None and material_source_for_id(observed) != expected_source:
            raise ValueError('SancBuild1_Trim section 0 is not Mati_SancBuild1a_04')
        existing_policy = target.get('material_policy')
        if target.get('material') not in (None, observed) or (
                target.get('material') is not None and
                (not isinstance(existing_policy, dict) or
                 existing_policy.get('method') != 'same_mesh_observed_material_v1')):
            raise ValueError('SancBuild1_Trim section 1 has an unexpected material')
        target['material'] = observed
        target['material_policy'] = {
            'version': SECTION_MATERIAL_POLICY_VERSION,
            'method': 'same_mesh_observed_material_v1',
            'status': 'partial_unverified',
            'native_material_index': 0,
            'source_slot': 0,
            'evidence': ('Section 0 observes Mati_SancBuild1a_04; the sibling '
                         'SancBuild1Base_Trim also uses that material for every '
                         'render section.')}
        return sections

    if short_name == 'prop_signs.VendingIcon':
        target = _section_for_slot(sections, 0)
        if target.get('material') is not None:
            raise ValueError('VendingIcon source section unexpectedly has a material')
        target['material_policy'] = {
            'version': SECTION_MATERIAL_POLICY_VERSION,
            'method': 'host_neutral_fallback_v1',
            'status': 'explicit_host_fallback',
            'native_material_index': 0,
            'fallback': 'M_OpenWillowNeutralFallback',
            'fallback_rgb': [0.5, 0.5, 0.5],
            'evidence': ('The source mesh is a 32-triangle icon with material '
                         'index 0/None and no observed mesh material.')}
        return sections

    if short_name == 'Common_Meshes.Blocking.Blocking_Cube':
        target = _section_for_slot(sections, 0)
        if target.get('material') is not None:
            raise ValueError('Blocking_Cube source section unexpectedly has a material')
        target['material_policy'] = {
            'version': SECTION_MATERIAL_POLICY_VERSION,
            'method': 'helper_hidden_collision_only_v1',
            'status': 'observed_helper_policy',
            'native_material_index': 0,
            'collision_body': 'Common_Meshes.Blocking.Blocking_Cube.RB_BodySetup_1',
            'evidence': ('The source mesh is material index 0/None and carries the '
                         'observed convex blocking body; placed helpers remain hidden.')}
        return sections

    return sections


def cooked_texture_references(payload, data):
    """Read the observed 832/46 Material resource prefix, never scan offsets.

    Only empty compile-error/dependency arrays are supported. The remaining
    resource/shader bytes are deliberately opaque. See DECISIONS.md.
    """
    offset = data['property_offset'] + data['consumed_bytes']
    if offset < 0 or offset > len(payload) or len(payload) - offset != data['trailing_bytes']:
        raise ValueError('Cooked material payload/property boundary mismatch')
    tail = memoryview(payload)[offset:]
    if len(tail) < 36:
        raise ValueError('Truncated cooked material resource prefix')
    if struct.unpack_from('<2i', tail) != (0, 0):
        raise ValueError('Unsupported cooked material error/dependency arrays')
    # Two empty arrays, resource int, GUID, resource int, texture-array count.
    count = struct.unpack_from('<i', tail, 32)[0]
    if count < 0 or count > (len(tail) - 36) // 4:
        raise ValueError('Invalid cooked material texture count')
    refs = list(struct.unpack_from(f'<{count}i', tail, 36))
    return refs, len(tail) - 36 - count * 4


class Scene:
    def __init__(self, reader, game, output, include_dlc=False, outer_shell=False,
                 matinee_first_key=False):
        self.reader, self.game, self.output = reader.resolve(), game.resolve(), output.resolve()
        self.cooked = self.game / 'WillowGame/CookedPCConsole'
        self.include_dlc = include_dlc
        self.outer_shell = outer_shell
        self.matinee_first_key = matinee_first_key
        self.package_root = self.game if include_dlc else self.cooked
        self.schema = Path(__file__).with_name('level-arrays.schema')
        self.packages = {}
        self.texture_caches = {}
        for path in content_files(self.game, include_dlc):
            if path.suffix.lower() in PACKAGE_SUFFIXES:
                self.packages.setdefault(path.stem.casefold(), []).append(path)
            elif path.suffix.lower() == '.tfc':
                self.texture_caches.setdefault(path.name.casefold(), []).append(path)
        self.records, self.export_records = {}, {}
        self.materials, self.meshes, self.textures = {}, {}, {}
        self.resolved, self.imports = {}, {}
        self.issues = []
        self.output.mkdir(parents=True, exist_ok=True)

    def package(self, name):
        paths = self.packages.get(name.casefold(), [])
        if len(paths) != 1:
            raise ValueError(f'Expected unique package {name}, found {len(paths)}')
        return paths[0]

    def call(self, package, *args):
        run = subprocess.run([str(self.reader), str(self.package(package)), *map(str, args)],
                             capture_output=True, text=True, encoding='utf-8')
        if run.returncode:
            raise ValueError(run.stderr.strip())
        return json.loads(run.stdout)

    def properties_batch(self, package, indices, offset, label):
        """Read same-class actor/component properties in one bounded reader call."""
        indices = list(dict.fromkeys(indices))
        if not indices:
            return {}
        index_file = self.output / f'.{label}-{package}.indices'
        index_file.write_text('\n'.join(str(i) for i in indices) + '\n', encoding='ascii')
        try:
            run = subprocess.run([str(self.reader), str(self.package(package)),
                                  '--properties-batch', str(index_file),
                                  '--property-offset', str(offset)],
                                 capture_output=True, text=True, encoding='utf-8')
        finally:
            index_file.unlink(missing_ok=True)
        if run.returncode:
            raise ValueError(run.stderr.strip())
        records = [json.loads(line) for line in run.stdout.splitlines() if line.strip()]
        result = {int(item['index']): item for item in records}
        if len(result) != len(indices):
            raise ValueError('Package reader property batch returned an incomplete record set')
        return result

    def anchored_properties(self, package, index, anchor, required):
        """Recover one class-specific property-stream start from an exact FName.

        This is used only for the directional-light component whose serialized
        lightmap payload precedes its UProperties. The candidate is accepted
        only when the package reader sees the anchor first, all required fields,
        and a property stream that consumes the rest of that component export.
        """
        names = self.call(package, '--names')
        payload = bytes(self.call(package, '--payload', index))
        candidates = []
        for offset in property_name_offsets(payload, names, anchor):
            try:
                record = self.call(package, '--properties', index,
                                   '--property-offset', offset)
            except ValueError:
                continue
            properties = record.get('properties', [])
            decoded_names = [item.get('name') for item in properties
                             if item.get('status') == 'decoded']
            decoded = values(properties)
            if (record.get('trailing_bytes') == 0 and decoded_names
                    and decoded_names[0] == anchor and set(required) <= set(decoded)):
                candidates.append((offset, decoded, properties))
        if len(candidates) != 1:
            raise ValueError(f'Expected one complete {anchor} property stream, found {len(candidates)}')
        return candidates[0]

    def load(self, package):
        if package not in self.records:
            self.records[package] = {r['index']: r for r in self.call(package, '--scene-records', self.schema)}
        return self.records[package]

    def export_rows(self, package):
        if package not in self.export_records:
            self.export_records[package] = {
                r['index']: r for r in self.call(package, '--exports')}
        return self.export_records[package]

    def resolve(self, package, ref):
        index = ref.get('index', 0) if isinstance(ref, dict) else ref
        if not index:
            return None
        if index > 0:
            return package, index
        if (package, index) in self.resolved:
            return self.resolved[package, index]
        path = ref.get('path') if isinstance(ref, dict) else None
        if package not in self.imports:
            self.imports[package] = {r['index']: r for r in self.call(package, '--imports')}
        metadata = self.imports[package].get(index, {})
        path = path or metadata.get('path')
        expected_class = metadata.get('class_name')
        if path:
            for loaded, records in self.records.items():
                match = next((r for r in records.values() if r['path'].casefold() == path.casefold()
                              and (not expected_class or r['class'].rsplit('.', 1)[-1] == expected_class)), None)
                if match:
                    self.resolved[package, index] = loaded, match['index']
                    return loaded, match['index']
        # Shared base resources retain the established lookup order in DLC
        # mode. Only an absent target expands the search to the full install.
        try:
            r = self.call(package, '--resolve', index, '--cooked', self.cooked)
        except ValueError as error:
            if not self.include_dlc or 'not found:' not in str(error):
                raise
            r = self.call(package, '--resolve', index, '--cooked', self.package_root)
        self.resolved[package, index] = r['resolved_package'], r['resolved_index']
        return self.resolved[package, index]

    def identity(self, key):
        return key[0] + ':' + self.load(key[0])[key[1]]['path']

    def filename(self, key, suffix):
        return hashlib.sha256(self.identity(key).encode()).hexdigest()[:24] + suffix

    def issue(self, context, error):
        self.issues.append({'object': context, 'error': str(error)})

    def owner_properties(self, level, owner):
        """Decoded properties of a component's owning actor.

        The bulk scene records decode only engine placement classes. Other
        owners are read on demand with the same observed 26-byte actor prefix,
        and must decode to their exact end (no trailing bytes).
        """
        if 'data' in owner or 'error' in owner:
            return props(owner)
        cache = self.__dict__.setdefault('_owner_properties', {})
        key = (level, owner['index'])
        if key not in cache:
            data = self.call(level, '--properties', owner['index'], '--property-offset', 26)
            if data.get('trailing_bytes') != 0:
                raise ValueError('Owner properties do not end at the object end')
            cache[key] = values(data.get('properties', []))
        return cache[key]

    def material_parameters(self, key, stack=()):
        if key in stack or len(stack) >= 32:
            raise ValueError('Material parent cycle/depth limit')
        p = props(self.load(key[0])[key[1]])
        parent = self.resolve(key[0], p.get('Parent', 0))
        parameters = self.material_parameters(parent, (*stack, key)) if parent else {}
        # Base-material named texture expressions supply defaults.
        for ref in p.get('Expressions', []):
            expr = self.resolve(key[0], ref)
            if not expr:
                continue
            expression = self.load(expr[0])[expr[1]]
            if 'TextureSampleParameter' not in expression.get('class', ''):
                continue
            e = props(expression)
            name = e.get('ParameterName', '').casefold()
            if (channel_for_parameter(name) or re.fullmatch(r'materialexpressiontexturesampleparameter2d_\d+', name)) and e.get('Texture', {}).get('index'):
                parameters[name] = self.resolve(expr[0], e['Texture'])
        for entry in p.get('TextureParameterValues', []):
            e = values(entry)
            name = e.get('ParameterName', '').casefold()
            if channel_for_parameter(name) or re.fullmatch(r'materialexpressiontexturesampleparameter2d_\d+', name):
                parameters[name] = self.resolve(key[0], e.get('ParameterValue', 0))
        return parameters

    def named_chain_parameters(self, key):
        """Every named sampler/scalar/vector input along a material chain.

        Unlike the Material v1 channel allow-list this keeps all names, with
        instance overrides applied over base defaults. Returns the base
        material key with the three parameter maps (names casefolded).
        """
        chain, current = [], key
        while current is not None:
            if current in chain or len(chain) >= 32:
                raise ValueError('Material parent cycle/depth limit')
            chain.append(current)
            current = self.resolve(current[0], props(self.load(current[0])[current[1]]).get('Parent', 0))
        samplers, scalars, vectors = {}, {}, {}
        for item in reversed(chain):
            p = props(self.load(item[0])[item[1]])
            for ref in p.get('Expressions', []):
                expr = self.resolve(item[0], ref)
                if not expr:
                    continue
                record = self.load(expr[0])[expr[1]]
                cls = record.get('class', '').rsplit('.', 1)[-1]
                if 'Parameter' not in cls or 'error' in record:
                    continue
                e = props(record)
                name = e.get('ParameterName', '').casefold()
                if not name:
                    continue
                if 'TextureSampleParameter' in cls:
                    samplers[name] = self.resolve(expr[0], e.get('Texture', 0))
                elif cls == 'MaterialExpressionScalarParameter':
                    scalars[name] = e.get('DefaultValue')
                elif cls == 'MaterialExpressionVectorParameter':
                    vectors[name] = e.get('DefaultValue')
            for entry in p.get('TextureParameterValues', []):
                e = values(entry)
                samplers[e.get('ParameterName', '').casefold()] = self.resolve(item[0], e.get('ParameterValue', 0))
            for entry in p.get('ScalarParameterValues', []):
                e = values(entry)
                scalars[e.get('ParameterName', '').casefold()] = e.get('ParameterValue')
            for entry in p.get('VectorParameterValues', []):
                e = values(entry)
                vectors[e.get('ParameterName', '').casefold()] = e.get('ParameterValue')
        return chain[-1], samplers, scalars, vectors

    def unlit_color_multiplier(self, key, material):
        """Recorded RGB multiplier for an inspected Unlit chain, or None.

        The value is the named vector constant with instance overrides last.
        It is recorded with its provenance; the host applies it to the
        visible Unlit color. This does not reconstruct the stripped graph.
        """
        entry = INSPECTED_UNLIT_COLOR_MULTIPLIERS.get(self.identity(key))
        if entry is None:
            return None
        if material.get('lighting_model') != 'MLM_Unlit':
            raise ValueError('Unlit color multiplier requires an Unlit chain')
        base, _, _, vectors = self.named_chain_parameters(key)
        if self.identity(base) != entry['base']:
            raise ValueError('Unlit color multiplier requires the inspected base material')
        value = vectors.get(entry['parameter'].casefold())
        if not isinstance(value, dict) or not all(
                isinstance(value.get(c), (int, float)) and not isinstance(value.get(c), bool)
                and math.isfinite(value[c]) and value[c] >= 0 for c in 'RGB'):
            raise ValueError(f"Unlit color multiplier {entry['parameter']} is missing or invalid")
        return {'method': entry['method'], 'status': 'partial_unverified',
                'parameter': entry['parameter'], 'source_material': self.identity(base),
                'rgb': [float(value[c]) for c in 'RGB'], 'omitted': entry['omitted']}

    def diffuse_tint(self, key, material):
        """Recorded RGB diffuse multiplier for a scoped lit instance, or None.

        The rule matches the exact base master and parameter name in
        INSPECTED_DIFFUSE_TINTS. The value is read from the instance chain,
        with instance overrides last. The host applies it to Base Color.
        """
        identity = self.identity(key)
        scoped = [(k, e) for k, e in INSPECTED_DIFFUSE_TINTS.items() if identity.startswith(e['scope'])]
        if not scoped or material.get('lighting_model') == 'MLM_Unlit':
            return None
        base, _, _, vectors = self.named_chain_parameters(key)
        for (base_identity, parameter), entry in scoped:
            if self.identity(base) != base_identity or parameter.casefold() not in vectors:
                continue
            value = vectors[parameter.casefold()]
            if not isinstance(value, dict) or not all(
                    isinstance(value.get(c), (int, float)) and not isinstance(value.get(c), bool)
                    and math.isfinite(value[c]) and 0 <= value[c] <= 64 for c in 'RGB'):
                raise ValueError(f"Diffuse tint {parameter} is missing or invalid")
            return {'method': entry['method'], 'status': 'partial_unverified',
                    'parameter': parameter, 'source_material': base_identity,
                    'rgb': [float(value[c]) for c in 'RGB'], 'omitted': entry['omitted']}
        return None

    def inspected_color_parameter(self, key, material, table):
        """Read one exact-chain RGB parameter for an inspected host recipe."""
        entry = table.get(self.identity(key))
        if entry is None:
            return None
        base, _, _, vectors = self.named_chain_parameters(key)
        if self.identity(base) != entry['base']:
            raise ValueError('Color parameter requires the inspected base material')
        value = vectors.get(entry['parameter'].casefold())
        if not isinstance(value, dict) or not all(
                isinstance(value.get(c), (int, float)) and not isinstance(value.get(c), bool)
                and math.isfinite(value[c]) and 0 <= value[c] <= 64 for c in 'RGB'):
            raise ValueError(f"Color parameter {entry['parameter']} is missing or invalid")
        if table is INSPECTED_UNLIT_CONSTANT_COLORS and material.get('lighting_model') != 'MLM_Unlit':
            raise ValueError('Unlit color requires an Unlit material')
        return {'method': entry['method'], 'status': 'partial_unverified',
                'parameter': entry['parameter'], 'source_material': self.identity(base),
                'rgb': [float(value[c]) for c in 'RGB'], 'omitted': entry['omitted']}

    def inspected_scene_material_recipe(self, key, base, textures):
        """Return a resource-validated visual recipe for exact Sanctuary materials."""
        entry = INSPECTED_SCENE_MATERIAL_RECIPES.get(self.identity(key))
        if entry is None:
            return None
        if self.identity(base) != entry['base']:
            raise ValueError('Scene material recipe requires the inspected base material')

        def inspected(identity):
            found = [item for item in textures if self.identity(item) == identity
                     and self.load(item[0])[item[1]]['class'].rsplit('.', 1)[-1] == 'Texture2D']
            if len(found) != 1:
                raise ValueError('Scene material recipe requires the inspected texture: ' + identity)
            return found[0]

        result = {'method': entry['method'], 'status': 'partial_unverified',
                  'base_material': entry['base'], 'omitted': entry['omitted'],
                  'issue': entry['issue']}
        calibration = float(entry.get('emissive_calibration_scale', 1.0))
        if not math.isfinite(calibration) or calibration <= 0:
            raise ValueError('Scene material emissive calibration must be finite and positive')
        if calibration != 1.0:
            result['emissive_calibration'] = {
                'scale': calibration, 'status': 'UNVERIFIED host screenshot calibration',
                'evidence': entry.get('emissive_calibration_evidence', 'named host calibration')}
        if entry.get('host_lighting_model'):
            result['host_lighting_model'] = entry['host_lighting_model']
        if entry.get('mask_channels'):
            result['mask_channels'] = dict(entry['mask_channels'])
        if entry.get('palette'):
            result['palette'] = {name: list(rgb) for name, rgb in entry['palette'].items()}
        if entry.get('diffuse'):
            diffuse = inspected(entry['diffuse'])
            result['diffuse'] = self.texture(diffuse, 'diffuse')
            result['diffuse_source'] = self.identity(diffuse)
        for channel in ('emissive_layers', 'unlit_color_layers'):
            layers = []
            for index, layer in enumerate(entry.get(channel, [])):
                texture = inspected(layer['texture'])
                prepared_layer = dict(layer)
                if channel == 'emissive_layers' and calibration != 1.0:
                    prepared_layer['color'] = [float(value) * calibration
                                               for value in layer['color']]
                layers.append({**prepared_layer, 'source': self.identity(texture),
                               'file': self.texture(texture, f'recipe_{channel}_{index}')})
            if layers:
                result[channel] = layers
        if entry.get('mask_texture'):
            texture = inspected(entry['mask_texture'])
            result['mask_texture_file'] = self.texture(texture, 'recipe_mask')
            result['mask_texture_source'] = self.identity(texture)
        return result

    def native_sky_approximation(self, key):
        """Named inputs of the observed time-of-day sky master, or None.

        The cooked master graph is stripped; only parameter names and the
        instance values survive. The returned record tells the host which
        textures and constants to combine and states the assumptions. It is
        an approximation policy, not a reconstruction of the UE3 sky graph.
        """
        base, samplers, scalars, vectors = self.named_chain_parameters(key)
        if self.identity(base).split(':', 1)[1] != NATIVE_SKY_MASTER:
            return None
        textures = {}
        for parameter, role in SKY_SAMPLERS.items():
            texture = samplers.get(parameter)
            if texture is None:
                raise ValueError(f'Sky master input {parameter} is missing')
            if self.load(texture[0])[texture[1]]['class'].rsplit('.', 1)[-1] != 'Texture2D':
                raise ValueError(f'Sky master input {parameter} is not a Texture2D')
            textures[role] = {'source': self.identity(texture),
                              'file': self.texture(texture, 'sky_' + role)}
        constants = {}
        for parameter, role in SKY_SCALARS.items():
            value = scalars.get(parameter)
            if not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(value):
                raise ValueError(f'Sky master scalar {parameter} is missing or invalid')
            constants[role] = float(value)
        colors = {}
        for parameter, role in SKY_VECTORS.items():
            value = vectors.get(parameter)
            if not isinstance(value, dict) or not all(
                    isinstance(value.get(c), (int, float)) and math.isfinite(value[c]) for c in 'RGBA'):
                raise ValueError(f'Sky master vector {parameter} is missing or invalid')
            colors[role] = [float(value[c]) for c in 'RGBA']
        if not 0 <= constants['time_of_day'] < SKY_TIME_AXIS:
            raise ValueError('Time_of_Day is outside the transition strip')
        return {'method': 'sky_time_of_day_strip_v1', 'status': 'partial_unverified',
                'master': self.identity(base), 'textures': textures,
                'scalars': constants, 'vectors': colors,
                'time_axis': {'divisor': SKY_TIME_AXIS,
                              'column_u': constants['time_of_day'] / SKY_TIME_AXIS,
                              'note': 'UNVERIFIED: Time_of_Day read as a strip pixel column'},
                'horizon_row_v': SKY_HORIZON_ROW,
                'cloud_channel': 'R',
                'omitted': ['sun spot (Sun_spot_brightness)', 'Masks (stars, cap gradient)',
                            'Horizion_track_color_multiplier', 'cloud channels G/B',
                            'cloud motion', 'time-of-day animation', 'Kismet control']}

    def material_metadata(self, key, stack=()):
        """Carry safe UE3 blend/shading flags without importing full graphs."""
        if key in stack or len(stack) >= 32:
            raise ValueError('Material parent cycle/depth limit')
        p = props(self.load(key[0])[key[1]])
        parent = self.resolve(key[0], p.get('Parent', 0))
        metadata = self.material_metadata(parent, (*stack, key)) if parent else {}
        if p.get('BlendMode'):
            metadata['blend_mode'] = p['BlendMode']
        if p.get('LightingModel'):
            metadata['lighting_model'] = p['LightingModel']
        if 'TwoSided' in p:
            metadata['two_sided'] = bool(p['TwoSided'])
        # A translucent/masked base graph has an opacity input even when its
        # supported texture channels are outside Material v1.
        opacity = p.get('Opacity')
        if isinstance(opacity, list):
            metadata['has_opacity'] = any(
                item.get('name') == 'Expression'
                and isinstance(item.get('value'), dict)
                and item['value'].get('index')
                for item in opacity if isinstance(item, dict))
        return metadata

    def glacier_primary_surface(self, key, base, textures):
        """Explicit primary-layer approximation, never a recovered snow shader.

        Restricted to the two inspected Sanctuary family members and the exact
        resource texture set. UV0 is an approximation: native static permutation
        data and the stripped blend graph are not interpreted by this policy.
        """
        base_path = 'Prop_Glacier.Materials.Mat_Glacier'
        if self.identity(base).split(':', 1)[1] != base_path:
            return None
        if self.identity(key).split(':', 1)[1] not in (
                base_path, 'Prop_Glacier.Materials.Mati_Glacier2x'):
            return None
        expected = {'Prop_Glacier.Textures.GlacierFront_Dif',
                    'Prop_Glacier.Textures.GlacierFront_Nrm',
                    'Prop_Terrain.Textures.Snow_Dif',
                    'Prop_Skybox.MoveMe.R2Tex_SnowTempCubeStaticNegY'}
        by_path = {self.identity(t).split(':', 1)[1]: t for t in textures}
        if len(textures) != 4 or set(by_path) != expected:
            return None
        if any(self.load(t[0])[t[1]]['class'].rsplit('.', 1)[-1] != 'Texture2D'
               for t in textures):
            return None
        name = 'p_texscalar_rgmain_basnow'
        scale = None
        for ref in props(self.load(base[0])[base[1]]).get('Expressions', []):
            expression = self.resolve(base[0], ref)
            if expression is None:
                continue
            row = self.load(expression[0])[expression[1]]
            p = props(row)
            if p.get('ParameterName', '').casefold() == name:
                if row['class'].rsplit('.', 1)[-1] != 'MaterialExpressionVectorParameter' or scale is not None:
                    raise ValueError('Ambiguous glacier primary-layer scale')
                scale = p.get('DefaultValue')
        chain, current = [], key
        while current != base:
            if current in chain or len(chain) >= 32:
                raise ValueError('Glacier parent cycle/depth limit')
            chain.append(current)
            current = self.resolve(current[0], props(self.load(current[0])[current[1]]).get('Parent', 0))
            if current is None:
                raise ValueError('Glacier parent does not reach inspected base')
        for instance in reversed(chain):
            overrides = [values(e).get('ParameterValue') for e in
                         props(self.load(instance[0])[instance[1]]).get('VectorParameterValues', [])
                         if values(e).get('ParameterName', '').casefold() == name]
            if len(overrides) > 1:
                raise ValueError('Duplicate glacier scale override')
            if overrides:
                scale = overrides[0]
        if not isinstance(scale, dict) or not all(
                isinstance(scale.get(c), (int, float)) and math.isfinite(scale[c]) for c in 'RGBA'):
            raise ValueError('Missing or invalid glacier primary-layer scale')
        return {'diffuse': by_path['Prop_Glacier.Textures.GlacierFront_Dif'],
                'normal': by_path['Prop_Glacier.Textures.GlacierFront_Nrm'],
                'scale': [float(scale[c]) for c in 'RGBA']}

    def cooked_material_textures(self, key, stack=()):
        if key in stack or len(stack) >= 32:
            raise ValueError('Material parent cycle/depth limit')
        record = self.load(key[0])[key[1]]
        if record['class'].rsplit('.', 1)[-1] != 'Material':
            parent = self.resolve(key[0], props(record).get('Parent', 0))
            return self.cooked_material_textures(parent, (*stack, key)) if parent else None
        refs, opaque = cooked_texture_references(
            bytes(self.call(key[0], '--payload', key[1])), record['data'])
        textures = []
        for ref in refs:
            texture = self.resolve(key[0], ref)
            if texture is None:
                continue
            target = self.load(texture[0]).get(texture[1])
            if target is None or target['class'].rsplit('.', 1)[-1] not in ('Texture2D', 'TextureCube'):
                raise ValueError('Cooked material reference is not a supported texture')
            textures.append(texture)
        return key, textures, opaque

    def texture(self, key, channel):
        if key is None:
            return None
        cache_key = (*key, channel)
        if cache_key not in self.textures:
            filename = self.filename(key, '_' + channel + '.png')
            tfc_root = self.cooked
            if self.include_dlc:
                cache = props(self.load(key[0])[key[1]]).get('TextureFileCacheName', 'None')
                cache_file = cache if cache.lower().endswith('.tfc') else cache + '.tfc'
                tfc_root = cache_directory(self.texture_caches.get(cache_file.casefold(), []),
                                           self.package(key[0]), self.cooked)
            self.call(key[0], '--texture', key[1], '--property-offset', 4,
                      '--output', self.output / filename, '--tfc', tfc_root)
            self.textures[cache_key] = filename
        return self.textures[cache_key]

    def material(self, key):
        if key is None:
            return None
        name = self.filename(key, '')
        if name not in self.materials:
            material = {'source': self.identity(key), 'channels': {}}
            self.materials[name] = material
            try:
                material.update(self.material_metadata(key))
                parameters = self.material_parameters(key)
                if material.get('lighting_model') == 'MLM_Unlit':
                    # The ordinary diffuse inference below still runs and
                    # stays as the host's fallback when this record is absent.
                    try:
                        sky = self.native_sky_approximation(key)
                    except ValueError as error:
                        sky = None
                        self.issue(material['source'] + ':sky', error)
                    if sky is not None:
                        material['sky_approximation'] = sky
                        self.issue(material['source'],
                                   'Approximation: Time_of_Day column of the transition strip over dome V with a horizon-tinted cloud layer; sky master graph not reconstructed')
                if self.identity(key) in INSPECTED_UNLIT_COLOR_MULTIPLIERS:
                    try:
                        multiplier = self.unlit_color_multiplier(key, material)
                    except ValueError as error:
                        multiplier = None
                        self.issue(material['source'] + ':multiplier', error)
                    if multiplier is not None:
                        material['unlit_color_multiplier'] = multiplier
                        self.issue(material['source'],
                                   f"Approximation: Unlit color scaled by {multiplier['parameter']}; shadow mask, relief and secondary color not reconstructed")
                if self.identity(key) in INSPECTED_EMISSIVE_MULTIPLIERS:
                    try:
                        multiplier = self.inspected_color_parameter(
                            key, material, INSPECTED_EMISSIVE_MULTIPLIERS)
                    except ValueError as error:
                        multiplier = None
                        self.issue(material['source'] + ':emissive-scale', error)
                    if multiplier is not None:
                        material['emissive_multiplier'] = multiplier
                        self.issue(material['source'],
                                   'Approximation: emissive texture scaled by the inspected p_EmissiveScale vector; emissive graph remains unresolved')
                if self.identity(key) in INSPECTED_UNLIT_CONSTANT_COLORS:
                    try:
                        color = self.inspected_color_parameter(
                            key, material, INSPECTED_UNLIT_CONSTANT_COLORS)
                    except ValueError as error:
                        color = None
                        self.issue(material['source'] + ':unlit-color', error)
                    if color is not None:
                        material['unlit_constant_color'] = color
                        material['host_blend_mode'] = 'BLEND_Opaque'
                        self.issue(material['source'],
                                   'Approximation: inspected p_EyeColorA is shown as a constant Unlit color; masked eye graph is not reconstructed')
                try:
                    tint = self.diffuse_tint(key, material)
                except ValueError as error:
                    tint = None
                    self.issue(material['source'] + ':diffuse-tint', error)
                if tint is not None:
                    material['diffuse_tint'] = tint
                    self.issue(material['source'],
                               f"Approximation: diffuse scaled by {tint['parameter']} from the shared Master_World chain; rest of the graph not reconstructed")
                regular_fallback = REGULAR_DIFFUSE_FALLBACKS.get(material['source'])
                if regular_fallback is not None:
                    parent = self.resolve(key[0], props(self.load(key[0])[key[1]]).get('Parent', 0))
                    if parent is None or self.identity(parent) != 'Sanctuary_P:Prop_SancBuildings.Optimization.Sanc_HLS_Master':
                        raise ValueError('Regular diffuse fallback requires the inspected HLS parent')
                    package, path = regular_fallback.split(':', 1)
                    if package != key[0]:
                        raise ValueError('Regular diffuse fallback package mismatch')
                    regular_index = material_index(self.load(package), path)
                    regular_name = self.material((package, regular_index))
                    regular = self.materials[regular_name]
                    diffuse = regular.get('channels', {}).get('diffuse')
                    if not diffuse:
                        raise ValueError('Regular diffuse fallback has no diffuse channel')
                    candidates = [i for i, row in self.load(key[0]).items()
                                  if row['path'] == 'Prop_SancBuildings.Textures.SancBuild4a_Dif'
                                  and row['class'].rsplit('.', 1)[-1] == 'Texture2D']
                    if len(candidates) != 1:
                        raise ValueError('Regular diffuse fallback requires a unique concrete atlas')
                    expected = (key[0], candidates[0])
                    if diffuse != self.filename(expected, '_diffuse.png'):
                        raise ValueError('Regular diffuse fallback requires the inspected concrete atlas')
                    material['channels']['diffuse'] = diffuse
                    material['diffuse_inference'] = regular['source']
                    material['diffuse_inference_method'] = 'same_package_regular_diffuse_fallback_v1'
                    material['surface_approximation'] = {
                        'method': 'same_package_regular_diffuse_fallback_v1',
                        'status': 'partial_unverified',
                        'source_material': regular['source'],
                        'source_texture': self.identity(expected),
                        'uv_selection': 'UV0 unchanged; regular atlas approximation',
                        'omitted': ['HLS Color/Luminosity combine',
                                    'static permutation', 'modulation']}
                    self.issue(material['source'],
                               'Approximation: HLS Color/Luminosity graph replaced by same-package regular diffuse; static permutation and modulation not reconstructed')
                    # Only replace diffuse; retain any supported explicit channels.
                    parameters = {p: t for p, t in parameters.items()
                                  if channel_for_parameter(p) != 'diffuse'}
                inferred = None if regular_fallback else unnamed_diffuse_candidate(parameters, self.identity)
                if inferred is not None:
                    parameters['p_diffuse'] = inferred
                    material['diffuse_inference'] = self.identity(inferred)
                    self.issue(material['source'], 'Approximation: sole unnamed _Dif texture used as diffuse; cooked graph and tint not reconstructed')
                if not regular_fallback and not any(channel_for_parameter(p) == 'diffuse' for p in parameters):
                    try:
                        cooked = self.cooked_material_textures(key)
                        if cooked is not None:
                            base, textures, opaque = cooked
                            material['cooked_texture_resource'] = {
                                'source': self.identity(base),
                                'textures': [self.identity(t) for t in textures],
                                'opaque_tail_bytes': opaque}
                            try:
                                recipe = self.inspected_scene_material_recipe(key, base, textures)
                            except ValueError as error:
                                recipe = None
                                self.issue(material['source'] + ':scene-recipe', error)
                            if recipe is not None:
                                material['scene_recipe'] = recipe
                                self.issue(material['source'], recipe['issue'])
                            inferred, method = cooked_diffuse_candidate(
                                textures, self.identity,
                                lambda t: self.load(t[0])[t[1]]['class'].rsplit('.', 1)[-1],
                                material.get('blend_mode', 'BLEND_Opaque'))
                            fallback = (INSPECTED_COLOR_FALLBACKS.get(self.identity(key))
                                        if material.get('blend_mode', 'BLEND_Opaque') == 'BLEND_Opaque'
                                        else None)
                            if fallback is not None:
                                # A base-keyed entry applies to that base only; an
                                # instance-keyed entry to the inspected parent only.
                                if self.identity(base) != fallback.get('base', self.identity(key)):
                                    raise ValueError(f"{fallback['label']} fallback requires the inspected base material")
                                def inspected(identity, kind):
                                    found = [t for t in textures if self.identity(t) == identity
                                             and self.load(t[0])[t[1]]['class'].rsplit('.', 1)[-1] == 'Texture2D']
                                    if len(found) != 1:
                                        raise ValueError(f"{fallback['label']} fallback requires the inspected {fallback['noun']} {kind}")
                                    return found[0]
                                inferred, method = inspected(fallback['color'], 'texture'), fallback['method']
                                normal = inspected(fallback['normal'], 'normal') if fallback['normal'] else None
                                if normal is not None and not any(channel_for_parameter(p) == 'normal' for p in parameters):
                                    parameters['p_normal'] = normal
                                emissive = inspected(fallback['emissive'], 'emissive') if fallback.get('emissive') else None
                                if emissive is not None and not any(channel_for_parameter(p) == 'emissive' for p in parameters):
                                    parameters['p_emissive'] = emissive
                                material['surface_approximation'] = {
                                    'method': method, 'status': 'partial_unverified',
                                    'source_texture': self.identity(inferred),
                                    'uv_selection': 'UV0 unchanged; native UV modulation unverified',
                                    'omitted': fallback['omitted']}
                                if normal is not None:
                                    material['surface_approximation']['normal_texture'] = self.identity(normal)
                                if emissive is not None:
                                    material['surface_approximation']['emissive_texture'] = self.identity(emissive)
                            glacier = (self.glacier_primary_surface(key, base, textures)
                                       if inferred is None and material.get('blend_mode', 'BLEND_Opaque') == 'BLEND_Opaque'
                                       else None)
                            if glacier is not None:
                                parameters['p_diffuse'] = glacier['diffuse']
                                glacier_channels = ['diffuse']
                                # Preserve any explicit normal parameter, including a null override.
                                if not any(channel_for_parameter(p) == 'normal' for p in parameters):
                                    parameters['p_normal'] = glacier['normal']
                                    glacier_channels.append('normal')
                                material['surface_approximation'] = {
                                    'method': 'glacier_primary_layer_v1',
                                    'status': 'partial_unverified',
                                    'source_scale_parameter': 'P_TexScalar_RGMain_BASnow',
                                    'source_scale': glacier['scale'],
                                    'omitted': ['snow_blend', 'reflection', 'glow'],
                                    'uv_selection': 'UV0 approximation; static permutation not decoded'}
                                material['channel_uv'] = {
                                    channel: {'index': 0, 'scale': glacier['scale'][:2]}
                                    for channel in glacier_channels}
                                self.issue(material['source'], 'Approximation: glacier primary diffuse/normal layer with retained tiling on UV0; snow blend, reflection, glow and static UV selection unverified')
                            if inferred is not None:
                                parameters['p_diffuse'] = inferred
                                material['diffuse_inference'] = self.identity(inferred)
                                material['diffuse_inference_method'] = method
                                self.issue(material['source'], fallback['issue']
                                           if fallback is not None and method == fallback['method'] else
                                           'Approximation: sole cooked resource _Dif texture used as diffuse; graph, UV mapping and tint not reconstructed'
                                           if method == 'sole_cooked_resource_dif_texture' else
                                           'Approximation: sole non-auxiliary cooked resource texture used as diffuse; graph, UV mapping and tint not reconstructed')
                            elif not textures and material.get('blend_mode', 'BLEND_Opaque') == 'BLEND_Opaque':
                                constant = unconnected_diffuse_constant(props(self.load(base[0])[base[1]]))
                                if constant is not None:
                                    material['constant_diffuse'] = constant
                                    self.issue(material['source'], 'Approximation: unconnected DiffuseColor input rendered as its constant; no cooked textures')
                    except ValueError as error:
                        self.issue(material['source'], error)
                for parameter, texture in parameters.items():
                    channel = channel_for_parameter(parameter)
                    if not channel or texture is None:
                        continue
                    try:
                        filename = self.texture(texture, channel)
                        current = material['channels'].get(channel)
                        # Parent defaults often point at StubGray. A concrete
                        # child override wins, while an alias never replaces a
                        # stronger exact parameter that was already selected.
                        priority = parameter_priority(parameter)
                        current_priority = material.setdefault('_channel_priority', {}).get(channel, -1)
                        is_stub = 'common_textures.stub.' in self.identity(texture).casefold()
                        current_is_stub = material.setdefault('_channel_stub', {}).get(channel, False)
                        if (current is None or (current_is_stub and not is_stub)
                                or (current_is_stub == is_stub and priority > current_priority)):
                            material['channels'][channel] = filename
                            material['_channel_priority'][channel] = priority
                            material['_channel_stub'][channel] = is_stub
                    except ValueError as error:
                        # A failed channel decode must not leave inference
                        # metadata claiming a diffuse that never decoded: the
                        # material renders the neutral fallback, so the record
                        # must say so too.
                        if channel == 'diffuse':
                            material.pop('diffuse_inference', None)
                            material.pop('diffuse_inference_method', None)
                            material.pop('surface_approximation', None)
                        self.issue(material['source'] + ':' + channel, error)
                material.pop('_channel_priority', None)
                material.pop('_channel_stub', None)
                recipe = material.get('scene_recipe')
                if recipe and recipe.get('diffuse'):
                    material['channels']['diffuse'] = recipe['diffuse']
                    material['diffuse_inference'] = recipe['diffuse_source']
                    material['diffuse_inference_method'] = recipe['method']
                if (not any(material['channels'].values()) and 'constant_diffuse' not in material
                        and 'unlit_constant_color' not in material and 'scene_recipe' not in material):
                    self.issue(material['source'], 'No supported named Material v1 texture parameters; neutral fallback')
            except ValueError as error:
                self.issue(material['source'], error)
        return name

    def mesh(self, key):
        name = self.filename(key, '')
        if name not in self.meshes:
            filename = name + '.obj'
            data = self.call(key[0], '--mesh', key[1], '--property-offset', 4, '--output', self.output / filename)
            # One OBJ per section preserves material slots without relying on FBX
            # importer's OBJ group merging or generated material-name order.
            lines = (self.output / filename).read_text().splitlines(keepends=True)
            header, groups = [], []
            for line in lines:
                if line.startswith('g '):
                    groups.append([])
                elif groups:
                    groups[-1].append(line)
                else:
                    header.append(line)
            if len(groups) != len(data['sections']):
                raise ValueError('Mesh section count differs from OBJ')
            sections = []
            for i, (group, section) in enumerate(zip(groups, data['sections'])):
                if not section['triangles']:
                    continue
                file = f'{name}_s{i}.obj'
                (self.output / file).write_text(''.join(header + group))
                sections.append({'slot': i, 'file': file,
                                 'material': self.material(self.resolve(key[0], section['material_index']))})
            apply_section_material_policies(
                self.identity(key), sections,
                material_for_path=lambda path: self.material(
                    (key[0], material_index(self.load(key[0]), path))),
                material_source_for_id=lambda name: self.materials[name]['source'])
            collision = {'status': 'absent', 'hulls': []}
            body = self.resolve(key[0], data.get('body_setup', 0))
            if body:
                try:
                    record = self.load(body[0])[body[1]]
                    if record['class'] != 'Engine.RB_BodySetup' or 'error' in record:
                        raise ValueError('Invalid collision body record')
                    collision = {'status': 'supported', 'source': self.identity(body),
                                 'hulls': collision_hulls(record['data']['properties'])}
                except (ValueError, KeyError) as error:
                    collision = {'status': 'unsupported', 'hulls': [], 'reason': str(error)}
                    self.issue(self.identity(key) + ':collision', error)
            source = self.identity(key)
            # Exact Sanctuary meshes with walkable surfaces but no
            # RB_BodySetup use their render triangles as a host fallback.
            collision = scooter_frontage_collision(source, collision)
            collision = archives_floor_collision(source, collision)
            self.meshes[name] = {'source': source, 'sections': sections, 'collision': collision}
        return name

    def levels(self, persistent):
        """Persistent level plus every streamed sublevel it names, in load order."""
        levels, pending, seen = [], [persistent], set()
        while pending:
            name = pending.pop(0)
            if name.casefold() in seen:
                continue
            seen.add(name.casefold())
            records = self.load(name)
            levels.append(name)
            for record in records.values():
                if record['class'].startswith('Engine.LevelStreaming'):
                    p = props(record)
                    sublevel = p.get('PackageName')
                    if sublevel and sublevel != 'None':
                        self.package(sublevel)  # missing dependencies are fatal
                        pending.append(sublevel)
        return levels

    def decal_component_defaults(self):
        """Read local cooked CDO values; keep omitted native defaults labelled."""
        defaults = dict(UE3_DECAL_COMPONENT_DEFAULTS_UNVERIFIED)
        evidence = {name: 'UNVERIFIED public UE3 DecalComponent default'
                    for name in defaults}
        try:
            rows = self.load('Engine')
            matches = [row for row in rows.values()
                       if row['path'] == 'Default__DecalComponent'
                       and row['class'].rsplit('.', 1)[-1] == 'DecalComponent']
            if len(matches) != 1:
                return defaults, evidence
            decoded = values(self.call('Engine', '--properties', matches[0]['index'],
                                       '--property-offset', 8).get('properties', []))
            for name in defaults:
                value = decoded.get(name)
                if isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value):
                    defaults[name] = float(value)
                    evidence[name] = 'Engine.Default__DecalComponent decoded property'
        except (ValueError, KeyError, TypeError):
            pass
        return defaults, evidence

    def decal_material_definition(self, key):
        """Use one cooked decal Texture2D as diffuse plus a bounded opacity source.

        This is a deliberately narrow resource translation. It does not
        reconstruct the DecalMaterial graph; materials with more than one
        cooked texture or no single Texture2D stay unsupported.
        """
        identity = self.identity(key)
        filename = self.filename(key, '')
        record = self.load(key[0])[key[1]]
        material = {'source': identity, 'status': 'unsupported',
                    'issue': 'UNVERIFIED: decal material graph is not reconstructed'}
        if record['class'].rsplit('.', 1)[-1] != 'DecalMaterial':
            material['issue'] = 'Unsupported decal reference class: ' + record['class']
            return filename, material
        try:
            refs, opaque = cooked_texture_references(
                bytes(self.call(key[0], '--payload', key[1])), record['data'])
            textures = [self.resolve(key[0], ref) for ref in refs]
            if len(textures) != 1 or textures[0] is None:
                material['issue'] = f'Unsupported decal texture set: expected one texture, found {len(textures)}'
                return filename, material
            texture = textures[0]
            texture_record = self.load(texture[0])[texture[1]]
            if texture_record['class'].rsplit('.', 1)[-1] != 'Texture2D':
                material['issue'] = 'Unsupported decal texture class: ' + texture_record['class']
                return filename, material
            properties = props(record)
            blend_mode = properties.get('BlendMode')
            texture_property_record = self.call(
                texture[0], '--properties', texture[1], '--property-offset', 8)
            texture_properties = decoded_texture_properties(texture_property_record)
            if blend_mode == 'BLEND_Translucent':
                material['issue'] = ('Unsupported translucent decal: the single texture does not '
                                     'expose a verified opacity graph')
                return filename, material
            if (blend_mode == 'BLEND_Modulate'
                    and texture_properties.get('CompressionNoAlpha') is True):
                material['issue'] = ('Unsupported Modulate decal: Texture2D CompressionNoAlpha is true; '
                                     'RGB channel roles are unresolved')
                return filename, material
            opacity_source = decal_opacity_source(blend_mode, texture_properties)
            if opacity_source is None:
                material['issue'] = 'Unsupported decal blend mode: ' + str(blend_mode)
                return filename, material
            material.update({
                'status': 'partial_unverified',
                'texture_file': self.texture(texture, 'decal'),
                'texture_source': self.identity(texture),
                'texture_count': len(textures),
                'opaque_tail_bytes': opaque,
                'blend_mode': blend_mode,
                'texture_compression_no_alpha': texture_properties.get('CompressionNoAlpha'),
                'texture_compression_no_alpha_evidence': (
                    'Texture2D CompressionNoAlpha decoded'
                    if isinstance(texture_properties.get('CompressionNoAlpha'), bool)
                    else 'Texture2D CompressionNoAlpha was not serialized/decoded'),
                'lighting_model': properties.get('LightingModel'),
                'opacity_source': opacity_source,
                'method': 'single_texture_deferred_decal_v1',
                'issue': ('UNVERIFIED approximation: the cooked DecalMaterial resource names one '
                          'Texture2D; host uses RGB with inverse-luminance opacity for Modulate '
                          'unless CompressionNoAlpha is explicitly true, '
                          'or texture alpha for Masked; native graph and exact blend behavior '
                          'remain unreconstructed')})
            if identity in SANCTUARY_DECAL_MATERIAL_ALLOWLIST:
                material.update({
                    'method': 'grafitto_dark_ink_deferred_decal_v1',
                    'opacity_source': 'grafitto_dark_ink_mask_v1',
                    'ink_color_linear': [0.001, 0.003, 0.008],
                    'opacity_luminance_ceiling': 0.55,
                    'opacity_gain': 6.0,
                    'issue': ('UNVERIFIED host calibration: the white-backed Modulate atlas uses '
                              'inverse-luminance coverage and a constant dark navy ink color; '
                              'the native DecalMaterial graph remains unreconstructed')})
        except (ValueError, KeyError, TypeError) as error:
            material['issue'] = str(error)
        return filename, material

    def decals_manifest(self, levels):
        """Export placed DecalActor transforms, projection inputs and materials."""
        defaults, default_evidence = self.decal_component_defaults()
        decal_materials, actors, issues = {}, [], []
        actor_count = component_count = 0
        component_batches = {}
        for level in levels:
            rows = self.load(level)
            source_actors = [row for row in rows.values()
                             if row['class'].rsplit('.', 1)[-1] == 'DecalActor'
                             and row['path'].startswith('TheWorld.PersistentLevel.')]
            actor_count += len(source_actors)
            if not source_actors:
                continue
            batch = self.properties_batch(level, [row['index'] for row in source_actors],
                                          26, 'decal-actors')
            for row in source_actors:
                result = batch.get(row['index'], {})
                if 'error' in result:
                    issues.append({'source': level + ':' + row['path'], 'error': result['error']})
                    actors.append({'source': level + ':' + row['path'], 'status': 'unsupported',
                                   'issue': result['error']})
                    continue
                actor_props = values(result.get('properties', []))
                component_key = self.resolve(level, actor_props.get('Decal'))
                if component_key is None:
                    issues.append({'source': level + ':' + row['path'],
                                   'error': 'DecalActor has no resolved DecalComponent'})
                    actors.append({'source': level + ':' + row['path'], 'status': 'unsupported',
                                   'issue': 'DecalActor has no resolved DecalComponent'})
                    continue
                component = self.load(component_key[0]).get(component_key[1])
                if component is None or component['class'].rsplit('.', 1)[-1] != 'DecalComponent':
                    error = 'DecalActor component is missing or is not a DecalComponent'
                    issues.append({'source': level + ':' + row['path'], 'error': error})
                    actors.append({'source': level + ':' + row['path'], 'status': 'unsupported',
                                   'issue': error})
                    continue
                component_count += 1
                component_batches.setdefault(component_key[0], []).append(
                    (level, row, actor_props, component_key, component))

        for package, records in component_batches.items():
            component_results = self.properties_batch(
                package, [record[3][1] for record in records], 8, 'decal-components')
            for level, row, actor_props, component_key, component in records:
                source = level + ':' + row['path']
                result = component_results.get(component_key[1], {})
                if 'error' in result:
                    issues.append({'source': source, 'error': result['error']})
                    actors.append({'source': source, 'status': 'unsupported',
                                   'component_source': package + ':' + component['path'],
                                   'issue': result['error']})
                    continue
                component_props = values(result.get('properties', []))
                material_key = self.resolve(package, component_props.get('DecalMaterial'))
                material_name = None
                material_source = None
                if material_key is not None:
                    material_name = self.filename(material_key, '')
                    if material_name not in decal_materials:
                        material_name, definition = self.decal_material_definition(material_key)
                        decal_materials[material_name] = definition
                    definition = decal_materials[material_name]
                    material_source = self.identity(material_key)

                dimensions, dimension_evidence = {}, {}
                for property_name, field_name in (
                        ('Width', 'width'), ('Height', 'height'), ('Thickness', 'thickness'),
                        ('NearPlane', 'near_plane'), ('FarPlane', 'far_plane'),
                        ('FieldOfView', 'field_of_view')):
                    if property_name in component_props:
                        value = component_props[property_name]
                        evidence = 'DecalComponent instance property'
                    else:
                        value = defaults[property_name]
                        evidence = default_evidence[property_name]
                    if (not isinstance(value, (int, float)) or isinstance(value, bool)
                            or not math.isfinite(value) or value < 0):
                        raise ValueError(f'Invalid decal projection dimension {property_name} on {source}')
                    dimensions[field_name] = float(value)
                    dimension_evidence[field_name] = evidence

                location = vector(actor_props.get('Location', {}))
                rotation = [actor_props.get('Rotation', {}).get(axis, 0) * 360 / 65536
                            for axis in ('Pitch', 'Yaw', 'Roll')]
                projection_basis = {name: vector(component_props.get(name, {}))
                                    for name in ('HitLocation', 'HitNormal', 'HitTangent', 'HitBinormal')}
                supported = (material_name is not None
                             and decal_materials[material_name]['status'] == 'partial_unverified'
                             and material_source in SANCTUARY_DECAL_MATERIAL_ALLOWLIST)
                excluded = (material_name is not None
                            and decal_materials[material_name]['status'] == 'excluded_default_deny')
                actor_record = {
                    'source': source, 'level': level,
                    'component_source': package + ':' + component['path'],
                    'material': material_name, 'material_source': material_source,
                    'transform': {'location': location, 'rotation': rotation},
                    'projection_basis': projection_basis,
                    'dimensions': dimensions, 'dimension_evidence': dimension_evidence,
                    'status': ('partial_unverified' if supported else
                               'excluded_default_deny' if excluded else 'unsupported'),
                    'projection_policy': ('UE3 HitNormal/HitTangent/HitBinormal map to UE5 local X/Y/Z; '
                                          'the UE5 center shifts inward by NearPlane plus half the bounded '
                                          'FarPlane-NearPlane depth (128 cm cap, UNVERIFIED calibration)'),
                    'issue': ('UNVERIFIED UE3 decal projection and blend are approximated by a UE5 deferred decal'
                              if supported else
                              DECAL_DEFAULT_DENY_REASON if excluded else
                              'No supported single-texture DecalMaterial resource')}
                if excluded:
                    actor_record['exclusion_reason'] = DECAL_DEFAULT_DENY_REASON
                actors.append(actor_record)

        candidate_materials = [material for material in decal_materials.values()
                               if material.get('status') == 'partial_unverified']
        for material in candidate_materials:
            if material.get('source') not in SANCTUARY_DECAL_MATERIAL_ALLOWLIST:
                material.update({'status': 'excluded_default_deny',
                                 'exclusion_reason': DECAL_DEFAULT_DENY_REASON,
                                 'issue': DECAL_DEFAULT_DENY_REASON})
        # Actors were classified before the material table's final deny pass;
        # apply that decision from exact source identity to every placement.
        for actor in actors:
            material = decal_materials.get(actor.get('material'), {})
            if material.get('status') == 'excluded_default_deny':
                actor.update({'status': 'excluded_default_deny',
                              'exclusion_reason': DECAL_DEFAULT_DENY_REASON,
                              'issue': DECAL_DEFAULT_DENY_REASON})

        supported = sum(item.get('status') == 'partial_unverified' for item in actors)
        excluded = sum(item.get('status') == 'excluded_default_deny' for item in actors)
        unique_supported_materials = sum(item.get('status') == 'partial_unverified'
                                         for item in decal_materials.values())
        unique_excluded_materials = sum(item.get('status') == 'excluded_default_deny'
                                        for item in decal_materials.values())
        return {'status': 'partial_unverified',
                'policy': 'round3_default_deny_graffitto_only_v1',
                'projection_policy': ('UE3 HitNormal/HitTangent/HitBinormal map to UE5 local X/Y/Z; '
                                      'graffiti uses source Width/Height, shifts the centered UE5 volume '
                                      'inward by NearPlane plus half the bounded FarPlane-NearPlane depth '
                                      '(128 cm cap, UNVERIFIED calibration)'),
                'census': {'actors': actor_count, 'components': component_count,
                           'supported_import_candidates': supported,
                           'excluded_import_candidates': excluded,
                           'unsupported': len(actors) - supported - excluded,
                           'unique_materials': len(decal_materials),
                           'supported_materials': unique_supported_materials,
                           'excluded_materials': unique_excluded_materials,
                           'unresolved_materials': (len(decal_materials) - unique_supported_materials
                                                   - unique_excluded_materials)},
                'materials': decal_materials, 'actors': actors, 'issues': issues,
                'defaults_status': 'local CDO values plus labelled UNVERIFIED defaults'}

    def decal_like_static_meshes(self, placements):
        """List explicitly named decal-like placements kept on the mesh path."""
        result = []
        for mesh_name, mesh in self.meshes.items():
            identity = mesh.get('source')
            if identity not in DECAL_LIKE_STATIC_MESH_IDENTITIES:
                continue
            instances = [item for item in placements if item.get('mesh') == mesh_name]
            by_level = {}
            material_ids = set()
            for instance in instances:
                by_level[instance['level']] = by_level.get(instance['level'], 0) + 1
                materials = instance.get('materials') or [
                    section.get('material') for section in mesh.get('sections', [])]
                material_ids.update(value for value in materials if value)
            material_sources = sorted(self.materials[item]['source'] for item in material_ids)
            result.append({'mesh': mesh_name, 'source': identity,
                           'placements': len(instances), 'placements_by_level': by_level,
                           'material_sources': material_sources,
                           'status': 'already_imported_as_static_mesh',
                           'evidence': ('The package records a planar mesh with water-effect material references; '
                                        'existing mesh export carries its geometry, transform and material slots. '
                                        'Decal projection conversion is not confirmed for this mesh.')})
        return result

    def light_component_class_defaults(self):
        """Decode the installed Engine CDO values used when an instance omits a field."""
        exports = self.export_rows('Engine')
        result = {}
        for path, required in (
                ('Default__LightComponent', {'Brightness', 'LightColor'}),
                ('Default__PointLightComponent', {'Radius'}),
                ('Default__SpotLightComponent', {'OuterConeAngle'})):
            row = next((item for item in exports.values() if item['path'] == path), None)
            if row is None:
                raise ValueError('Engine package is missing ' + path)
            record = self.call('Engine', '--properties', row['index'], '--property-offset', 4)
            if record.get('trailing_bytes') != 0:
                raise ValueError(path + ' property stream is incomplete')
            properties = values(record.get('properties', []))
            if not required <= set(properties):
                raise ValueError(path + ' is missing required light defaults')
            result[path] = properties
        return result

    def static_light_collection_records(self, level, rows, class_defaults, issues):
        """Read collection references and their placed point/spot component values."""
        exports = self.export_rows(level)
        collections = [row for row in rows.values()
                       if row['class'].rsplit('.', 1)[-1] == 'StaticLightCollectionActor'
                       and row['path'].startswith('TheWorld.PersistentLevel.')]
        component_records, membership_errors = [], []
        for actor in collections:
            source = level + ':' + actor['path']
            try:
                data = self.call(level, '--properties', actor['index'], '--property-offset', 4,
                                 '--array-schema', self.schema)
                properties = data.get('properties', [])
                light_list = next((item for item in properties if item.get('name') == 'LightComponents'), None)
                if (light_list is None or light_list.get('status') != 'decoded'
                        or light_list.get('element_type') != 'ObjectProperty'
                        or not isinstance(light_list.get('value'), list)):
                    raise ValueError('LightComponents object-reference array is not decoded')
                references = light_list['value']
                if light_list.get('element_count') != len(references):
                    raise ValueError('LightComponents count does not match its decoded references')
                if data.get('trailing_bytes') != len(references) * 64:
                    raise ValueError('StaticLightCollectionActor tail does not match its component count')
                # Stage this array's records locally so a malformed element
                # cannot leave earlier lights of the same actor in the output.
                staged_records = []
                for reference in references:
                    component = rows.get(reference.get('index'))
                    if component is None:
                        raise ValueError('LightComponents contains an unresolved component reference')
                    component_kind = component['class'].rsplit('.', 1)[-1]
                    if component_kind not in ('PointLightComponent', 'SpotLightComponent'):
                        raise ValueError('LightComponents contains an unsupported light component class')
                    if component.get('outer') != actor['index']:
                        raise ValueError('Light component outer does not match collection owner')
                    staged_records.append((component, component_kind))
                component_records.extend(staged_records)
            except (ValueError, KeyError, TypeError) as error:
                issues.append({'source': source, 'error': str(error)})
                membership_errors.append(source)

        seen = set()
        unique = []
        for component, kind in component_records:
            if component['index'] in seen:
                issues.append({'source': level + ':' + component['path'],
                               'error': 'Light component appears in more than one collection array'})
                continue
            seen.add(component['index'])
            unique.append((component, kind))
        if not unique:
            return [], {'collection_actors': len(collections), 'components': 0,
                        'membership_errors': membership_errors}

        component_props = self.properties_batch(level, [row['index'] for row, _ in unique],
                                                8, 'light-components')
        if level not in self.imports:
            self.imports[level] = {item['index']: item for item in self.call(level, '--imports')}

        def archetype_path(component):
            info = exports.get(component['index'], {})
            reference = info.get('archetype_index', 0)
            if reference > 0:
                return (exports.get(reference) or {}).get('path')
            if reference < 0:
                return (self.imports[level].get(reference) or {}).get('path')
            return None

        base_defaults = class_defaults['Default__LightComponent']
        point_defaults = class_defaults['Default__PointLightComponent']
        spot_defaults = class_defaults['Default__SpotLightComponent']
        records_out = []
        for component, component_class in unique:
            source = level + ':' + component['path']
            parsed = component_props.get(component['index'], {})
            if 'error' in parsed or parsed.get('trailing_bytes') != 0:
                records_out.append({'source': source,
                                    'kind': 'Point' if component_class == 'PointLightComponent' else 'Spot',
                                    'status': 'unsupported',
                                    'skip_reason': 'component_properties_incomplete'})
                continue
            instance = values(parsed.get('properties', []))
            kind = 'Point' if component_class == 'PointLightComponent' else 'Spot'
            archetype = archetype_path(component)
            standard_archetype = (archetype == ('Engine.Default__PointLight.PointLightComponent0'
                                                 if kind == 'Point'
                                                 else 'Engine.Default__SpotLight.SpotLightComponent0'))
            inherited = dict(base_defaults)
            inherited.update(point_defaults)
            if kind == 'Spot':
                inherited.update(spot_defaults)
            fields = dict(inherited if standard_archetype else {})
            fields.update(instance)
            required = {'Brightness', 'LightColor', 'Radius', 'CachedParentToWorld'}
            if kind == 'Spot':
                required.add('OuterConeAngle')
            if not required <= set(fields):
                records_out.append({'source': source, 'kind': kind, 'archetype': archetype,
                                    'status': 'unsupported',
                                    'skip_reason': 'missing_instance_or_class_default_light_value'})
                continue
            try:
                brightness = fields['Brightness']
                radius = fields['Radius']
                if (not isinstance(brightness, (int, float)) or isinstance(brightness, bool)
                        or not math.isfinite(brightness) or not isinstance(radius, (int, float))
                        or isinstance(radius, bool) or not math.isfinite(radius)):
                    raise ValueError('Light brightness/radius is not finite')
                matrix = fields['CachedParentToWorld']
                if not isinstance(matrix, dict):
                    raise ValueError('CachedParentToWorld is not a decoded matrix')
                position = [matrix['WPlane'][axis] for axis in ('X', 'Y', 'Z')]
                forward = [matrix['XPlane'][axis] for axis in ('X', 'Y', 'Z')]
                up = [matrix['ZPlane'][axis] for axis in ('X', 'Y', 'Z')]
                if any(not isinstance(value, (int, float)) or isinstance(value, bool)
                       or not math.isfinite(value) for value in position + forward + up):
                    raise ValueError('Light transform matrix contains a nonfinite value')
                light_color = byte_color_rgb(fields['LightColor'])
                light = {'source': source, 'level': level, 'kind': kind,
                         'archetype': archetype, 'archetype_defaults_used': standard_archetype,
                         'position': [float(value) for value in position],
                         'forward': [float(value) for value in forward],
                         'up': [float(value) for value in up],
                         'brightness': float(brightness), 'radius': float(radius),
                         'light_color_rgb': light_color,
                         'brightness_source': ('component instance' if 'Brightness' in instance
                                               else 'Engine.Default__LightComponent'),
                         'color_source': ('component instance' if 'LightColor' in instance
                                          else 'Engine.Default__LightComponent'),
                         'radius_source': ('component instance' if 'Radius' in instance
                                           else 'Engine.Default__PointLightComponent'),
                         'transform_source': 'CachedParentToWorld',
                         'enabled': fields.get('bEnabled', True) is not False,
                         'status': 'candidate'}
                enabled = fields.get('bEnabled', True)
                if enabled is False:
                    light.update({'status': 'unsupported', 'skip_reason': 'source_light_disabled'})
                if kind == 'Spot':
                    outer = fields['OuterConeAngle']
                    inner = fields.get('InnerConeAngle', 0.0)
                    if (not isinstance(outer, (int, float)) or isinstance(outer, bool)
                            or not math.isfinite(outer) or not 0 < outer < 90
                            or not isinstance(inner, (int, float)) or isinstance(inner, bool)
                            or not math.isfinite(inner) or inner < 0 or inner > outer):
                        raise ValueError('Spot light cone angles are outside the supported range')
                    light['outer_cone_angle'] = float(outer)
                    light['inner_cone_angle'] = float(inner)
                    light['inner_cone_source'] = ('component instance' if 'InnerConeAngle' in instance
                                                  else 'class default zero (UNVERIFIED)')
                records_out.append(light)
            except (ValueError, KeyError, TypeError) as error:
                records_out.append({'source': source, 'kind': kind, 'archetype': archetype,
                                    'status': 'unsupported',
                                    'skip_reason': 'invalid_source_light_value', 'issue': str(error)})
        return records_out, {'collection_actors': len(collections),
                             'components': len(unique),
                             'membership_errors': membership_errors}

    def lighting_manifest(self, levels):
        """Export the decoded Sanctuary light actors and explicitly unresolved inputs."""
        manifest = {'status': 'partial_unverified', 'source_levels': list(levels),
                    'host_policy': 'decoded_light_components_worldinfo_pp_with_inspection_calibration_v2',
                    'sky_light': None, 'height_fog': None, 'directional_light': None,
                    'post_process_volumes': [], 'world_info': [],
                    'placed_actor_counts': {name: 0 for name in (
                        'PointLight', 'SpotLight', 'DirectionalLight', 'DominantDirectionalLight',
                        'SkyLightToggleable', 'HeightFog', 'PostProcessVolume')},
                    'static_light_collections': {'actors': 0, 'point_components': 0,
                                                 'spot_components': 0},
                    'light_component_owners': {'PointLightComponent': {},
                                               'SpotLightComponent': {}},
                    'static_light_import': {'status': 'not decoded', 'components': [],
                                             'selection_policy': SANCTUARY_LIGHT_SELECTION,
                                             'counts': {'source_components': 0, 'imported': 0,
                                                        'skipped': 0, 'skip_reasons': {}}},
                    'issues': []}

        def read_properties(level, record, offset):
            result = self.call(level, '--properties', record['index'], '--property-offset', offset)
            if result.get('trailing_bytes') != 0:
                raise ValueError('Property decode did not reach the object end')
            return values(result.get('properties', []))

        def read_component(level, reference, offset=8):
            key = self.resolve(level, reference)
            if key is None:
                raise ValueError('Light actor has no serialized component reference')
            component_package, component_index = key
            component = self.load(component_package).get(component_index)
            if component is None:
                raise ValueError('Light component reference is unresolved')
            props_value = read_properties(component_package, component, offset)
            return component, props_value

        for level in levels:
            rows = self.load(level)
            class_counts = {}
            owner_counts = {'PointLightComponent': {}, 'SpotLightComponent': {}}
            for row in rows.values():
                cls = row['class'].rsplit('.', 1)[-1]
                class_counts[cls] = class_counts.get(cls, 0) + 1
                if cls in manifest['placed_actor_counts']:
                    manifest['placed_actor_counts'][cls] += 1
                if cls in manifest['light_component_owners']:
                    owner = rows.get(row.get('outer'), {})
                    owner_class = owner.get('class', 'unresolved').rsplit('.', 1)[-1]
                    owners = owner_counts[cls]
                    owners[owner_class] = owners.get(owner_class, 0) + 1
            manifest['static_light_collections']['actors'] += class_counts.get('StaticLightCollectionActor', 0)
            for class_name, component_name in (('PointLightComponent', 'point_components'),
                                               ('SpotLightComponent', 'spot_components')):
                owners = manifest['light_component_owners'][class_name]
                for owner_class, count in owner_counts[class_name].items():
                    owners[owner_class] = owners.get(owner_class, 0) + count
                manifest['static_light_collections'][component_name] += owner_counts[class_name].get(
                    'StaticLightCollectionActor', 0)
            for row in rows.values():
                cls = row['class'].rsplit('.', 1)[-1]
                if cls not in ('SkyLightToggleable', 'HeightFog', 'DominantDirectionalLight',
                               'PostProcessVolume', 'WorldInfo'):
                    continue
                source = level + ':' + row['path']
                try:
                    actor = read_properties(level, row, 26)
                    location = vector(actor.get('Location', {}))
                    if cls == 'SkyLightToggleable':
                        component, data = read_component(level, actor.get('LightComponent'))
                        light_color = data.get('LightColor')
                        lower_color = data.get('LowerColor')
                        manifest['sky_light'] = {
                            'source': source, 'component': level + ':' + component['path'],
                            'location': location,
                            'light_color_rgb': byte_color_rgb(light_color),
                            'lower_color_rgb': byte_color_rgb(lower_color),
                            'light_brightness': data.get('Brightness'),
                            'light_brightness_status': ('decoded' if 'Brightness' in data
                                                        else 'not serialized on this component'),
                            'lower_brightness': data.get('LowerBrightness'),
                            'precomputed_lighting_valid': data.get('bPrecomputedLightingIsValid'),
                            'component_property_offset': 8,
                            'status': 'partial_unverified'}
                    elif cls == 'HeightFog':
                        component, data = read_component(level, actor.get('Component'))
                        manifest['height_fog'] = {
                            'source': source, 'component': level + ':' + component['path'],
                            'location': location, 'height': data['Height'], 'density': data['Density'],
                            'light_brightness': data['LightBrightness'],
                            'light_color_rgb': byte_color_rgb(data['LightColor']),
                            'start_distance': data['StartDistance'],
                            'extinction_distance': data['ExtinctionDistance'],
                            'component_property_offset': 8, 'status': 'partial_unverified'}
                    elif cls == 'DominantDirectionalLight':
                        entry = {'source': source, 'location': location,
                                 'rotation': [actor.get('Rotation', {}).get(k, 0) * 360 / 65536
                                              for k in ('Pitch', 'Yaw', 'Roll')],
                                 'status': 'actor_transform_only_unverified'}
                        try:
                            component_key = self.resolve(level, actor.get('LightComponent'))
                            if component_key is None:
                                raise ValueError('Directional light has no serialized component reference')
                            component_package, component_index = component_key
                            component = self.load(component_package).get(component_index)
                            if (component is None or
                                    component['class'].rsplit('.', 1)[-1] != 'DominantDirectionalLightComponent'):
                                raise ValueError('Directional light component class is unresolved')
                            property_offset, data, _ = self.anchored_properties(
                                component_package, component_index, 'LightmassSettings',
                                {'Brightness', 'LightColor'})
                            entry.update({
                                'component': component_package + ':' + component['path'],
                                'component_property_offset': property_offset,
                                'component_property_anchor': 'LightmassSettings',
                                'light_brightness': data['Brightness'],
                                'light_color_rgb': byte_color_rgb(data['LightColor']),
                                'status': 'component_light_values_decoded_unverified'})
                        except ValueError as error:
                            entry['component_error'] = str(error)
                        manifest['directional_light'] = entry
                    elif cls == 'PostProcessVolume':
                        manifest['post_process_volumes'].append({
                            'source': source, 'location': location,
                            'priority': actor.get('Priority'),
                            'settings': decoded_settings(actor.get('Settings')),
                            'b_unbound': actor.get('bUnbound'),
                            'b_unbound_status': ('decoded' if 'bUnbound' in actor
                                                 else 'not serialized on this actor'),
                            'bounds_status': 'brush bounds not decoded',
                            'status': 'settings_only_unverified'})
                    else:
                        default_settings = decoded_settings(actor.get('DefaultPostProcessSettings'))
                        manifest['world_info'].append({
                            'source': source,
                            'default_post_process_settings_present': 'DefaultPostProcessSettings' in actor,
                            'default_post_process_settings': default_settings,
                            'property_offset': 26, 'status': 'decoded_properties_only_unverified'})
                except (ValueError, KeyError, TypeError) as error:
                    manifest['issues'].append({'source': source, 'error': str(error)})
        if manifest['directional_light'] is not None and manifest['directional_light'].get('component_error'):
            manifest['issues'].append({'source': manifest['directional_light']['source'] + ':LightComponent',
                                       'error': manifest['directional_light']['component_error']})
        if manifest['static_light_collections']['actors']:
            try:
                class_defaults = self.light_component_class_defaults()
                source_components = []
                membership = []
                for level in levels:
                    rows = self.load(level)
                    if not any(row['class'].rsplit('.', 1)[-1] == 'StaticLightCollectionActor'
                               and row['path'].startswith('TheWorld.PersistentLevel.')
                               for row in rows.values()):
                        continue
                    level_components, level_membership = self.static_light_collection_records(
                        level, rows, class_defaults, manifest['issues'])
                    source_components.extend(level_components)
                    membership.append({'level': level, **level_membership})
                selected_components, counts = select_sanctuary_light_components(source_components)
                expected_count = (manifest['static_light_collections']['point_components']
                                  + manifest['static_light_collections']['spot_components'])
                if len(source_components) != expected_count:
                    manifest['issues'].append({
                        'source': 'StaticLightCollectionActor.LightComponents',
                        'error': f'Component arrays decoded {len(source_components)} of {expected_count} counted components'})
                manifest['static_light_import'] = {
                    'status': 'partial_unverified',
                    'policy': 'collection_light_decode_threshold_spatial_quota_cap_v1',
                    'selection_policy': SANCTUARY_LIGHT_SELECTION,
                    'membership': membership,
                    'counts': counts,
                    'skip_reason_counts': dict(sorted(Counter(
                        item.get('skip_reason') for item in selected_components
                        if item.get('skip_reason')).items())),
                    'class_default_sources': [
                        'Engine:Default__LightComponent',
                        'Engine:Default__PointLightComponent',
                        'Engine:Default__SpotLightComponent'],
                    'class_defaults_used_only_for_standard_point_spot_archetypes': True,
                    'components': selected_components,
                }
            except (ValueError, KeyError, TypeError) as error:
                manifest['issues'].append({'source': 'StaticLightCollectionActor.LightComponents',
                                           'error': str(error)})
        return manifest

    def build(self, persistent):
        self.load('Startup')
        levels = self.levels(persistent)
        actors, camera, excluded, owner_components = [], None, [], set()
        matinee_first_key_applied = 0
        matinee_first_key = getattr(self, 'matinee_first_key', False)
        for level in levels:
            print(f'Preparing {level}', flush=True)
            records = self.load(level)
            placements = {}
            for record in records.values():
                if record['class'] == 'Engine.StaticMeshCollectionActor':
                    p = props(record)
                    refs = p.get('StaticMeshComponents', [])
                    payload = bytes(self.call(level, '--payload', record['index']))
                    transforms = collection_transforms(payload, record['data'], len(refs))
                    for ref, pose in zip(refs, transforms):
                        if ref['index'] in placements:
                            raise ValueError('Duplicate collection component')
                        placements[ref['index']] = pose
                elif 'PlayerStart' in record['class'] and camera is None:
                    camera = transform(props(record))
                    camera['location'][2] += 100
            for record in records.values():
                if record['class'] != 'Engine.StaticMeshComponent' or not record['path'].startswith('TheWorld.PersistentLevel.'):
                    continue
                try:
                    p = props(record)
                    key = self.resolve(level, p.get('StaticMesh', 0))
                    if not key:
                        continue
                    pose = placements.get(record['index'])
                    if pose is not None:
                        pose = dict(pose, scale=collection_scale(p, pose['scale']))
                    owner = records.get(record['outer'])
                    owner_props = {}
                    if pose is None:
                        if owner is None:
                            self.issue(level + ':' + record['path'], 'Unsupported component owner')
                            continue
                        owner_props = self.owner_properties(level, owner)
                        disposition = owner_disposition(owner['class'], owner_props, record['path'])
                        if disposition is None:
                            self.issue(level + ':' + record['path'], 'Unsupported component owner')
                            continue
                        if disposition == 'placed' and owner['class'] in DEDUPLICATED_OWNER_CLASSES:
                            signature = (level, owner['index'], key,
                                         json.dumps(p.get('Materials', []), sort_keys=True),
                                         json.dumps(transform(p, True)),
                                         json.dumps([p.get(name) for name in DUPLICATE_SIGNATURE_PROPERTIES]))
                            if signature in owner_components:
                                disposition = 'duplicate_component'
                            owner_components.add(signature)
                        if disposition != 'placed':
                            excluded.append({'level': level, 'source': record['path'],
                                             'owner_class': owner['class'],
                                             'mesh': self.identity(key), 'reason': disposition})
                            continue
                        pose = {'actor': transform(owner_props), 'component': transform(p, True)}
                    if matinee_first_key:
                        pose, applied = apply_matinee_first_key_pose(record['path'], pose)
                        matinee_first_key_applied += int(applied)
                    overrides = [self.material(self.resolve(level, ref)) for ref in p.get('Materials', [])]
                    mesh_identity = self.identity(key)
                    mesh_name = self.mesh(key)
                    effective_materials = [overrides[i] if i < len(overrides) and overrides[i]
                                           else section['material']
                                           for i, section in enumerate(self.meshes[mesh_name]['sections'])]
                    is_native_skybox = native_skybox_placement(
                        mesh_identity, effective_materials, self.materials)
                    if is_native_skybox:
                        # The observed dome's faces point outward while the
                        # player camera is inside it. Preserve the source mesh
                        # and use the narrow host-side two-sided policy needed
                        # for an interior visual shell.
                        for material_name in effective_materials:
                            if material_name in self.materials:
                                self.materials[material_name]['two_sided'] = True
                    source_hidden = bool(p.get('HiddenGame')) or bool(
                        owner_props.get('bHidden') if owner_props else
                        owner and props(owner).get('bHidden'))
                    actor = {'source': record['path'], 'level': level, 'mesh': mesh_name,
                             'transform': pose, 'materials': overrides, 'static': True,
                             'collision_enabled': placement_collision(level, record['path'], p),
                             'native_skybox': is_native_skybox,
                             'native_skybox_source': mesh_identity if is_native_skybox else None,
                             # Observed component HiddenGame or owner bHidden,
                             # kept so refresh_materials.py can re-apply the
                             # same hidden-visual rule offline.
                             'source_hidden': source_hidden,
                             'hidden_visual': hidden_visual_mesh(
                                 mesh_identity, effective_materials, self.materials,
                                 record['path'], source_hidden)}
                    apply_outer_shell_policy(actor, mesh_identity, self.meshes[mesh_name]['sections'],
                                             self.materials, getattr(self, 'outer_shell', False))
                    actors.append(actor)
                except ValueError as error:
                    self.issue(level + ':' + record['path'], error)
        if not actors:
            raise ValueError('No static mesh placements loaded')
        if matinee_first_key and matinee_first_key_applied != 1:
            raise ValueError('Matinee first-key experiment expected exactly one '
                             f'{MATINEE_FIRST_KEY_COMPONENT}, found '
                             f'{matinee_first_key_applied}')
        # The lighting policy is Sanctuary-only; other maps keep the pre-round-1
        # scene.json shape with no 'lighting' key.
        lighting = self.lighting_manifest(levels) if sanctuary_lighting_enabled(persistent) else None
        decals = self.decals_manifest(levels)
        decals['static_mesh_candidates'] = self.decal_like_static_meshes(actors)
        decals['census']['static_mesh_candidate_placements'] = sum(
            item['placements'] for item in decals['static_mesh_candidates'])
        result = {'schema': 1, 'map': persistent, 'levels': levels, 'actors': actors,
                  'package_scope': 'base_and_dlc' if getattr(self, 'include_dlc', False) else 'base',
                  'meshes': self.meshes, 'materials': self.materials, 'camera': camera,
                  'decals': decals, 'excluded_components': excluded,
                  'issues': self.issues, 'dynamic_policy': 'frozen', 'visual_validation': 'pending',
                  'collision_policy': 'observed_convex_and_box_v1',
                  'matinee_first_key_policy': (
                      'relative_to_initial_first_key_experiment_v1'
                      if matinee_first_key else 'serialized_placement'),
                  'matinee_first_key_applied': matinee_first_key_applied,
                  'outer_shell_policy': ('mesh_default_for_teleported_overrides_v1'
                                         if getattr(self, 'outer_shell', False) else 'placed_overrides')}
        if lighting is not None:
            result['lighting'] = lighting
        temporary = self.output / 'scene.json.tmp'
        temporary.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
        temporary.replace(self.output / 'scene.json')
        print(json.dumps({'levels': levels, 'placements': len(actors), 'excluded': len(excluded),
                          'meshes': len(self.meshes),
                          'materials': len(self.materials), 'issues': len(self.issues),
                          'decals': decals['census']}), flush=True)
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--map', default='Ash_P')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--include-dlc', action='store_true', help='Index installed DLC packages and named texture caches')
    parser.add_argument('--outer-shell', action='store_true',
                        help='Render the observed outer hull meshes with their mesh-default materials '
                             'instead of the unrecoverable masked _Teleported overrides')
    parser.add_argument('--matinee-first-key', action='store_true',
                        help='Experimentally place Sanctuary InterpActor_29 at its observed '
                             'RelativeToInitial first move key')
    args = parser.parse_args()
    if not (args.game / 'Binaries/Win32/Borderlands2.exe').is_file():
        parser.error('An installed Borderlands 2 is required')
    if not args.map.endswith('_P'):
        parser.error('Start with a persistent _P package')
    if args.matinee_first_key and args.map.casefold() != 'sanctuary_p':
        parser.error('--matinee-first-key is only supported for Sanctuary_P')
    if not args.map.replace('_', '').isascii() or not args.map.replace('_', '').isalnum():
        parser.error('Map names must contain only ASCII letters, digits and underscores')
    if args.output is None:
        args.output = Path('local') / args.map[:-2].lower()
    Scene(args.reader, args.game, args.output, include_dlc=args.include_dlc,
          outer_shell=args.outer_shell,
          matinee_first_key=args.matinee_first_key).build(args.map)


if __name__ == '__main__':
    main()
