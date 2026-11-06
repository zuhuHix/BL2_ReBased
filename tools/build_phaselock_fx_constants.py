"""Rebuild the ignored local/phaselock/fx_constants.json from regenerable local data.

AI-assisted (Claude). Writes only under this checkout's ignored local/. Nothing game-derived is tracked.

- The palm orb's blue: the inner-orb material's vector parameter default, in the decoded emitter JSON
  (research/particle_system.py, local/phaselock/emitters/).
- The tattoo glow: p_PowerEmissiveColor in local/phaselock/fx_manifest.json (tools/prepare_phaselock_fx.py).
- The bubble twirls' violet is NOT rebuilt. It exists only as a literal in the compiled pixel shader, so it stays hand-kept in
  the existing fx_constants.json (maintainer's option C, 2026-11-06). This tool keeps that value, and without one it stops and
  writes nothing. A tracked shader-map reader that could regenerate it (option B) comes after the slice.

Run: python tools/build_phaselock_fx_constants.py
"""
import argparse
import json
import os
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
LOCAL_ROOT = ROOT / 'local'
sys.path.insert(0, str(ROOT / 'host' / 'ue5'))
from phaselock_fx_inputs import load_fx_constants  # noqa: E402

ORB_TEMPLATE, ORB_MATERIAL = 'Part_SirenASHandOrb', 'Mat_SirenHandInnerOrb'
GLOW_PARAMETER = 'p_PowerEmissiveColor'
SOURCE_NOTE = ('Stock shader colours for host/ue5/import_phaselock_fx.py, rebuilt by tools/build_phaselock_fx_constants.py. '
               'Mat_SirenHandInnerOrb.blue and Master_Player.power_emissive come from regenerable local data. '
               'Mat_PowerUpTwirls.tint is hand-kept: it was read from the compiled pixel shader, and no tool regenerates it yet.')


class BuildError(Exception):
    """A required input is missing or malformed; nothing was written."""


def read_json(path, what):
    try:
        return json.loads(Path(path).read_text(encoding='utf-8'))
    except FileNotFoundError:
        raise BuildError(f'{what}: {path} is missing') from None
    except (OSError, ValueError) as error:
        raise BuildError(f'{what}: {path} is unreadable: {error}') from None


def rgb(colour, what):
    """[r, g, b] from a {R, G, B, ...} object."""
    if not isinstance(colour, dict) or not all(isinstance(colour.get(c), (int, float)) and not isinstance(colour.get(c), bool) for c in 'RGB'):
        raise BuildError(f'{what}: no R, G, B values')
    return [colour[c] for c in 'RGB']


def orb_blue(emitter_dir):
    what = f'{ORB_TEMPLATE} {ORB_MATERIAL}'
    data = read_json(Path(emitter_dir) / f'{ORB_TEMPLATE}.json', ORB_TEMPLATE)
    materials = data.get('materials') if isinstance(data, dict) else None
    if not isinstance(materials, dict):
        raise BuildError(f'{what}: the template has no materials object')
    matches = [entry for path, entry in materials.items() if path.rsplit('.', 1)[-1] == ORB_MATERIAL]
    if len(matches) != 1 or not isinstance(matches[0], dict):
        raise BuildError(f'{what}: expected one material entry, found {len(matches)}')
    vectors = [e for e in matches[0].get('parameter_expressions') or []
               if isinstance(e, dict) and e.get('class') == 'MaterialExpressionVectorParameter']
    if len(vectors) != 1:
        raise BuildError(f'{what}: expected one vector parameter, found {len(vectors)}')
    return rgb(vectors[0].get('DefaultValue'), what)


def power_emissive(manifest_path):
    data = read_json(manifest_path, 'manifest')
    glow = data.get('tattooGlow') if isinstance(data, dict) else None
    colours = glow.get('materialColours') if isinstance(glow, dict) else None
    if not isinstance(colours, dict) or GLOW_PARAMETER not in colours:
        raise BuildError(f'manifest {manifest_path}: no tattooGlow.materialColours.{GLOW_PARAMETER}')
    return rgb(colours[GLOW_PARAMETER], f'manifest {manifest_path} {GLOW_PARAMETER}')


def hand_kept_tint(out_path):
    """The existing hand-kept violet; stops when there is none, because no tool can rebuild it."""
    message = (f'{out_path} holds no Mat_PowerUpTwirls.tint, and no tool can rebuild it. Restore the file from another worktree or '
               'the maintainer; the value is also in the git history from before PR #104.')
    if not Path(out_path).is_file():
        raise BuildError(message)
    existing = read_json(out_path, 'existing constants')
    tint = existing.get('Mat_PowerUpTwirls', {}).get('tint') if isinstance(existing, dict) and isinstance(existing.get('Mat_PowerUpTwirls'), dict) else None
    if tint is None:
        raise BuildError(message)
    return tint


def build(emitter_dir, manifest_path, out_path, local_root=LOCAL_ROOT):
    """Rebuild out_path, which must be under local_root (game-derived output stays in ignored local/; tests pass a temporary
    root). Every input is read and the result checked before the file is replaced."""
    out_path = Path(out_path).resolve()
    if Path(local_root).resolve() not in out_path.parents:
        raise BuildError(f'{out_path} is outside {local_root}; game-derived output stays there')
    constants = {'_source': SOURCE_NOTE,
                 'Mat_SirenHandInnerOrb': {'blue': orb_blue(emitter_dir)},
                 'Mat_PowerUpTwirls': {'tint': hand_kept_tint(out_path)},
                 'Master_Player': {'power_emissive': power_emissive(manifest_path)}}
    temporary = out_path.with_name(out_path.name + '.tmp')
    temporary.write_text(json.dumps(constants, indent=1) + '\n', encoding='utf-8')
    try:
        problems = load_fx_constants(temporary)[1]
        if problems:
            raise BuildError('; '.join(problems).replace(str(temporary), str(out_path)))
        os.replace(temporary, out_path)
    finally:
        if temporary.exists():
            temporary.unlink()
    return constants


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--emitters', default=ROOT / 'local/phaselock/emitters')
    parser.add_argument('--manifest', default=ROOT / 'local/phaselock/fx_manifest.json')
    parser.add_argument('--out', default=ROOT / 'local/phaselock/fx_constants.json')
    args = parser.parse_args(argv)
    out = Path(args.out).resolve()
    try:
        build(args.emitters, args.manifest, out)
    except BuildError as error:
        print(f'build_phaselock_fx_constants: {error}', file=sys.stderr)
        return 1
    print(f'build_phaselock_fx_constants: wrote {out} (blue and glow rebuilt, tint kept)')
    return 0


if __name__ == '__main__':
    sys.exit(main())
