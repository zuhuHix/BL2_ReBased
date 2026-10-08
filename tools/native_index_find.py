"""Print the NATIVE_INDEX.md header and the rows matching a native-name fragment or a note file."""
import argparse
from pathlib import Path
import re
import sys

REPO = Path(__file__).resolve().parents[1]
DEFAULT_FILE = REPO / 'docs' / 'verification' / 'NATIVE_INDEX.md'
SEPARATOR = re.compile(r'^:?-+:?$')


def cells_of(line):
    inner = line.strip()[1:]
    if inner.endswith('|') and not inner.endswith('\\|'):
        inner = inner[:-1]
    return [cell.strip() for cell in re.split(r'(?<!\\)\|', inner)]


def find(lines, fragments, note):
    header = None
    rows = []
    for line in lines:
        if not line.lstrip().startswith('|'):
            continue
        cells = cells_of(line)
        if header is None and cells and cells[0] == 'Native':
            header = line.rstrip('\r\n')
            continue
        if header is None or all(SEPARATOR.match(cell) for cell in cells):
            continue
        native = cells[0].lower() if cells else ''
        note_cell = cells[1].lower() if len(cells) > 1 else ''
        if fragments and not any(fragment in native for fragment in fragments):
            continue
        if note is not None and note not in note_cell:
            continue
        rows.append(line.rstrip('\r\n'))
    return header, rows


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('fragments', nargs='*', help='text the native name (first column) must contain')
    parser.add_argument('--note', metavar='NAME', help='only rows whose note column contains NAME')
    parser.add_argument('--file', type=Path, default=DEFAULT_FILE, help='table to search (default: NATIVE_INDEX.md)')
    args = parser.parse_args(argv)
    if not args.fragments and args.note is None:
        parser.error('give a fragment or --note NAME')
    fragments = [fragment.lower() for fragment in args.fragments if fragment]
    note = args.note.lower() if args.note is not None else None
    lines = args.file.read_text(encoding='utf-8').splitlines(keepends=True)
    header, rows = find(lines, fragments, note)
    if header is None:
        sys.stderr.write('no table with a Native header in %s\n' % args.file)
        return 1
    if not rows:
        sys.stderr.write('no rows match\n')
        return 1
    sys.stdout.buffer.write(('\n'.join([header, lines_separator(lines)] + rows) + '\n').encode('utf-8'))
    return 0


def lines_separator(lines):
    for line in lines:
        cells = cells_of(line) if line.lstrip().startswith('|') else []
        if cells and all(SEPARATOR.match(cell) for cell in cells):
            return line.rstrip('\r\n')
    return '|---|---|---|---|---|---|'


if __name__ == '__main__':
    sys.exit(main())
