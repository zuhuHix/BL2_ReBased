"""Read muzzle-flash and shell-casing particle layers from cooked particle systems.

What is read, from cooked Startup.upk through ow-package:
- EffectCollectionDefinition.ParticleEffects: which ParticleSystem plays for each
  damage element (Expression names WeaponDamage_Is_<X>; no Expression is Default).
- ParticleSystem.Emitters -> LODLevels[0]: RequiredModule (material, screen
  alignment, local space, duration), SpawnModule (burst count) and the Modules
  list (lifetime, size, location, colour, axis lock, velocity, acceleration).

Distribution tables: the LookupTable layout is FITTED from the cooked data and
is UNVERIFIED against the engine. Assumed: two header floats, then chunks of
LookupTableChunkSize floats (the component count when absent). A uniform
distribution (Op 2 or 3) holds two halves (low, high) per chunk. Only the first
chunk is used; the number of distinct chunks is reported as `samples`.

The host draws these layers with a stand-in (flat billboards), not the game's
particle renderer, so the drawing is UNVERIFIED too.

  python tools/weapon_muzzle_fx.py --game "<BL2 install>" --collection <EffectCollectionDefinition path>
"""
import argparse
import json
import os
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import weapon_recipe  # noqa: E402


SCHEMA_LINES = [
    'ParticleEffects=StructProperty:ParticleEffectData',
    'Emitters=ObjectProperty',
    'LODLevels=ObjectProperty',
    'Modules=ObjectProperty',
    'LookupTable=FloatProperty',
    'BurstList=StructProperty:ParticleBurst',
]
DAMAGE_PREFIX = 'D_Attributes.WeaponDamageType.WeaponDamage_Is_'
# Expression damage type -> the host's element name. Element names in the art
# (for example Part_SMG_MuzzleFlash_Etec) do not decide the element; the expression does.
DAMAGE_ELEMENTS = {'Corrosive': 'Corrosive', 'Explosive': 'Explosive', 'Incendiary': 'Fire',
                   'Shock': 'Shock', 'Amp': 'Slag'}
HEADER_FLOATS = 2
UNIFORM_OPS = (2, 3)


def distribution(value, components):
    """Min/max of the first table chunk of a cooked distribution struct, or None if no table.

    Returns {'min': [...], 'max': [...], 'samples': n}; n counts distinct chunks, so 1 means
    constant over life. A distribution that names an object (Distribution set, table empty)
    is not resolved here and returns None.
    """
    if not isinstance(value, dict):
        return None
    table = [float(v) for v in (value.get('LookupTable') or [])][HEADER_FLOATS:]
    chunk = int(value.get('LookupTableChunkSize') or components)
    if not table or chunk < components or len(table) % chunk:
        return None
    chunks = [table[i:i + chunk] for i in range(0, len(table), chunk)]
    first = chunks[0]
    if int(value.get('Op') or 0) in UNIFORM_OPS and chunk == 2 * components:
        low, high = first[:components], first[components:]
        lo = [min(a, b) for a, b in zip(low, high)]
        hi = [max(a, b) for a, b in zip(low, high)]
    else:
        lo = hi = first[:components]
    return {'min': lo, 'max': hi, 'samples': len({tuple(c) for c in chunks})}


def _field(modules, class_name, field):
    return (modules.get(class_name) or {}).get(field)


def _magnitude(dist, index):
    """Size range of a signed component: a random flip makes |value| the size."""
    lo, hi = dist['min'][index], dist['max'][index]
    near = 0.0 if lo <= 0.0 <= hi else min(abs(lo), abs(hi))
    return [near, max(abs(lo), abs(hi))]


def _mid(dist):
    return [(a + b) / 2.0 for a, b in zip(dist['min'], dist['max'])]


def layer(emitter_name, required, modules_by_class, spawn):
    """One emitter as a host layer. Absent fields are None; alpha defaults to 1 as the engine does."""
    required = required or {}
    spawn = spawn or {}
    modules = modules_by_class or {}
    life = distribution(_field(modules, 'ParticleModuleLifetime', 'Lifetime'), 1)
    size = distribution(_field(modules, 'ParticleModuleSize', 'StartSize'), 3)
    location = distribution(_field(modules, 'ParticleModuleLocation', 'StartLocation'), 3)
    color = distribution(_field(modules, 'ParticleModuleColor', 'StartColor'), 3)
    if color is None:
        color = distribution(_field(modules, 'ParticleModuleColorOverLife', 'ColorOverLife'), 3)
    alpha = distribution(_field(modules, 'ParticleModuleColor', 'StartAlpha'), 1)
    material = required.get('Material')
    material_name = str(material).rsplit('.', 1)[-1] if material else None
    return {
        'name': emitter_name,
        'material': material_name,
        'alignment': required.get('ScreenAlignment') or 'PSA_Square',
        'axis': _field(modules, 'ParticleModuleOrientationAxisLock', 'LockAxisFlags'),
        'life': [life['min'][0], life['max'][0]] if life else None,
        'size_x': _magnitude(size, 0) if size else None,
        'size_y': _magnitude(size, 1) if size else None,
        'offset_x': _mid(location)[0] if location else 0.0,
        'color': _mid(color) if color else None,
        'alpha': [alpha['min'][0], alpha['max'][0]] if alpha else [1.0, 1.0],
        'smoke': bool(material_name) and 'smoke' in material_name.casefold(),
        'burst': int(sum(float(b.get('Count') or 0) for b in (spawn.get('BurstList') or []))),
    }


def _preload(package, paths):
    paths = [p for p in dict.fromkeys(paths) if p]
    if paths and hasattr(package, 'preload'):
        package.preload(paths)


def _emitter_parts(package, ps_path):
    """(emitter name, RequiredModule props, {class short name: props}, SpawnModule props) per LOD 0 emitter."""
    _preload(package, [ps_path])
    emitter_paths = list(package.props(ps_path).get('Emitters') or [])
    _preload(package, emitter_paths)
    lod_paths = []
    for path in emitter_paths:
        levels = package.props(path).get('LODLevels') or []
        if levels:
            lod_paths.append(levels[0])
    _preload(package, lod_paths)
    parts = []
    for path in emitter_paths:
        levels = package.props(path).get('LODLevels') or []
        if not levels:
            continue
        lod = package.props(levels[0])
        module_paths = list(lod.get('Modules') or [])
        for key in ('RequiredModule', 'SpawnModule'):
            if lod.get(key):
                module_paths.append(lod[key])
        _preload(package, module_paths)
        modules = {}
        for module in lod.get('Modules') or []:
            name = str(package.classes.get(module, '')).rsplit('.', 1)[-1]
            modules.setdefault(name, package.props(module))
        required = package.props(lod['RequiredModule']) if lod.get('RequiredModule') else {}
        spawn = package.props(lod['SpawnModule']) if lod.get('SpawnModule') else {}
        parts.append((package.props(path).get('EmitterName'), required, modules, spawn))
    return parts


def particle_system_layers(package, ps_path):
    return [layer(name, required, modules, spawn)
            for name, required, modules, spawn in _emitter_parts(package, ps_path)]


def damage_element(evaluator):
    """Host element name for one AttributeExpressionEvaluator's damage-type operand."""
    operand = str((evaluator.get('Expression') or {}).get('AttributeOperand1') or '')
    if not operand.startswith(DAMAGE_PREFIX):
        return operand or 'Unmapped'
    damage = operand[len(DAMAGE_PREFIX):]
    return DAMAGE_ELEMENTS.get(damage, damage)


def muzzle_collection(package, coll_path):
    """{element name: ParticleSystem path} for an EffectCollectionDefinition."""
    items = package.props(coll_path).get('ParticleEffects') or []
    _preload(package, [item.get('Expression') for item in items if item.get('Expression')])
    result = {}
    for item in items:
        system = item.get('ParticleEffect')
        if not system:
            continue
        expression = item.get('Expression')
        name = damage_element(package.props(expression)) if expression else 'Default'
        result[name] = system
    return result


def casing_fx(package, ps_path):
    """Shell-casing motion from the system's first emitter (life, velocity, acceleration, spaces)."""
    parts = _emitter_parts(package, ps_path)
    required, modules = (parts[0][1], parts[0][2]) if parts else ({}, {})
    life = distribution(_field(modules, 'ParticleModuleLifetime', 'Lifetime'), 1)
    velocity = distribution(_field(modules, 'ParticleModuleVelocity', 'StartVelocity'), 3)
    accel_module = modules.get('ParticleModuleAcceleration') or {}
    accel = distribution(accel_module.get('Acceleration'), 3)

    def bounds(dist):
        return {'min': dist['min'], 'max': dist['max']} if dist else None

    return {
        'system': ps_path,
        'life': [life['min'][0], life['max'][0]] if life else None,
        'start_velocity': bounds(velocity),
        'acceleration': bounds(accel),
        'acceleration_world': bool(accel_module.get('bAlwaysInWorldSpace')),
        'local_space': bool(required.get('bUseLocalSpace')),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--system', help='ParticleSystem path: print its layers')
    parser.add_argument('--collection', help='EffectCollectionDefinition path: print element -> layers')
    args = parser.parse_args()
    if not args.system and not args.collection:
        parser.error('give --system or --collection')
    handle, name = tempfile.mkstemp(suffix='.schema')
    os.close(handle)
    schema = Path(name)
    try:
        schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')
        package = weapon_recipe.Package(str(Path(args.reader).resolve()),
                                        args.game / 'WillowGame/CookedPCConsole/Startup.upk',
                                        str(schema.resolve()))
        if args.collection:
            out = {}
            for element, system in muzzle_collection(package, args.collection).items():
                out[element] = {'system': system, 'layers': particle_system_layers(package, system)}
        else:
            out = {'system': args.system, 'layers': particle_system_layers(package, args.system)}
    finally:
        schema.unlink(missing_ok=True)
    print(json.dumps(out, indent=1))


if __name__ == '__main__':
    main()
