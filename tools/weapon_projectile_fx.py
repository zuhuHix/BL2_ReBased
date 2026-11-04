"""Add the shot's cooked particle layers to recipes: tracer, projectile trail, explosion bands (the `fx` block).

What is read, from cooked Startup.upk through ow-package (the recipe's `firing` block names the objects):
- tracer: FiringModeDefinition.PartSysTemplate (`firing.tracer_template`).
- trail: ProjectileDefinition.InFlightEffects and InFlightEffectsScale (`firing.projectile.definition`).
- explosions: ExplosionDefinition.ExplosionScale (`firing.projectile.splash.explosion`, `firing.splash.explosion` or
  `firing.impact_explosion.explosion`): one band per entry with MinRadius / MaxRadius, MinParticleScale /
  MaxParticleScale and ExplosionPSTemplate. The band that holds the explosion's damage radius is the one the game
  plays (the band choice by radius is read from the field names only: UNVERIFIED).
Each particle system is read into layers with tools/weapon_muzzle_fx.py (the same fitted lookup-table reading,
UNVERIFIED against the engine). The host draws the layers with its billboard stand-in, not the game's particle renderer.

Writes `firing.fx` into each recipe given, in place. Recipes are game-derived: every recipe path is resolved (symlinks
followed) and must lie strictly inside the repository's ignored local/ folder, or the tool stops before reading anything;
its temporary schema file is written under local/ too.

  python tools/weapon_projectile_fx.py --game "<BL2 install>" local/items/slice/*.json local/items/firing/coverage/*.json
"""
import argparse
import json
import os
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import weapon_muzzle_fx  # noqa: E402
import weapon_recipe  # noqa: E402

SCHEMA_LINES = weapon_muzzle_fx.SCHEMA_LINES + ['ExplosionScale=StructProperty:ExplosionScaleData']


def local_path(path, repo=ROOT):
    """The path resolved (symlinks followed); refuses anything not strictly inside the repository's local/ folder,
    because the tool writes game-derived particle data into it (the skills lane's local_output rule)."""
    resolved = Path(path).resolve()
    local = (Path(repo) / 'local').resolve()
    if local not in resolved.parents:
        raise SystemExit(f'refusing {path}: recipes must be inside {local} (ignored game-derived data)')
    return resolved


def system_layers(package, path, cache):
    """Layers of a ParticleSystem path, or None when the path is absent from the package."""
    if not path:
        return None
    if path not in cache:
        cache[path] = weapon_muzzle_fx.particle_system_layers(package, path) if path in package.index else None
    return cache[path]


def explosion_bands(package, explosion, cache):
    """The explosion definition's ExplosionScale bands with their layers; [] when it has none."""
    if not explosion or explosion not in package.index:
        return []
    bands = []
    for entry in package.props(explosion).get('ExplosionScale') or []:
        template = entry.get('ExplosionPSTemplate')
        bands.append({
            'min_radius': entry.get('MinRadius'), 'max_radius': entry.get('MaxRadius'),
            'min_scale': entry.get('MinParticleScale'), 'max_scale': entry.get('MaxParticleScale'),
            'template': template, 'layers': system_layers(package, template, cache),
            'impact_effect': entry.get('ImpactEffect'),
        })
    return bands


def explosion_of(firing, package=None):
    """(explosion definition, radius) of the shot: the projectile's splash, the firing block's splash, or the firing
    mode's impact explosion, first present; a splash that names none takes the projectile definition's
    DefaultExplosionDefinition (the Tediore throw's)."""
    projectile = firing.get('projectile') if isinstance(firing.get('projectile'), dict) else {}
    for block in (projectile.get('splash'), firing.get('splash'), firing.get('impact_explosion')):
        if isinstance(block, dict) and block.get('explosion'):
            return block['explosion'], block.get('radius')
    definition = projectile.get('definition')
    splash = projectile.get('splash')
    if package is not None and isinstance(splash, dict) and definition and definition in package.index:
        default = package.props(definition).get('DefaultExplosionDefinition')
        if default:
            return default, splash.get('radius')
    return None, None


def build(package, firing, cache=None):
    """The `fx` block for one recipe's firing block."""
    cache = {} if cache is None else cache
    fx = {'tracer': None, 'trail': None, 'explosion': None}
    tracer = firing.get('tracer_template')
    if tracer:
        fx['tracer'] = {'template': tracer, 'layers': system_layers(package, tracer, cache)}
    projectile = firing.get('projectile') if isinstance(firing.get('projectile'), dict) else {}
    definition = projectile.get('definition')
    if definition and definition in package.index:
        props = package.props(definition)
        trail = props.get('InFlightEffects')
        if trail:
            fx['trail'] = {'template': trail, 'scale': props.get('InFlightEffectsScale'),
                           'layers': system_layers(package, trail, cache)}
    explosion, radius = explosion_of(firing, package)
    if explosion:
        fx['explosion'] = {'definition': explosion, 'radius': radius, 'bands': explosion_bands(package, explosion, cache)}
    return fx


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('recipes', nargs='+', type=Path)
    args = parser.parse_args()
    recipes = [local_path(path) for path in args.recipes]   # all checked before anything is read or written
    scratch = ROOT / 'local' / 'projectiles'
    scratch.mkdir(parents=True, exist_ok=True)
    handle, name = tempfile.mkstemp(suffix='.schema', dir=scratch)
    os.close(handle)
    schema = Path(name)
    try:
        schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')
        package = weapon_recipe.Package(str(Path(args.reader).resolve()),
                                        args.game / 'WillowGame/CookedPCConsole/Startup.upk', str(schema.resolve()))
        cache = {}
        for path in recipes:
            recipe = json.loads(path.read_text(encoding='utf-8'))
            firing = recipe.get('firing')
            if not isinstance(firing, dict):
                print(f'{path.name}: no firing block, skipped')
                continue
            firing['fx'] = build(package, firing, cache)
            if isinstance(firing.get('reload_throw'), dict):
                firing['reload_throw']['fx'] = build(package, firing['reload_throw'], cache)
            path.write_text(json.dumps(recipe, indent=1), encoding='utf-8')
            fx = firing['fx']
            print(f"{path.name}: tracer {bool(fx['tracer'] and fx['tracer']['layers'])}, "
                  f"trail {bool(fx['trail'] and fx['trail']['layers'])}, "
                  f"explosion bands {len(fx['explosion']['bands']) if fx['explosion'] else 0}")
    finally:
        schema.unlink(missing_ok=True)


if __name__ == '__main__':
    main()
