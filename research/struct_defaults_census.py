"""
struct_defaults_census.py - where a ScriptStruct export keeps its default property values.

AI-assisted. Clean-room: no game data in this file; output goes to stdout / ignored local/.

Hypothesis under test (FITTED from the packages, see docs/verification/BEHAVIOR_DATA_DECODE.md):
a ScriptStruct export is [header of fixed size with a script-size word][script bytes][StructFlags u32]
[tagged default properties ending in None], and that tag stream ends exactly at the end of the export.
The offsets below were read off one export (Engine AttributeInitializationData) and are then checked on
every ScriptStruct of the listed packages: the stream must parse with the self-describing tag walker and
end exactly at the export end.
"""
import argparse
import collections
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import behavior_census as bc  # noqa: E402

SCRIPT_SIZE_AT = 44      # u32 script (storage) size
SCRIPT_AT = 48           # script bytes, then StructFlags u32, then the default tags


def defaults_offset(data, start):
    size = struct.unpack_from('<i', data, start + SCRIPT_SIZE_AT)[0]
    return start + SCRIPT_AT + size + 4, size


def main():
    ap = argparse.ArgumentParser(description='ScriptStruct default-property census')
    ap.add_argument('--packages', nargs='*', default=['Core', 'Engine', 'GameFramework', 'GearboxFramework', 'WillowGame'])
    ap.add_argument('--show', type=int, default=5)
    args = ap.parse_args()
    totals = collections.Counter()
    for name in args.packages:
        pkg = bc.Pkg(os.path.join(bc.GAME, name + '.upk'))
        failures = []
        for idx, e in enumerate(pkg.exports, 1):
            c = e['class']
            if c >= 0 or pkg.imports[-c - 1]['name'] != 'ScriptStruct':
                continue
            totals['structs'] += 1
            start, end = e['off'], e['off'] + e['size']
            try:
                at, size = defaults_offset(pkg.data, start)
                props, after = bc.walk_tags(pkg, pkg.data, at, end)
                if after != end:
                    raise ValueError(f'ends at {after - start} of {e["size"]}')
                totals['exact'] += 1
                totals['with_defaults' if props else 'empty_defaults'] += 1
                if size:
                    totals['with_script'] += 1
            except Exception as error:  # noqa: BLE001 - counted and shown, never dropped
                failures.append((pkg.path(idx), str(error)))
        totals['failures'] += len(failures)
        print(name, 'failures', len(failures), failures[:args.show])
    print(dict(totals))


if __name__ == '__main__':
    main()
