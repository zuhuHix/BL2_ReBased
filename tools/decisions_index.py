"""Index, tail and check the decisions log: one file per decision under docs/decisions/YYYY-MM/.

A decision is the file docs/decisions/YYYY-MM/YYYY-MM-DD-NN-slug.md. Its line 1 is `## YYYY-MM-DD: Title`, and a new
one's body opens with "AI-assisted (<tool>)". NN is the entry's place within its day; slug is the title in lower case,
every run of other characters replaced by one hyphen, cut at a hyphen to at most 60 characters. DECISIONS.md is a
stub that holds no entry. A decision file is never deleted or renamed. It changes only when a file added in the same
change names it on a line `Edits: <path>`; a decision that replaces it is a new file with `Supersedes: <path>`.

The tool also reads the old layout (entries in DECISIONS.md and monthly archives docs/decisions/YYYY-MM.md), in the
working tree and at a baseline revision, so that the migration can be checked against the commit before it.
"""
import argparse
from collections import Counter
import datetime
from pathlib import Path
import re
import subprocess
import sys

HEADER = re.compile(rb'^## ((\d{4}-\d{2})-\d{2}): ([^\r\n]*)')
REPO = Path(__file__).resolve().parents[1]
LOG = 'DECISIONS.md'
DIR = 'docs/decisions'
ARCHIVE = re.compile(r'^docs/decisions/(\d{4}-\d{2})\.md$')
NAME = re.compile(r'^docs/decisions/(\d{4}-\d{2})/((\d{4}-\d{2})-\d{2})-(\d{2})-([a-z0-9]+(?:-[a-z0-9]+)*)\.md$')
LINK = re.compile(rb'^(Edits|Supersedes): `?(docs/decisions/[^\s`]+)`?[ \t]*$', re.M)
# In the stub, any line that looks like an entry heading, at any heading level and with or without the colon.
LOOKS_LIKE_ENTRY = re.compile(rb'^#+[ \t]*\d{4}-\d{2}-\d{2}', re.M)
SLUG_MAX = 60


def emit(text):
    sys.stdout.buffer.write((text + '\n').encode('utf-8'))


def fail(message):
    sys.stderr.buffer.write(('FAIL: ' + message + '\n').encode('utf-8'))
    sys.exit(1)


def normalize(data):
    return data.replace(b'\r\n', b'\n')


def slug(title):
    text = re.sub(r'[^a-z0-9]+', '-', title.lower()).strip('-')
    if len(text) > SLUG_MAX:
        cut = text.rfind('-', 0, SLUG_MAX + 1)
        text = text[:cut] if cut > 0 else text[:SLUG_MAX]
    return text or 'entry'


def split_entries(data):
    parts = data.split(b'\n')
    lines = [p + b'\n' for p in parts[:-1]] + ([parts[-1]] if parts[-1] else [])
    found = [(number, match) for number, match in
             ((n, HEADER.match(line)) for n, line in enumerate(lines)) if match]
    entries = []
    for k, (number, match) in enumerate(found):
        end = found[k + 1][0] if k + 1 < len(found) else len(lines)
        entries.append({'line': number + 1,
                        'date': match.group(1).decode('ascii'),
                        'month': match.group(2).decode('ascii'),
                        'title': match.group(3).decode('utf-8', 'replace'),
                        'body': b''.join(lines[number:end])})
    return entries


# A tree is {repository-relative path: bytes} for DECISIONS.md and everything under docs/decisions/.

def worktree_tree(root):
    tree = {}
    if (root / LOG).is_file():
        tree[LOG] = (root / LOG).read_bytes()
    if (root / DIR).is_dir():
        for path in (root / DIR).rglob('*'):
            if path.is_file():
                tree[path.relative_to(root).as_posix()] = path.read_bytes()
    return tree


def git_tree(root, rev):
    listing = subprocess.run(['git', 'ls-tree', '-r', '-z', rev, '--', LOG, DIR], cwd=str(root), capture_output=True)
    if listing.returncode != 0:
        fail('cannot list %s at %s: %s' % (DIR, rev, listing.stderr.decode('utf-8', 'replace').strip()))
    blobs = []
    for item in listing.stdout.split(b'\0'):
        if item:
            meta, path = item.split(b'\t', 1)
            if meta.split()[1] == b'blob':
                blobs.append((meta.split()[2], path.decode('utf-8')))
    cat = subprocess.run(['git', 'cat-file', '--batch'], input=b''.join(sha + b'\n' for sha, _ in blobs),
                         cwd=str(root), capture_output=True)
    if cat.returncode != 0:
        fail('cannot read the blobs at %s' % rev)
    tree, data, at = {}, cat.stdout, 0
    for _, path in blobs:
        end = data.index(b'\n', at)
        size = int(data[at:end].split()[2])
        tree[path] = data[end + 1:end + 1 + size]
        at = end + 1 + size + 1
    return tree


def decision_paths(tree):
    return [p for p in tree if NAME.match(p)]


def is_old_layout(tree):
    """No decision file yet, and entries in DECISIONS.md or a monthly archive."""
    if decision_paths(tree):
        return False
    return any(ARCHIVE.match(p) for p in tree) or bool(LOG in tree and split_entries(tree[LOG]))


def links(data):
    return [(kind.decode('ascii'), target.decode('utf-8')) for kind, target in LINK.findall(normalize(data))]


def opens_with_ai_line(data):
    rest = [line for line in normalize(data).split(b'\n')[1:] if line.strip()]
    return bool(rest) and rest[0].startswith(b'AI-assisted (')


def records(tree):
    """Old-layout entries (archives oldest month first, then DECISIONS.md), then decision files by date, NN, slug."""
    out = []
    for path in sorted(p for p in tree if ARCHIVE.match(p)) + ([LOG] if LOG in tree else []):
        for entry in split_entries(tree[path]):
            entry['path'] = path
            out.append(entry)
    files = []
    for path in decision_paths(tree):
        m = NAME.match(path)
        head = HEADER.match(tree[path])
        files.append(((m.group(2), int(m.group(4)), m.group(5)),
                      {'path': path, 'line': 1, 'date': m.group(2), 'body': tree[path],
                       'title': head.group(3).decode('utf-8', 'replace') if head else '(no heading on line 1)'}))
    return out + [entry for _, entry in sorted(files, key=lambda pair: pair[0])]


def list_entries(tree, needle=None):
    superseded = {}
    for path in decision_paths(tree):
        for kind, target in links(tree[path]):
            if kind == 'Supersedes':
                superseded.setdefault(target, []).append(path.rsplit('/', 1)[1])
    for entry in records(tree):
        if needle is None or needle in entry['title'].lower() or needle in entry['date']:
            mark = '  (superseded by %s)' % ', '.join(superseded[entry['path']]) if entry['path'] in superseded else ''
            emit('%s:%d  %s  %s%s' % (entry['path'], entry['line'], entry['date'], entry['title'], mark))


def tail(tree, count):
    if count < 1:
        raise SystemExit('--tail needs a positive count')
    for entry in records(tree)[-count:]:
        sys.stdout.buffer.write(entry['body'])


def next_name(tree, date, title):
    if not re.match(r'^\d{4}-\d{2}-\d{2}$', date):
        raise SystemExit('--date needs YYYY-MM-DD')
    taken = [int(NAME.match(p).group(4)) for p in decision_paths(tree) if NAME.match(p).group(2) == date]
    number = max(taken, default=0) + 1
    if number > 99:
        raise SystemExit('%s already has 99 decisions' % date)
    return '%s/%s/%s-%02d-%s.md' % (DIR, date[:7], date, number, slug(title))


def structure(tree, blobs=False):
    """Rules on the tree alone. blobs: the tree holds committed blobs (--at), which must hold no CR byte."""
    problems = []
    if is_old_layout(tree):
        for path in sorted(p for p in tree if ARCHIVE.match(p)):
            for entry in split_entries(tree[path]):
                if entry['month'] != ARCHIVE.match(path).group(1):
                    problems.append('%s holds %s entry %s' % (path, entry['date'], entry['title']))
        return problems
    if LOG not in tree:
        problems.append('%s is missing: the stub that says where decisions live stays' % LOG)
    for path in sorted(tree):
        if path == LOG:
            held = LOOKS_LIKE_ENTRY.findall(normalize(tree[LOG]))
            if held:
                problems.append('%s holds %d line(s) that look like an entry heading, the first "%s": it holds no '
                                'entry; each decision is its own file (python tools/decisions_index.py --name "<title>" '
                                'prints the next name)' % (LOG, len(held), held[0].decode('utf-8', 'replace')))
            continue
        m = NAME.match(path)
        if not m:
            problems.append('%s is not a decision file: only %s/YYYY-MM/YYYY-MM-DD-NN-slug.md lives there' % (path, DIR))
            continue
        if m.group(1) != m.group(3):
            problems.append('%s sits in the folder of another month' % path)
        if len(m.group(5)) > SLUG_MAX:
            problems.append('%s: the slug is longer than %d characters' % (path, SLUG_MAX))
        if blobs and b'\r' in tree[path]:
            problems.append('%s holds a CR byte: a decision file is committed with LF line ends' % path)
        data = normalize(tree[path])
        head = HEADER.match(data)
        if not head:
            problems.append('%s: line 1 is not "## YYYY-MM-DD: Title"' % path)
        elif head.group(1).decode('ascii') != m.group(2):
            problems.append('%s: the heading date %s is not the date in the name' % (path, head.group(1).decode('ascii')))
        if len(split_entries(data)) > 1:
            problems.append('%s holds %d dated headings; one decision per file' % (path, len(split_entries(data))))
        for kind, target in links(data):
            if target == path or not NAME.match(target) or target not in tree:
                problems.append('%s: "%s: %s" does not name another decision file' % (path, kind, target))
    return problems


def against(tree, base):
    """Rules against the baseline tree. Returns (problems, summary)."""
    problems = []
    files = {p: normalize(tree[p]) for p in decision_paths(tree)}
    if is_old_layout(base):
        wanted = Counter(normalize(e['body']) for e in records(base))
        present = Counter(normalize(e['body']) for e in records(tree) if e['path'] not in files) + Counter(files.values())
        reported = set()
        for entry in records(base):
            key = normalize(entry['body'])
            if present[key] != wanted[key] and key not in reported:
                reported.add(key)
                problems.append('baseline entry %s %s is present %d time(s), expected %d (missing or changed)'
                                % (entry['date'], entry['title'], present[key], wanted[key]))
        added = sorted(p for p, data in files.items() if data not in wanted)
        summary = '%d baseline entries (old layout) present exactly as often as at the baseline' % sum(wanted.values())
        edited = []
    else:
        old = {p: normalize(base[p]) for p in decision_paths(base)}
        added = sorted(p for p in files if p not in old)
        allowed = {target for p in added for kind, target in links(files[p]) if kind == 'Edits'}
        edited = []
        for path in sorted(old):
            if path not in files:
                problems.append('%s was deleted or renamed: a decision file stays (a replacement is a new file '
                                'with "Supersedes: %s")' % (path, path))
            elif files[path] != old[path]:
                if path in allowed:
                    edited.append(path)
                else:
                    problems.append('%s changed: an edit needs a new file in the same change with the line '
                                    '"Edits: %s" and the reason' % (path, path))
        summary = '%d baseline decision files unchanged or edited under an Edits: line' % len(old)
    for path in added:
        if not opens_with_ai_line(files[path]):
            problems.append('%s is new, and its body does not open with "AI-assisted (<tool>)"' % path)
    return problems, '%s; %d added, %d edited' % (summary, len(added), len(edited))


def check(tree, base, label, blobs=False):
    problems = structure(tree, blobs)
    found, summary = against(tree, base)
    problems += found
    for problem in problems:
        emit('FAIL: ' + problem)
    if problems:
        return 1
    emit('OK: %s (baseline %s)' % (summary, label))
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--root', type=Path, default=REPO, help='repository root (default: this checkout)')
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--tail', type=int, metavar='N', help='print the last N entries verbatim')
    mode.add_argument('--grep', metavar='WORD',
                      help='list entries whose title (case-insensitive) or date (YYYY-MM-DD) contains WORD')
    mode.add_argument('--check', action='store_true', help='check the layout, and the changes against the baseline')
    mode.add_argument('--name', metavar='TITLE', help='print the file name the next decision with this title takes')
    parser.add_argument('--date', default=datetime.date.today().isoformat(), help='date for --name (default: today)')
    parser.add_argument('--baseline', default='HEAD', metavar='REV',
                        help='git revision the working tree is checked against (default: HEAD)')
    parser.add_argument('--baseline-dir', metavar='PATH', help='read the baseline from a directory instead of git')
    parser.add_argument('--at', metavar='REV', help='read the decisions from the commit REV instead of the working tree; '
                        '--check then also refuses a CR byte in a decision blob')
    args = parser.parse_args(argv)
    root = args.root.resolve()
    tree = git_tree(root, args.at) if args.at else worktree_tree(root)
    if args.tail is not None:
        tail(tree, args.tail)
    elif args.grep is not None:
        list_entries(tree, args.grep.lower())
    elif args.name is not None:
        emit(next_name(tree, args.date, args.name))
    elif args.check:
        blobs = args.at is not None
        if args.baseline_dir:
            return check(tree, worktree_tree(Path(args.baseline_dir).resolve()), args.baseline_dir, blobs)
        return check(tree, git_tree(root, args.baseline), args.baseline, blobs)
    else:
        list_entries(tree)
    return 0


if __name__ == '__main__':
    sys.exit(main())
