"""Synthetic checks for tools/decisions_index.py: listing, tail, grep and the archive check (no game data)."""
from pathlib import Path
import subprocess
import sys
import tempfile

TOOL = Path(__file__).resolve().parents[1] / 'tools' / 'decisions_index.py'

TITLE = b'# Decisions and evidence\r\n\r\n'
OLDER = b'Older entries: [2026-09](docs/decisions/2026-09.md).\r\n\r\n'
ENTRY_A = b'## 2026-09-03: invented alpha entry\r\n\r\nAlpha body line.\r\n\r\n'
ENTRY_B = b'## 2026-10-02: invented beta entry\nBeta body, LF only.\n\n'
ENTRY_C = b'## 2026-09-20: invented gamma entry\r\n\r\nGamma body.\r\n\r\n'
ENTRY_D = b'## 2026-10-05: invented delta entry\r\n\r\nDelta body.\r\n'


def run(root, *args):
    return subprocess.run([sys.executable, '-I', str(TOOL), '--root', str(root)] + list(args),
                          capture_output=True)


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def build(root, log_entries, archive_entries, link=True):
    intro = TITLE + (OLDER if link else b'') + b'Newest entries are at the bottom.\r\n\r\n'
    write(root / 'DECISIONS.md', intro + b''.join(log_entries))
    write(root / 'docs' / 'decisions' / '2026-09.md',
          b'# Decisions and evidence, 2026-09 (moved verbatim from DECISIONS.md)\r\n\r\n' + b''.join(archive_entries))


def baseline(root):
    data = TITLE + b''.join([ENTRY_A, ENTRY_B, ENTRY_C, ENTRY_D])
    path = root / 'baseline.md'
    write(path, data)
    return path


with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp) / 'repo'
    build(root, [ENTRY_B, ENTRY_D], [ENTRY_A, ENTRY_C])
    base = baseline(Path(tmp))

    listing = run(root)
    assert listing.returncode == 0, listing.stderr
    lines = listing.stdout.decode('utf-8').splitlines()
    assert len(lines) == 4, lines
    assert lines[0].startswith('docs/decisions/2026-09.md:3  2026-09-03  invented alpha entry'), lines[0]
    assert lines[-1].startswith('DECISIONS.md:'), lines[-1]

    assert run(root, '--grep', 'BETA').stdout.decode('utf-8').count('\n') == 1
    assert run(root, '--grep', 'nothing-here').stdout == b''

    tail = run(root, '--tail', '1')
    assert tail.stdout == ENTRY_D, tail.stdout
    assert run(root, '--tail', '2').stdout == ENTRY_B + ENTRY_D

    assert run(root, '--check', '--baseline-file', str(base)).returncode == 0
    ok = run(root, '--check', '--baseline-file', str(base)).stdout.decode('utf-8')
    assert ok.startswith('OK: 4 baseline entries'), ok

    added = ENTRY_D.replace(b'delta', b'epsilon')
    build(root, [ENTRY_B, ENTRY_D, added], [ENTRY_A, ENTRY_C])
    assert run(root, '--check', '--baseline-file', str(base)).returncode == 0

    build(root, [ENTRY_B], [ENTRY_A, ENTRY_C])
    dropped = run(root, '--check', '--baseline-file', str(base))
    assert dropped.returncode == 1 and b'missing or changed' in dropped.stdout, dropped.stdout

    build(root, [ENTRY_B, ENTRY_D], [ENTRY_A, ENTRY_C])
    changed = ENTRY_C.replace(b'Gamma body.', b'Gamma body edited.')
    write(root / 'docs' / 'decisions' / '2026-09.md', b'# header\r\n\r\n' + ENTRY_A + changed)
    assert run(root, '--check', '--baseline-file', str(base)).returncode == 1

    build(root, [ENTRY_B, ENTRY_C, ENTRY_D], [ENTRY_A, ENTRY_C])
    leftover = run(root, '--check', '--baseline-file', str(base))
    assert leftover.returncode == 1 and b'still has 2026-09-20' in leftover.stdout, leftover.stdout

    build(root, [ENTRY_B, ENTRY_D], [ENTRY_A, ENTRY_C], link=False)
    unlinked = run(root, '--check', '--baseline-file', str(base))
    assert unlinked.returncode == 1 and b'lacks the link line' in unlinked.stdout, unlinked.stdout

    build(root, [ENTRY_B, ENTRY_D], [ENTRY_A, ENTRY_C])
    assert run(root, '--check', '--baseline-file', str(base)).returncode == 0
    write(root / 'docs' / 'decisions' / '2026-09.md', b'# header\r\n\r\n' + ENTRY_A + ENTRY_C + ENTRY_B)
    misfiled = run(root, '--check', '--baseline-file', str(base))
    assert misfiled.returncode == 1 and b'holds 2026-10-02' in misfiled.stdout, misfiled.stdout

print('decisions_index_test: passed')
