"""Convert the HUD's action-skill icon movie for the HUD overlay.

The HUD movie's action-skill clip (`p1.character.cooldown.icon.placeholderTexture`)
shows a stand-in until the game loads the class's icon movie into it. That movie
is the `GFxUI.SwfMovie` `UI_HUD_AAIcons.AA-<Class>` (found by export name; which
call loads it in the game is native and not read). This writes it, converted with
tools/extract_swfmovie.py and tools/gfx_to_swf.py through prepare_skill_tree's
`convert_movie`, to `<output>/UI_HUD_AAIcons/AA-<Class>.swf`, which the overlay
server maps from "/ package/UI_HUD_AAIcons/AA-<Class>". Its one external image
record names no export (only the file `AA-<Class>_I1.tga`), so the texture
`UI_HUD_AAIcons.AA-<Class>_I1` is decoded first under that file name.

Game data: the output stays under ignored local/ (docs/LEGAL.md).
"""
import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from prepare_skill_tree import Package, convert_movie  # noqa: E402

PACKAGES = ('Startup.upk', 'WillowGame.upk')


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--movie', default='UI_HUD_AAIcons.AA-Siren')
    parser.add_argument('--output', default='local/ui/run')
    args = parser.parse_args()
    cooked = Path(args.game) / 'WillowGame' / 'CookedPCConsole'
    output = Path(args.output)
    reader = str(Path(args.reader).resolve())
    for name in PACKAGES:
        package = Package(reader, cooked / name)
        if package.classes.get(args.movie) == 'GFxUI.SwfMovie':
            owner, movie = args.movie.split('.', 1)
            texture = f'{args.movie}_I1'
            if package.classes.get(texture) == 'Engine.Texture2D':
                folder = output.parent / 'tex' / owner
                folder.mkdir(parents=True, exist_ok=True)
                package.run('--texture', package.index[texture], '--property-offset', 4,
                            '--output', folder / f'{movie}_I1.png', '--tfc', package.path.parent)
            print(f'wrote {convert_movie(package, args.movie, output, output.parent)} (from {name})')
            return 0
    print(f'{args.movie} is not a SwfMovie export of {", ".join(PACKAGES)}', file=sys.stderr)
    return 1


if __name__ == '__main__':
    sys.exit(main())
