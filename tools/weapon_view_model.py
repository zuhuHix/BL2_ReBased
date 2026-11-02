"""Write per-recipe first-person weapon data read from installed BL2 packages.

The view manifest is generated under ignored local/ folders. It keeps the
package-owned weapon type values, selected-fragment socket transforms, and the
default unconditioned reload animation name together for the host importer.

  python tools/weapon_view_model.py --game "<BL2 install>" --dir local/items/slice
"""
import argparse
import json
import math
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import weapon_recipe  # noqa: E402


SCHEMA_ADDITIONS = [
    'FirstPersonMeshFOV=FloatProperty',
    'PlayerViewOffset=StructProperty:Vector',
    'ReloadCompletePercent=FloatProperty',
    'ZoomedEndFOV=FloatProperty',
    'ZoomTime=FloatProperty',
    'ZoomSocketPreferred=NameProperty',
    'ZoomSocketFallback=NameProperty',
    'WeaponAnimSet=ObjectProperty',
    'WeaponReloadAnimations=StructProperty:ConditionalAnimationData',
    'FirstPersonParticle=ObjectProperty',
    'AnimSet=ObjectProperty',
    'CameraAnim=ObjectProperty',
    'AnimationName=NameProperty',
    'Expression=ObjectProperty',
    'MuzzleFlashSocket=NameProperty',
    'MuzzleFlashDuration=FloatProperty',
    'MuzzleFlashPSTemplates=ObjectProperty',
    'MuzzleFlashLightTemplate=ObjectProperty',
    'ShellCasingSocket=NameProperty',
    'ShellCasingPSCTemplate=ObjectProperty',
    'TracerColor=StructProperty:Color',
    'TracerColorParameterName=NameProperty',
    'TracerTemplate=ObjectProperty',
    'WeaponKickUp=FloatProperty',
    'WeaponKickDown=FloatProperty',
    'WeaponKickLeft=FloatProperty',
    'WeaponKickRight=FloatProperty',
    'WeaponKickSpeed=FloatProperty',
    'WeaponKickRecoveryTime=FloatProperty',
    'ZoomedRecoilAnimScale=FloatProperty',
    'GestaltMesh=ObjectProperty',
    'GestaltSkeletalMesh=ObjectProperty',
    'SocketName=NameProperty',
    'BoneName=NameProperty',
    'RelativeLocation=StructProperty:Vector',
    'RelativeRotation=StructProperty:Rotator',
    'RelativeScale=StructProperty:Vector',
]

# Camera-local translation added after the selected cooked eye socket is
# centered. The pistol's fallback body socket does not coincide with the
# visible sight line, so this is a separate frame fit.
ZOOM_POSITION_FITS_CM = {
    'slice_pistol': (0.0, -3.2, 11.0),
}


def zoom_position_fit_cm(recipe_id):
    offset = ZOOM_POSITION_FITS_CM.get(recipe_id, (0.0, 0.0, 0.0))
    source = ('UNVERIFIED fit to lane E ADS frame' if recipe_id in ZOOM_POSITION_FITS_CM
              else 'no recipe-specific fit')
    return [float(value) for value in offset], source


def merge_type_properties(class_defaults, type_properties):
    """Apply explicit cooked type tags over the package class defaults."""
    result = dict(class_defaults)
    result.update({key: value for key, value in type_properties.items() if value is not None})
    return result


def reload_animation_names(value):
    """Return decoded animation-name/expression pairs from the type array."""
    result = []
    if not isinstance(value, list):
        return result
    for entry in value:
        if not isinstance(entry, dict):
            continue
        name = entry.get('AnimationName')
        if name:
            result.append({
                'animation_name': str(name),
                'expression': entry.get('Expression'),
                'anim_set': entry.get('AnimSet'),
            })
    return result


def choose_default_reload_animation(animations):
    """Choose the last unconditioned type entry; condition evaluation is not hosted yet."""
    for animation in reversed(animations):
        if not animation.get('expression'):
            return animation.get('animation_name')
    return None


def choose_reload_animation(animations, card_reload_time=None, fast_threshold=None):
    """Apply the measured fast-reload threshold to the type's named fast clip.

    The type stores expressions rather than an exported evaluator. Mapping the
    fast threshold to entries named ``Fast`` is a bounded host interpretation;
    it remains UNVERIFIED until compared against the original game's animation.
    """
    try:
        reload_time = float(card_reload_time)
        threshold = float(fast_threshold)
    except (TypeError, ValueError):
        return choose_default_reload_animation(animations)
    if not math.isfinite(reload_time) or not math.isfinite(threshold):
        return choose_default_reload_animation(animations)
    if reload_time <= threshold:
        for animation in reversed(animations):
            name = str(animation.get('animation_name') or '')
            if animation.get('expression') and 'fast' in name.casefold():
                return name
    return choose_default_reload_animation(animations)


def fast_reload_threshold(value):
    """Read a constant WeaponMaxFastReloadAnimTime from measured type entries."""
    if not isinstance(value, list):
        return None
    for entry in value:
        if not isinstance(entry, dict) or not str(entry.get('Attribute', '')).endswith(
                'WeaponMaxFastReloadAnimTime'):
            continue
        base = entry.get('BaseValue')
        if not isinstance(base, dict) or base.get('BaseValueAttribute') or base.get('InitializationDefinition'):
            continue
        try:
            constant = float(base['BaseValueConstant'])
            scale = float(base.get('BaseValueScaleConstant', 1.0))
        except (KeyError, TypeError, ValueError):
            continue
        result = constant * scale
        if math.isfinite(result) and result > 0:
            return result
    return None


def reload_play_rate(clip_length, reload_time):
    """Scale a clip so its duration spans the evaluated item ReloadTime."""
    clip_length = float(clip_length)
    reload_time = max(0.1, float(reload_time))
    if not math.isfinite(clip_length) or clip_length <= 0:
        return 1.0
    return clip_length / reload_time


def recoil_animation_weight(kick, legacy_weight=0.45):
    """UNVERIFIED host mapping: scale the old clip weight by mean absolute type kick."""
    values = [abs(float(kick.get(name, 0.0))) for name in ('up', 'down', 'left', 'right')]
    return max(0.0, min(1.0, float(legacy_weight) * (sum(values) / len(values))))


def _measurement_value(raw):
    value = raw.strip().strip('`').strip()
    if not value or value.lower() in ('null', 'none'):
        return None
    try:
        return json.loads(value)
    except json.JSONDecodeError:
        try:
            return float(value)
        except ValueError:
            return value


def parse_live_measurements(markdown):
    """Read selected live fields from the lane E measurement tables.

    The markdown remains ignored local evidence; this parser only gives those
    measurements precedence when regenerating the ignored runtime manifest.
    For ZoomedEndFOV it prefers the live item value, which includes sight/part
    zoom settings, over the type-definition value.
    """
    sections = {}
    current = None
    for line in markdown.splitlines():
        if line.startswith('### '):
            current = line[4:].split(':', 1)[0].strip()
            sections.setdefault(current, {})
            continue
        if not current or not line.startswith('|'):
            continue
        cells = [cell.strip() for cell in line.split('|')[1:-1]]
        if len(cells) < 3:
            continue
        name, raw, source = cells[:3]
        if name == 'ZoomedEndFOV':
            if 'live read, unheld backpack weapon' not in source:
                continue
            sections[current]['zoom_fov'] = _measurement_value(raw)
            sections[current]['zoom_fov_source'] = 'live measurement: unheld weapon ZoomedEndFOV'
            continue
        if 'live read, WeaponTypeDefinition' not in source:
            continue
        value = _measurement_value(raw)
        if name == 'AnimThresholdValues':
            threshold = fast_reload_threshold(value)
            if threshold is not None:
                sections[current]['fast_reload_threshold'] = threshold
                sections[current]['fast_reload_threshold_source'] = 'live measurement: WeaponMaxFastReloadAnimTime'
            continue
        if name == 'ReloadCompletePercent':
            try:
                fraction = float(value)
            except (TypeError, ValueError):
                continue
            if math.isfinite(fraction) and 0.0 <= fraction <= 1.0:
                sections[current]['reload_complete_percent'] = fraction
                sections[current]['reload_complete_percent_source'] = 'live measurement: WeaponTypeDefinition'
            continue
        if value is None:
            if name in ('TracerTemplate', 'ShellCasingPSCTemplate'):
                field = 'tracer_template' if name == 'TracerTemplate' else 'shell_casing_template'
                sections[current][field] = None
                sections[current][f'{field}_source'] = 'live measurement: WeaponTypeDefinition'
            continue
        if name == 'PlayerViewOffset':
            sections[current]['player_view_offset'] = [float(value[k]) for k in ('X', 'Y', 'Z')]
        elif name == 'FirstPersonMeshFOV':
            sections[current]['first_person_fov'] = float(value)
        elif name == 'ZoomTime':
            sections[current]['zoom_blend_time'] = float(value)
        elif name == 'ZoomedRecoilAnimScale':
            sections[current]['zoomed_recoil_animation_scale'] = float(value)
        elif name in ('WeaponKickUp', 'WeaponKickDown', 'WeaponKickLeft', 'WeaponKickRight'):
            channel = name.removeprefix('WeaponKick').lower()
            sections[current].setdefault('weapon_kick', {})[channel] = float(value)
        elif name == 'MuzzleFlashDuration':
            sections[current]['muzzle_flash_duration'] = float(value)
        elif name == 'MuzzleFlashSocket':
            sections[current]['muzzle_socket'] = str(value)
            sections[current]['muzzle_socket_source'] = 'live measurement: WeaponTypeDefinition'
        elif name == 'ShellCasingSocket':
            sections[current]['shell_casing_socket'] = str(value)
            sections[current]['shell_casing_socket_source'] = 'live measurement: WeaponTypeDefinition'
        elif name == 'MuzzleFlashPSTemplates':
            sections[current]['muzzle_flash_collection'] = value
        elif name == 'TracerColor' and isinstance(value, dict):
            sections[current]['tracer_color'] = [int(value.get(k, d)) for k, d in
                (('R', 200), ('G', 200), ('B', 200), ('A', 255))]
        elif name == 'TracerTemplate':
            sections[current]['tracer_template'] = value
        elif name == 'ShellCasingPSCTemplate':
            sections[current]['shell_casing_template'] = value
    return sections


def apply_live_measurement(entry, measurement):
    """Overlay live values on one cooked projection, retaining per-field provenance."""
    result = dict(entry)
    for field in ('player_view_offset', 'first_person_fov', 'zoom_fov', 'zoom_blend_time',
                  'zoomed_recoil_animation_scale', 'muzzle_flash_duration', 'tracer_color',
                  'tracer_template', 'shell_casing_template', 'muzzle_socket', 'shell_casing_socket',
                  'reload_complete_percent',
                  'muzzle_flash_collection', 'fast_reload_threshold'):
        if field in measurement:
            result[field] = measurement[field]
            result[f'{field}_source'] = measurement.get(f'{field}_source', 'live measurement')
    if 'weapon_kick' in measurement:
        result['weapon_kick'] = {**result.get('weapon_kick', {}), **measurement['weapon_kick']}
        result['weapon_kick_sources'] = {key: 'live measurement: WeaponTypeDefinition'
                                         for key in result['weapon_kick']}
        result['recoil_animation_weight'] = recoil_animation_weight(result['weapon_kick'])
        result['recoil_animation_weight_source'] = (
            'UNVERIFIED host mapping from live WeaponKick channels')
    if 'zoom_fov' in measurement:
        result['zoom_fov_source'] = measurement['zoom_fov_source']
    result['tracer_enabled'] = bool(result.get('tracer_template'))
    result['shell_casing_enabled'] = bool(result.get('shell_casing_template'))
    result['tracer_enabled_source'] = result.get('tracer_template_source', 'cooked type field')
    result['shell_casing_enabled_source'] = result.get('shell_casing_template_source', 'cooked type field')
    return result


def angle_to_degrees(value):
    """Convert an Unreal rotator component stored as a signed 16-bit turn."""
    units = ((int(value) + 32768) % 65536) - 32768
    return units * (360.0 / 65536.0)


def socket_record(socket, fragment, generic_name):
    """Reduce a cooked SkeletalMeshSocket export to host-space bone-local data."""
    location = socket.get('RelativeLocation') or {}
    rotation = socket.get('RelativeRotation') or {}
    return {
        'name': str(socket.get('SocketName') or f'{fragment}_{generic_name}'),
        'bone': str(socket.get('BoneName') or ''),
        'location': [float(location.get(axis, 0.0)) for axis in ('X', 'Y', 'Z')],
        'rotation': [angle_to_degrees(rotation.get(axis, 0)) for axis in ('Pitch', 'Yaw', 'Roll')],
        'fragment': str(fragment),
    }


def resolve_fragment_socket(sockets, fragments, generic_name):
    """Resolve a generic type socket name through the selected part fragment."""
    by_name = {str(socket.get('SocketName', '')).casefold(): socket for socket in sockets}
    for fragment in fragments:
        short = str(fragment).rsplit('.', 1)[-1]
        candidate = by_name.get(f'{short}_{generic_name}'.casefold())
        if candidate is not None:
            return socket_record(candidate, short, generic_name)
    candidate = by_name.get(str(generic_name).casefold())
    if candidate is not None:
        return socket_record(candidate, '', generic_name)
    return None


def vector(value, keys=('X', 'Y', 'Z')):
    if not isinstance(value, dict):
        return None
    if not all(key in value for key in keys):
        return None
    return [float(value[key]) for key in keys]


def type_view_fields(recipe_id, class_defaults, type_properties):
    """Pure projection of the cooked type fields used by the host."""
    data = merge_type_properties(class_defaults, type_properties)
    offset = vector(data.get('PlayerViewOffset'))
    if offset is None:
        return None
    fov = data.get('FirstPersonMeshFOV')
    kick = {name.lower().replace('weaponkick', ''): float(data.get(name, 0.0))
            for name in ('WeaponKickUp', 'WeaponKickDown', 'WeaponKickLeft', 'WeaponKickRight')}
    kick_sources = {name.lower().replace('weaponkick', ''):
                    ('cooked type' if type_properties.get(name) is not None else 'cooked class default')
                    for name in ('WeaponKickUp', 'WeaponKickDown', 'WeaponKickLeft', 'WeaponKickRight')}
    reloads = reload_animation_names(data.get('WeaponReloadAnimations'))
    reload_name = choose_default_reload_animation(reloads)
    threshold = fast_reload_threshold(data.get('AnimThresholdValues'))
    anim_set = data.get('WeaponAnimSet')
    if not isinstance(anim_set, str):
        anim_set = None
    mesh = data.get('GestaltMesh')
    if not isinstance(mesh, str):
        mesh = None
    zoom_fov = data.get('ZoomedEndFOV')
    zoom_time = data.get('ZoomTime')
    muzzle_duration = data.get('MuzzleFlashDuration')
    try:
        reload_complete_percent = float(type_properties['ReloadCompletePercent'])
        b_has_reload_complete_percent = math.isfinite(reload_complete_percent) and 0.0 <= reload_complete_percent <= 1.0
    except (KeyError, TypeError, ValueError):
        reload_complete_percent = 1.0
        b_has_reload_complete_percent = False
    if not b_has_reload_complete_percent:
        reload_complete_percent = 1.0
    reload_complete_percent_source = ('cooked type' if b_has_reload_complete_percent else
                                      'default: missing cooked type field; legacy full-duration refill')
    tracer = data.get('TracerColor')
    if isinstance(tracer, list):
        tracer = {v.get('name'): v.get('value') for v in tracer if isinstance(v, dict)}
    if not isinstance(tracer, dict):
        tracer = {}
    recoil_scale = data.get('ZoomedRecoilAnimScale')
    try:
        recoil_scale = float(recoil_scale)
    except (TypeError, ValueError):
        recoil_scale = 1.0
    zoom_position_fit, zoom_position_fit_source = zoom_position_fit_cm(recipe_id)
    entry = {
        'weapon_type': str(type_properties.get('weapon_type') or ''),
        'player_view_offset': offset,
        'first_person_fov': float(fov) if fov is not None else None,
        'first_person_fov_source': 'cooked type' if type_properties.get('FirstPersonMeshFOV') is not None else 'cooked class default',
        'zoom_position_fit_cm': zoom_position_fit,
        'zoom_position_fit_source': zoom_position_fit_source,
        'zoom_fov': float(zoom_fov) if zoom_fov is not None else None,
        'zoom_fov_source': 'cooked type' if type_properties.get('ZoomedEndFOV') is not None else 'cooked class default',
        'zoom_blend_time': float(zoom_time) if zoom_time is not None else 0.2,
        'zoom_blend_time_source': 'cooked type' if type_properties.get('ZoomTime') is not None else 'cooked class default',
        'zoom_socket_preferred': str(data.get('ZoomSocketPreferred') or ''),
        'zoom_socket_fallback': str(data.get('ZoomSocketFallback') or ''),
        # The type's selected EyeSocket is used by the host to centre the sight in ADS.
        # The original view-model motion formula was not recovered; this alignment is UNVERIFIED.
        'zoom_offset_method': 'align selected zoom socket to camera centre (UNVERIFIED)',
        'weapon_anim_set': anim_set,
        'weapon_preview_mesh': mesh,
        'reload_animations': reloads,
        'reload_animation': reload_name,
        'reload_complete_percent': reload_complete_percent,
        'reload_complete_percent_source': reload_complete_percent_source,
        'fast_reload_threshold': threshold,
        'fast_reload_threshold_source': ('cooked type AnimThresholdValues' if threshold is not None else 'unavailable'),
        'reload_animation_source': 'cooked type WeaponReloadAnimations; unconditioned fallback (UNVERIFIED selection)',
        'muzzle_socket': str(data.get('MuzzleFlashSocket') or ''),
        'muzzle_socket_source': ('cooked type' if type_properties.get('MuzzleFlashSocket') is not None
                                 else 'cooked class default'),
        'muzzle_flash_duration': float(muzzle_duration) if muzzle_duration is not None else 0.33,
        'muzzle_flash_duration_source': 'cooked type' if type_properties.get('MuzzleFlashDuration') is not None else 'cooked class default',
        'muzzle_flash_collection': data.get('MuzzleFlashPSTemplates'),
        # No numeric muzzle-flash size is declared on WeaponTypeDefinition.
        # Keep the existing visual scale until a particle-size reader or live measurement supplies one.
        'muzzle_flash_radius_cm': float(data.get('MuzzleFlashRadius', 3.5)) if data.get('MuzzleFlashRadius') is not None else 3.5,
        'muzzle_flash_radius_source': 'UNVERIFIED host stand-in (type has no scalar size field)',
        'shell_casing_socket': str(data.get('ShellCasingSocket') or ''),
        'shell_casing_template': data.get('ShellCasingPSCTemplate'),
        'shell_casing_enabled': bool(data.get('ShellCasingPSCTemplate')),
        'shell_casing_enabled_source': 'cooked type field',
        'tracer_color': [int(tracer.get(axis, default)) for axis, default in (('R', 200), ('G', 200), ('B', 200), ('A', 255))],
        'tracer_color_source': 'cooked type' if type_properties.get('TracerColor') is not None else 'cooked class default',
        'tracer_template': data.get('TracerTemplate'),
        'tracer_enabled': bool(data.get('TracerTemplate')),
        'tracer_enabled_source': 'cooked type field',
        'tracer_color_parameter': str(data.get('TracerColorParameterName') or 'TracerColor'),
        'tracer_radius_cm': 0.8,
        'tracer_radius_source': 'UNVERIFIED host stand-in; type has no scalar size field',
        'tracer_lifetime': 0.09,
        'tracer_lifetime_source': 'UNVERIFIED host stand-in',
        'weapon_kick': kick,
        'weapon_kick_sources': kick_sources,
        # The stock field-to-animation weighting is not measured. This retains the old 0.45 baseline and
        # scales it with the mean absolute merged kick channels; this mapping is UNVERIFIED.
        'recoil_animation_weight': recoil_animation_weight(kick),
        'recoil_animation_weight_source': 'UNVERIFIED host mapping from merged cooked WeaponKick channels',
        'zoomed_recoil_animation_scale': recoil_scale,
        'zoomed_recoil_animation_scale_source': ('cooked type' if type_properties.get('ZoomedRecoilAnimScale') is not None
                                                else 'cooked class default'),
    }
    if reload_name and recipe_id:
        asset_name = f'Anim_WeaponReload_{reload_name}'
        asset_folder = f'/Game/OpenWillow/Weapons/Items/WeaponAnims/{recipe_id}'
        entry['weapon_reload_asset'] = f'{asset_folder}/{asset_name}.{asset_name}'
        entry['weapon_reload_asset_name'] = asset_name
    else:
        entry['weapon_reload_asset'] = None
        entry['weapon_reload_asset_name'] = None
    return entry


def _schema_file(args):
    lines = [line for line in args.schema.read_text(encoding='utf-8').splitlines()
             if line.strip() and not line.startswith('FirstPersonMeshFOV=')]
    merged = list(dict.fromkeys(lines + SCHEMA_ADDITIONS))
    handle, name = tempfile.mkstemp(suffix='.schema')
    Path(name).write_text('\n'.join(merged) + '\n', encoding='utf-8')
    return Path(name)


def _property_path(value):
    return value if isinstance(value, str) else None


def build_recipe_entries(recipe_rows, package, class_defaults):
    """Build data entries and batch-load only sockets referenced by these recipes."""
    prepared = []
    socket_paths = set()
    for recipe_id, recipe in recipe_rows:
        weapon_type = recipe.get('weapon_type') if isinstance(recipe, dict) else None
        if not weapon_type or not isinstance(recipe.get('parts'), dict):
            continue
        type_properties = package.props(weapon_type)
        entry = type_view_fields(recipe_id, class_defaults, type_properties)
        if entry is None:
            continue
        entry['weapon_type'] = weapon_type
        stats = recipe.get('stats') if isinstance(recipe.get('stats'), dict) else {}
        card = stats.get('card') if isinstance(stats.get('card'), dict) else {}
        try:
            entry['card_reload_time'] = float(card['reload_time'])
        except (KeyError, TypeError, ValueError):
            entry['card_reload_time'] = None
        gestalt_path = _property_path(type_properties.get('GestaltMesh'))
        skeletal_mesh = None
        if gestalt_path and gestalt_path in package.index:
            gestalt = package.props(gestalt_path)
            skeletal_mesh = _property_path(gestalt.get('GestaltSkeletalMesh'))
        selected_fragments = {}
        for slot, part_entry in recipe['parts'].items():
            part = part_entry.get('part') if isinstance(part_entry, dict) else part_entry
            if not isinstance(part, str) or not part:
                continue
            selected_fragments[slot] = weapon_recipe.part_fragments(package, part)
        sight_entry = recipe['parts'].get('Sight')
        sight_path = sight_entry.get('part') if isinstance(sight_entry, dict) else sight_entry
        sight_properties = package.props(sight_path) if isinstance(sight_path, str) and sight_path in package.index else {}
        scope_particle = sight_properties.get('FirstPersonParticle')
        entry['scope_overlay'] = bool(scope_particle)
        entry['scope_overlay_source'] = (
            'cooked selected Sight.FirstPersonParticle; overlay interpretation UNVERIFIED'
            if scope_particle else 'cooked selected Sight has no FirstPersonParticle')
        if skeletal_mesh:
            prefix = skeletal_mesh + '.SkeletalMeshSocket_'
            socket_paths.update(path for path in package.index if path.startswith(prefix))
        prepared.append((recipe_id, entry, type_properties, skeletal_mesh, selected_fragments))

    if socket_paths:
        package.preload(sorted(socket_paths))

    result = {}
    for recipe_id, entry, type_properties, skeletal_mesh, selected_fragments in prepared:
        sockets = [package.cache[path] for path in package.index
                   if skeletal_mesh and path.startswith(skeletal_mesh + '.SkeletalMeshSocket_')
                   and path in package.cache]
        muzzle_name = entry['muzzle_socket']
        body_socket_name = entry['shell_casing_socket']
        muzzle = resolve_fragment_socket(sockets, selected_fragments.get('Barrel', []), muzzle_name) if muzzle_name else None
        casing = resolve_fragment_socket(sockets, selected_fragments.get('Body', []), body_socket_name) if body_socket_name else None
        zoom_fragments = []
        for slot in ('Sight', 'Body', 'Barrel', 'Stock', 'Grip', 'Elemental', 'Accessory1', 'Accessory2', 'Material'):
            zoom_fragments.extend(selected_fragments.get(slot, []))
        zoom_socket = resolve_fragment_socket(sockets, zoom_fragments, entry['zoom_socket_preferred']) if entry['zoom_socket_preferred'] else None
        if zoom_socket is None and entry['zoom_socket_fallback']:
            zoom_socket = resolve_fragment_socket(sockets, zoom_fragments, entry['zoom_socket_fallback'])
        entry['muzzle_socket_transform'] = muzzle
        entry['muzzle_socket_transform_source'] = 'selected fragment SkeletalMeshSocket export' if muzzle else 'unresolved'
        entry['shell_casing_socket_transform'] = casing
        entry['shell_casing_socket_transform_source'] = 'selected fragment SkeletalMeshSocket export' if casing else 'unresolved'
        entry['zoom_socket_transform'] = zoom_socket
        entry['zoom_socket_transform_source'] = 'selected fragment SkeletalMeshSocket export' if zoom_socket else 'unresolved'
        result[recipe_id] = entry
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--dir', type=Path, nargs='+', required=True)
    parser.add_argument('--schema', type=Path, default=ROOT / 'local/items/slice/slice_gear.schema')
    parser.add_argument('--measurements', type=Path,
                        help='optional ignored local lane E live-measurements markdown')
    args = parser.parse_args()
    live = parse_live_measurements(args.measurements.read_text(encoding='utf-8')) if args.measurements else {}
    schema = _schema_file(args)
    cooked = args.game / 'WillowGame/CookedPCConsole'
    package = weapon_recipe.Package(str(Path(args.reader).resolve()), cooked / 'Startup.upk', str(schema.resolve()))
    defaults_package = weapon_recipe.Package(str(Path(args.reader).resolve()), cooked / 'WillowGame.upk', str(schema.resolve()))
    defaults = defaults_package.props('Default__WeaponTypeDefinition')
    for folder in args.dir:
        recipe_rows = []
        for path in sorted(folder.glob('*.json')):
            if path.name == 'weapon_view.json':
                continue
            try:
                recipe = json.loads(path.read_text(encoding='utf-8-sig'))
            except ValueError:
                continue
            recipe_rows.append((path.stem, recipe))
        view = build_recipe_entries(recipe_rows, package, defaults)
        for recipe_id, entry in list(view.items()):
            if recipe_id in live:
                view[recipe_id] = apply_live_measurement(entry, live[recipe_id])
            entry = view[recipe_id]
            selected = choose_reload_animation(entry.get('reload_animations', []),
                                               entry.get('card_reload_time'),
                                               entry.get('fast_reload_threshold'))
            entry['reload_animation'] = selected
            if selected:
                entry['reload_animation_source'] = (
                    'cooked type clip names + live WeaponMaxFastReloadAnimTime vs recipe card ReloadTime; '
                    'expression interpretation UNVERIFIED'
                    if entry.get('fast_reload_threshold') is not None and entry.get('card_reload_time') is not None
                    else 'cooked type WeaponReloadAnimations; last unconditioned fallback (UNVERIFIED selection)')
                asset_name = f'Anim_WeaponReload_{selected}'
                asset_folder = f'/Game/OpenWillow/Weapons/Items/WeaponAnims/{recipe_id}'
                entry['weapon_reload_asset'] = f'{asset_folder}/{asset_name}.{asset_name}'
                entry['weapon_reload_asset_name'] = asset_name
            else:
                entry['reload_animation_source'] = 'no matching cooked type reload entry'
                entry['weapon_reload_asset'] = None
                entry['weapon_reload_asset_name'] = None
        out = folder / 'weapon_view.json'
        out.write_text(json.dumps(view, indent=1), encoding='utf-8')
        print(f'{out}: {len(view)} recipes; reload clips {sum(bool(v["reload_animation"]) for v in view.values())}; '
              f'muzzle sockets {sum(bool(v["muzzle_socket_transform"]) for v in view.values())}; '
              f'zoom sockets {sum(bool(v["zoom_socket_transform"]) for v in view.values())}; '
              f'live overrides {sum(recipe_id in live for recipe_id in view)}')


if __name__ == '__main__':
    main()
