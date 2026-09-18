"""Read OpenBLCMM's Borderlands 2 object dumps as an observed-game oracle.

OpenBLCMM ships the output of the game's own ``obj dump`` console command for
every object it indexes: an SQLite database (``data.db``) mapping object names
to a byte range inside per-class dump files packed in a data jar. Each dump is
the object's full property state as the running game reports it, defaults
included, in ``Key=Value`` text with UnrealScript literal syntax.

This module locates a dump, slices it out of the jar and parses the text into
Python values. Nothing here reads game packages; it reads what the game printed.
The dump data is not redistributed and stays on the user's machine. Compare
results with our decoder through ``crosscheck_blcmm_dumps.py``.

Usage: ``python tools/blcmm_dumps.py <object name> [--db PATH] [--jar PATH]``
prints the parsed properties as JSON. Level objects are named
``Sanctuary_P.TheWorld:PersistentLevel.Terrain_2`` (note the colon).
"""
import argparse
import json
import os
from pathlib import Path
import re
import sqlite3
import sys
import zipfile

DEFAULT_DB = Path(os.environ.get('LOCALAPPDATA', '')) / 'OpenBLCMM' / 'extracted-data' / 'BL2' / 'data.db'
DEFAULT_JAR_DIR = Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)')) / 'OpenBLCMM'

HEADER = re.compile(r"^\*\*\* Property dump for object '(\S+) (.+)' \*\*\*$")
ARRAY_KEY = re.compile(r'^(\w+)\((\d+)\)$')
STATIC_KEY = re.compile(r'^(\w+)\[(\d+)\]$')
REFERENCE = re.compile(r"^(\w+)'([^']*)'$")
NUMBER = re.compile(r'^-?\d+(\.\d+)?$')


def level_object(level, path):
    """Our ``TheWorld.PersistentLevel.X`` path inside ``level`` -> the game's dump name."""
    if path.startswith('TheWorld.PersistentLevel.'):
        return f"{level}.TheWorld:PersistentLevel.{path[len('TheWorld.PersistentLevel.'):]}"
    return path


def split_top_level(text, separator=','):
    """Split on ``separator`` outside parentheses and quotes."""
    parts, depth, quoted, start = [], 0, False, 0
    for i, ch in enumerate(text):
        if ch == '"':
            quoted = not quoted
        elif quoted:
            continue
        elif ch == '(':
            depth += 1
        elif ch == ')':
            depth -= 1
        elif ch == separator and depth == 0:
            parts.append(text[start:i])
            start = i + 1
    parts.append(text[start:])
    return parts


def parse_value(text):
    """UnrealScript literal -> Python: structs to dicts, refs to {'class','path'}, numbers, bools, None."""
    text = text.strip()
    if text == '':
        return None
    if text == 'None':
        return None
    if text in ('True', 'False'):
        return text == 'True'
    if text.startswith('"') and text.endswith('"') and len(text) >= 2:
        return text[1:-1]
    if text.startswith('(') and text.endswith(')'):
        struct = {}
        for part in split_top_level(text[1:-1]):
            if '=' not in part:
                continue
            key, value = part.split('=', 1)
            struct[key.strip()] = parse_value(value)
        return struct
    reference = REFERENCE.match(text)
    if reference:
        return dict(**{'class': reference.group(1)}, path=reference.group(2))
    if NUMBER.match(text):
        return float(text) if '.' in text else int(text)
    return text


def parse_dump(text):
    """Parse one dump into dict(class, name, properties). Arrays ``Key(N)`` become lists, ``Key[N]`` too."""
    lines = text.splitlines()
    header = HEADER.match(lines[0].strip()) if lines else None
    result = dict(**{'class': header.group(1) if header else None}, name=header.group(2) if header else None, properties={})
    properties = result['properties']
    for line in lines[1:]:
        line = line.rstrip('\r')
        if not line or line.startswith('===') or '=' not in line:
            continue
        key, value = line.split('=', 1)
        key = key.strip()
        indexed = ARRAY_KEY.match(key) or STATIC_KEY.match(key)
        parsed = parse_value(value)
        if indexed:
            name, index = indexed.group(1), int(indexed.group(2))
            bucket = properties.setdefault(name, [])
            if not isinstance(bucket, list):
                continue
            while len(bucket) <= index:
                bucket.append(None)
            bucket[index] = parsed
        else:
            properties[key] = parsed
    return result


class Dumps:
    def __init__(self, db=DEFAULT_DB, jar=None):
        self.connection = sqlite3.connect(str(db))
        self.jar = zipfile.ZipFile(str(jar or find_jar()))
        self.files = {}
        self.cache = {}

    def locate(self, name):
        row = self.connection.execute(
            'select c.name, o.file_index, o.file_position, o.bytes from object o join class c on c.id = o.class where o.name = ?',
            (name,)).fetchone()
        return None if row is None else dict(**{'class': row[0]}, file_index=row[1], position=row[2], size=row[3])

    def text(self, name):
        located = self.locate(name)
        if located is None:
            return None
        key = f"data/BL2/dumps/{located['class']}.dump.{located['file_index']}"
        if key not in self.files:
            self.files[key] = self.jar.read(key)
        return self.files[key][located['position']:located['position'] + located['size']].decode('latin-1')

    def dump(self, name):
        if name not in self.cache:
            text = self.text(name)
            self.cache[name] = None if text is None else parse_dump(text)
        return self.cache[name]


def find_jar(directory=DEFAULT_JAR_DIR):
    jars = sorted(Path(directory).glob('blcmm_data_BL2-*.jar'))
    if not jars:
        raise FileNotFoundError(f'no blcmm_data_BL2-*.jar under {directory}')
    return jars[-1]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('name', help="object name, e.g. Prop_Buildings_02.Materials.Mati_BanditHouse_01")
    parser.add_argument('--db', type=Path, default=DEFAULT_DB)
    parser.add_argument('--jar', type=Path, default=None)
    parser.add_argument('--raw', action='store_true', help='print the dump text instead of parsed JSON')
    args = parser.parse_args()
    dumps = Dumps(args.db, args.jar)
    if args.raw:
        text = dumps.text(args.name)
        if text is None:
            sys.exit(f'no dump for {args.name}')
        print(text)
        return 0
    dump = dumps.dump(args.name)
    if dump is None:
        sys.exit(f'no dump for {args.name}')
    print(json.dumps(dump, indent=1))
    return 0


if __name__ == '__main__':
    sys.exit(main())
