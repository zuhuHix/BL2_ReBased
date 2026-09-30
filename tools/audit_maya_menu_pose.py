"""Audit Maya's local hold/AnimSet references using the existing package CLI.

Does not decode new layouts or choose menu clips. Unsupported WeaponActions
remain explicit. All package-derived output stays under ignored local/.
"""
import argparse
import json
from pathlib import Path
import subprocess
import time


def run(reader, package, *options):
    return json.loads(subprocess.check_output(
        [str(reader), str(package), *map(str, options)], text=True))


def audit(reader, cooked, output):
    started = time.perf_counter()
    reflection = run(reader, cooked / 'WillowGame.upk', '--exports')
    inner = next(row for row in reflection
                 if row['path'] == 'BodyWeaponHoldDefinition.AnimSetList.AnimSetList')
    if inner['class'] != 'Core.ObjectProperty':
        raise ValueError('AnimSetList reflected inner type is not ObjectProperty')
    # Supply metadata to the existing bounded decoder, not a new byte layout.
    schema = output.parent / 'hold-arrays.schema'
    schema.write_text('AnimSetList=ObjectProperty\n')
    package = cooked / 'GD_Siren_Streaming_SF.upk'
    exports = run(reader, package, '--exports')
    by_index = {row['index']: row for row in exports}
    holds = []
    for row in exports:
        if row['class'] != 'WillowGame.BodyWeaponHoldDefinition' or not row['path'].startswith(
                'GD_Siren_Streaming.WeaponHolds.'):
            continue
        data = run(reader, package, '--properties', row['index'], '--property-offset', 4,
                   '--array-schema', schema)
        if data['trailing_bytes'] != 0:
            raise ValueError(f"Unconsumed hold bytes: {row['path']}")
        props = {prop['name']: prop for prop in data['properties']}
        references = props.get('AnimSetList')
        resolved = []
        if references:
            if references['status'] != 'decoded':
                raise ValueError(f"Unsupported AnimSetList: {row['path']}")
            for ref in references['value']:
                target = by_index.get(ref['index'])
                if not target or target['path'] != ref['path'] or target['class'] != 'Engine.AnimSet':
                    raise ValueError(f"Invalid local AnimSet reference: {row['path']}")
                resolved.append(target['path'])
        holds.append({'path': row['path'], 'anim_sets': resolved,
                      'unsupported': [name for name, prop in props.items()
                                      if prop['status'] == 'unsupported'],
                      'properties': data})
    if not holds:
        raise ValueError('No Maya third-person hold definitions found')
    report = {'reflection_inner': inner, 'holds': holds,
              'elapsed_seconds': time.perf_counter() - started,
              'menu_action_selection_verified': False, 'ik_verified': False}
    output.write_text(json.dumps(report, indent=2))
    for hold in holds:
        print(f"{hold['path'].split('.')[-1]}: {hold['anim_sets']}; unsupported={hold['unsupported']}")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    local = Path(__file__).resolve().parents[1] / 'local'
    if not args.output.resolve().is_relative_to(local.resolve()):
        parser.error('Output must remain under local/')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    audit(args.reader.resolve(), args.game / 'WillowGame/CookedPCConsole', args.output)
