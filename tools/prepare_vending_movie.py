"""Convert the stock vending machine movie for the vending page (tools/hud_overlay/vending.js).

Reads the installed package that holds the movie (default Sanctuary_P, where the vending machines are placed) with
ow-package, extracts GFxUI.SwfMovie `UI_VendingMachine.VendingMachineInterface` (tools/extract_swfmovie.py) and
converts it with tools/gfx_to_swf.py into `<output>/UI_VendingMachine/VendingMachineInterface.swf`, where the overlay
server (tools/hud_overlay/serve.py) answers "/ package/UI_VendingMachine/VendingMachineInterface".

Textures: an image record that names an export is decoded from `<Movie owner>.<export>`; an image record with no
export name (the movie's own pictures, e.g. VendingMachineInterface_I7) is decoded from `<owner>.<file stem>`, the
name the package uses (as tools/prepare_minimap.py does). Images the package does not hold are left out and the
converter draws them transparent (the report lists them).

All of it is game data and stays under ignored local/ (docs/LEGAL.md, "UI movies"). The page only uses the movie as
frame art; its layout is UNVERIFIED until compared with the live game, and no 3D projection is tuned here.
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from extract_swfmovie import extract  # noqa: E402
from prepare_skill_tree import Package  # noqa: E402
from prepare_slice_world import Reader  # noqa: E402

REPO = Path(__file__).resolve().parents[1]
MOVIE = 'UI_VendingMachine.VendingMachineInterface'
CONVERTER = Path(__file__).with_name('gfx_to_swf.py')


def local_path(value, repo=REPO):
    """Resolve an output path and refuse anything outside this checkout's ignored local/."""
    path = Path(value) if Path(value).is_absolute() else Path(repo) / value
    path = path.resolve()
    local = (Path(repo) / 'local').resolve()
    if path == local or local not in path.parents:
        raise ValueError(f'{value}: output must be inside {local}; game data stays in local/')
    return path


def convert(gfx, textures, swf):
    subprocess.run([sys.executable, str(CONVERTER), '--gfx', str(gfx), '--textures', str(textures),
                    '--output', str(swf)], check=True, capture_output=True, text=True)
    return json.loads(swf.with_suffix('.json').read_text(encoding='utf-8'))


def decode_textures(package, movie, report, textures):
    """Decode each missing image the package holds; return the file names it could not find."""
    owner = movie.split('.', 1)[0]
    unresolved = []
    for image in report['external_images']:
        if image.get('found'):
            continue
        stem = Path(image['file']).stem
        texture = f'{owner}.{image["export"] or stem}'
        if package.classes.get(texture) != 'Engine.Texture2D':
            unresolved.append(image['file'])
            continue
        package.run('--texture', package.index[texture], '--property-offset', 4,
                    '--output', textures / f'{stem}.png', '--tfc', package.path.parent)
    return unresolved


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--package', default='Sanctuary_P', help='package that holds the movie (CookedPCConsole)')
    parser.add_argument('--movie', default=MOVIE)
    parser.add_argument('--output', default='local/ui/run', help='overlay movie folder (serve.py --movies)')
    args = parser.parse_args()
    try:
        output = local_path(args.output)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    reader = Reader(args.reader, args.game, None)
    package = Package(str(Path(args.reader).resolve()), reader.cooked / f'{args.package}.upk')
    if package.classes.get(args.movie) != 'GFxUI.SwfMovie':
        print(f'{args.movie} is not a GFxUI.SwfMovie export of {args.package}', file=sys.stderr)
        return 1
    owner, name = args.movie.split('.', 1)
    gfx = output.parent / 'gfx' / owner / f'{name}.gfx'
    textures = output.parent / 'tex' / owner
    swf = output / owner / f'{name}.swf'
    for folder in (gfx.parent, textures, swf.parent):
        folder.mkdir(parents=True, exist_ok=True)
    data, _ = extract(package.reader, package.path, args.movie)
    gfx.write_bytes(data)
    report = convert(gfx, textures, swf)
    unresolved = decode_textures(package, args.movie, report, textures)
    if len(unresolved) < sum(1 for i in report['external_images'] if not i.get('found')):
        report = convert(gfx, textures, swf)
    print(json.dumps({'swf': str(swf), 'bytes': swf.stat().st_size, 'images': len(report['external_images']),
                      'images_missing_after_decode': unresolved}))
    return 0


if __name__ == '__main__':
    sys.exit(main())
