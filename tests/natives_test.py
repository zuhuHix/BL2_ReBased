"""Synthetic native-stub dispatch coverage (Phase 2 seed, no game needed)."""
import json
from pathlib import Path
import subprocess
import sys

reader = str(Path(sys.argv[1]).resolve())

selftest = subprocess.run([reader, '--native-selftest'],
                          capture_output=True, text=True, encoding='utf-8')
assert selftest.returncode == 0, selftest.stderr
summary = json.loads(selftest.stdout)
assert summary == {'registered': 1, 'unimplemented': 1,
                   'duplicate_rejected': True, 'passed': True}, summary

stub = subprocess.run([reader, '--native', 'WillowGame.WillowWeapon.Fire',
                       '--native-args', 'a,b'],
                      capture_output=True, text=True, encoding='utf-8')
assert stub.returncode == 0, stub.stderr
data = json.loads(stub.stdout)
assert data == {'name': 'WillowGame.WillowWeapon.Fire', 'status': 'unimplemented',
                'log': 'UNIMPLEMENTED WillowGame.WillowWeapon.Fire(a,b)'}, data

bare = subprocess.run([reader, '--native', 'Engine.Actor.Trace'],
                      capture_output=True, text=True, encoding='utf-8')
assert bare.returncode == 0, bare.stderr
assert json.loads(bare.stdout)['log'] == 'UNIMPLEMENTED Engine.Actor.Trace()'

for bad in (['--native'], ['--native', '', '--native-args', 'x'],
            ['--native', 'A', '--native-args'], ['--native', 'A', '--bogus', 'x'],
            ['--native', 'A', '--native-args', 'x', 'extra'],
            ['--native-selftest', 'extra']):
    result = subprocess.run([reader] + bad, capture_output=True, text=True, encoding='utf-8')
    assert result.returncode != 0 and not result.stdout, bad
print('native stub dispatch passed.')
