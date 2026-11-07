"""Synthetic tests for the store's global fallback (invented packages): the answer is the first file in index order that has the path, whatever the index does.
Every case runs with OPENWILLOW_RESOLVE_CHECK, which also runs the full scan and stops on any difference. Run: python tests/global_resolve_test.py <ow-package>"""
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

reader = str(Path(sys.argv[1]).resolve())


def words(*values):
    return struct.pack('<' + 'I' * len(values), *values)


def package(path, imports=(), exports=()):
    """imports: (class package, class, outer, name). exports: (name, outer). Every string is added to the name table."""
    names = ['None', 'Core', 'Package', 'Object']
    for item in list(imports) + list(exports):
        for value in item:
            if isinstance(value, str) and value not in names:
                names.append(value)
    fname = lambda value: words(names.index(value), 0)
    table = b''.join(words(len(name) + 1) + name.encode() + b'\0' + bytes(8) for name in names)
    io = 48 + len(table)
    import_data = b''.join(fname(class_package) + fname(class_name) + words(outer & 0xffffffff) + fname(name) for class_package, class_name, outer, name in imports)
    eo = io + len(import_data)
    payload = eo + 68 * len(exports)
    export_data = b''.join(words(0, 0, outer & 0xffffffff) + fname(name) + words(0, 0, 0, 0, payload, 0, 0) + bytes(20) for name, outer in exports)
    header = words(0x9e2a83c1, 832 | (46 << 16), 48, 0, 0, len(names), 48, len(exports), eo, len(imports), io, 0)
    path.write_bytes(header + table + import_data + export_data)


def resolve(root, reference=-2, **env):
    done = subprocess.run([reader, str(root / 'Source.upk'), '--resolve', str(reference), '--cooked', str(root)], capture_output=True, text=True, encoding='utf-8',
                          env=dict(os.environ, OPENWILLOW_RESOLVE_CHECK='1', **env))
    return done


IMPORT_GDX_OBJ = [('Core', 'Package', 0, 'GDX'), ('Core', 'Object', -1, 'Obj')]   # the import GDX.Obj; no file is called GDX


def case(files):
    folder = tempfile.TemporaryDirectory()
    root = Path(folder.name)
    package(root / 'Source.upk', IMPORT_GDX_OBJ)
    for name, exports in files.items():
        package(root / f'{name}.upk', exports=exports)
    return folder, root


def answer(files):
    folder, root = case(files)
    with folder:
        done = resolve(root)
        return done


HOLDS = [('GDX', 0), ('Obj', 1)]

# Copies of one path: the first file in order wins, and the check ran and agreed.
done = answer({'Mid': HOLDS, 'Zeta': HOLDS, 'Other': [('Unrelated', 0), ('Obj', 1)]})
assert done.returncode == 0, done.stderr
data = json.loads(done.stdout)
assert data['resolved_package'] == 'Mid' and data['resolved_index'] == 2, data
assert 'OWRESOLVECHECK same GDX.Obj -> Mid 2' in done.stderr, done.stderr

# Case folded, as findExport folds it.
done = answer({'Later': HOLDS, 'Early': [('gdx', 0), ('OBJ', 1)]})
assert done.returncode == 0 and json.loads(done.stdout)['resolved_package'] == 'Early', done.stdout + done.stderr

# An outermost export whose own name holds the dot has that same path: its first segment is GDX, not the whole name.
done = answer({'Dotted': [('GDX.Obj', 0)], 'Later': HOLDS})
assert done.returncode == 0 and json.loads(done.stdout)['resolved_package'] == 'Dotted', done.stdout + done.stderr

# A file with a broken outer chain: it stays a candidate. A match before the break is found, and a throw at the break moves on to the next file.
done = answer({'Afirst': HOLDS + [('Loop', 3)], 'Later': HOLDS})
assert done.returncode == 0, done.stderr
assert json.loads(done.stdout)['resolved_package'] == 'Afirst', done.stdout
done = answer({'Afirst': [('Loop', 1)] + HOLDS, 'Later': HOLDS})
assert done.returncode == 0, done.stderr
assert json.loads(done.stdout)['resolved_package'] == 'Later', done.stdout

# A file that does not load is no candidate, and a path no file has is still an error.
folder, root = case({'Later': HOLDS})
with folder:
    (root / 'Broken.upk').write_bytes(b'not a package')
    done = resolve(root)
    assert done.returncode == 0 and json.loads(done.stdout)['resolved_package'] == 'Later', done.stdout + done.stderr
folder, root = case({'Other': [('Unrelated', 0)]})
with folder:
    done = resolve(root)
    assert done.returncode != 0 and 'import target not found' in done.stderr, done.stderr
    assert 'OWRESOLVECHECK same GDX.Obj -> none 0' in done.stderr, done.stderr
# An empty outermost name: Package::path drops it, so the export's real path starts at its child. The file's roots are not known, and it stays a candidate.
folder = tempfile.TemporaryDirectory()
with folder:
    root = Path(folder.name)
    package(root / 'Source.upk', [('Core', 'Package', 0, 'Obj')])     # the import Obj; no file is called Obj
    package(root / 'Afirst.upk', exports=[('', 0), ('Obj', 1)])
    package(root / 'Later.upk', exports=[('Obj', 0)])
    done = resolve(root, -1)
    assert done.returncode == 0, done.stderr
    data = json.loads(done.stdout)
    assert data['resolved_package'] == 'Afirst' and data['resolved_index'] == 2, data
    assert 'any_root=1' in done.stderr, done.stderr

# A file that fails during the one pass and loads later: a scan tried it again on each lookup, so it must still win. Without that, the check flag's scan disagrees and this fails.
folder, root = case({'Afirst': HOLDS, 'Later': HOLDS})
with folder:
    done = resolve(root, OPENWILLOW_RESOLVE_CHECK_FAIL_BUILD='Afirst')
    assert done.returncode == 0, done.stderr
    assert json.loads(done.stdout)['resolved_package'] == 'Afirst', done.stdout
    assert 'any_root=1' in done.stderr and 'OWRESOLVECHECK same GDX.Obj -> Afirst 2' in done.stderr, done.stderr
print('ok')
