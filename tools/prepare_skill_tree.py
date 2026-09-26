"""Prepare a character's skill tree for the StatusMenu skills screen overlay.

Reads the class's SkillTreeDefinition from the installed packages through
`ow-package --properties`, and writes, under ignored local/:
- `<output>/skilltree_<name>.json`: class and branch names, tiers with their
  unlock points, and per skill its name, description (as the HTML the movie
  shows), max grade, kill-skill flag, icon movie path and cell position;
- the skill icon movies and the class portrait, converted to SWF with
  tools/extract_swfmovie.py and tools/gfx_to_swf.py (textures decoded with
  `ow-package --texture`) at `<output>/<Package>/<Movie>.swf`, which the
  overlay server maps from the "/ package/<Package>/<Movie>" paths the game
  uses (the portrait path was observed in a UI trace, DECISIONS.md 2026-09-26).

All of it is game data and stays local (docs/LEGAL.md). Rules below are
observations, not read from Gearbox's scripts:
- Cell columns follow the tier's skill count: one skill in column 1, two in
  columns 0 and 2, three in 0, 1, 2. This matched all 28 tree cells hovered in
  two traces; the layout's own bCellIsOccupied (a bool array) is not decoded yet.
- Branch order is the root branch's Children order (Motion, Harmony, Cataclysm
  for Maya), which matched the traced branch numbers.
- `[skill]X[-skill]` becomes `<font color="#FFDEAD"><i>X</i></font>`, as in the
  traced SetInfo text. The per-grade "Next Level" block is not produced: it
  needs the skills' attribute math, which is not evaluated yet.
"""
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from extract_swfmovie import extract  # noqa: E402

SCHEMA_LINES = [
    'Children=ObjectProperty',
    'Tiers=StructProperty:SkillTreeTier',
    'Skills=ObjectProperty',
]
SKILL_TAG = re.compile(r'\[skill\](.*?)\[-skill\]', re.S)


class Package:
    def __init__(self, reader, path, schema=None):
        self.reader, self.path, self.schema = reader, Path(path), schema
        exports = json.loads(self.run('--exports'))
        self.index = {e['path']: e['index'] for e in exports}
        self.classes = {e['path']: e['class'] for e in exports}

    def run(self, *args):
        result = subprocess.run([self.reader, str(self.path), *map(str, args)], capture_output=True, text=True)
        if result.returncode:
            raise RuntimeError(f'ow-package {args}: {result.stderr.strip()}')
        return result.stdout

    def props(self, path):
        args = ['--properties', self.index[path], '--property-offset', 4]
        if self.schema:
            args += ['--array-schema', self.schema]
        data = json.loads(self.run(*args))
        return {p['name']: plain(p.get('value')) for p in data['properties'] if p.get('status') == 'decoded'}


def plain(value):
    """ow-package property JSON -> Python values (structs become dicts, objects their path)."""
    if isinstance(value, list):
        if value and all(isinstance(v, dict) and 'name' in v for v in value):
            return {v['name']: plain(v.get('value')) for v in value if v.get('status') == 'decoded'}
        return [plain(v) for v in value]
    if isinstance(value, dict) and 'path' in value:
        return value['path']
    return value


def read_int(localization, file, section, key):
    """One value from an install .int file (UTF-16)."""
    current = None
    for line in (Path(localization) / file).read_text(encoding='utf-16').splitlines():
        if line.startswith('['):
            current = line.strip()[1:-1]
        elif current == section and line.startswith(key + '='):
            return line.split('=', 1)[1].strip()
    raise KeyError(f'{file} [{section}] {key}')


def description_html(text):
    return SKILL_TAG.sub(r'<font color="#FFDEAD"><i>\1</i></font>', text or '')


def columns(count):
    return {1: [1], 2: [0, 2], 3: [0, 1, 2]}[count]


def movie_url(path):
    package, name = path.split('.', 1)
    return f'/ package/{package}/{name}'


def convert_movie(package, path, output, local):
    """Extract a SwfMovie and convert it, decoding the textures it reports missing."""
    owner, name = path.split('.', 1)
    gfx = local / 'gfx' / owner / f'{name}.gfx'
    textures = local / 'tex' / owner
    swf = output / owner / f'{name}.swf'
    for folder in (gfx.parent, textures, swf.parent):
        folder.mkdir(parents=True, exist_ok=True)
    data, _ = extract(package.reader, package.path, path)
    gfx.write_bytes(data)
    convert = [sys.executable, str(Path(__file__).parent / 'gfx_to_swf.py'), '--gfx', str(gfx),
               '--textures', str(textures), '--output', str(swf)]
    for _ in range(2):
        subprocess.run(convert, check=True, capture_output=True, text=True)
        report = json.loads(swf.with_suffix('.json').read_text(encoding='utf-8'))
        missing = [i for i in report['external_images'] if not i.get('found')]
        if not missing:
            return swf
        for image in missing:
            texture = f'{owner}.{image["export"]}'
            if package.classes.get(texture) != 'Engine.Texture2D':
                raise RuntimeError(f'{path}: texture {texture} is not an export of {package.path.name}')
            package.run('--texture', package.index[texture], '--property-offset', 4,
                        '--output', textures / (Path(image['file']).stem + '.png'), '--tfc', package.path.parent)
    raise RuntimeError(f'{path}: textures still missing after decoding: {[i["file"] for i in missing]}')


def skill_entry(package, path, column, converted, output, local):
    props = package.props(path)
    icon = props.get('SkillIcon')
    if icon and icon not in converted:
        convert_movie(package, icon, output, local)
        converted.add(icon)
    return {
        'id': path,
        'name': props.get('SkillName', ''),
        'description': description_html(props.get('SkillDescription')),
        'maxGrade': props.get('MaxGrade', 1),
        'killSkill': props.get('SkillType') == 'SKILL_TYPE_Kill',
        'icon': movie_url(icon) if icon else '',
        'cell': column,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--package', default='GD_Siren_Streaming_SF.upk')
    parser.add_argument('--tree', default='GD_Siren_Streaming.SkillTree.SkillTree_Siren')
    parser.add_argument('--class-string', default='SkillsSirenClassString',
                        help='[SkillTreeMovie] key in WillowGame.int for the class name')
    parser.add_argument('--portrait', default='UI_CharacterPortraits.Siren')
    parser.add_argument('--name', default='siren')
    parser.add_argument('--output', default='local/ui/run')
    args = parser.parse_args()

    game = Path(args.game)
    cooked = game / 'WillowGame' / 'CookedPCConsole'
    localization = game / 'WillowGame' / 'Localization' / 'INT'
    output = Path(args.output)
    local = output.parent
    schema = local / 'skilltree.schema'
    schema.parent.mkdir(parents=True, exist_ok=True)
    schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')
    reader = str(Path(args.reader).resolve())
    package = Package(reader, cooked / args.package, str(schema.resolve()))

    converted = set()
    root = package.props(package.props(args.tree)['Root'])
    action = skill_entry(package, root['Tiers'][0]['Skills'][0], 0, converted, output, local)
    branches = []
    for branch_path in root['Children']:
        branch = package.props(branch_path)
        tiers = []
        for tier in branch['Tiers']:
            skills = tier.get('Skills') or []
            tiers.append({
                'pointsToUnlockNext': tier.get('PointsToUnlockNextTier', 0),
                'skills': [skill_entry(package, s, c, converted, output, local)
                           for s, c in zip(skills, columns(len(skills)))],
            })
        branches.append({'id': branch_path, 'name': branch.get('BranchName', ''), 'tiers': tiers})

    startup = Package(reader, cooked / 'Startup.upk')
    convert_movie(startup, args.portrait, output, local)

    tree = {
        'className': read_int(localization, 'WillowGame.int', 'SkillTreeMovie', args.class_string),
        'pointsTitle': read_int(localization, 'WillowGame.int', 'SkillTreeGFxObject', 'SkillPointsRemainingString'),
        'portrait': movie_url(args.portrait),
        'actionSkill': action,
        'actionSkillPoints': root['Tiers'][0].get('PointsToUnlockNextTier', 1),
        'branches': branches,
    }
    target = output / f'skilltree_{args.name}.json'
    target.write_text(json.dumps(tree, indent=1, ensure_ascii=False), encoding='utf-8')
    count = sum(len(t['skills']) for b in branches for t in b['tiers'])
    print(f'wrote {target}: {len(branches)} branches, {count} skills, {len(converted) + 1} movies converted')


if __name__ == '__main__':
    main()
