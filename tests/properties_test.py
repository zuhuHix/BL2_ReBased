"""Synthetic object/property coverage. No game files or values are fixtures."""
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

reader = str(Path(sys.argv[1]).resolve())
def ints(*values):
    return struct.pack('<' + 'i' * len(values), *values)

names = ['None', 'Root', 'Child', 'Class', 'IntProperty', 'FloatProperty',
         'BoolProperty', 'NameProperty', 'ObjectProperty', 'StrProperty',
         'ByteProperty', 'StructProperty', 'ArrayProperty', 'Test', 'Enum',
         'Choice', 'Vector', 'UnknownProperty', 'Wide😀', 'Quoted"\\\n']
def fname(name, number=0):
    return ints(names.index(name), number)

def tag(kind, data=b'', extra=b'', name='Test', index=0):
    return fname(name) + fname(kind) + ints(len(data), index) + extra + data

def fstring(value):
    data = (value + '\0').encode('utf-16le')
    return ints(-len(data) // 2) + data

def package(payload, outer=0, child_name='Child'):
    nt = b''.join(fstring(name) + bytes(8) for name in names)
    io = 48 + len(nt)
    imports = fname('Root') + fname('Class') + ints(0) + fname('Root')
    eo = io + len(imports)
    po = eo + 68
    export = ints(-1, 0, outer) + fname(child_name, 2) + ints(0) + bytes(8)
    export += ints(len(payload), po, 0, 0) + bytes(20)
    header = struct.pack('<II', 0x9e2a83c1, 832 | (46 << 16))
    header += ints(48, 0, 0, len(names), 48, 1, eo, 1, io, 0)
    return header + nt + imports + export + payload

fields = [
    tag('IntProperty', ints(-42)),
    tag('FloatProperty', struct.pack('<f', 1.25)),
    tag('BoolProperty', extra=b'\x01'),
    tag('NameProperty', fname('Wide😀', 3)),
    tag('ObjectProperty', ints(-1)),
    tag('ObjectProperty', ints(0)),
    tag('StrProperty', fstring('hello😀"\n')),
    tag('ByteProperty', b'\x7f', fname('None')),
    tag('ByteProperty', fname('Choice'), fname('Enum')),
    tag('StructProperty', bytes(12), fname('Vector')),
    tag('ArrayProperty', ints(2, 11, 22)),
    tag('UnknownProperty', b'opaque'),
    tag('IntProperty', ints(99), name='Quoted"\\\n', index=3),
]
with tempfile.TemporaryDirectory() as folder:
    path = Path(folder) / 'synthetic.bin'
    def run(payload, *options, **kwargs):
        path.write_bytes(package(payload, **kwargs))
        return subprocess.run([reader, str(path), *options], capture_output=True, text=True, encoding='utf-8')
    options = ('--properties', '1', '--property-offset', '4')
    payload = ints(123) + b''.join(fields) + fname('None') + b'tail'
    result = run(payload, *options)
    assert result.returncode == 0, result.stderr
    data = json.loads(result.stdout)
    assert data['path'] == 'Child_1'
    assert data['trailing_bytes'] == 4
    props = data['properties']
    assert [p['value'] for p in props] == [-42, 1.25, True, 'Wide😀_2',
        {'index': -1, 'path': 'Root'}, {'index': 0, 'path': None},
        'hello😀"\n', 127, 'Choice', {'X': 0, 'Y': 0, 'Z': 0}, None, None, 99]
    assert [p['status'] for p in props[9:12]] == ['decoded', 'unsupported', 'unsupported']
    assert props[-1]['array_index'] == 3 and props[-1]['name'] == 'Quoted"\\\n'
    schema = Path(folder) / 'arrays.schema'
    schema.write_text('Test=IntProperty\n')
    array_options = (*options, '--array-schema', str(schema))
    result = run(ints(0) + tag('ArrayProperty', ints(2, 11, -22)) + fname('None'), *array_options)
    assert json.loads(result.stdout)['properties'][0]['value'] == [11, -22], result.stderr
    assert run(ints(0) + tag('ArrayProperty', ints(3, 11, 22)) + fname('None'), *array_options).returncode != 0
    assert run(ints(0) + tag('ArrayProperty', ints(-1)) + fname('None'), *options).returncode != 0
    nested = tag('StructProperty', tag('IntProperty', ints(81)) + fname('None'), fname('Root'))
    result = run(ints(0) + nested + fname('None'), *options)
    assert json.loads(result.stdout)['properties'][0]['value'][0]['value'] == 81, result.stderr
    schema.write_text('Test=StructProperty:Root\n')
    result = run(ints(0) + tag('ArrayProperty', ints(2) + (fields[0] + fname('None')) * 2) + fname('None'), *array_options)
    assert [v[0]['value'] for v in json.loads(result.stdout)['properties'][0]['value']] == [-42, -42], result.stderr
    too_deep = fname('None')
    for _ in range(34):
        too_deep = tag('StructProperty', too_deep, fname('Root')) + fname('None')
    assert run(ints(0) + too_deep, *options).returncode != 0
    listing = json.loads(run(payload, '--exports', outer=-1, child_name='Wide😀').stdout)
    assert listing[0]['path'] == 'Root.Wide😀_1'
    assert listing[0]['class'] == 'Root'
    assert run(payload, '--exports', outer=1).returncode != 0
    bad_tags = [
        tag('IntProperty', b'\x00'), tag('IntProperty', bytes(8)),
        tag('BoolProperty', extra=b'\x02'), tag('BoolProperty', b'\x00', b'\x01'),
        tag('ObjectProperty', ints(2)), tag('NameProperty', ints(999, 0)),
        tag('FloatProperty', struct.pack('<f', float('nan'))),
        tag('StrProperty', ints(-2) + b'\x00\xd8\x00\x00'),
        tag('IntProperty', ints(0), index=-1),
        fname('Test') + fname('IntProperty') + ints(-1, 0),
        fname('Test') + fname('IntProperty') + ints(999999, 0),
    ]
    for bad in bad_tags:
        result = run(ints(123) + bad + fname('None'), *options)
        assert result.returncode != 0 and not result.stdout, result
    # The package itself remains valid while the export's property data is truncated.
    single = fields[0] + fname('None')
    for end in range(len(single)):
        assert run(ints(123) + single[:end], *options).returncode != 0, end
    for offset in ['0', '999999', '-1', '4x']:
        assert run(payload, '--properties', '1', '--property-offset', offset).returncode != 0
    assert run(payload, '--properties', '0', '--property-offset', '4').returncode != 0
print('Property values, unsupported fields, Unicode, references, cycles and bounds passed.')
