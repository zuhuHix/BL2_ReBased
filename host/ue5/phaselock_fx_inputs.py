"""Read and check the Phaselock effect import's local inputs before it deletes the imported effects.

AI-assisted (Claude). Plain Python, no `unreal`, so tests/phaselock_fx_inputs_test.py can run it without the editor.
host/ue5/import_phaselock_fx.py calls load_fx_constants and check_fx_inputs before delete_directory and stops on any problem.
"""
import json
import math
import os
from pathlib import Path

# The templates the host plays (tools/prepare_phaselock_fx.py's manifest: the hand, the bubble, the screen effect).
REQUIRED_TEMPLATES = ('Part_SirenASHandOrb', 'Part_SirenASHandFizzle', 'Part_PhaseLockScreenEffect',
                      'Part_SirenASEnemyOrb', 'Part_SirenASEnemyOrbBegin', 'Part_SirenASEnemyOrbEnd')

# The stock shader colours the importer reads from the ignored local/phaselock/fx_constants.json ({material: {name: [r, g, b]}}).
REQUIRED_CONSTANTS = (('Mat_SirenHandInnerOrb', 'blue'), ('Mat_PowerUpTwirls', 'tint'), ('Master_Player', 'power_emissive'))

# The only materials the import may skip; a skip of anything else stops the run. Each is a skip the importer makes today.
OPTIONAL_MATERIALS = {
    'Mat_PhaseLockImpactMiss_HalfMoonGlow': 'no texture parameters, so UModel lists no texture for it',
}

# The slot guess of UModel's .mat names the wrong mask for the screen effect: the shader reads the second referenced
# texture, PhaseLockScreenMask02_Dif_Tex (WillowGame.upk).
TEXTURE_CHOICE = {'Mat_PhaseLockScreenEffect': 'PhaseLockScreenMask02_Dif_Tex'}
HOSTED_BLENDS = ('BLEND_Additive', 'BLEND_Translucent', 'BLEND_Modulate')
SCREEN_TEMPLATES = ('Part_PhaseLockScreenEffect',)


def load_fx_constants(path):
    """(constants, problems): the parsed file, and every required colour that is missing or is not three numbers."""
    path = Path(path)
    if not path.is_file():
        return {}, [f'{path} is missing: the stock effect colour constants (ignored local input) are required']
    try:
        constants = json.loads(path.read_text(encoding='utf-8'))
    except (OSError, ValueError) as error:
        return {}, [f'{path} is unreadable: {error}']
    if not isinstance(constants, dict):
        return {}, [f'{path} is not a JSON object']
    problems = []
    for material, name in REQUIRED_CONSTANTS:
        value = constants.get(material, {}).get(name) if isinstance(constants.get(material), dict) else None
        if not (isinstance(value, list) and len(value) == 3
                and all(isinstance(c, (int, float)) and not isinstance(c, bool) and math.isfinite(c) for c in value)):
            problems.append(f'{path}: {material}.{name} is missing or is not three finite numbers')
    return constants, problems


def _template_shape_problems(name, data):
    """What is wrong with a decoded template's shape, for the parts the importer reads."""
    if not isinstance(data, dict):
        return [f'template {name} is not a JSON object']
    problems = []
    if not isinstance(data.get('materials') or {}, dict):
        problems.append(f'template {name}: materials is not an object')
    digest = data.get('digest')
    if not digest:
        return problems + [f'template {name} has no emitters']
    if not isinstance(digest, list):
        return problems + [f'template {name}: digest is not a list']
    for index, emitter in enumerate(digest):
        if not isinstance(emitter, dict) or not isinstance(emitter.get('required'), dict):
            problems.append(f'template {name}: emitter {index} has no required block')
        elif not isinstance(emitter.get('type_data') or {}, dict) \
                or not isinstance((emitter.get('type_data') or {}).get('properties') or {}, dict):
            problems.append(f'template {name}: emitter {index} has a malformed type_data')
    return problems


def read_templates(emitter_dir):
    """({template name: decoded data}, problems). Only well-formed templates are returned."""
    emitter_dir = Path(emitter_dir)
    if not emitter_dir.is_dir():
        return {}, [f'emitter directory {emitter_dir} does not exist']
    templates, problems = {}, []
    for name in REQUIRED_TEMPLATES:
        if not (emitter_dir / f'{name}.json').is_file():
            problems.append(f'template {name}.json is missing from {emitter_dir}')
    for path in sorted(emitter_dir.glob('Part_*.json')):
        try:
            data = json.loads(path.read_text(encoding='utf-8'))
        except (OSError, ValueError) as error:
            problems.append(f'template {path.name} is unreadable: {error}')
            continue
        shape = _template_shape_problems(path.name, data)
        if shape:
            problems.extend(shape)
        else:
            templates[path.stem] = data
    return templates, problems


def _blend_of(materials, path, depth=0):
    entry = materials.get(path) or {}
    props = entry.get('properties') or {} if isinstance(entry, dict) else {}
    if props.get('BlendMode') or depth > 4:
        return props.get('BlendMode')
    return _blend_of(materials, props.get('Parent'), depth + 1) if props.get('Parent') else None


def collect_uses(templates):
    """The stock materials and meshes the templates name, with the emitter layouts that use them: (uses, blend, meshes)."""
    uses, blend, meshes = {}, {}, set()
    for stem, data in sorted(templates.items()):
        materials = data.get('materials') or {}
        for emitter in data['digest']:
            req = emitter['required']
            path = req.get('Material')
            if not path:
                continue
            name = path.rsplit('.', 1)[-1]
            layout = uses.setdefault(name, {'subimages': False, 'mesh': False, 'screen': False, 'templates': set()})
            layout['subimages'] |= (req.get('SubImages_Horizontal', 1) or 1) * (req.get('SubImages_Vertical', 1) or 1) > 1
            layout['screen'] |= stem in SCREEN_TEMPLATES
            layout['templates'].add(stem)
            mesh = ((emitter.get('type_data') or {}).get('properties') or {}).get('Mesh')
            if mesh:
                layout['mesh'] = True
                meshes.add(mesh.rsplit('.', 1)[-1])
            blend[name] = _blend_of(materials, path)
    return uses, blend, meshes


def parse_mat_slots(text):
    """The slot = texture lines of UModel's .mat file."""
    slots = {}
    for line in text.splitlines():
        if '=' in line:
            key, value = line.split('=', 1)
            slots[key.strip()] = value.strip()
    return slots


def choose_texture(name, slots, subimages):
    """The texture a generic material draws with, and its optional soft mask: (chosen, mask)."""
    chosen = TEXTURE_CHOICE.get(name) or slots.get('Diffuse')
    if subimages:
        chosen = next((v for v in slots.values() if 'SubUV' in v), chosen)
    # A *_Mirror texture listed besides the colour texture is read as a soft mask over the quad (host reading).
    mask = next((v for v in slots.values() if v != chosen and v.endswith('_Mirror')), None)
    return chosen, mask


def _index_files(roots):
    """{file name: first path by sorted order}, as the importer's find() picks across the roots."""
    found = {}
    for root in roots:
        for folder, _, names in os.walk(root):
            for name in names:
                path = Path(folder) / name
                if name not in found or path < found[name]:
                    found[name] = path
    return found


def _generic_problems(name, layout, mode, files):
    """Why a generic (not hand-written) material would be skipped or built incomplete; empty when it builds."""
    mat = files.get(name + '.mat')
    if not mat:
        slots = {}
    else:
        try:
            slots = parse_mat_slots(mat.read_text(encoding='utf-8', errors='replace'))
        except OSError as error:
            return [f'{name}: {mat} is unreadable: {error}']
    chosen, mask = choose_texture(name, slots, layout['subimages'])
    if mode not in HOSTED_BLENDS:
        return [f'{name}: blend mode {mode} is not hosted']
    if chosen and chosen.endswith(('_Nrm', '_Nrm_Tex')):
        return [f'{name}: the chosen texture {chosen} is a normal map, so it would not be drawn']
    problems = []
    if chosen and chosen + '.png' not in files:
        problems.append(f'{name}: texture {chosen} is not in the UModel output')
    if not chosen and mode != 'BLEND_Modulate':
        problems.append(f'{name}: no texture is listed for it (no .mat slot found)')
    if mask and mask + '.png' not in files:
        problems.append(f'{name}: mask {mask} is not in the UModel output')
    return problems


def check_fx_inputs(emitter_dir, roots, exact_textures, not_drawn=()):
    """Every problem that would break the import or leave it partial; an empty list means go ahead.

    emitter_dir: the decoded templates (Part_*.json). roots: the UModel export roots. exact_textures: for each material with a
    hand-written shader, the textures it needs ({material: [texture, ...]}). not_drawn: materials the host leaves out on purpose.
    Only materials and meshes a well-formed template uses count. Optional skips are OPTIONAL_MATERIALS only.
    """
    templates, problems = read_templates(emitter_dir)
    if not Path(emitter_dir).is_dir():
        return problems
    live_roots = [Path(root) for root in roots if Path(root).is_dir()]
    if not live_roots:
        problems.append(f'none of the UModel export roots exists: {list(map(str, roots))}')
        return problems
    files = _index_files(live_roots)
    uses, blend, meshes = collect_uses(templates)
    for name in sorted(uses):
        if name in not_drawn or name in OPTIONAL_MATERIALS:
            continue
        if name in exact_textures:
            problems.extend(f'{name}: texture {texture} is not in the UModel output'
                            for texture in exact_textures[name] if texture + '.png' not in files)
        else:
            problems.extend(_generic_problems(name, uses[name], blend.get(name), files))
    problems.extend(f'mesh {mesh}: {mesh}.gltf is not in the UModel output' for mesh in sorted(meshes) if mesh + '.gltf' not in files)
    return problems
