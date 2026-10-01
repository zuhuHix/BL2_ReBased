"""
behavior_census.py - structural census of BehaviorProviderDefinition data in the cooked packages.

AI-assisted. Clean-room: no game data in this file. Everything derived from the installed game is
written under the ignored ``local/`` directory (default ``local/behavior/behavior_census.json``).

What it measures (see docs/verification/BEHAVIOR_DATA_DECODE.md):

1. **Variable value block.** A provider export has bytes *after* its tagged-property stream. Hypothesis
   under test (UNVERIFIED as semantics, checked structurally here): the block is one 4-byte value per
   ``VariableData`` entry, in sequence order then variable order, with no header. Oracles: the block
   length equals 4 x the total variable count for every provider (exact consumption); every non-zero
   value of a ``BVAR_Object`` variable is an in-range import/export index; copies of one provider path
   in different packages resolve to the same object paths.
2. **Variable links.** ``ConsolidatedVariableLinkData`` (PropertyName, VariableLinkType, ConnectionIndex,
   packed LinkedVariables range into ``ConsolidatedLinkedVariables``) for behaviors and events; which
   behavior inputs read a constant, an event output or another behavior's output.
3. **Output-link id byte.** For every behavior class: the high byte of ``LinkIdAndLinkedBehavior``
   on its outgoing links, distinct ids per class, and duplicate (same target) links per event.

The tag walker relies only on the self-describing UE3 tag header (name, type, size, array index, struct
or enum name, bool byte). Arrays are walked as tagged structs only for the provider arrays listed in
STRUCT_ARRAYS; any other array is skipped by its recorded size.
"""
import argparse
import collections
import json
import math
import os
import sqlite3
import struct
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import native_count as nc  # noqa: E402  pure-Python LZO and table reader

GAME = os.path.join(os.environ.get('OPENWILLOW_BL2', r'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2'),
                    'WillowGame', 'CookedPCConsole')
PROVIDER_CLASSES = {'BehaviorProviderDefinition', 'AIBehaviorProviderDefinition'}
STRUCT_ARRAYS = {'BehaviorSequences', 'EventData2', 'BehaviorData2', 'VariableData',
                 'ConsolidatedOutputLinkData', 'ConsolidatedVariableLinkData'}
INT_ARRAYS = {'ConsolidatedLinkedVariables'}
# Bytes per VariableData entry in the untagged value block that follows a provider's tagged properties.
# FITTED from the packages (exact consumption over the census, see the record); field meanings beyond
# "first word is the scalar/object value" are UNVERIFIED. Types absent here are reported, never guessed.
VALUE_SIZES = {
    'BVAR_Bool': 4, 'BVAR_Int': 4, 'BVAR_Float': 4, 'BVAR_Object': 4,
    'BVAR_Vector': 12, 'BVAR_DirectionVector': 48,
    'BVAR_InstanceData': 12,      # word 0 unknown (0/1 observed), then an FName (index, number)
    'BVAR_Attribute': 20,         # word 0 packed (index << 16 | 1 observed), words 1..4 read as AttributeInitializationData
    'BVAR_UnaryMath': 8, 'BVAR_BinaryMath': 12,
    'BVAR_NamedVariable': 0, 'BVAR_NamedKismetVariable': 0, 'BVAR_AllPlayers': 0,
    'BVAR_AttachmentLocation': 32,  # fitted on the 59 unique providers that use these two types (59/59); words 1-2 an FName
    'BVAR_Flag': 8,                 # word 1 observed to be an object reference to a FlagDefinition
}
VERSION = (832 | (46 << 16))


def word(b, o):
    return struct.unpack_from('<I', b, o)[0]


def unwrap_blocks(chunk):
    block, usize = word(chunk, 4), word(chunk, 12)
    count = (usize + block - 1) // block
    pos, out = 16 + 8 * count, bytearray()
    for k in range(count):
        stored, size = struct.unpack_from('<II', chunk, 16 + 8 * k)
        out += nc.lzo1x_decompress(chunk[pos:pos + stored], size)
        pos += stored
    if len(out) != usize:
        raise ValueError('block container size mismatch')
    return bytes(out)


def load_package(path):
    """Decoded package bytes: fully compressed container, partially compressed (chunk table) or plain."""
    b = open(path, 'rb').read()
    if word(b, 0) != 0x9E2A83C1:
        raise ValueError('not a package')
    if word(b, 4) != VERSION:
        return unwrap_blocks(b)
    length = struct.unpack_from('<i', b, 12)[0]
    flags_at = 16 + (-length * 2 if length < 0 else length)
    if not word(b, flags_at) & 0x02000000:
        return b
    # Same summary walk as src/container.cpp (BL2 832/46).
    cur = flags_at + 4 + 24 + 20 + 16
    cur += 4 + word(b, cur) * 12 + 8
    codec, count = word(b, cur), word(b, cur + 4)
    cur += 8
    if codec != 2:
        raise ValueError('unsupported codec')
    out = bytearray()
    for i in range(count):
        dest, size, src, stored = struct.unpack_from('<IIII', b, cur + i * 16)
        if i == 0:
            out += b[:dest]
        piece = unwrap_blocks(b[src:src + stored])
        if len(piece) != size:
            raise ValueError('chunk size mismatch')
        out += piece
    return bytes(out)


class Pkg:
    def __init__(self, path):
        self.name = os.path.splitext(os.path.basename(path))[0]
        self.data = load_package(path)
        _, _, self.names, self.imports, self.exports = nc.parse_package(self.data)

    def fname(self, b, p):
        i, n = struct.unpack_from('<ii', b, p)
        if not 0 <= i < len(self.names):
            raise ValueError('name index out of range')
        return self.names[i] + (f'_{n - 1}' if n else '')

    def valid_ref(self, ref):
        return ref == 0 or (0 < ref <= len(self.exports)) or (0 < -ref <= len(self.imports))

    def path(self, ref):
        parts = []
        guard = 0
        while ref and guard < 64:
            guard += 1
            if ref > 0:
                e = self.exports[ref - 1]
                parts.append(e['name']); ref = e['outer']
            else:
                x = self.imports[-ref - 1]
                parts.append(x['name']); ref = x['outer']
        return '.'.join(reversed(parts))

    def class_name(self, ref):
        if ref > 0:
            c = self.exports[ref - 1]['class']
            return self.imports[-c - 1]['name'] if c < 0 else (self.exports[c - 1]['name'] if c > 0 else 'Class')
        if ref < 0:
            return self.imports[-ref - 1]['class']
        return None


def walk_tags(pkg, b, p, end, depth=0):
    """Returns (dict of name -> value, position after the None tag)."""
    if depth > 16:
        raise ValueError('nesting')
    out = {}
    while True:
        if p + 8 > end:
            raise ValueError('tag past end')
        name = pkg.fname(b, p); p += 8
        if name == 'None':
            return out, p
        kind = pkg.fname(b, p); p += 8
        size, index = struct.unpack_from('<ii', b, p); p += 8
        if size < 0 or index < 0:
            raise ValueError('negative tag')
        detail = None
        if kind in ('StructProperty', 'ByteProperty'):
            detail = pkg.fname(b, p); p += 8
        if kind == 'BoolProperty':
            detail = b[p]; p += 1
        stop = p + size
        if stop > end:
            raise ValueError('tag body past end')
        if kind == 'IntProperty': value = struct.unpack_from('<i', b, p)[0]
        elif kind == 'FloatProperty': value = struct.unpack_from('<f', b, p)[0]
        elif kind == 'BoolProperty': value = bool(detail)
        elif kind == 'NameProperty': value = pkg.fname(b, p)
        elif kind in ('ObjectProperty', 'ClassProperty', 'ComponentProperty', 'InterfaceProperty'):
            value = struct.unpack_from('<i', b, p)[0]
        elif kind == 'ByteProperty': value = pkg.fname(b, p) if detail != 'None' else b[p]
        elif kind == 'StructProperty':
            # Atomic structs (Vector, Rotator, Color...) are serialized natively, not as tags: keep the body raw
            # (its size is recorded by the tag, so the enclosing stream stays exactly consumed).
            try:
                value, q = walk_tags(pkg, b, p, stop, depth + 1)
                if q != stop:
                    raise ValueError('struct size mismatch')
            except (ValueError, struct.error, IndexError):
                value = ('native', detail, size)
        elif kind == 'ArrayProperty':
            count = struct.unpack_from('<i', b, p)[0]
            q = p + 4
            if name in STRUCT_ARRAYS:
                value = []
                for _ in range(count):
                    item, q = walk_tags(pkg, b, q, stop, depth + 1)
                    value.append(item)
                if q != stop:
                    raise ValueError('array size mismatch: ' + name)
            elif name in INT_ARRAYS:
                if 4 + 4 * count != size:
                    raise ValueError('int array size mismatch')
                value = list(struct.unpack_from(f'<{count}i', b, q))
            else:
                value = ('skipped', count)
        else:
            value = ('skipped', kind)
        out[name] = value
        p = stop


def unpack(packed):
    raw = packed.get('ArrayIndexAndLength', 0) & 0xFFFFFFFF if isinstance(packed, dict) else 0
    return raw >> 16, raw & 0xFFFF


def analyze_provider(pkg, idx, report):
    e = pkg.exports[idx - 1]
    b = pkg.data
    start, end = e['off'], e['off'] + e['size']
    props, after = walk_tags(pkg, b, start + 4, end)  # 4 = net index prefix (established for these objects)
    sequences = props.get('BehaviorSequences', [])
    types = [v.get('Type', 'BVAR_None') for s in sequences for v in s.get('VariableData', [])]
    trailing = end - after
    record = {'package': pkg.name, 'index': idx, 'path': pkg.path(idx), 'vars': len(types), 'trailing': trailing}
    report['providers'] += 1
    unknown = sorted({t for t in types if t not in VALUE_SIZES})
    if unknown:
        record['unknown_types'] = unknown
        report['unknown_type_providers'].append(record)
        return None
    if trailing != sum(VALUE_SIZES[t] for t in types):
        report['trailing_mismatch'].append(record)
        return None
    cursor = after
    parsed = []
    for s in sequences:
        variables = []
        for v in s.get('VariableData', []):
            vtype = v.get('Type', 'BVAR_None')
            size = VALUE_SIZES[vtype]
            words = list(struct.unpack_from(f'<{size // 4}i', b, cursor))
            cursor += size
            raw = words[0] if words else 0
            entry = {'name': v.get('Name', 'None'), 'type': vtype, 'raw': raw, 'words': words}
            if vtype == 'BVAR_InstanceData':
                entry['instance_name'] = pkg.fname(b, cursor - 8)
            if vtype == 'BVAR_DirectionVector':  # first 8 bytes observed to be an FName (e.g. a DIRECTION_* value)
                entry['direction_name'] = pkg.fname(b, cursor - size)
            if vtype == 'BVAR_AttachmentLocation':   # words 1-2 observed to be an FName
                entry['word1_name'] = pkg.fname(b, cursor - size + 4)
            if vtype == 'BVAR_Flag' and words[1]:     # word 1 observed to be an object reference (a FlagDefinition)
                if pkg.valid_ref(words[1]):
                    entry['word1_name'] = pkg.path(words[1])
                    report['flag_object_classes'][pkg.class_name(words[1])] += 1
                else:
                    report['object_out_of_range'].append(record['path'])
            if vtype == 'BVAR_Attribute':
                # words 1..4 read as AttributeInitializationData (BaseValueConstant, BaseValueAttribute,
                # InitializationDefinition, BaseValueScaleConstant); words 2 and 3 are checked as object references.
                for key, w in (('attribute_object', words[2]), ('init_object', words[3])):
                    if not w:
                        continue
                    if pkg.valid_ref(w):
                        entry[key] = pkg.path(w)
                        report[key + '_classes'][pkg.class_name(w)] += 1
                    else:
                        report['object_out_of_range'].append(record['path'])
            report['var_types'][vtype] += 1
            if raw:
                report['var_types_nonzero'][vtype] += 1
            if vtype == 'BVAR_Object' and raw:
                if not pkg.valid_ref(raw):
                    report['object_out_of_range'].append(record['path'])
                else:
                    entry['object'] = pkg.path(raw)
                    entry['object_class'] = pkg.class_name(raw)
                    report['object_value_classes'][entry['object_class']] += 1
            if vtype == 'BVAR_Float' and raw:
                f = struct.unpack('<f', struct.pack('<i', raw))[0]
                entry['float'] = f
                if not math.isfinite(f):
                    report['float_not_finite'] += 1
            if vtype == 'BVAR_Bool' and raw not in (0, 1):
                report['bool_not_01'] += 1
            variables.append(entry)
        parsed.append((s, variables))
    return record, parsed


def normalized(v):
    """A package-independent form of a decoded value (object references and names are package-relative)."""
    t = v['type']
    if t == 'BVAR_Object':
        return (t, v.get('object', ''))
    if t == 'BVAR_InstanceData':
        return (t, v['words'][0], v['instance_name'])
    if t == 'BVAR_DirectionVector':
        return (t, v['direction_name'], tuple(v['words'][2:]))
    if t == 'BVAR_AttachmentLocation':   # words 1-2: FName (package-relative index)
        return (t, v['words'][0], v.get('word1_name'), tuple(v['words'][3:]))
    if t == 'BVAR_Flag':                 # word 1: object reference (package-relative)
        return (t, v['words'][0], v.get('word1_name'))
    if t == 'BVAR_Attribute':
        return (t, v['words'][0], v['words'][1], v.get('attribute_object', ''), v.get('init_object', ''), v['words'][4])
    return (t, tuple(v['words']))


def link_census(pkg, provider_path, parsed, report):
    for s, variables in parsed:
        links = s.get('ConsolidatedOutputLinkData', [])
        behaviors = s.get('BehaviorData2', [])
        vlinks = s.get('ConsolidatedVariableLinkData', [])
        clv = s.get('ConsolidatedLinkedVariables', [])
        classes = []
        for bd in behaviors:
            ref = bd.get('Behavior', 0)
            classes.append(pkg.class_name(ref) if ref and pkg.valid_ref(ref) else None)
        for i, bd in enumerate(behaviors):
            first, length = unpack(bd.get('OutputLinks', {}))
            ids = []
            for k in range(first, first + length):
                raw = links[k].get('LinkIdAndLinkedBehavior', 0) & 0xFFFFFFFF
                ids.append(raw >> 24)
                target = raw & 0xFFFFFF
                report['behavior_link_target_ids'][f'{classes[target] if 0 <= target < len(classes) else "?"}:{raw >> 24}'] += 1
            cls = classes[i] or '?'
            c = report['link_ids'].setdefault(cls, {'instances': 0, 'ids': collections.Counter(), 'id_sets': collections.Counter()})
            c['instances'] += 1
            c['ids'].update(ids)
            c['id_sets'][','.join(str(x) for x in sorted(set(ids)))] += 1
        for ev in s.get('EventData2', []):
            first, length = unpack(ev.get('OutputLinks', {}))
            targets = [(links[k].get('LinkIdAndLinkedBehavior', 0) & 0xFFFFFF) for k in range(first, first + length)]
            report['event_links'] += len(targets)
            report['event_duplicate_targets'] += len(targets) - len(set(targets))
            ids = [((links[k].get('LinkIdAndLinkedBehavior', 0) & 0xFFFFFFFF) >> 24) for k in range(first, first + length)]
            report['event_link_ids'].update(ids)
            for target, link_id in zip(targets, ids):
                cls = classes[target] if 0 <= target < len(classes) else '?'
                report['event_link_target_ids'][f'{cls}:{link_id}'] += 1
            pairs = list(zip(targets, ids))
            report['event_duplicate_same_id'] += len(pairs) - len(set(pairs))
            report['event_duplicate_other_id'] += len(set(pairs)) - len(set(targets))
            for target in {t for t in targets if targets.count(t) > 1}:
                report['event_duplicate_target_classes'][classes[target] if 0 <= target < len(classes) else '?'] += 1
        for i, bd in enumerate(behaviors):
            first, length = unpack(bd.get('OutputLinks', {}))
            targets = [(links[k].get('LinkIdAndLinkedBehavior', 0) & 0xFFFFFF) for k in range(first, first + length)]
            ids = [((links[k].get('LinkIdAndLinkedBehavior', 0) & 0xFFFFFFFF) >> 24) for k in range(first, first + length)]
            report['behavior_links'] += len(targets)
            report['behavior_duplicate_targets'] += len(targets) - len(set(targets))
            pairs = list(zip(targets, ids))
            report['behavior_duplicate_same_id'] += len(pairs) - len(set(pairs))
            report['behavior_duplicate_other_id'] += len(set(pairs)) - len(set(targets))

        # Variable writers: event outputs and behavior outputs (VariableLinkType BVARLINK_Output).
        def linked(owner):
            first, length = unpack(owner.get('LinkedVariables', owner.get('OutputVariables', {})))
            result = []
            for k in range(first, first + length):
                vl = vlinks[k]
                vf, vn = unpack(vl.get('LinkedVariables', {}))
                result.append((vl.get('PropertyName'), vl.get('VariableLinkType'), vl.get('ConnectionIndex', 0), clv[vf:vf + vn]))
            return result
        writers = collections.defaultdict(list)
        for ev in s.get('EventData2', []):
            for prop, kind, conn, targets in linked(ev):
                report['event_var_link_types'][kind] += 1
                for t in targets:
                    writers[t].append('event:' + ev.get('UserData', {}).get('EventName', '?') + '.' + str(prop))
        for i, bd in enumerate(behaviors):
            for prop, kind, conn, targets in linked(bd):
                report['behavior_var_link_types'][kind] += 1
                if kind == 'BVARLINK_Output':
                    for t in targets:
                        writers[t].append('behavior:' + (classes[i] or '?') + '.' + str(prop))
        for i, bd in enumerate(behaviors):
            if classes[i] != 'Behavior_CompareObject':
                continue
            entry = {'provider': provider_path, 'sequence': s.get('BehaviorSequenceName'), 'behavior': pkg.path(bd['Behavior'])}
            for prop, kind, conn, targets in linked(bd):
                sources = []
                for t in targets:
                    var = variables[t] if 0 <= t < len(variables) else None
                    if var is None:
                        sources.append('variable index out of range'); continue
                    if var.get('object'):
                        sources.append('constant:' + var['object'] + ' (' + var['object_class'] + ')')
                    if writers.get(t):
                        sources.extend(writers[t])
                    if var['name'] != 'None' or var['type'] not in ('BVAR_Object',):
                        sources.append('variable:' + var['type'] + ':' + var['name'])
                    if not var.get('object') and not writers.get(t) and var['name'] == 'None':
                        sources.append('unwritten')
                entry[str(prop)] = sources
            report['compare_object'].append(entry)


def provider_packages(index_path):
    if index_path and os.path.exists(index_path):
        db = sqlite3.connect(index_path)
        rows = db.execute("select distinct pkg from ex where class in "
                          "('GearboxFramework.BehaviorProviderDefinition','GearboxFramework.AIBehaviorProviderDefinition')").fetchall()
        return sorted(r[0] for r in rows)
    return sorted(os.path.splitext(f)[0] for f in os.listdir(GAME) if f.lower().endswith('.upk'))


def main():
    ap = argparse.ArgumentParser(description='BehaviorProviderDefinition structural census (writes under local/)')
    ap.add_argument('--packages', nargs='*', help='package names (default: every package with a provider per the path index)')
    ap.add_argument('--index', default='local/census/path_index.sqlite')
    ap.add_argument('--output', default='local/behavior/behavior_census.json')
    args = ap.parse_args()
    packages = args.packages or provider_packages(args.index)
    report = {
        'providers': 0, 'trailing_mismatch': [], 'unknown_type_providers': [], 'parse_errors': [], 'object_out_of_range': [],
        'var_types': collections.Counter(), 'var_types_nonzero': collections.Counter(),
        'object_value_classes': collections.Counter(), 'float_not_finite': 0, 'bool_not_01': 0,
        'attribute_object_classes': collections.Counter(), 'init_object_classes': collections.Counter(),
        'flag_object_classes': collections.Counter(),
        'link_ids': {}, 'event_link_ids': collections.Counter(), 'event_link_target_ids': collections.Counter(),
        'behavior_link_target_ids': collections.Counter(), 'event_links': 0, 'event_duplicate_targets': 0,
        'event_duplicate_same_id': 0, 'event_duplicate_other_id': 0, 'behavior_duplicate_same_id': 0,
        'behavior_duplicate_other_id': 0, 'event_duplicate_target_classes': collections.Counter(),
        'behavior_links': 0, 'behavior_duplicate_targets': 0,
        'event_var_link_types': collections.Counter(), 'behavior_var_link_types': collections.Counter(),
        'compare_object': [], 'copies_inconsistent': [],
    }
    copies = {}
    started = time.time()
    for n, name in enumerate(packages):
        path = os.path.join(GAME, name + '.upk')
        try:
            pkg = Pkg(path)
        except Exception as error:  # noqa: BLE001 - recorded, never silently dropped
            report['parse_errors'].append({'package': name, 'error': 'load: ' + str(error)})
            continue
        for idx, e in enumerate(pkg.exports, 1):
            if e['class'] >= 0 or pkg.imports[-e['class'] - 1]['name'] not in PROVIDER_CLASSES:
                continue
            try:
                result = analyze_provider(pkg, idx, report)
            except Exception as error:  # noqa: BLE001
                report['parse_errors'].append({'package': name, 'index': idx, 'error': str(error)})
                continue
            if not result:
                continue
            record, parsed = result
            signature = [[normalized(v) for v in variables] for _, variables in parsed]
            previous = copies.setdefault(record['path'], signature)
            if previous != signature:
                report['copies_inconsistent'].append(record['path'])
            if previous is signature:  # first copy of this provider path: count its links once
                link_census(pkg, record['path'], parsed, report)
        print(f'[{n + 1}/{len(packages)}] {name} providers={report["providers"]} {time.time() - started:.0f}s', file=sys.stderr)
    report['unique_provider_paths'] = len(copies)
    report['packages'] = len(packages)
    for value in report['link_ids'].values():
        value['ids'] = dict(value['ids']); value['id_sets'] = dict(value['id_sets'])
    os.makedirs(os.path.dirname(args.output), exist_ok=True)
    with open(args.output, 'w', encoding='utf-8') as out:
        json.dump(report, out, indent=1, default=lambda o: dict(o) if isinstance(o, collections.Counter) else str(o))
    summary = {k: report[k] for k in ('packages', 'providers', 'unique_provider_paths', 'float_not_finite', 'bool_not_01',
                                      'event_links', 'event_duplicate_targets', 'event_duplicate_same_id',
                                      'event_duplicate_other_id', 'behavior_links', 'behavior_duplicate_targets',
                                      'behavior_duplicate_same_id', 'behavior_duplicate_other_id')}
    summary['unknown_types'] = sorted({t for r in report['unknown_type_providers'] for t in r['unknown_types']})
    summary.update({'trailing_mismatch': len(report['trailing_mismatch']), 'parse_errors': len(report['parse_errors']),
                    'unknown_type_providers': len(report['unknown_type_providers']),
                    'object_out_of_range': len(report['object_out_of_range']), 'copies_inconsistent': len(report['copies_inconsistent']),
                    'compare_object_instances': len(report['compare_object'])})
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
