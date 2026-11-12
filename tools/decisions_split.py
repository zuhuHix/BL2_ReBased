"""One-off: split the old decisions log into one file per decision, and prove the split byte for byte.

AI-assisted (Claude Code), 2026-11-12. Used once, for the migration to docs/decisions/YYYY-MM/YYYY-MM-DD-NN-slug.md, and
deleted in the commit after its output. To redo the split from a newer main, take this file from the commit that added
it (git log --diff-filter=A --format=%H -- tools/decisions_split.py) and run it; the steps are in that PR's text.

  python tools/decisions_split.py [--from REV]          write the split of REV's log into the working tree
  python tools/decisions_split.py --verify --from REV --to REV2

The split reads the committed blobs of DECISIONS.md and docs/decisions/YYYY-MM.md at REV (default HEAD), so the new
files hold LF bytes whatever the checkout does with line ends. Each entry, from its `## YYYY-MM-DD: Title` line up to the
next one, becomes one file, byte for byte. NN numbers a day's entries in the order they stand in the old files (the
archive first, then DECISIONS.md), which is the order they landed in. The archives are deleted, and DECISIONS.md keeps
only its text before the first entry.

--verify proves the split at REV2 against REV from the blobs alone: each old file at REV equals its text before the
first entry at REV followed by the new files' blobs at REV2, in the old order; every decision file at REV2 is used
exactly once; no new blob holds a CR byte; DECISIONS.md at REV2 is exactly that leading text; no archive is left.
This file carries its own copy of the entry and slug rules so that it gives the same output after the tools change.
"""
import argparse
from pathlib import Path
import re
import subprocess
import sys

HEADER = re.compile(rb'^## ((\d{4}-\d{2})-\d{2}): ([^\r\n]*)')
LOG = 'DECISIONS.md'
DIR = 'docs/decisions'
ARCHIVE = re.compile(r'^docs/decisions/\d{4}-\d{2}\.md$')
NAME = re.compile(r'^docs/decisions/\d{4}-\d{2}/\d{4}-\d{2}-\d{2}-\d{2}-[a-z0-9]+(?:-[a-z0-9]+)*\.md$')
SLUG_MAX = 60


def slug(title):
    text = re.sub(r'[^a-z0-9]+', '-', title.lower()).strip('-')
    if len(text) > SLUG_MAX:
        cut = text.rfind('-', 0, SLUG_MAX + 1)
        text = text[:cut] if cut > 0 else text[:SLUG_MAX]
    return text or 'entry'


def git(root, *args, data=None):
    result = subprocess.run(['git'] + list(args), cwd=str(root), input=data, capture_output=True)
    if result.returncode != 0:
        sys.exit('git %s failed: %s' % (' '.join(args), result.stderr.decode('utf-8', 'replace').strip()))
    return result.stdout


def blobs(root, rev):
    """{path: blob bytes} for DECISIONS.md and everything under docs/decisions/ at rev."""
    listing = git(root, 'ls-tree', '-r', '-z', '--name-only', rev, '--', LOG, DIR)
    paths = [p.decode('utf-8') for p in listing.split(b'\0') if p]
    out = git(root, 'cat-file', '--batch', data=b''.join(('%s:%s\n' % (rev, p)).encode('utf-8') for p in paths))
    tree, at = {}, 0
    for path in paths:
        end = out.index(b'\n', at)
        size = int(out[at:end].split()[2])
        tree[path] = out[end + 1:end + 1 + size]
        at = end + 1 + size + 1
    return tree


def split(data):
    """(text before the first entry, [entry bytes, ...]) for one old file."""
    parts = data.split(b'\n')
    lines = [p + b'\n' for p in parts[:-1]] + ([parts[-1]] if parts[-1] else [])
    starts = [n for n, line in enumerate(lines) if HEADER.match(line)]
    if not starts:
        return data, []
    bounds = starts + [len(lines)]
    return (b''.join(lines[:starts[0]]),
            [b''.join(lines[bounds[k]:bounds[k + 1]]) for k in range(len(starts))])


def plan(old):
    """[(old file, leading text, [(new path, entry bytes), ...]), ...] in the old order (archives, then DECISIONS.md)."""
    files = sorted(p for p in old if ARCHIVE.match(p)) + [LOG]
    if any(NAME.match(p) for p in old):
        sys.exit('the log at this revision already has decision files: nothing to split')
    per_day, out = {}, []
    for path in files:
        lead, entries = split(old[path])
        named = []
        for entry in entries:
            head = HEADER.match(entry)
            date, title = head.group(1).decode('ascii'), head.group(3).decode('utf-8')
            per_day[date] = per_day.get(date, 0) + 1
            if per_day[date] > 99:
                sys.exit('%s has more than 99 entries' % date)
            named.append(('%s/%s/%s-%02d-%s.md' % (DIR, date[:7], date, per_day[date], slug(title)), entry))
        out.append((path, lead, named))
    return out


def write(root, rev):
    old = blobs(root, rev)
    steps = plan(old)
    count = 0
    for path, lead, named in steps:
        for new, entry in named:
            target = root / new
            if target.exists():
                sys.exit('%s exists already' % new)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(entry)
            count += 1
        if path == LOG:
            (root / LOG).write_bytes(lead)
        else:
            (root / path).unlink()
    print('split %s: %d entries from %d file(s) into %d decision files' % (rev, count, len(steps), count))


def verify(root, rev, rev2):
    old, new = blobs(root, rev), blobs(root, rev2)
    problems, used = [], []
    steps = plan(old)
    for path, lead, named in steps:
        rebuilt = lead + b''.join(new.get(p, b'<missing %s>' % p.encode()) for p, _ in named)
        if rebuilt != old[path]:
            problems.append('%s at %s is not rebuilt byte for byte from the new blobs at %s' % (path, rev, rev2))
        used += [p for p, _ in named]
    decisions = sorted(p for p in new if NAME.match(p))
    if sorted(used) != decisions or len(set(used)) != len(used):
        problems.append('the decision files at %s are not exactly the %d split entries, each once' % (rev2, len(used)))
    problems += ['%s holds a CR byte' % p for p in decisions if b'\r' in new[p]]
    if new.get(LOG) != steps[-1][1]:
        problems.append('%s at %s is not exactly its leading text at %s' % (LOG, rev2, rev))
    others = sorted(p for p in new if p != LOG and not NAME.match(p))
    if others:
        problems.append('other files under %s at %s: %s' % (DIR, rev2, ', '.join(others)))
    for problem in problems:
        print('FAIL: ' + problem)
    if problems:
        return 1
    print('OK: %d old file(s) at %s rebuilt byte for byte from %d decision blobs at %s, each used once, no CR byte; '
          '%s is its leading text' % (len(steps), rev, len(used), rev2, LOG))
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1], help='repository root')
    parser.add_argument('--from', dest='rev', default='HEAD', help='revision holding the old log (default: HEAD)')
    parser.add_argument('--verify', action='store_true', help='prove the split at --to against --from')
    parser.add_argument('--to', default='HEAD', help='revision holding the split, for --verify (default: HEAD)')
    args = parser.parse_args(argv)
    if args.verify:
        return verify(args.root.resolve(), args.rev, args.to)
    write(args.root.resolve(), args.rev)
    return 0


if __name__ == '__main__':
    sys.exit(main())
