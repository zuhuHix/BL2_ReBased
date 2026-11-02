"""Synthetic checks for tools/native_index_find.py: fragment and note filters on a temp table (no game data)."""
from pathlib import Path
import subprocess
import sys
import tempfile

TOOL = Path(__file__).resolve().parents[1] / 'tools' / 'native_index_find.py'

TABLE = (
    '# Native function index\n'
    '\n'
    '| Native | Note | Slice use | Status | Confidence | Implemented |\n'
    '|---|---|---|---|---|---|\n'
    '| Invented.Alpha.Run | [NOTE_ONE.md](NOTE_ONE.md) | first use | UNVERIFIED | high | src/a.cpp |\n'
    '| Invented.Beta.Stop | [NOTE_TWO.md](NOTE_TWO.md) | pipe \\| inside | UNVERIFIED | low | |\n'
    '| Invented.Alpha.Jump | [NOTE_TWO.md](NOTE_TWO.md) | second | VERIFIED | high | src/b.cpp |\n'
    '\n'
    'Second section, not a table.\n'
)


def run(*args):
    return subprocess.run([sys.executable, '-I', str(TOOL)] + list(args), capture_output=True)


with tempfile.TemporaryDirectory() as tmp:
    table = Path(tmp) / 'NATIVE_INDEX.md'
    table.write_bytes(TABLE.encode('utf-8'))
    file_args = ['--file', str(table)]

    by_fragment = run('alpha', *file_args)
    assert by_fragment.returncode == 0, by_fragment.stderr
    lines = by_fragment.stdout.decode('utf-8').splitlines()
    assert lines == [
        '| Native | Note | Slice use | Status | Confidence | Implemented |',
        '|---|---|---|---|---|---|',
        '| Invented.Alpha.Run | [NOTE_ONE.md](NOTE_ONE.md) | first use | UNVERIFIED | high | src/a.cpp |',
        '| Invented.Alpha.Jump | [NOTE_TWO.md](NOTE_TWO.md) | second | VERIFIED | high | src/b.cpp |',
    ], lines

    case = run('INVENTED.BETA', *file_args).stdout.decode('utf-8').splitlines()
    assert case[-1].startswith('| Invented.Beta.Stop |') and 'pipe \\| inside' in case[-1], case

    by_note = run('--note', 'note_two', *file_args).stdout.decode('utf-8').splitlines()
    assert [row.split('|')[1].strip() for row in by_note[2:]] == ['Invented.Beta.Stop', 'Invented.Alpha.Jump'], by_note

    both = run('alpha', '--note', 'NOTE_TWO', *file_args).stdout.decode('utf-8').splitlines()
    assert len(both) == 3 and 'Invented.Alpha.Jump' in both[2], both

    none = run('missing', *file_args)
    assert none.returncode == 1 and none.stdout == b'', none

    no_args = run(*file_args)
    assert no_args.returncode == 2, no_args

print('native_index_find_test: passed')
