"""Synthetic PackageStore and cross-package import resolution coverage."""
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

reader = str(Path(sys.argv[1]).resolve())
names = ['None', 'Core', 'Package', 'B', 'Object', 'Target']

def words(*values):
    return struct.pack('<' + 'I' * len(values), *values)

def fname(value):
    return words(names.index(value), 0)

def package(path, imports, export_name=None):
    table = b''.join(words(len(name) + 1) + name.encode() + b'\0' + bytes(8) for name in names)
    io = 48 + len(table)
    import_data = b''.join(fname(class_package) + fname(class_name) + words(outer & 0xffffffff) + fname(name)
                          for class_package, class_name, outer, name in imports)
    eo = io + len(import_data)
    export_data = b''
    if export_name:
        export_data = words(0, 0, 0) + fname(export_name) + words(0, 0, 0, 0, eo + 68, 0, 0) + bytes(20)
    header = words(0x9e2a83c1, 832 | (46 << 16), 48, 0, 0, len(names), 48,
                   1 if export_name else 0, eo, len(imports), io, 0)
    path.write_bytes(header + table + import_data + export_data)

with tempfile.TemporaryDirectory() as folder:
    root = Path(folder)
    source = root / 'A.upk'
    package(root / 'B.upk', [], 'Target')
    package(source, [('Core', 'Package', 0, 'B'), ('Core', 'Object', -1, 'Target')])
    result = subprocess.run([reader, str(source), '--resolve', '-2', '--cooked', str(root)],
                            capture_output=True, text=True, encoding='utf-8')
    assert result.returncode == 0, result.stderr
    data = json.loads(result.stdout)
    assert data['reference_path'] == 'B.Target'
    assert data['resolved_package'] == 'B'
    assert data['resolved_path'] == 'Target'
    assert data['resolved_index'] == 1
    assert data['indexed_packages'] == 2
    package(source, [('Core', 'Object', 1, 'Target')], 'B')
    nested = subprocess.run([reader, str(source), '--resolve', '-1', '--cooked', str(root)],
                            capture_output=True, text=True, encoding='utf-8')
    assert nested.returncode == 0, nested.stderr
    nested_data = json.loads(nested.stdout)
    assert nested_data['reference_path'] == 'B.Target'
    assert nested_data['resolved_package'] == 'B'
    assert nested_data['resolved_path'] == 'Target'
    missing = subprocess.run([reader, str(source), '--resolve', '-2', '--cooked', str(root / 'missing')],
                             capture_output=True, text=True, encoding='utf-8')
    assert missing.returncode != 0 and not missing.stdout
    schema = root / 'scene.schema'
    schema.write_text('Materials=ObjectProperty\n')
    scene = subprocess.run([reader, str(root / 'B.upk'), '--scene-records', str(schema)],
                           capture_output=True, text=True, encoding='utf-8')
    assert scene.returncode == 0, scene.stderr
    records = json.loads(scene.stdout)
    assert len(records) == 1 and records[0]['path'] == 'Target'
    payload = subprocess.run([reader, str(root / 'B.upk'), '--payload', '1'],
                             capture_output=True, text=True, encoding='utf-8')
    assert payload.returncode == 0 and json.loads(payload.stdout) == []
    for bad_index in ('0', '-1', '2'):
        bad = subprocess.run([reader, str(root / 'B.upk'), '--payload', bad_index],
                             capture_output=True, text=True, encoding='utf-8')
        assert bad.returncode != 0 and not bad.stdout
print('lazy package indexing and cross-package import resolution passed.')
