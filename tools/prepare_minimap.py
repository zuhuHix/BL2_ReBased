"""Prepare the HUD minimap of a map for the overlay and the host (docs/verification/HUD_MINIMAP.md).

Reads the installed packages with ow-package and writes, under ignored local/ (game data, docs/LEGAL.md):

- the level's tactical map movie (WillowMapInfo.TacticalMapMovie, e.g. UI_TacticalMap_Sanctuary.Sanctuary_P) and the
  shared fog-of-war movie it imports (SharedWillowTacMaps), converted with tools/gfx_to_swf.py into <output>, where the
  overlay server answers "/ package/<Package>/<Movie>". The map movie's one image record names no export, so its
  texture (<Movie>_I1) is decoded first under that file name (as tools/prepare_hud_icons.py does);
- a manifest (--manifest) for the host's -owminimap: the movie path, the centre of the WillowTacticalMapVolume's
  bounds (the map movie's origin, checked against the live game), the discovery areas' fog blobs, and one icon per
  world object whose interactive-object definition carries a CompassIcon the minimap shows, at its location:
  placed actors (InteractiveObjectDefinition) and population opportunity points (PopulationDef -> its factory ->
  ObjectBalanceDefinition -> DefaultInteractiveObject).

What is read and what is assumed:
- the volume centre is its BrushComponent's convex element box (BrushAggGeom) moved by the actor's Location; the
  volume carries no Rotation or DrawScale in Sanctuary, and one that does is refused;
- the spawned object stands at its opportunity point's Location (true for all seven points the live game showed);
- objects spawned by other means (Kismet, other levels' populations) are not found: the record lists what the
  live game showed and this tool did not;
- the fog blobs are the map movie's root instances named <MAP>_PWDA_<n> (the names the game's DiscoveredWorldAreas
  use; WorldDiscoveryArea actors store no name, so how the game derives it is not read), every one "not uncovered" (the host has no discovery state; one save observed: UNVERIFIED for others).
"""
import argparse
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from prepare_skill_tree import Package, convert_movie  # noqa: E402
from prepare_slice_world import Reader, levels, ref, vector  # noqa: E402
from gfx_to_swf import movie_tags  # noqa: E402

SCHEMA = Path(__file__).with_name('collision-arrays.schema')
# InteractiveObjectDefinition.CompassIcon values (ERadarIconType names as decoded) -> the overlay's icon kinds
# (tools/hud_overlay/minimap.js). Which widget array each feeds was matched against the live game's icon slots.
KINDS = {
    'RadarIconType_Shop': 'shop',
    'RadarIconType_Health': 'health',
    'RadarIconType_TravelStation': 'fasttravel',
    'RadarIconType_CustomizationStation': 'customize',
    'RadarIconType_LevelTransition': 'leveltravel',
}
PLACED = ('WillowInteractiveObject', 'FastTravelStation', 'LevelTravelStation', 'ResurrectTravelStation',
          'WillowVendingMachine', 'WillowVendingMachineBlackMarket')


def _skip_bits(data, pos, fields):
    """Byte position after a bit-packed record: `fields` lists (count, width) groups as in the SWF spec, where a width
    of None means "read the field width from the next 5 bits"; flag bits are (1, 1)."""
    bit = pos * 8

    def take(n):
        nonlocal bit
        value = 0
        for _ in range(n):
            value = (value << 1) | ((data[bit // 8] >> (7 - bit % 8)) & 1)
            bit += 1
        return value
    for kind in fields:
        kind(take)
    return (bit + 7) // 8


def _matrix(take):
    if take(1):
        n = take(5); take(2 * n)
    if take(1):
        n = take(5); take(2 * n)
    n = take(5); take(2 * n)


def _cxform_alpha(take):
    add, mult = take(1), take(1)
    n = take(4)
    take(4 * n * (add + mult))


def placed_names(body, code):
    """Instance name of a PlaceObject2 (26) or PlaceObject3 (70) tag, or None (Adobe SWF spec field order)."""
    flags = body[0]
    pos = 1
    flags3 = 0
    if code == 70:
        flags3 = body[1]
        pos = 2
    pos += 2                                         # depth
    if code == 70 and (flags3 & 0x08 or (flags3 & 0x10 and flags & 0x02)):
        pos = body.index(b'\0', pos) + 1             # class name
    if flags & 0x02:
        pos += 2                                     # character id
    if flags & 0x04:
        pos = _skip_bits(body, pos, [_matrix])
    if flags & 0x08:
        pos = _skip_bits(body, pos, [_cxform_alpha])
    if flags & 0x10:
        pos += 2                                     # ratio
    if flags & 0x20:
        end = body.index(b'\0', pos)
        return body[pos:end].decode('latin-1')
    return None


def fog_blobs(swf_bytes):
    """Names of the fog-of-war blob instances (<MAP>_PWDA_<n>) placed on the movie's root timeline."""
    _, _, tags = movie_tags(swf_bytes)
    names = {placed_names(body, code) for code, body in tags if code in (26, 70)}
    return sorted(n for n in names if n and re.fullmatch(r'[A-Za-z0-9]+_PWDA_[0-9]+', n))


def volume_centre(reader, pkg, path):
    """Centre (X, Y) of a brush volume's bounds in world units."""
    actor = reader.props(pkg, path)
    for field in ('Rotation', 'DrawScale', 'DrawScale3D', 'PrePivot'):
        if field in actor:
            raise ValueError(f'{path}: carries {field}; how it composes with the brush is not modelled')
    component = reader.props(pkg, ref(actor.get('BrushComponent')), allow_tail=True)
    elems = (component.get('BrushAggGeom') or {}).get('ConvexElems') or []
    if not elems:
        raise ValueError(f'{path}: no convex element in BrushAggGeom')
    lo = [min(e['ElemBox']['Min'][a] for e in elems) for a in 'XY']
    hi = [max(e['ElemBox']['Max'][a] for e in elems) for a in 'XY']
    location = vector(actor.get('Location'))
    return [location[i] + (lo[i] + hi[i]) / 2 for i in range(2)]


def compass(reader, pkg, definition):
    if not definition or definition not in reader.exports(pkg)['by_path']:
        return None
    return reader.props(pkg, definition).get('CompassIcon')


def population_definition(reader, pkg, popdef):
    """PopulationDef -> the interactive object definition its factory spawns (None when it spawns none)."""
    exports = reader.exports(pkg)['by_path']
    for path, x in exports.items():
        if path.startswith(popdef + '.') and 'PopulationFactory' in x['class']:
            props = reader.props(pkg, path)
            definition = ref(props.get('InteractiveObjectDefinition'))
            balance = ref(props.get('ObjectBalanceDefinition'))
            if not definition and balance and balance in exports:
                definition = ref(reader.props(pkg, balance).get('DefaultInteractiveObject'))
            if definition:
                return definition
    return None


def collect(reader, persistent):
    icons, skipped, notes = [], {}, []
    centre, movie = None, None
    for pkg in levels(reader, persistent):
        for x in reader.exports(pkg)['by_index'].values():
            path, cls = x['path'], x['class'].rsplit('.', 1)[-1]
            if path.count('.') != 2 or not path.startswith('TheWorld.PersistentLevel.'):
                continue
            try:
                if cls in PLACED:
                    props = reader.props(pkg, path)
                    definition = ref(props.get('InteractiveObjectDefinition'))
                    source = path
                elif cls == 'WillowPopulationOpportunityPoint':
                    props = reader.props(pkg, path)
                    popdef = ref(props.get('PopulationDef'))
                    definition = population_definition(reader, pkg, popdef) if popdef else None
                    source = f'{path} ({popdef})'
                else:
                    continue
                icon = compass(reader, pkg, definition)
                kind = KINDS.get(icon)
                if not kind:
                    skipped[icon or 'none'] = skipped.get(icon or 'none', 0) + 1
                    continue
                icons.append({'kind': kind, 'location': vector(props.get('Location')), 'definition': definition,
                              'compass': icon, 'source': f'{pkg}:{source}'})
            except ValueError as error:
                notes.append(f'{pkg}:{path}: {error}')
        # The map info lives under WorldInfo, one level deeper than the actors.
        for x in reader.exports(pkg)['by_index'].values():
            if x['class'] == 'WillowGame.WillowMapInfo' and movie is None:
                info = reader.props(pkg, x['path'])
                volume = ref(info.get('TacticalMapVolume'))
                if ref(info.get('TacticalMapMovie')) and volume:
                    movie = ref(info.get('TacticalMapMovie'))
                    centre = volume_centre(reader, pkg, volume)
    return {'movie': movie, 'centre': centre, 'icons': icons, 'skipped_compass_icons': skipped,
            'notes': notes}


REPO = Path(__file__).resolve().parents[1]


def local_path(value, repo=REPO):
    """`value` resolved (relative paths against the repository root) and refused unless it lies inside the repository's
    ignored local/ (local/ itself is refused too, since the movie folder's parent also receives the extracted movies
    and textures): everything this tool writes is game data (docs/LEGAL.md)."""
    local = (Path(repo) / 'local').resolve()
    path = Path(value)
    path = (path if path.is_absolute() else Path(repo) / path).resolve()
    if path == local or local not in path.parents:
        raise ValueError(f'{value}: output must be inside {local}; game data stays in local/')
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--map', default='Sanctuary_P')
    parser.add_argument('--output', default='local/ui/run', help='overlay movie folder (serve.py --movies)')
    parser.add_argument('--manifest', default='local/slice/minimap.json')
    args = parser.parse_args()
    try:
        output, manifest_path = local_path(args.output), local_path(args.manifest)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    reader = Reader(args.reader, args.game, str(SCHEMA))
    data = collect(reader, args.map)
    if not data['movie'] or not data['centre']:
        print(f'{args.map}: no WillowMapInfo with a tactical map movie and volume', file=sys.stderr)
        return 1
    package = Package(str(Path(args.reader).resolve()), reader.cooked / f'{args.map}.upk')
    owner, name = data['movie'].split('.', 1)
    texture = f'{data["movie"]}_I1'
    if package.classes.get(texture) == 'Engine.Texture2D':
        folder = output.parent / 'tex' / owner
        folder.mkdir(parents=True, exist_ok=True)
        package.run('--texture', package.index[texture], '--property-offset', 4,
                    '--output', folder / f'{name}_I1.png', '--tfc', package.path.parent)
    written = [convert_movie(package, data['movie'], output, output.parent)]
    data['blobs'] = {blob: False for blob in fog_blobs(written[0].read_bytes())}
    # The fog-of-war blob symbol the map movie imports (SharedWillowTacMaps) is cooked into the same package.
    for shared in [p for p, c in package.classes.items() if c == 'GFxUI.SwfMovie' and p.startswith('SharedWillowTacMaps.')]:
        written.append(convert_movie(package, shared, output, output.parent))
    manifest = {'map': args.map, 'movie': f'/ package/{owner}/{name}', 'centre': data['centre'],
                'blobs': data['blobs'], 'icons': data['icons'], 'skipped_compass_icons': data['skipped_compass_icons'],
                'notes': data['notes']}
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=1), encoding='utf-8')
    counts = {}
    for icon in data['icons']:
        counts[icon['kind']] = counts.get(icon['kind'], 0) + 1
    print(f'wrote {", ".join(map(str, written))} and {manifest_path}: centre {data["centre"]}, icons {counts}, '
          f'blobs {sorted(data["blobs"])}, skipped {data["skipped_compass_icons"]}, notes {len(data["notes"])}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
