"""Firing templates for generated guns: what tools/weapon_firing.py resolves, per weapon type, firing mode and element.

The C++ generator rolls a gun at run time but has no firing resolver. Almost nothing in a firing block depends on the roll
beyond those three keys, so this tool rolls every balance in Python, runs the one resolver (weapon_firing.build) on each roll,
and writes one template per key to ignored local/weapons/gen/. The host fills a template from the rolled card
(OpenWillowWeaponGen::ToItem) and hands the result to the parser recipes already use.

ROLL_FIELDS come from the card, not the template. Three other values vary inside a key and are normalised here:
- a splash's evaluated damage becomes a scale of the card's damage (it follows the weapon's damage);
- bullet and projectile speeds are stored before the WeaponProjectileSpeedMultiplier, which the host applies;
- the accuracy pool keeps the type's base values only. Parts change it per roll, which is NOT modelled for generated guns yet.

Structural oracle: every roll is merged back (apply_roll) and compared with the resolver's own block. Any difference stops the
tool before it writes. It covers the template's fields, speeds and splash damage in Python only. It does NOT cover the card
overrides (pellets, spread, burst, status) or the C++ merge: the automation test OpenWillow.Vendor.RolledGunFiringTemplate does.

The file carries the MD5 of the manifest it was made with; the host refuses templates whose manifest differs, or an older
FILE_SCHEMA. So: weapon_gen_manifest.py first, then this tool. A template without the speed its shot needs stops the tool.

UNVERIFIED: the sampled rolls may not reach every key (a missing key makes the host warn and fire its fallback trace), and
nothing here has been compared with a generated gun in the running game.

  python tools/weapon_gen_firing.py --game "<BL2 install>"
"""
import argparse
import copy
import hashlib
import json
import math
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import skill_stats  # noqa: E402
import weapon_firing  # noqa: E402
import weapon_gen_manifest  # noqa: E402
import weapon_projectile_fx  # noqa: E402
import weapon_recipe  # noqa: E402
import weapon_stats  # noqa: E402

SPEED_MULTIPLIER = weapon_stats.ATTR + 'WeaponProjectileSpeedMultiplier'
# Filled from the rolled card by apply_roll, or only documentation of one roll.
ROLL_FIELDS = ('damage', 'pellets', 'spread', 'burst_count', 'burst_interval', 'fire_interval', 'shot_cost', 'spin_up', 'spin_down',
               'additional_ricochets', 'damage_type', 'element', 'explosive', 'status', 'sources', 'unverified',
               'firing_mode_source', 'other_custom_firing_modes')
STAGES = (8, 30)
SEEDS = (1, 2, 3)
RATIO_TOLERANCE = 1e-3
FILE_SCHEMA = 2   # the host's OpenWillowWeaponGen.cpp FiringTemplatesSchema
COMPARED = ('compared: every field but the roll fields and the accuracy pool, with speeds and splash damage merged in Python; '
            'NOT compared: the card overrides and the C++ merge (automation test OpenWillow.Vendor.RolledGunFiringTemplate)')


def local_output(path, repo=ROOT):
    """The path if it is strictly inside the repository's ignored local/ folder, or a refusal, before anything is created. Checked as
    given and resolved: local/ itself may be a junction to another folder, but no path may leave it, through `..` or a nested link."""
    given = Path(os.path.abspath(path))
    local = Path(os.path.abspath(Path(repo) / 'local'))
    resolved, resolved_local = Path(path).resolve(), (Path(repo) / 'local').resolve()
    if local not in given.parents or resolved_local not in resolved.parents:
        raise SystemExit(f'refusing {path}: output must be inside {local} (ignored game-derived data)')
    return given


def output_paths(output, repo=ROOT):
    """The output and the schema file written next to it, both checked before anything is created."""
    return local_output(output, repo), local_output(Path(output).with_suffix('.schema'), repo)


def fingerprint(manifest):
    """The MD5 of the manifest's bytes, or a stop naming the tool that makes it. The host hashes the same bytes."""
    try:
        return hashlib.md5(Path(manifest).read_bytes()).hexdigest()
    except OSError as error:
        raise SystemExit(f'no manifest at {manifest} ({error}): run python tools/weapon_gen_manifest.py --game <BL2 folder> first')


def positive(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value) and value > 0


def incomplete(template):
    """What a template lacks for its shot kind (the host's CompleteForShotKind): empty when it can fire."""
    missing = []
    if template.get('schema') != 1:
        missing.append(f"schema {template.get('schema')}")
    if not template.get('firing_mode'):
        missing.append('firing_mode')
    fire_type = template.get('fire_type')
    if fire_type == 'EWWFT_Rocket':
        if not positive((template.get('projectile') or {}).get('speed')):
            missing.append('projectile.speed')
    elif fire_type != 'EWWFT_HitScan' and not positive(template.get('bullet_speed')):
        missing.append('bullet_speed')
    thrown = template.get('reload_throw')
    if thrown and not positive((thrown.get('projectile') or {}).get('speed')):
        missing.append('reload_throw.projectile.speed')
    return missing


def key_of(weapon_type, firing_mode, element):
    return f"{weapon_type}|{firing_mode}|{element or ''}"


def splashes_of(block):
    """The splash blocks a firing block holds (they may be absent)."""
    found = [block.get('impact_explosion')]
    for holder in (block.get('projectile'), (block.get('reload_throw') or {}).get('projectile')):
        if holder:
            found.append(holder.get('splash'))
    return [splash for splash in found if splash]


def speed_holders(block):
    return [holder for holder in (block.get('projectile'),) if holder]


def template_of(block, card_damage, speed_multiplier):
    """The roll-independent part of one resolved block."""
    template = {key: copy.deepcopy(value) for key, value in block.items() if key not in ROLL_FIELDS}
    multiplier = speed_multiplier or 1.0
    if template.get('bullet_speed') is not None:
        template['bullet_speed'] /= multiplier
    for holder in speed_holders(template):
        if holder.get('speed') is not None:
            holder['speed'] /= multiplier
    for splash in splashes_of(template):
        # An evaluated damage that follows the card becomes the scale the parser already reads.
        if splash.get('damage') is not None and card_damage:
            splash['damage_scale'] = splash['damage'] / card_damage
            splash['damage'] = None
    pool = template.get('accuracy_pool')
    if pool:
        base = pool.get('base') or {}
        template['accuracy_pool'] = dict(pool, **{key: base[key] for key in ('min', 'max', 'idle_rate') if key in base})
        template['accuracy_pool'].pop('modifiers', None)
        template['accuracy_pool'].pop('base', None)
    return template


def apply_roll(template, roll):
    """Template + what the rolled card knows = a firing block. The host's ToItem does the same."""
    block = copy.deepcopy(template)
    multiplier = roll.get('speed_multiplier') or 1.0
    if block.get('bullet_speed') is not None:
        block['bullet_speed'] *= multiplier
    for holder in speed_holders(block):
        if holder.get('speed') is not None:
            holder['speed'] *= multiplier
    for splash in splashes_of(block):
        if splash.get('damage') is None and roll.get('damage') is not None:
            scale = splash.get('damage_scale')
            splash['damage'] = (1.0 if scale is None else scale) * roll['damage']
    return block


def close(a, b):
    """Equal, floats to within RATIO_TOLERANCE relative, dicts and lists by item."""
    if isinstance(a, dict) and isinstance(b, dict):
        return set(a) == set(b) and all(close(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(close(x, y) for x, y in zip(a, b))
    if isinstance(a, (int, float)) and isinstance(b, (int, float)) and not isinstance(a, bool) and not isinstance(b, bool):
        return abs(a - b) <= RATIO_TOLERANCE * max(abs(a), abs(b), 1e-9)
    return a == b


def differences(a, b, path=''):
    """Dotted paths where two blocks differ (see close)."""
    if isinstance(a, dict) and isinstance(b, dict):
        out = []
        for key in sorted(set(a) | set(b)):
            out += differences(a.get(key), b.get(key), f'{path}.{key}' if path else key)
        return out
    return [] if close(a, b) else [path]


def comparable(block, card_damage):
    """What the merge must reproduce: a block without the roll fields and the pool, with each splash's damage as the host's parser
    reads it (the evaluated damage, else card damage x damage_scale), and without the scale that produced it."""
    out = {key: copy.deepcopy(value) for key, value in block.items() if key not in ROLL_FIELDS and key != 'accuracy_pool'}
    for splash in splashes_of(out):
        scale = splash.pop('damage_scale', None)
        if splash.get('damage') is None and card_damage is not None:
            splash['damage'] = card_damage * (1.0 if scale is None else scale)
    return out


class Collector:
    """Templates by key, with the merge check on every roll."""

    def __init__(self):
        self.templates, self.rolls, self.problems = {}, 0, []

    def add(self, block, card, attributes, weapon_type, where):
        self.rolls += 1
        multiplier = attributes.get(SPEED_MULTIPLIER, 1.0)
        key = key_of(weapon_type, block['firing_mode'], card.get('element'))
        # The host builds its key from the card: the card and the resolver must name the same firing mode.
        if card.get('firing_mode') != block['firing_mode']:
            self.problems.append(f"{where}: firing mode: card {card.get('firing_mode')}, resolver {block['firing_mode']}")
        template = template_of(block, card.get('damage'), multiplier)
        kept = self.templates.setdefault(key, template)
        merged = apply_roll(kept, {'damage': card.get('damage'), 'speed_multiplier': multiplier})
        for path in differences(comparable(merged, card.get('damage')), comparable(block, card.get('damage'))):
            self.problems.append(f'{where}: {path}')


def add_fx(templates, package):
    """The cooked particle layers of each template (tools/weapon_projectile_fx.py), as a recipe has them."""
    cache = {}
    for template in templates.values():
        template['fx'] = weapon_projectile_fx.build(package, template, cache)
        if isinstance(template.get('reload_throw'), dict):
            template['reload_throw']['fx'] = weapon_projectile_fx.build(package, template['reload_throw'], cache)


def collect(package, source, localize, balances, stages=STAGES, seeds=SEEDS):
    collector = Collector()
    for balance in balances:
        for stage in stages:
            for seed in seeds:
                recipe = weapon_recipe.roll(package, balance, seed, stage, rng=weapon_recipe.CrtRand(seed))
                stats = weapon_stats.evaluate(package, recipe, recipe['game_stage'], localize)
                block = weapon_firing.build(source, recipe, stats)
                collector.add(block, stats['card'], stats['attributes'], recipe['weapon_type'], f'{balance} stage {stage} seed {seed}')
    return collector


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', type=Path, required=True, help='Borderlands 2 install folder')
    parser.add_argument('--output', type=Path, default=ROOT / 'local/weapons/gen/weapon_gen_firing.json')
    parser.add_argument('--manifest', type=Path, default=ROOT / 'local/weapons/gen/weapon_gen_manifest.txt')
    args = parser.parse_args()
    args.output, schema = output_paths(args.output)   # refused before anything is read, created or written
    manifest_md5 = fingerprint(args.manifest)
    cooked = args.game / 'WillowGame/CookedPCConsole'
    package = weapon_gen_manifest.open_package(args.reader, cooked / 'Startup.upk')
    localize = weapon_gen_manifest.make_localize(package, args.game)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    schema.write_text('\n'.join(dict.fromkeys(weapon_firing.SCHEMA_LINES + weapon_projectile_fx.SCHEMA_LINES)) + '\n', encoding='utf-8')
    reader = str(Path(args.reader).resolve())
    source = weapon_firing.Source(skill_stats.Package(reader, cooked / 'Startup.upk', str(schema.resolve())),
                                  skill_stats.Package(reader, cooked / 'WillowGame.upk', str(schema.resolve())))
    balances = sorted(p for p, c in package.classes.items() if c in weapon_gen_manifest.BALANCE_CLASSES)
    collector = collect(package, source, localize, balances)
    print(f'{collector.rolls} rolls, {len(collector.templates)} templates, {len(collector.problems)} merge differences')
    print(COMPARED)
    if collector.problems:
        for problem in collector.problems[:40]:
            print('  ' + problem)
        raise SystemExit('templates do not reproduce the resolver: nothing written')
    lacking = {key: incomplete(template) for key, template in collector.templates.items() if incomplete(template)}
    if lacking:
        for key, missing in list(lacking.items())[:40]:
            print(f'  {key}: {", ".join(missing)}')
        raise SystemExit(f'{len(lacking)} templates cannot fire: nothing written')
    add_fx(collector.templates, source.package)
    # Status rows come from the card, except the level-difference scale, which is global.
    document = {'schema': FILE_SCHEMA, 'manifest_md5': manifest_md5, 'level_chance_scales': weapon_firing.level_chance_scales(source), 'templates': collector.templates}
    args.output.write_text(json.dumps(document), encoding='utf-8')
    print(f'wrote {args.output}')


if __name__ == '__main__':
    main()
