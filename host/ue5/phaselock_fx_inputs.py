"""Check the Phaselock effect import's local inputs before it deletes the imported effects.

AI-assisted (Claude). Plain Python, no `unreal`, so tests/phaselock_fx_inputs_test.py can run it without the editor.
host/ue5/import_phaselock_fx.py calls check_fx_inputs before delete_directory and stops on any problem.
"""
import json
from pathlib import Path

# The templates the host plays (tools/prepare_phaselock_fx.py's manifest: the hand, the bubble, the screen effect).
REQUIRED_TEMPLATES = ('Part_SirenASHandOrb', 'Part_SirenASHandFizzle', 'Part_PhaseLockScreenEffect',
                      'Part_SirenASEnemyOrb', 'Part_SirenASEnemyOrbBegin', 'Part_SirenASEnemyOrbEnd')

# The stock shader colours the importer reads from the ignored local/phaselock/fx_constants.json ({material: {name: [r, g, b]}}).
REQUIRED_CONSTANTS = (('Mat_SirenHandInnerOrb', 'blue'), ('Mat_PowerUpTwirls', 'tint'), ('Master_Player', 'power_emissive'))


def load_fx_constants(path):
    """(constants, problems): the parsed file, and every required colour that is missing or is not three numbers."""
    path = Path(path)
    if not path.is_file():
        return {}, [f'{path} is missing: the stock effect colour constants (ignored local input) are required']
    try:
        constants = json.loads(path.read_text(encoding='utf-8'))
    except (OSError, ValueError) as error:
        return {}, [f'{path} is unreadable: {error}']
    problems = []
    for material, name in REQUIRED_CONSTANTS:
        value = constants.get(material, {}).get(name) if isinstance(constants.get(material), dict) else None
        if not (isinstance(value, list) and len(value) == 3
                and all(isinstance(c, (int, float)) and not isinstance(c, bool) for c in value)):
            problems.append(f'{path}: {material}.{name} is missing or is not three numbers')
    return constants, problems


def _materials_used(data):
    """The material names a decoded template's emitters draw with."""
    names = set()
    for emitter in data.get('digest') or []:
        path = (emitter.get('required') or {}).get('Material')
        if path:
            names.add(path.rsplit('.', 1)[-1])
    return names


def check_fx_inputs(emitter_dir, roots, exact_textures):
    """Every problem that would break the import or leave it partial; an empty list means go ahead.

    emitter_dir: the decoded templates (Part_*.json). roots: the UModel export roots. exact_textures: for each material with
    a hand-written shader, the textures it needs ({material: [texture, ...]}); a material only counts when a template uses it.
    """
    problems = []
    emitter_dir = Path(emitter_dir)
    if not emitter_dir.is_dir():
        return [f'emitter directory {emitter_dir} does not exist']
    used = set()
    for name in REQUIRED_TEMPLATES:
        path = emitter_dir / f'{name}.json'
        if not path.is_file():
            problems.append(f'template {path.name} is missing from {emitter_dir}')
    for path in sorted(emitter_dir.glob('Part_*.json')):
        try:
            data = json.loads(path.read_text(encoding='utf-8'))
        except (OSError, ValueError) as error:
            problems.append(f'template {path.name} is unreadable: {error}')
            continue
        if not data.get('digest'):
            problems.append(f'template {path.name} has no emitters')
        used |= _materials_used(data)
    live_roots = [Path(root) for root in roots if Path(root).is_dir()]
    if not live_roots:
        problems.append(f'none of the UModel export roots exists: {list(map(str, roots))}')
        return problems
    for material in sorted(used & set(exact_textures)):
        for texture in exact_textures[material]:
            if not any(next(root.rglob(texture + '.png'), None) for root in live_roots):
                problems.append(f'{material}: texture {texture} is not in the UModel output')
    return problems
