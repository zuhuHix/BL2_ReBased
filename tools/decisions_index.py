"""Index, tail and check the decisions log: DECISIONS.md plus monthly archives in docs/decisions/."""
import argparse
from collections import Counter
from pathlib import Path
import re
import subprocess
import sys

HEADER = re.compile(rb'^## ((\d{4}-\d{2})-\d{2}): ([^\r\n]*)')
REPO = Path(__file__).resolve().parents[1]
LOG = 'DECISIONS.md'
ARCHIVE_DIR = Path('docs') / 'decisions'
ARCHIVE_NAME = re.compile(r'^\d{4}-\d{2}\.md$')


def emit(text):
    sys.stdout.buffer.write((text + '\n').encode('utf-8'))


def fail(message):
    sys.stderr.buffer.write(('FAIL: ' + message + '\n').encode('utf-8'))
    sys.exit(1)


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


def normalize(body):
    return body.replace(b'\r\n', b'\n')


def log_path(root):
    return root / LOG


def archive_paths(root):
    return sorted(p for p in (root / ARCHIVE_DIR).glob('*.md') if ARCHIVE_NAME.match(p.name))


def records(root):
    """Every entry in archive order (oldest month first), then DECISIONS.md."""
    out = []
    for path in archive_paths(root) + [log_path(root)]:
        rel = path.relative_to(root).as_posix()
        for entry in split_entries(path.read_bytes()):
            entry['file'] = rel
            out.append(entry)
    return out


def list_entries(root, needle=None):
    for entry in records(root):
        if needle is None or needle in entry['title'].lower():
            emit('%s:%d  %s  %s' % (entry['file'], entry['line'], entry['date'], entry['title']))


def tail(root, count):
    if count < 1:
        raise SystemExit('--tail needs a positive count')
    for entry in split_entries(log_path(root).read_bytes())[-count:]:
        sys.stdout.buffer.write(entry['body'])


def baseline_bytes(root, rev, baseline_file):
    if baseline_file:
        return Path(baseline_file).read_bytes()
    result = subprocess.run(['git', 'show', '%s:%s' % (rev, LOG)], cwd=str(root), capture_output=True)
    if result.returncode != 0:
        fail('cannot read %s:%s from git' % (rev, LOG))
    return result.stdout


def check(root, rev, baseline_file):
    problems = []
    baseline = split_entries(baseline_bytes(root, rev, baseline_file))
    wanted = Counter(normalize(e['body']) for e in baseline)
    present = Counter(normalize(e['body']) for e in records(root))
    for entry in baseline:
        key = normalize(entry['body'])
        if present[key] != wanted[key]:
            problems.append('baseline entry %s %s is present %d time(s), expected %d (missing or changed)'
                            % (entry['date'], entry['title'], present[key], wanted[key]))
    new = sum(n for key, n in present.items() if key not in wanted)
    log_text = normalize(log_path(root).read_bytes()).decode('utf-8', 'replace')
    archives = archive_paths(root)
    for archive in archives:
        month = archive.stem
        for entry in split_entries(archive.read_bytes()):
            if entry['month'] != month:
                problems.append('%s holds %s entry %s' % (archive.name, entry['date'], entry['title']))
        for entry in split_entries(log_path(root).read_bytes()):
            if entry['month'] == month:
                problems.append('%s still has %s entry %s (line %d)' % (LOG, entry['date'], entry['title'], entry['line']))
        link = '[%s](%s/%s.md)' % (month, ARCHIVE_DIR.as_posix(), month)
        if link not in log_text:
            problems.append('%s lacks the link line for %s' % (LOG, link))
    if problems:
        for problem in problems:
            emit('FAIL: ' + problem)
        return 1
    emit('OK: %d baseline entries present exactly as often as in the baseline (byte-identical after CRLF '
         'normalization); %d entries added since the baseline; %d archive(s) checked'
         % (len(baseline), new, len(archives)))
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=REPO, help='repository root (default: this checkout)')
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--tail', type=int, metavar='N', help='print the last N entries of DECISIONS.md verbatim')
    mode.add_argument('--grep', metavar='WORD', help='list entries whose title contains WORD (case-insensitive)')
    mode.add_argument('--check', action='store_true', help='verify the split against the baseline')
    parser.add_argument('--baseline', default='HEAD', metavar='REV',
                        help='git revision holding the pre-split DECISIONS.md (default: HEAD)')
    parser.add_argument('--baseline-file', metavar='PATH', help='read the baseline from a file instead of git')
    args = parser.parse_args(argv)
    root = args.root.resolve()
    if args.tail is not None:
        tail(root, args.tail)
    elif args.grep is not None:
        list_entries(root, args.grep.lower())
    elif args.check:
        return check(root, args.baseline, args.baseline_file)
    else:
        list_entries(root)
    return 0


if __name__ == '__main__':
    sys.exit(main())
