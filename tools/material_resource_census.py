"""Inspect bounded Material resources; opaque bytes are not shader graphs.

Uses the existing reader and observed prefix only. Game-derived reports belong
under ignored local/. No offsets are searched and no rendering policy changes.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct

from prepare_level import Scene, cooked_texture_references, props


def observed_tail_layout(tail):
    """Observed 832/46 structure only; all field semantics are UNVERIFIED.

    Six words, count, count * 16-byte records, final word. Exact consumption
    is required: another layout must not be silently accepted as this one.
    """
    if len(tail) < 32:
        raise ValueError('Truncated observed Material tail')
    count = struct.unpack_from('<i', tail, 24)[0]
    if count < 0 or count > (len(tail) - 32) // 16:
        raise ValueError('Invalid observed Material tail record count')
    if len(tail) != 32 + count * 16:
        raise ValueError('Unsupported observed Material tail length')
    return {'status': 'structure_observed_semantics_UNVERIFIED',
            'prefix_words_u32': list(struct.unpack_from('<6I', tail)),
            'record_count': count,
            'records_u32': [list(struct.unpack_from('<4I', tail, 28 + i * 16))
                            for i in range(count)],
            'final_word_u32': struct.unpack_from('<I', tail, len(tail) - 4)[0]}


def inspect_resource(payload, data):
    refs, opaque = cooked_texture_references(payload, data)
    start = data['property_offset'] + data['consumed_bytes']
    end = len(payload) - opaque
    tail = payload[end:]
    # These are raw words, deliberately without guessed semantic field names.
    words = list(struct.unpack('<' + 'I' * (len(tail) // 4), tail[:len(tail) // 4 * 4]))
    result = {
        'resource_offset': start,
        'texture_references': refs,
        'opaque_offset': end,
        'opaque_bytes': opaque,
        'opaque_sha256': hashlib.sha256(tail).hexdigest(),
        'opaque_words_u32': words,
        'opaque_remainder_hex': tail[len(words) * 4:].hex(),
        'graph_status': 'unreconstructed',
    }
    try:
        result['observed_tail_layout'] = observed_tail_layout(tail)
    except ValueError as error:
        result['tail_layout_error'] = str(error)
    return result


def census(scene, packages):
    materials, errors = [], []
    for package in packages:
        try:
            rows = {i: r for i, r in scene.load(package).items()
                    if r.get('class', '').rsplit('.', 1)[-1] == 'Material'}
            payloads = {}
            indices = list(rows)
            # Bound command length on Windows; each batch loads the map once.
            for start in range(0, len(indices), 128):
                payloads.update({r['index']: bytes(r['payload']) for r in
                    scene.call(package, '--payloads', *indices[start:start + 128])})
        except ValueError as error:
            errors.append({'source': package, 'error': str(error)})
            continue
        for index, record in rows.items():
            identity = scene.identity((package, index))
            try:
                payload = payloads[index]
                result = inspect_resource(payload, record['data'])
                expressions = props(record).get('Expressions')
                # Absent and explicitly empty/stripped arrays remain distinct.
                result['expression_slots'] = len(expressions) if expressions is not None else None
                result['non_null_expression_slots'] = (sum(bool(e.get('index', 0))
                    if isinstance(e, dict) else bool(e) for e in expressions)
                    if expressions is not None else None)
                result['source'] = identity
                materials.append(result)
            except (ValueError, KeyError, TypeError) as error:
                errors.append({'source': identity, 'error': str(error)})
        print(f'{package}: inspected {len(rows)} Material exports', flush=True)
    lengths = Counter(m['opaque_bytes'] for m in materials)
    return {'schema': 1, 'status': 'observations_only', 'packages': packages,
            'materials': materials, 'errors': errors,
            'summary': {'materials': len(materials), 'errors': len(errors),
                        'opaque_byte_histogram': dict(sorted(lengths.items())),
                        'unique_opaque_payloads': len({m['opaque_sha256'] for m in materials}),
                        'recognized_tail_layouts': sum('observed_tail_layout' in m for m in materials),
                        'unrecognized_tail_layouts': sum('tail_layout_error' in m for m in materials),
                        'materials_with_no_surviving_expression_slots': sum(
                            m['non_null_expression_slots'] == 0 for m in materials)}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--package', action='append', required=True)
    parser.add_argument('--output', type=Path, default=Path('local/material-resources'))
    args = parser.parse_args()
    local = Path(__file__).resolve().parents[1] / 'local'
    if not args.output.resolve().is_relative_to(local):
        parser.error('Game-derived output must stay under this checkout\'s ignored local/')
    if not (args.game / 'Binaries/Win32/Borderlands2.exe').is_file():
        parser.error('An installed Borderlands 2 is required')
    report = census(Scene(args.reader, args.game, args.output), args.package)
    (args.output / 'material_resources.json').write_text(
        json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report['summary'], indent=2))
    if report['errors'] or report['summary']['unrecognized_tail_layouts']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
