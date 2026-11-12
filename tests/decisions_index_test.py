"""Synthetic checks for tools/decisions_index.py: one file per decision, the old layout at a baseline, and one case per
rule of --check (no game data; every entry is invented)."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

TOOL = Path(__file__).resolve().parents[1] / 'tools' / 'decisions_index.py'
D = 'docs/decisions/'
STUB = b'# Decisions\r\n\r\nEach decision is its own file under docs/decisions/.\r\n'

A = (D + '2026-09/2026-09-03-01-invented-alpha.md', b'## 2026-09-03: Invented alpha\n\nAlpha body.\n\n')
B = (D + '2026-10/2026-10-02-01-invented-beta.md', b'## 2026-10-02: Invented beta\n\nBeta body, LF.\n\n')
C = (D + '2026-10/2026-10-02-02-a-later-one.md', b'## 2026-10-02: A later one\r\n\r\nGamma body, CRLF.\r\n\r\n')
NEW = (D + '2026-10/2026-10-05-01-invented-delta.md', b'## 2026-10-05: Invented delta\n\nAI-assisted (synthetic).\n')


def run(root, *args):
    return subprocess.run([sys.executable, '-I', str(TOOL), '--root', str(root)] + list(args), stdin=subprocess.DEVNULL,
                          capture_output=True)


def write_tree(root, files):
    """Replace DECISIONS.md and docs/ under root (a .git folder there stays)."""
    if (root / 'docs').exists():
        shutil.rmtree(root / 'docs')
    if (root / 'DECISIONS.md').exists():
        (root / 'DECISIONS.md').unlink()
    for path, data in files.items():
        target = root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)


def new_layout(*extra):
    files = {'DECISIONS.md': STUB, A[0]: A[1], B[0]: B[1], C[0]: C[1]}
    files.update(dict(extra))
    return files


class Case:
    def __init__(self, tmp):
        self.root = tmp / 'repo'
        self.base = tmp / 'base'
        self.count = 0

    def check(self, tree, base, code, needle=b''):
        write_tree(self.root, tree)
        write_tree(self.base, base)
        result = run(self.root, '--check', '--baseline-dir', str(self.base))
        assert result.returncode == code and needle in result.stdout, (code, needle, result.stdout, result.stderr)
        self.count += 1
        return result.stdout


with tempfile.TemporaryDirectory() as tmp:
    tmp = Path(tmp)
    case = Case(tmp)
    base = new_layout()

    # Reading: the listing sorts by date, NN, slug; --grep; --tail prints the stored bytes in that order.
    write_tree(case.root, new_layout(NEW))
    lines = run(case.root).stdout.decode('utf-8').splitlines()
    assert lines == ['%s:1  %s  %s' % (path, path.rsplit('/', 1)[1][:10], data.split(b'\n')[0][15:].decode().strip())
                     for path, data in (A, B, C, NEW)], lines
    assert run(case.root, '--grep', 'BETA').stdout.decode().splitlines() == [lines[1]]
    assert run(case.root, '--grep', '2026-10-02').stdout.decode().splitlines() == lines[1:3]
    assert run(case.root, '--grep', '2026-09').stdout.decode().splitlines() == [lines[0]]
    assert run(case.root, '--grep', 'nothing-here').stdout == b''
    assert run(case.root, '--tail', '2').stdout == C[1] + NEW[1]
    assert run(case.root, '--tail', '1').stdout == NEW[1]

    # --name: one more than the day's highest NN; the slug rule (lower case, runs to one hyphen, cut at a hyphen to 60).
    name = run(case.root, '--name', 'Third: one, of "the" day!', '--date', '2026-10-02').stdout.decode().strip()
    assert name == D + '2026-10/2026-10-02-03-third-one-of-the-day.md', name
    long = run(case.root, '--name', 'word ' * 20, '--date', '2026-12-01').stdout.decode().strip()
    assert long == D + '2026-12/2026-12-01-01-' + '-'.join(['word'] * 12) + '.md', long
    case.count += 1

    # A Supersedes: line marks the old entry in the listing; the old file stays as it was.
    sup = (D + '2026-10/2026-10-06-01-replaces-beta.md',
           b'## 2026-10-06: Replaces beta\n\nAI-assisted (synthetic). Supersedes: is on its own line:\n\n'
           b'Supersedes: ' + B[0].encode() + b'\n')
    write_tree(case.root, new_layout(sup))
    marked = run(case.root, '--grep', 'beta').stdout.decode()
    assert '(superseded by 2026-10-06-01-replaces-beta.md)' in marked, marked
    case.check(new_layout(sup), base, 0, b'1 added, 0 edited')

    # Structure.
    case.check(base, base, 0, b'OK: 3 baseline decision files')
    for appended in (b'## 2026-10-07: Appended', b'### 2026-10-07: Deeper', b'## 2026-10-07 No colon'):
        case.check(dict(new_layout(), **{'DECISIONS.md': STUB + b'\r\n' + appended + b'\r\n\r\nx\r\n'}), base, 1,
                   b'DECISIONS.md holds 1 line(s) that look like an entry heading')
    no_stub = new_layout()
    del no_stub['DECISIONS.md']
    case.check(no_stub, base, 1, b'DECISIONS.md is missing')
    case.check(new_layout((D + '2026-09.md', b'# archive\n')), base, 1, b'is not a decision file')
    case.check(new_layout((D + 'README.md', b'# readme\n')), base, 1, b'is not a decision file')
    case.check(new_layout((D + '2026-10/2026-10-05-1-short-number.md', NEW[1])), base, 1, b'is not a decision file')
    case.check(new_layout((D + '2026-10/2026-10-05-01-Upper.md', NEW[1])), base, 1, b'is not a decision file')
    case.check(new_layout((D + '2026-11/2026-10-05-01-invented-delta.md', NEW[1])), base, 1, b'folder of another month')
    case.check(new_layout((NEW[0], b'Preamble\n' + NEW[1])), base, 1, b'line 1 is not')
    case.check(new_layout((NEW[0], NEW[1].replace(b'10-05', b'10-06'))), base, 1, b'is not the date in the name')
    case.check(new_layout((NEW[0], NEW[1] + b'\n## 2026-10-05: Second\n\nx\n')), base, 1, b'holds 2 dated headings')
    case.check(new_layout((D + '2026-10/2026-10-05-01-' + 'x' * 61 + '.md', NEW[1])), base, 1, b'longer than 60')
    dangling = (NEW[0], NEW[1] + b'\nSupersedes: ' + D.encode() + b'2026-10/2026-10-01-01-nothing.md\n')
    case.check(new_layout(dangling), base, 1, b'does not name another decision file')

    # Against a baseline in the new layout.
    case.check(new_layout(NEW), base, 0, b'1 added, 0 edited')
    case.check(new_layout((NEW[0], NEW[1].replace(b'AI-assisted (synthetic).', b'No disclosure.'))), base, 1,
               b'does not open with "AI-assisted')
    edited_b = (B[0], B[1].replace(b'Beta body', b'Beta body, redacted'))
    case.check(new_layout(edited_b), base, 1, b'changed: an edit needs')
    reason = (NEW[0], NEW[1] + b'\nEdits: `' + B[0].encode() + b'`\n\nThe beta body named something it should not.\n')
    case.check(new_layout(edited_b, reason), base, 0, b'1 added, 1 edited')
    old_with_line = (C[0], C[1] + b'Edits: ' + B[0].encode() + b'\r\n')
    case.check(new_layout(edited_b, old_with_line), base, 1, b'changed: an edit needs')
    # Only a file added since the baseline may authorise an edit: an unchanged one carrying the line does not.
    base_with_line = new_layout(old_with_line)
    case.check(new_layout(edited_b, old_with_line), base_with_line, 1, b'changed: an edit needs')
    case.check(new_layout(edited_b, old_with_line, reason), base_with_line, 0, b'1 added, 1 edited')
    gone = new_layout()
    del gone[B[0]]
    case.check(gone, base, 1, b'was deleted or renamed')
    renamed = dict(gone, **{B[0].replace('-01-', '-03-'): B[1]})
    case.check(renamed, base, 1, b'was deleted or renamed')
    case.check(new_layout((B[0], B[1].replace(b'\n', b'\r\n'))), base, 0)

    # Against a baseline in the old layout: the migration, byte for byte.
    old = {'DECISIONS.md': b'# Decisions\r\n\r\nOlder: [2026-09](docs/decisions/2026-09.md).\r\n\r\n' + B[1] + C[1],
           D + '2026-09.md': b'# Decisions, 2026-09\n\n' + A[1]}
    case.check(new_layout(), old, 0, b'3 baseline entries (old layout)')
    case.check(new_layout(NEW), old, 0, b'1 added')
    case.check(new_layout((C[0], C[1].replace(b'Gamma', b'Gamma!'))), old, 1, b'A later one is present 0 time(s)')
    dropped = new_layout()
    del dropped[A[0]]
    case.check(dropped, old, 1, b'Invented alpha is present 0 time(s), expected 1')
    case.check(new_layout((NEW[0], NEW[1].replace(b'AI-assisted (synthetic).', b'Undisclosed.'))), old, 1,
               b'does not open with')

    # The old layout in the working tree too (until the migration lands): entries kept, archive months right.
    case.check(old, old, 0, b'3 baseline entries (old layout)')
    misfiled = dict(old, **{D + '2026-09.md': old[D + '2026-09.md'] + B[1]})
    case.check(misfiled, old, 1, b'holds 2026-10-02 entry')
    write_tree(case.root, old)
    assert run(case.root, '--tail', '2').stdout == B[1] + C[1]
    assert run(case.root).stdout.decode().splitlines()[0] == D + '2026-09.md:3  2026-09-03  Invented alpha'

    # The baseline read from git, as lanes and the landing script use it.
    git = shutil.which('git')
    assert git, 'decisions_index_test needs git'
    env = dict(os.environ, GIT_CONFIG_GLOBAL=str(tmp / 'none'), GIT_CONFIG_NOSYSTEM='1', GIT_AUTHOR_NAME='t',
               GIT_AUTHOR_EMAIL='t@example.invalid', GIT_COMMITTER_NAME='t', GIT_COMMITTER_EMAIL='t@example.invalid')
    (tmp / 'none').write_text('')

    def g(*args):
        result = subprocess.run([git] + list(args), cwd=str(case.root), env=env, stdin=subprocess.DEVNULL,
                                capture_output=True)
        assert result.returncode == 0, (args, result.stderr)

    write_tree(case.root, old)
    g('init', '-q')
    g('add', '-A')
    g('commit', '-q', '-m', 'old layout')
    write_tree(case.root, new_layout())
    git_ok = run(case.root, '--check', '--baseline', 'HEAD')
    assert git_ok.returncode == 0 and b'3 baseline entries (old layout)' in git_ok.stdout, git_ok.stdout
    g('add', '-A')
    g('commit', '-q', '-m', 'migrated')
    write_tree(case.root, new_layout(edited_b))
    git_edit = run(case.root, '--check', '--baseline', 'HEAD')
    assert git_edit.returncode == 1 and b'changed: an edit needs' in git_edit.stdout, git_edit.stdout
    bad = run(case.root, '--check', '--baseline', 'no-such-rev')
    assert bad.returncode == 1 and b'cannot list' in bad.stderr, bad.stderr
    # --at reads the commit, not the working tree: the "migrated" commit holds C with CRLF bytes, which only the
    # committed blob shows; the working tree (here also CRLF) passes the plain check.
    crlf = run(case.root, '--check', '--at', 'HEAD', '--baseline', 'HEAD~1')
    assert crlf.returncode == 1 and b'holds a CR byte' in crlf.stdout and C[0].encode() in crlf.stdout, crlf.stdout
    write_tree(case.root, new_layout((C[0], C[1].replace(b'\r\n', b'\n'))))
    g('add', '-A')
    g('commit', '-q', '-m', 'LF only')
    write_tree(case.root, new_layout(edited_b))
    at_ok = run(case.root, '--check', '--at', 'HEAD', '--baseline', 'HEAD~2')
    assert at_ok.returncode == 0, at_ok.stdout
    case.count += 5

print('decisions_index_test: passed (%d check cases)' % case.count)
