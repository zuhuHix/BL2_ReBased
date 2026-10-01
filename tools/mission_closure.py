"""Recover the reference closure of one stock mission with the project's own reader.

AI-assisted. Clean-room: this file contains no game data. Everything it derives
from the installed game (decoded properties, Kismet link graphs, payload gap
lists) is written under the ignored repository ``local/`` directory only.

Method (all of it bounded and deterministic):

* package/object identity comes from ``ow-package`` (``--exports``, ``--imports``,
  ``--properties``, ``--payload``) and from the export lists cached by
  ``tools/export_index.py``; a path index over those caches (SQLite, ignored)
  enumerates *every* package that contains an object path, so duplicates across
  map copies are listed instead of silently picking a first match;
* array element types come from the cooked property declarations. For every
  ArrayProperty of the object's class chain (and of struct types met while
  decoding) the element type is the declared inner property; the inner export
  must be a child of the array property export (structural oracle) and, for
  struct elements, the struct is named by the last four bytes of the inner
  property export (checked to be a ScriptStruct). That tail layout is an
  observation, not a published spec: UNVERIFIED beyond the structural checks;
* object property prefixes are limited to the three established values 4/8/26
  (see DECISIONS.md); the first that decodes with exact consumption is used.
  Prefix semantics: UNVERIFIED.

Nothing here executes game logic. Classification of Kismet/Behavior classes into
"data only" or "needs native executor" is a statement about this tool's
evidence, not about the original engine.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor

TOOL = 'tools/mission_closure.py'
TOOL_VERSION = '1'
DECODE_REV = 4  # bump when decode/schema logic changes; invalidates cached property records
ROOT = Path(__file__).resolve().parents[1]
PREFIXES = (4, 8, 26)
# Declared inner property classes this tool maps onto the reader's array schema vocabulary.
SIMPLE_INNER = {
    'Core.IntProperty': 'IntProperty', 'Core.FloatProperty': 'FloatProperty',
    'Core.NameProperty': 'NameProperty', 'Core.StrProperty': 'StrProperty',
    'Core.ObjectProperty': 'ObjectProperty', 'Core.ClassProperty': 'ObjectProperty',
    'Core.ComponentProperty': 'ObjectProperty', 'Core.ByteProperty': 'ByteProperty',
}
# Arrays that appear in tagged data but have no declaration in any cooked package (checked against the
# path index: no ArrayProperty export of that name exists). The element type is FITTED: every observed
# element is 8 bytes and decodes as a valid name with exact tag consumption, and the names match Kismet
# remote-event names. Label: UNVERIFIED (no declaration to confirm it).
UNDECLARED_ARRAYS = {
    'SupportedRemoteEvents': 'NameProperty',
    'SupportedCustomEvents': 'NameProperty',
    'ImplementedCustomEvents': 'NameProperty',
}
# Exported classes that are code/type definitions, not data to follow.
TYPE_CLASSES = {
    'Class', 'Core.Class', 'Core.Function', 'Core.State', 'Core.ScriptStruct', 'Core.Enum', 'Core.Const',
    'Core.Package', 'Package', 'Core.Field', 'Core.TextBuffer', 'Core.Struct',
}


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')


def sha256(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for block in iter(lambda: f.read(1 << 20), b''):
            h.update(block)
    return h.hexdigest()


class Failure(Exception):
    pass


def locked(fn):
    def wrapper(self, *a, **k):
        with self.lock:
            return fn(self, *a, **k)
    wrapper.__name__ = fn.__name__
    return wrapper


class Env:
    """Reader executable, game directory, export caches and the path index."""

    def __init__(self, game, reader, cache, use_cache=True):
        self.game = Path(game)
        self.cooked = self.game / 'WillowGame' / 'CookedPCConsole'
        if not (self.cooked / 'Startup.upk').exists():
            raise SystemExit('Not a Borderlands 2 install: ' + str(self.cooked))
        self.reader = Path(reader).resolve()
        if not self.reader.exists():
            raise SystemExit('Reader not found: ' + str(self.reader))
        self.cache = Path(cache).resolve()
        if not self.cache.is_relative_to(ROOT / 'local'):
            raise SystemExit('Cache/output must stay under repository local/')
        self.cache.mkdir(parents=True, exist_ok=True)
        self.exports_dir = ROOT / 'local' / 'census' / 'exports'
        self.exports_dir.mkdir(parents=True, exist_ok=True)
        self.use_cache = use_cache
        self.calls = 0
        self.reader_sha = sha256(self.reader)
        self._exports = {}
        self._imports = {}
        self._arrays = {}
        self._chains = {}
        self.db = None
        self.lock = threading.RLock()

    # -- low-level reader access ------------------------------------------------------
    def upk(self, pkg):
        p = self.cooked / (pkg + '.upk')
        if not p.exists():
            raise Failure('package not found: ' + pkg)
        return p

    def run(self, pkg, *args, timeout=300):
        self.calls += 1
        proc = subprocess.run([str(self.reader), str(self.upk(pkg)), *map(str, args)],
                              capture_output=True, text=True, encoding='utf-8', timeout=timeout)
        if proc.returncode:
            raise Failure((proc.stderr or proc.stdout).strip()[:300] or 'reader failed')
        return json.loads(proc.stdout)

    def stat_key(self, pkg):
        st = self.upk(pkg).stat()
        return [st.st_size, int(st.st_mtime)]

    @locked
    def exports(self, pkg):
        """Export list (list position = index - 1) cached by tools/export_index.py."""
        if pkg in self._exports:
            return self._exports[pkg]
        cached = self.exports_dir / (pkg + '.json')
        index_path = self.exports_dir / '_index.json'
        index = json.loads(index_path.read_text()) if index_path.exists() else {}
        if cached.exists() and index.get(pkg, {}).get('stat') == self.stat_key(pkg):
            data = json.loads(cached.read_text(encoding='utf-8'))
        else:
            data = self.run(pkg, '--exports')
            cached.write_text(json.dumps(data, separators=(',', ':')), encoding='utf-8')
            index[pkg] = {'stat': self.stat_key(pkg), 'exports': len(data)}
            index_path.write_text(json.dumps(index))
        for n, x in enumerate(data, 1):
            if x['index'] != n:
                raise Failure('export list not contiguous: ' + pkg)
        self._exports[pkg] = data
        return data

    @locked
    def imports(self, pkg):
        if pkg in self._imports:
            return self._imports[pkg]
        cached = self.cache / ('imports-' + pkg + '.json')
        key = self.stat_key(pkg)
        if self.use_cache and cached.exists():
            blob = json.loads(cached.read_text(encoding='utf-8'))
            if blob['stat'] == key and blob['reader'] == self.reader_sha:
                self._imports[pkg] = {x['index']: x for x in blob['imports']}
                return self._imports[pkg]
        data = self.run(pkg, '--imports')
        cached.write_text(json.dumps({'stat': key, 'reader': self.reader_sha, 'imports': data}), encoding='utf-8')
        self._imports[pkg] = {x['index']: x for x in data}
        return self._imports[pkg]

    def export(self, pkg, idx):
        data = self.exports(pkg)
        if not 1 <= idx <= len(data):
            raise Failure('export index out of range %s:%s' % (pkg, idx))
        return data[idx - 1]

    def class_of(self, pkg, x):
        """Package-qualified class path of an export (local classes are listed unqualified)."""
        c = x['class']
        if x['class_index'] > 0 and '.' not in c:
            return pkg + '.' + c
        return c

    # -- path index across every cached package ---------------------------------------
    @locked
    def open_index(self):
        if self.db:
            return self.db
        db = sqlite3.connect(str(ROOT / 'local' / 'census' / 'path_index.sqlite'), check_same_thread=False)
        db.execute('PRAGMA journal_mode=OFF')
        db.execute('PRAGMA synchronous=OFF')
        db.execute('CREATE TABLE IF NOT EXISTS pkgs(pkg TEXT PRIMARY KEY, size INT, mtime INT, n INT)')
        db.execute('CREATE TABLE IF NOT EXISTS ex(pkg TEXT, idx INT, name TEXT, lpath TEXT, path TEXT, class TEXT,'
                   ' outer INT, super INT, arch INT, size INT)')
        db.execute('CREATE INDEX IF NOT EXISTS ex_l ON ex(lpath)')
        db.execute('CREATE INDEX IF NOT EXISTS ex_n ON ex(name, class)')
        known = {r[0]: (r[1], r[2]) for r in db.execute('SELECT pkg,size,mtime FROM pkgs')}
        t0 = time.time()
        built = 0
        for pkg_file in sorted(self.cooked.glob('*.upk')):
            pkg = pkg_file.stem
            st = pkg_file.stat()
            if known.get(pkg) == (st.st_size, int(st.st_mtime)):
                continue
            data = self.exports(pkg)
            db.execute('DELETE FROM ex WHERE pkg=?', (pkg,))
            db.executemany('INSERT INTO ex VALUES(?,?,?,?,?,?,?,?,?,?)',
                           [(pkg, x['index'], x['name'], x['path'].lower(), x['path'], x['class'], x['outer_index'],
                             x['super_index'], x['archetype_index'], x['size']) for x in data])
            db.execute('INSERT OR REPLACE INTO pkgs VALUES(?,?,?,?)', (pkg, st.st_size, int(st.st_mtime), len(data)))
            db.commit()
            self._exports.pop(pkg, None)  # keep memory bounded while building
            built += 1
        if built:
            print('path index: (re)built %d packages in %ds' % (built, time.time() - t0), flush=True)
        self.db = db
        return db

    @locked
    def find_path(self, path):
        """Every (package, index, class) whose object path equals ``path`` (case-insensitive)."""
        rows = self.open_index().execute('SELECT pkg,idx,class FROM ex WHERE lpath=? ORDER BY pkg,idx',
                                         (path.lower(),)).fetchall()
        return [{'package': r[0], 'index': r[1], 'class': r[2]} for r in rows]

    @locked
    def find_name(self, name, cls):
        rows = self.open_index().execute('SELECT pkg,idx,path,outer FROM ex WHERE name=? AND class=? ORDER BY pkg,idx',
                                         (name, cls)).fetchall()
        return [{'package': r[0], 'index': r[1], 'path': r[2], 'outer': r[3]} for r in rows]

    # -- identity ---------------------------------------------------------------------
    def ref_info(self, pkg, ref):
        """Describe an object reference (an ObjectProperty value) seen inside ``pkg``."""
        idx = ref['index']
        if idx > 0:
            x = self.export(pkg, idx)
            return {'kind': 'export', 'package': pkg, 'index': idx, 'path': x['path'], 'class': x['class']}
        imp = self.imports(pkg).get(idx)
        if imp is None:
            raise Failure('import index %d not in %s' % (idx, pkg))
        return {'kind': 'import', 'source_package': pkg, 'source_index': idx, 'path': imp['path'],
                'class': imp['class_package'] + '.' + imp['class_name']}

    def resolve_import(self, pkg, info, prefer=()):
        """Resolve an import to concrete exports. Returns (chosen, candidates, rule)."""
        path = info['path']
        cands = []
        root = path.split('.', 1)[0]
        # (a) rooted in a package that is a disk file: look inside that file first
        if '.' in path and (self.cooked / (root + '.upk')).exists():
            sub = path.split('.', 1)[1]
            cands = [c for c in self.find_path(sub) if c['package'].lower() == root.lower()]
        # (b) otherwise the global object path (GD_* data is present in many files)
        if not cands:
            cands = self.find_path(path)
        if not cands:
            return None, [], 'unresolved'
        if len(cands) == 1:
            return cands[0], cands, 'unique'
        # Rule for duplicates: the referencing package's own copy, then Startup, then a
        # non-map package, else the alphabetically first. All candidates are kept for the report.
        for c in cands:
            if c['package'] == pkg:
                return c, cands, 'duplicate:self-package'
        for want in prefer:
            for c in cands:
                if c['package'] == want:
                    return c, cands, 'duplicate:preferred:' + want
        for c in cands:
            if c['package'] == 'Startup':
                return c, cands, 'duplicate:Startup'
        base = [c for c in cands if not is_map_package(c['package'])]
        if base:
            return base[0], cands, 'duplicate:non-map-first'
        return cands[0], cands, 'duplicate:alphabetical-first'

    # -- class chain / array declarations --------------------------------------------
    @locked
    def class_export(self, class_path):
        pkg, _, name = class_path.partition('.')
        if not name:
            raise Failure('class path without package: ' + class_path)
        for x in self.find_path(name):
            if x['package'].lower() == pkg.lower() and x['class'] in ('Class', 'Core.Class'):
                return x['package'], x['index']
        raise Failure('class export not found: ' + class_path)

    @locked
    def class_chain(self, class_path):
        """[(class_path, package, index)] from the class up to Object."""
        if class_path in self._chains:
            return self._chains[class_path]
        out = []
        pkg, idx = self.class_export(class_path)
        seen = set()
        while True:
            if (pkg, idx) in seen or len(out) > 64:
                raise Failure('cyclic class chain')
            seen.add((pkg, idx))
            x = self.export(pkg, idx)
            out.append((pkg + '.' + x['path'], pkg, idx))
            s = x['super_index']
            if s == 0:
                break
            if s > 0:
                idx = s
            else:
                imp = self.imports(pkg).get(s)
                if imp is None:
                    raise Failure('super import missing')
                pkg, idx = self.class_export(imp['path'])
        self._chains[class_path] = out
        return out

    def payload_tail(self, pkg, idx):
        data = self.run(pkg, '--payload', idx)
        if len(data) < 4:
            raise Failure('short property export')
        return int.from_bytes(bytes(data[-4:]), 'little', signed=True)

    @locked
    def declared_arrays(self, pkg, owner_idx):
        """Declared ArrayProperty element types for the children of one class/struct export.

        Returns {name: [element_type | None, note | None]}.
        """
        key = (pkg, owner_idx)
        if key in self._arrays:
            return self._arrays[key]
        disk = self.cache / ('arrays-%s-%d.json' % (pkg, owner_idx))
        stat = self.stat_key(pkg)
        if self.use_cache and disk.exists():
            blob = json.loads(disk.read_text(encoding='utf-8'))
            if blob['stat'] == stat and blob['reader'] == self.reader_sha:
                self._arrays[key] = blob['arrays']
                return blob['arrays']
        data = self.exports(pkg)
        result = {}
        for x in data:
            if x['outer_index'] != owner_idx or x['class'] != 'Core.ArrayProperty':
                continue
            tail = self.payload_tail(pkg, x['index'])
            note = None
            elem = None
            if tail <= 0:
                note = 'array property tail is not a local export'
            else:
                inner = data[tail - 1]
                if inner['outer_index'] != x['index']:
                    note = 'inner export is not a child of the array property (oracle failed)'
                elif inner['class'] in SIMPLE_INNER:
                    elem = SIMPLE_INNER[inner['class']]
                elif inner['class'] == 'Core.StructProperty':
                    ref = self.payload_tail(pkg, inner['index'])
                    if ref > 0:
                        s = data[ref - 1]
                        if s['class'] == 'Core.ScriptStruct':
                            elem = 'StructProperty:' + s['name']
                        else:
                            note = 'struct tail is not a ScriptStruct'
                    elif ref < 0 and ref in self.imports(pkg):
                        elem = 'StructProperty:' + self.imports(pkg)[ref]['name']
                    else:
                        note = 'struct tail unresolved'
                else:
                    note = 'unsupported inner property class ' + inner['class']
            result[x['name']] = [elem, note]
        disk.write_text(json.dumps({'stat': stat, 'reader': self.reader_sha, 'arrays': result}), encoding='utf-8')
        self._arrays[key] = result
        return result

    def array_declared_elsewhere(self, name):
        """Element type of an array property that this class chain does not declare, if every other
        declaration of that property name in the install agrees (otherwise None)."""
        decls = set()
        for c in self.find_name(name, 'Core.ArrayProperty'):
            spec = self.declared_arrays(c['package'], c['outer']).get(name)
            if spec is None or spec[0] is None:
                return None
            decls.add(spec[0])
        if len(decls) == 1:
            return [decls.pop(), 'declared-elsewhere']
        return None

    def struct_arrays(self, struct_name, prefer_pkgs):
        """Declared arrays of a ScriptStruct located by name (preferring the object's own packages)."""
        cands = self.find_name(struct_name, 'Core.ScriptStruct')
        if not cands:
            return None, 'ScriptStruct %s not found' % struct_name
        for pref in prefer_pkgs:
            for c in cands:
                if c['package'] == pref:
                    return self.declared_arrays(c['package'], c['index']), None
        distinct = {json.dumps(self.declared_arrays(c['package'], c['index']), sort_keys=True) for c in cands}
        if len(distinct) > 1:
            return None, 'ScriptStruct %s declared differently in %d packages' % (struct_name, len(cands))
        return self.declared_arrays(cands[0]['package'], cands[0]['index']), None

    # -- property decoding ------------------------------------------------------------
    def prefixes_for(self, class_chain_paths):
        if 'Engine.ActorComponent' in class_chain_paths or 'Engine.Component' in class_chain_paths:
            return (8, 4, 26)
        if 'Engine.Actor' in class_chain_paths:
            return (26, 4, 8)
        return (4, 8, 26)

    def decode(self, pkg, idx, probe_prefixes=False):
        """Decode the tagged properties of one export. Never raises; returns a status record.

        status: decoded | partial (unsupported tags remain) | skipped (type definition) | failed.
        """
        x = self.export(pkg, idx)
        cache_file = self.cache / ('props-%s-%d-%s.json' % (pkg, idx, 'p' if probe_prefixes else 'n'))
        stat = self.stat_key(pkg)
        if self.use_cache and cache_file.exists():
            blob = json.loads(cache_file.read_text(encoding='utf-8'))
            if blob['stat'] == stat and blob['reader'] == self.reader_sha and blob['tool'] == DECODE_REV:
                return blob['record']
        record = self._decode(pkg, idx, x, probe_prefixes)
        cache_file.write_text(json.dumps({'stat': stat, 'reader': self.reader_sha, 'tool': DECODE_REV,
                                          'record': record}), encoding='utf-8')
        return record

    def _decode(self, pkg, idx, x, probe_prefixes):
        record = {'package': pkg, 'index': idx, 'path': x['path'], 'class': self.class_of(pkg, x), 'size': x['size'],
                  'status': 'failed', 'reason': None, 'prefix': None, 'alternate_prefixes': [],
                  'trailing_bytes': None, 'properties': None, 'unsupported': [], 'array_schema': {}}
        if record['class'] in TYPE_CLASSES or x['class'] in TYPE_CLASSES:
            record.update(status='skipped', reason='type/code definition, not decoded as data')
            return record
        try:
            chain = self.class_chain(record['class'])
        except Failure as e:
            record['reason'] = 'class chain: %s' % e
            return record
        chain_paths = [c[0] for c in chain]
        chain_pkgs = []
        for _, p, _ in chain:
            if p not in chain_pkgs:
                chain_pkgs.append(p)
        declared = {}
        try:
            for _, p, i in chain:
                for name, v in self.declared_arrays(p, i).items():
                    declared.setdefault(name, v)  # most derived declaration wins
        except Failure as e:
            record['reason'] = 'array declarations: %s' % e
            return record
        good = []
        last = None
        for prefix in self.prefixes_for(chain_paths):
            try:
                good.append((prefix, self._decode_at(pkg, idx, prefix, declared, chain_pkgs)))
                if not probe_prefixes:
                    break
            except Failure as e:
                last = '%s (prefix %d)' % (e, prefix)
        if not good:
            record['reason'] = last or 'decode failed'
            return record
        prefix, (res, used, array_failures) = good[0]
        record.update(status='decoded', prefix=prefix, alternate_prefixes=[p for p, _ in good[1:]],
                      trailing_bytes=res['trailing_bytes'], properties=res['properties'], array_schema=used)
        record['unsupported'] = sorted(set(unsupported_names(res['properties'])))
        if array_failures:
            record['array_failures'] = array_failures
        if record['unsupported']:
            record['status'] = 'partial'
        return record

    def _decode_at(self, pkg, idx, prefix, declared, chain_pkgs):
        schema = {}
        notes = {}
        for _ in range(12):
            args = ['--properties', idx, '--property-offset', prefix]
            tmp = self.cache / ('schema-%d-%d.tmp' % (os.getpid(), threading.get_ident()))
            if schema:
                tmp.write_text(''.join('%s=%s\n' % kv for kv in sorted(schema.items())), encoding='ascii')
                args += ['--array-schema', tmp]
            try:
                res = self.run(pkg, *args)
            finally:
                if tmp.exists():
                    tmp.unlink()
            pending = []
            collect_unsupported_arrays(res['properties'], None, pending)
            added = False
            for name, owner in pending:
                if name in schema or name in notes:
                    continue
                if owner is None:
                    spec = declared.get(name)
                    if spec is None:
                        spec = self.array_declared_elsewhere(name)
                    if spec is None and name in UNDECLARED_ARRAYS and not self.find_name(name, 'Core.ArrayProperty'):
                        spec = [UNDECLARED_ARRAYS[name], 'undeclared-fitted']
                else:
                    arrays, err = self.struct_arrays(owner, chain_pkgs)
                    spec = arrays.get(name) if arrays else None
                    if spec is None:
                        notes[name] = err or 'array %s not declared in struct %s' % (name, owner)
                        continue
                if spec is None:
                    notes[name] = 'no declaration for array ' + name
                elif spec[0] is None:
                    notes[name] = spec[1] or 'unsupported element type'
                else:
                    schema[name] = spec[0]
                    added = True
            if not added:
                return res, schema, {n: notes[n] for n, _ in pending if n in notes and n not in schema}
        raise Failure('array schema did not converge')


def is_map_package(name):
    return name.endswith(('_P', '_Dynamic', '_Combat', '_Audio', '_Side', '_Lighting', '_Mission', '_Art', '_SubMissions'))


def collect_unsupported_arrays(tags, owner, out):
    """(name, struct-type-or-None) of non-empty arrays the reader could not decode."""
    if not isinstance(tags, list):
        return
    for t in tags:
        if t.get('type') == 'ArrayProperty' and t.get('status') == 'unsupported' and t.get('element_count'):
            out.append((t['name'], owner))
        v = t.get('value')
        if t.get('type') == 'StructProperty' and isinstance(v, list):
            collect_unsupported_arrays(v, t.get('type_name'), out)
        elif t.get('type') == 'ArrayProperty' and isinstance(v, list) and t.get('element_type', '').startswith('StructProperty:'):
            for el in v:
                collect_unsupported_arrays(el, t['element_type'].split(':', 1)[1], out)


def unsupported_names(tags, prefix=''):
    names = []
    if not isinstance(tags, list):
        return names
    for t in tags:
        if t.get('status') == 'unsupported':
            names.append(prefix + t['name'] + ':' + t['type'])
        v = t.get('value')
        if t.get('type') == 'StructProperty' and isinstance(v, list):
            names += unsupported_names(v, prefix + t['name'] + '.')
        elif t.get('type') == 'ArrayProperty' and isinstance(v, list):
            for el in v:
                if isinstance(el, list):
                    names += unsupported_names(el, prefix + t['name'] + '[].')
    return names


def fields(tags):
    """Flatten tagged properties into {name: value}; static-array slots become name[i]."""
    out = {}
    if not isinstance(tags, list):
        return out
    for t in tags:
        if t.get('status') != 'decoded':
            continue
        name = t['name'] if not t.get('array_index') else '%s[%d]' % (t['name'], t['array_index'])
        out[name] = t['value']
    return out


def refs_in(value, found):
    """Collect object references ({'index','path'}) from a decoded value, any depth."""
    if isinstance(value, dict):
        if set(value) == {'index', 'path'}:
            if value['index']:
                found.append(value)
            return
        for v in value.values():
            refs_in(v, found)
    elif isinstance(value, list):
        for v in value:
            if isinstance(v, dict) and 'name' in v and 'type' in v and 'value' in v:
                refs_in(v['value'], found)
            else:
                refs_in(v, found)


# ======================================================================================
# Reference closure
# ======================================================================================
# Class paths treated as payload: referenced and recorded, never decoded here.
PAYLOAD_EXACT = {
    'Engine.StaticMesh', 'Engine.SkeletalMesh', 'Engine.AnimSet', 'Engine.AnimSequence', 'Engine.PhysicsAsset',
    'Engine.SoundCue', 'Engine.SoundNodeWave', 'Engine.AkEvent', 'Engine.AkBank', 'Engine.ParticleSystem',
    'Engine.Material', 'Engine.MaterialInstanceConstant', 'Engine.MaterialInstanceTimeVarying',
    'Engine.Texture2D', 'Engine.TextureCube', 'Engine.TextureRenderTarget2D', 'Engine.Texture2DComposite',
    'Engine.FaceFXAsset', 'Engine.FaceFXAnimSet', 'Engine.MorphTargetSet', 'Engine.LensFlare', 'Engine.Font',
    'Engine.PostProcessChain', 'Engine.SwfMovie', 'GFxUI.SwfMovie', 'Engine.RB_BodySetup',
    'Engine.AnimTree', 'WillowGame.WillowAnimTree', 'Engine.CameraAnim', 'Engine.ForceFeedbackWaveform',
}
PAYLOAD_PREFIXES = ('Engine.AnimNode', 'GearboxFramework.AnimNode', 'WillowGame.WillowAnimNode',
                    'Engine.MaterialExpression', 'Engine.ParticleModule', 'Engine.SoundNode', 'AkAudio.')
# Properties that point back up the object tree; following them would leave the mission's scope.
SKIP_PROPERTIES = {'ParentSequence', 'Outer', 'ObjectArchetype', 'ParentGroup'}
# Decoded and recorded, but their references are not expanded: shared catalogues (attribute system, NPC name
# tags, emotes, weapon name parts) that every mission points into and that would swamp the closure.
LEAF_CLASSES = {
    'Engine.AttributeDefinition', 'Engine.AttributeInitializationDefinition', 'WillowGame.WeaponNamePartDefinition',
    'WillowGame.WillowDialogNameTag', 'WillowGame.WillowDialogEventTag', 'WillowGame.WillowDialogEmoteDefinition',
    'GearboxFramework.GearboxDialogGroup_ParentPlaceholder',
}
# In-map (level) properties whose targets are part of the placed gameplay data.
INMAP_FOLLOW = {'SpawnPoints', 'NextNodes', 'Node', 'Aspect', 'Aspects', 'Originator', 'ObjValue',
                'StaticMeshComponent', 'CollisionComponent', 'Mesh', 'CylinderComponent'}


def is_payload(cls):
    return cls in PAYLOAD_EXACT or cls.startswith(PAYLOAD_PREFIXES)


def is_type_class(cls):
    return cls in TYPE_CLASSES


def walk_refs(tags, out, trail=()):
    """Collect (path-string, innermost property name, ref) for every object reference in tagged values."""
    if not isinstance(tags, list):
        return
    for t in tags:
        name = t['name'] + ('[%d]' % t['array_index'] if t.get('array_index') else '')
        _walk_value(t.get('value'), out, trail + (name,), t['name'])


def _is_tag_list(x):
    return isinstance(x, list) and x and isinstance(x[0], dict) and 'type' in x[0] and 'name' in x[0]


def _walk_value(v, out, trail, leaf):
    if isinstance(v, dict):
        if set(v) == {'index', 'path'}:
            if v['index']:
                out.append(('.'.join(trail), leaf, v))
            return
        for k, x in v.items():
            _walk_value(x, out, trail + (k,), k)
    elif isinstance(v, list):
        if _is_tag_list(v):
            walk_refs(v, out, trail)
            return
        for n, x in enumerate(v):
            if _is_tag_list(x):
                walk_refs(x, out, trail + ('[%d]' % n,))
            else:
                _walk_value(x, out, trail + ('[%d]' % n,), leaf)


def simplify(tags, refmap):
    """Tagged-property list -> compact dict. Object refs become 'pkg:idx path' strings."""
    out = {}
    if not isinstance(tags, list):
        return out
    for t in tags:
        name = t['name'] + ('[%d]' % t['array_index'] if t.get('array_index') else '')
        if t.get('status') == 'unsupported':
            out[name] = '<unsupported %s>' % t['type']
        else:
            out[name] = _simplify_value(t.get('value'), refmap)
    return out


def _simplify_value(v, refmap):
    if isinstance(v, dict):
        if set(v) == {'index', 'path'}:
            if not v['index']:
                return None
            return refmap.get(v['index'], 'unresolved:' + v['path'])
        return {k: _simplify_value(x, refmap) for k, x in v.items()}
    if isinstance(v, list):
        if _is_tag_list(v):
            return simplify(v, refmap)
        return [_simplify_value(x, refmap) for x in v]
    return v


class Closure:
    def __init__(self, env, prefer, kismet_prefix, max_nodes=6000, max_depth=40):
        self.env = env
        self.prefer = prefer
        self.kismet_prefix = kismet_prefix
        self.max_nodes = max_nodes
        self.max_depth = max_depth
        self.nodes = {}
        self.order = []
        self.edges = []
        self._edge_set = set()
        self.failures = []
        self.type_refs = {}
        self.duplicates = {}
        self.queue = []
        self._resolve_cache = {}
        self.truncated = False
        self.root = None
        self.root_path = ''

    @staticmethod
    def key(pkg, idx):
        return '%s:%d' % (pkg, idx)

    def fail(self, kind, where, reason):
        self.failures.append({'kind': kind, 'where': where, 'reason': reason})

    def add_seed(self, pkg, idx, seed, via=None):
        return self._add(pkg, idx, seed, 0, via, None)

    def _add(self, pkg, idx, seed, depth, via, parent):
        k = self.key(pkg, idx)
        if k in self.nodes:
            if parent:
                self._edge(parent, k, via)
            return k
        if len(self.nodes) >= self.max_nodes:
            if not self.truncated:
                self.fail('truncated', k, 'node cap %d reached; closure incomplete' % self.max_nodes)
            self.truncated = True
            return None
        x = self.env.export(pkg, idx)
        cls = self.env.class_of(pkg, x)
        kind = 'payload' if is_payload(cls) else ('type' if (is_type_class(cls) or is_type_class(x['class'])) else 'data')
        if x['path'].split('.')[-1].startswith('Default__'):
            kind = 'class_default'
        node = {'key': k, 'package': pkg, 'index': idx, 'path': x['path'], 'class': cls, 'seed': seed, 'depth': depth,
                'reached_via': via, 'reached_from': parent, 'kind': kind}
        self.nodes[k] = node
        self.order.append(k)
        if parent:
            self._edge(parent, k, via)
        if kind == 'data':
            self.queue.append(k)
        else:
            node['status'] = 'not-decoded'
            node['reason'] = {'payload': 'payload class: recorded for the gap list, not decoded here',
                              'type': 'type/code definition', 'class_default': 'class default object'}[kind]
        return k

    def _edge(self, a, b, via):
        e = (a, b, via)
        if e not in self._edge_set:
            self._edge_set.add(e)
            self.edges.append({'from': a, 'to': b, 'via': via})

    def resolve(self, pkg, ref):
        """Resolve one object reference value from ``pkg``. Cached; duplicates recorded."""
        ck = (pkg, ref['index'])
        if ck in self._resolve_cache:
            return self._resolve_cache[ck]
        env = self.env
        out = {'status': 'resolved'}
        try:
            info = env.ref_info(pkg, ref)
            out['path'] = info['path']
            out['class'] = info['class']
            if info['kind'] == 'export':
                out.update(package=pkg, index=info['index'])
            elif is_type_class(info['class']):
                out['status'] = 'type'
            else:
                chosen, cands, rule = env.resolve_import(pkg, info, self.prefer)
                out['rule'] = rule
                if not chosen:
                    out['status'] = 'unresolved'
                    out['reason'] = 'import target not found in any package: ' + info['path']
                    self.fail('unresolved-reference', '%s:%d %s' % (pkg, ref['index'], info['path']), out['reason'])
                else:
                    out.update(package=chosen['package'], index=chosen['index'])
                    if len(cands) > 1:
                        sizes = {env.export(c['package'], c['index'])['size'] for c in cands}
                        self.duplicates[info['path']] = {
                            'path': info['path'], 'rule': rule, 'chosen': self.key(chosen['package'], chosen['index']),
                            'candidates': [self.key(c['package'], c['index']) for c in cands],
                            'same_size': len(sizes) == 1, 'first_seen_in': pkg}
        except Failure as e:
            out = {'status': 'unresolved', 'path': ref.get('path'), 'reason': str(e)}
            self.fail('unresolved-reference', '%s:%d' % (pkg, ref['index']), str(e))
        self._resolve_cache[ck] = out
        return out

    def den_binding(self, node, rec):
        """Which mission a placed PopulationOpportunityDen serves, from its MissionPopulationAspect.

        Returns (True|False|None, explanation). None = no mission aspect found, so the den is followed.
        """
        env = self.env
        f = fields(rec['properties'])
        aspect = f.get('Aspect')
        if not isinstance(aspect, dict) or aspect['index'] <= 0:
            return None, 'no Aspect property'
        a = env.decode(node['package'], aspect['index'])
        if a['status'] == 'failed':
            return None, 'aspect decode failed: %s' % a['reason']
        af = fields(a['properties'])
        paths = [v['path'] for k, v in af.items() if isinstance(v, dict) and 'path' in v and 'Objective' in k]
        prefix = self.root_path + '.'
        if not paths:
            return None, 'aspect has no objective reference'
        mine = [p for p in paths if p.startswith(prefix)]
        return bool(mine), 'aspect objectives: %s' % paths

    def in_scope(self, node, leaf, target_pkg, target_path):
        """Scope rule for level objects; data objects outside levels are always in scope."""
        if not target_path.startswith('TheWorld.'):
            return True
        if target_path.startswith('TheWorld.PersistentLevel.Main_Sequence.'):
            return target_path == self.kismet_prefix or target_path.startswith(self.kismet_prefix + '.')
        if node['path'].startswith('TheWorld.PersistentLevel.Main_Sequence'):
            return True  # a Kismet op pointing at a placed actor/point
        if target_path.startswith(node['path'] + '.'):
            return True  # sub-object of this actor
        return leaf in INMAP_FOLLOW

    def run(self, progress=None, workers=8):
        env = self.env
        i = 0
        done = set()
        pool = ThreadPoolExecutor(max_workers=workers)
        futures = {}
        while i < len(self.queue):
            # keep a window of decodes in flight; results are consumed in queue order (deterministic)
            for j in range(i, min(len(self.queue), i + workers * 4)):
                kj = self.queue[j]
                if kj not in futures:
                    n = self.nodes[kj]
                    futures[kj] = pool.submit(env.decode, n['package'], n['index'])
            k = self.queue[i]
            i += 1
            node = self.nodes[k]
            rec = futures.pop(k).result()
            node.update(status=rec['status'], reason=rec['reason'], prefix=rec['prefix'],
                        trailing_bytes=rec['trailing_bytes'], unsupported=rec['unsupported'],
                        array_schema=rec['array_schema'])
            if rec.get('array_failures'):
                node['array_failures'] = rec['array_failures']
                for name, why in rec['array_failures'].items():
                    self.fail('array-undecoded', k + ' ' + name, why)
            if rec['status'] == 'failed':
                self.fail('decode-failed', k + ' ' + node['path'], rec['reason'])
                continue
            for u in rec['unsupported']:
                self.fail('unsupported-property', k + ' ' + node['path'], u)
            refs = []
            walk_refs(rec['properties'], refs)
            refmap = {}
            expand = True
            if node['class'] in LEAF_CLASSES:
                expand = False
                node['expansion'] = 'leaf class: references recorded, not followed'
            elif node['class'] == 'WillowGame.PopulationOpportunityDen':
                belongs, why = self.den_binding(node, rec)
                node['mission_binding'] = why
                if belongs is False:
                    expand = False
                    node['expansion'] = 'bound to another mission: references recorded, not followed'
            for trail, leaf, ref in refs:
                r = self.resolve(node['package'], ref)
                if r['status'] == 'type':
                    self.type_refs[r['path']] = self.type_refs.get(r['path'], 0) + 1
                    refmap[ref['index']] = 'type:' + r['path']
                    continue
                if r['status'] != 'resolved':
                    refmap[ref['index']] = 'unresolved:' + str(r.get('path'))
                    continue
                tk = self.key(r['package'], r['index'])
                refmap[ref['index']] = tk + ' ' + r['path']
                if not expand or leaf in SKIP_PROPERTIES:
                    node.setdefault('not_followed', []).append(tk + ' ' + r['path'])
                    continue
                if node['depth'] >= self.max_depth:
                    self.fail('depth', k, 'depth cap reached')
                    continue
                tx = env.export(r['package'], r['index'])
                target_path = tx['path']
                if env.class_of(r['package'], tx) == 'WillowGame.MissionDefinition' and tk != self.root:
                    node.setdefault('out_of_scope', []).append(tk + ' ' + r['path'] + ' (other mission: recorded, not expanded)')
                    continue
                if not self.in_scope(node, leaf, r['package'], target_path):
                    node.setdefault('out_of_scope', []).append(tk + ' ' + r['path'])
                    continue
                self._add(r['package'], r['index'], node['seed'], node['depth'] + 1, trail, k)
            node['values'] = simplify(rec['properties'], refmap)
            if progress and i % 100 == 0:
                progress(i, len(self.queue), len(self.nodes))
        pool.shutdown()


# ======================================================================================
# Class reports: which stock classes carry script and which are native-only
# ======================================================================================
def disasm_text(env, pkg, idx):
    """('script', text) | ('native', None): ``function has no script`` is how the reader reports bodyless functions."""
    env.calls += 1
    proc = subprocess.run([str(env.reader), str(env.upk(pkg)), '--disasm', str(idx)],
                          capture_output=True, text=True, encoding='utf-8', timeout=120)
    if proc.returncode == 0:
        return 'script', proc.stdout
    msg = (proc.stderr or '').strip()
    if 'no script' in msg:
        return 'native', None
    return 'error', msg


CALL_RE = None


def called_names(text):
    import re
    names = set(re.findall(r'(?:VirtualFunction|FinalFunction|GlobalFunction|DelegateFunction)\(([A-Za-z_0-9]+)', text))
    natives = set(re.findall(r'native_(\d+)', text))
    return sorted(names), sorted(natives, key=int)


def class_report(env, class_path, children_cache):
    """Functions declared by each class of the chain, split into script bodies and bodyless (native) ones."""
    chain = env.class_chain(class_path)
    rep = {'class': class_path, 'chain': [c[0] for c in chain], 'functions': []}
    for cp, pkg, idx in chain:
        if pkg == 'Core':
            continue
        kids = children_cache.get(pkg)
        if kids is None:
            kids = {}
            for x in env.exports(pkg):
                if x['class'] == 'Core.Function' and x['outer_index']:
                    kids.setdefault(x['outer_index'], []).append(x)
            children_cache[pkg] = kids
        for f in kids.get(idx, []):
            kind, text = disasm_text(env, pkg, f['index'])
            entry = {'declared_in': cp, 'name': f['name'], 'kind': kind, 'index': '%s:%d' % (pkg, f['index'])}
            if kind == 'script':
                lines = text.splitlines()
                entry['statements'] = max(0, len(lines) - 1)
                entry['empty_body'] = len(lines) <= 4  # header + Return + EndOfScript (+1)
                names, natives = called_names(text)
                entry['calls'] = names
                entry['native_ops'] = natives
            elif kind == 'error':
                entry['error'] = text
            rep['functions'].append(entry)
    return rep


# ======================================================================================
# Kismet graph
# ======================================================================================
KISMET_LAYOUT = {'ObjPosX', 'ObjPosY', 'DrawWidth', 'DrawHeight', 'MaxWidth', 'ObjInstanceVersion', 'ParentSequence',
                 'OutputLinks', 'InputLinks', 'VariableLinks', 'ObjColor', 'ObjName'}
KIND_ORDER = (('Engine.SequenceEvent', 'event'), ('Engine.SequenceAction', 'action'),
              ('Engine.SequenceCondition', 'condition'), ('Engine.SequenceVariable', 'variable'),
              ('Engine.Sequence', 'sequence'), ('Engine.InterpData', 'matinee-data'),
              ('Engine.InterpGroup', 'matinee-data'), ('Engine.InterpTrack', 'matinee-data'),
              ('Engine.BehaviorBase', 'behavior'))
REMOTE_LISTENERS = ('Engine.SeqEvent_RemoteEvent', 'WillowGame.WillowSeqEvent_MissionRemoteEvent',
                    'WillowGame.WillowSeqEvent_CustomEvent')
REMOTE_ACTIVATORS = ('Engine.SeqAct_ActivateRemoteEvent', 'WillowGame.WillowSeqAct_MissionCustomEvent',
                     'WillowGame.WillowSeqAct_RunCustomEvent', 'WillowGame.Behavior_MissionRemoteEvent',
                     'WillowGame.Behavior_RemoteCustomEvent')


def op_kind(env, class_path):
    chain = [c[0] for c in env.class_chain(class_path)]
    for base, kind in KIND_ORDER:
        if base in chain:
            return kind
    return 'other'


def deep_resolve(closure, pkg, v):
    """Copy of a decoded value with every object reference replaced by a resolved 'pkg:idx path' string."""
    if isinstance(v, dict):
        if set(v) == {'index', 'path'}:
            if not v['index']:
                return None
            r = closure.resolve(pkg, v)
            if r['status'] == 'type':
                return 'type:' + r['path']
            if r['status'] != 'resolved':
                return 'unresolved:' + str(r.get('path'))
            return '%s:%d %s' % (r['package'], r['index'], r['path'])
        return {k: deep_resolve(closure, pkg, x) for k, x in v.items()}
    if isinstance(v, list):
        return [deep_resolve(closure, pkg, x) for x in v]
    return v


def class_default_fields(env, class_path, closure):
    """Tagged defaults of the most-derived Default__ object that sets each property.

    References are resolved in the package that owns the default object, never in the caller's package.
    """
    merged = {}
    for cp, pkg, idx in env.class_chain(class_path):
        name = cp.split('.', 1)[1]
        for c in env.find_path('Default__' + name):
            if c['package'] == pkg:
                rec = env.decode(pkg, c['index'])
                if rec['status'] in ('decoded', 'partial'):
                    for k, v in fields(deep_resolve(closure, pkg, rec['properties'])).items():
                        merged.setdefault(k, v)
                break
    return merged


def link_descs(links):
    out = []
    for el in links or []:
        out.append(fields(el).get('LinkDesc') if isinstance(el, list) else None)
    return out


def event_name(props):
    for key in ('EventName', 'CustomEventName', 'EventTag'):
        v = props.get(key)
        if isinstance(v, str) and v != 'None':
            return v
    return None


class Kismet:
    """The Kismet sub-sequence as a node/edge graph, built from tagged properties only."""

    def __init__(self, env, closure, pkg, seq_idx):
        self.env, self.closure, self.pkg, self.seq_idx = env, closure, pkg, seq_idx
        self.seq_path = env.export(pkg, seq_idx)['path']
        self.nodes = {}
        self.edges = []
        self.var_edges = []
        self.failures = []
        self._defaults = {}

    def nid(self, idx):
        return '%s:%d' % (self.pkg, idx)

    def defaults(self, class_path):
        if class_path not in self._defaults:
            self._defaults[class_path] = class_default_fields(self.env, class_path, self.closure)
        return self._defaults[class_path]

    def local_ref(self, ref):
        if not ref or not ref.get('index'):
            return None
        if ref['index'] > 0:
            return ref['index']
        return None

    def value(self, v):
        """JSON-able form of a property value: refs resolved to ids, structs flattened."""
        if isinstance(v, dict):
            if set(v) == {'index', 'path'}:
                if not v['index']:
                    return None
                r = self.closure.resolve(self.pkg, v)
                if r['status'] == 'type':
                    return 'type:' + r['path']
                if r['status'] != 'resolved':
                    return 'unresolved:' + str(r.get('path'))
                return '%s:%d %s' % (r['package'], r['index'], r['path'])
            return {k: self.value(x) for k, x in v.items()}
        if isinstance(v, list):
            if _is_tag_list(v):
                return {k: self.value(x) for k, x in fields(v).items()}
            return [self.value(x) for x in v]
        return v

    def load(self, idx):
        nid = self.nid(idx)
        if nid in self.nodes:
            return self.nodes[nid]
        env = self.env
        x = env.export(self.pkg, idx)
        cls = env.class_of(self.pkg, x)
        rec = env.decode(self.pkg, idx)
        node = {'id': nid, 'package': self.pkg, 'index': idx, 'name': x['name'], 'path': x['path'], 'class': cls,
                'kind': op_kind(env, cls), 'status': rec['status'], 'props': {}, 'inputs': [], 'outputs': [],
                'var_links': []}
        self.nodes[nid] = node
        if rec['status'] == 'failed':
            node['reason'] = rec['reason']
            self.failures.append({'node': nid, 'reason': rec['reason']})
            return node
        f = fields(rec['properties'])
        for k, v in f.items():
            if k not in KISMET_LAYOUT:
                node['props'][k] = self.value(v)
        if 'ObjName' in f:
            node['label'] = f['ObjName']
        dflt = self.defaults(cls) if node['kind'] in ('event', 'action', 'condition', 'sequence') else {}
        ins = f.get('InputLinks', dflt.get('InputLinks'))
        outs = f.get('OutputLinks', dflt.get('OutputLinks'))
        node['inputs'] = link_descs(ins)
        node['inputs_source'] = 'instance' if 'InputLinks' in f else ('class default' if ins is not None else 'none')
        for n, el in enumerate(outs or []):
            o = fields(el)
            links = []
            for lk in o.get('Links', []):
                lf = fields(lk)
                t = self.local_ref(lf.get('LinkedOp'))
                links.append({'to': self.nid(t) if t else None, 'to_index': t, 'input': lf.get('InputLinkIdx')})
            node['outputs'].append({'index': n, 'desc': o.get('LinkDesc'), 'delay': o.get('ActivateDelay', 0),
                                    'disabled': bool(o.get('bDisabled')), 'links': links})
        for el in f.get('VariableLinks', dflt.get('VariableLinks')) or []:
            vf = fields(el)
            vars_ = [self.nid(i) for i in (self.local_ref(r) for r in vf.get('LinkedVariables', [])) if i]
            node['var_links'].append({'desc': vf.get('LinkDesc'), 'property': vf.get('PropertyName'),
                                      'expected_type': self.value(vf.get('ExpectedType')),
                                      'writeable': bool(vf.get('bWriteable')), 'vars': vars_})
        node['_f'] = f
        return node

    def build(self):
        env = self.env
        seq = self.load(self.seq_idx)
        queue = [self.local_ref(r) for r in seq['_f'].get('SequenceObjects', [])]
        seen = set()
        while queue:
            idx = queue.pop(0)
            if idx is None or idx in seen:
                continue
            seen.add(idx)
            node = self.load(idx)
            if node['status'] == 'failed':
                continue
            # follow ops, variables and attached sub-objects that live inside this sequence
            refs = []
            walk_refs(env.decode(self.pkg, idx)['properties'], refs)
            for trail, leaf, ref in refs:
                t = self.local_ref(ref)
                if not t or leaf in SKIP_PROPERTIES:
                    continue
                path = env.export(self.pkg, t)['path']
                if path.startswith(self.seq_path + '.') and t not in seen:
                    queue.append(t)
        # edges
        for nid, node in self.nodes.items():
            for o in node['outputs']:
                for lk in o['links']:
                    dest = self.nodes.get(lk['to']) if lk['to'] else None
                    din = None
                    if dest and lk['input'] is not None and lk['input'] < len(dest['inputs']):
                        din = dest['inputs'][lk['input']]
                    self.edges.append({'from': nid, 'from_output': o['index'], 'from_desc': o['desc'], 'to': lk['to'],
                                       'to_input': lk['input'], 'to_desc': din, 'delay': o['delay'],
                                       'disabled': o['disabled']})
            for vl in node['var_links']:
                for v in vl['vars']:
                    self.var_edges.append({'node': nid, 'link_desc': vl['desc'], 'property': vl['property'],
                                           'variable': v, 'writeable': vl['writeable']})
        for node in self.nodes.values():
            node.pop('_f', None)
        return self

    def flow_paths(self, sources, targets, limit=3):
        """Shortest flow paths (node-id lists) from each source to each target; output links only."""
        adj = {}
        for e in self.edges:
            if e['to'] and not e['disabled']:
                adj.setdefault(e['from'], []).append(e['to'])
        found = {}
        for s in sources:
            prev = {s: None}
            order = [s]
            for cur in order:
                for nxt in adj.get(cur, []):
                    if nxt not in prev:
                        prev[nxt] = cur
                        order.append(nxt)
            for t in targets:
                if t in prev:
                    path = []
                    cur = t
                    while cur:
                        path.append(cur)
                        cur = prev[cur]
                    found.setdefault(t, []).append(list(reversed(path)))
        return found


def remote_event_index(env, closure, packages):
    """Every remote-event listener/activator in the given level packages (and mission behaviors), by name."""
    entries = []
    classes = set(REMOTE_LISTENERS) | set(REMOTE_ACTIVATORS)
    for pkg in packages:
        for x in env.exports(pkg['name'] if isinstance(pkg, dict) else pkg):
            p = pkg['name'] if isinstance(pkg, dict) else pkg
            cls = env.class_of(p, x)
            if cls in classes:
                rec = env.decode(p, x['index'])
                if rec['status'] == 'failed':
                    closure.fail('remote-event-decode', '%s:%d' % (p, x['index']), rec['reason'])
                    continue
                f = fields(rec['properties'])
                assoc = f.get('AssociatedMissionDefinition')
                entries.append({'id': '%s:%d' % (p, x['index']), 'class': cls, 'path': x['path'],
                                'role': 'listener' if cls in REMOTE_LISTENERS else 'activator',
                                'event_name': event_name(f), 'mission': assoc['path'] if isinstance(assoc, dict) else None,
                                'sequence': x['path'].rsplit('.', 1)[0] if '.' in x['path'] else None})
    for k, n in closure.nodes.items():
        if n['class'] in classes and n.get('status') in ('decoded', 'partial'):
            v = n.get('values', {})
            if any(e['id'] == k for e in entries):
                continue
            entries.append({'id': k, 'class': n['class'], 'path': n['path'],
                            'role': 'listener' if n['class'] in REMOTE_LISTENERS else 'activator',
                            'event_name': event_name(v), 'mission': None, 'sequence': None})
    return entries


# ======================================================================================
# Mission BehaviorProvider (behavior-sequence graph)
# ======================================================================================
def unpack_range(v):
    """ArrayIndexAndLength packing: start = value >> 16, length = value & 0xFFFF (checked by the contiguity oracle)."""
    return (v >> 16) & 0xFFFF, v & 0xFFFF


def behavior_provider_graphs(env, closure, node_key):
    """Decode the BehaviorSequences of one BehaviorProviderDefinition into explicit events/behaviors/links.

    The packed-integer layouts are NOT documented. They are accepted only when two structural oracles hold:
    the per-behavior/per-event (start, length) ranges tile the consolidated link array exactly, and every
    linked behavior index is in range. Which half of LinkIdAndLinkedBehavior is the behavior index is
    UNVERIFIED; the low 24 bits are assumed because the alternative produces self-loops.
    """
    n = closure.nodes[node_key]
    pkg = n['package']
    rec = env.decode(pkg, n['index'])
    f = fields(rec['properties'])
    out = []
    for seq_tags in f.get('BehaviorSequences', []):
        s = fields(seq_tags)
        behaviors = []
        for el in s.get('BehaviorData2', []):
            b = fields(el)
            ref = b.get('Behavior')
            r = closure.resolve(pkg, ref) if isinstance(ref, dict) and ref.get('index') else None
            behaviors.append({'ref': ('%s:%d' % (r['package'], r['index'])) if r and r['status'] == 'resolved' else None,
                              'class': r.get('class') if r else None, 'path': ref['path'] if r else None,
                              'outputs_range': unpack_range(struct_value(el, 'OutputLinks').get('ArrayIndexAndLength', 0)),
                              'variables_range': unpack_range(struct_value(el, 'LinkedVariables').get('ArrayIndexAndLength', 0))})
        events = []
        for el in s.get('EventData2', []):
            ud = struct_value(el, 'UserData')
            events.append({'name': ud.get('EventName'), 'enabled': ud.get('bEnabled'),
                           'outputs_range': unpack_range(struct_value(el, 'OutputLinks').get('ArrayIndexAndLength', 0))})
        consolidated = []
        for el in s.get('ConsolidatedOutputLinkData', []):
            c = fields(el)
            raw = c.get('LinkIdAndLinkedBehavior', 0)
            consolidated.append({'raw': raw, 'behavior_index': raw & 0xFFFFFF, 'link_field': (raw >> 24) & 0xFF,
                                 'delay': c.get('ActivateDelay', 0)})
        ranges = sorted([(b['outputs_range'][0], b['outputs_range'][1]) for b in behaviors] +
                        [(e['outputs_range'][0], e['outputs_range'][1]) for e in events])
        cursor = 0
        tiles = True
        for start, length in ranges:
            if length == 0:
                continue
            if start != cursor:
                tiles = False
            cursor = start + length
        tiles = tiles and cursor == len(consolidated)
        in_range = all(c['behavior_index'] < len(behaviors) for c in consolidated)
        alt_self_loops = 0
        self_loops = 0
        for bi, b in enumerate(behaviors):
            st, ln = b['outputs_range']
            for c in consolidated[st:st + ln]:
                if c['behavior_index'] == bi:
                    self_loops += 1
                if (c['raw'] >> 24) & 0xFF == bi:
                    alt_self_loops += 1
        for e in events:
            st, ln = e['outputs_range']
            e['links'] = consolidated[st:st + ln]
        for bi, b in enumerate(behaviors):
            st, ln = b['outputs_range']
            b['links'] = consolidated[st:st + ln]
        out.append({'sequence_name': s.get('BehaviorSequenceName'), 'enabled_on_spawn': s.get('bEnabledOnSpawn'),
                    'events': events, 'behaviors': behaviors,
                    'variables': [{'name': fields(v).get('Name'), 'type': fields(v).get('Type')}
                                  for v in s.get('VariableData', [])],
                    'oracle': {'ranges_tile_consolidated_array': tiles, 'link_behavior_index_in_range': in_range,
                               'self_loops_if_low24_is_behavior': self_loops,
                               'self_loops_if_high8_is_behavior': alt_self_loops,
                               'consolidated_links': len(consolidated)}})
    return out


def struct_value(tag_list, name):
    """Field ``name`` of a struct element (itself a tag list) as {field: value}."""
    for t in tag_list:
        if t['name'] == name and isinstance(t.get('value'), list):
            return fields(t['value'])
    return {}



# ======================================================================================
# Placements, identities, payload gap
# ======================================================================================
def archetype_path(env, pkg, x):
    a = x['archetype_index']
    if not a:
        return None
    if a > 0:
        return env.export(pkg, a)['path']
    imp = env.imports(pkg).get(a)
    return imp['path'] if imp else None


def placements_by_archetype(env, packages, prefix):
    """Level exports whose archetype path starts with ``prefix`` (the established way a placed NPC pawn is typed)."""
    out = []
    for pkg in packages:
        for x in env.exports(pkg):
            if x['archetype_index'] and x['path'].startswith('TheWorld.PersistentLevel.') and x['path'].count('.') == 2:
                a = archetype_path(env, pkg, x)
                if a and a.lower().startswith(prefix.lower()):
                    out.append({'package': pkg, 'index': x['index'], 'path': x['path'],
                                'class': env.class_of(pkg, x), 'archetype': a})
    return out


def reachable(closure, start_keys, stop_kinds=()):
    adj = {}
    for e in closure.edges:
        adj.setdefault(e['from'], []).append(e['to'])
    seen = list(start_keys)
    seen_set = set(seen)
    for k in seen:
        for t in adj.get(k, []):
            if t not in seen_set:
                seen_set.add(t)
                seen.append(t)
    return seen


def summarize_reachable(closure, keys):
    by_class = {}
    payload = []
    for k in keys:
        n = closure.nodes[k]
        by_class[n['class']] = by_class.get(n['class'], 0) + 1
        if n['kind'] == 'payload':
            payload.append({'key': k, 'class': n['class'], 'path': n['path']})
    return {'nodes': len(keys), 'by_class': dict(sorted(by_class.items())), 'payload': sorted(payload, key=lambda p: p['key'])}


UMODEL_SUPPORT = {
    'Engine.SkeletalMesh': ('yes', 'exported with -md5 (Skel_SirenBody) and -gltf (Skel_BugMorph) in earlier project runs'),
    'Engine.AnimSet': ('yes', '-md5 writes one .md5anim per sequence (Base_Siren AnimSet run)'),
    'Engine.AnimSequence': ('via AnimSet', 'exported as part of its AnimSet'),
    'Engine.StaticMesh': ('yes', '-gltf (Ash_Road01) in the 2026-09-16 smoke check'),
    'Engine.Texture2D': ('yes', '-png/-dds; streamed mips need the .tfc beside the package (smoke check)'),
    'Engine.MaterialInstanceConstant': ('heuristic only', 'UModel writes a property dump / .mat text; not a UE5 material graph'),
    'Engine.Material': ('heuristic only', 'UModel material output is heuristic (CLAUDE.md)'),
    'Engine.AkEvent': ('no', 'Wwise event identity only; UModel is not used for Wwise banks (UNVERIFIED: not tested here)'),
    'Engine.SoundNodeWave': ('UNVERIFIED', 'UModel -sounds exists for UE3 waves; not benchmarked for BL2 here'),
    'Engine.SoundCue': ('UNVERIFIED', 'cue graph is not a UModel export type; waves would be'),
    'Engine.ParticleSystem': ('no', 'UModel does not export particle systems (UNVERIFIED for build 1590)'),
    'Engine.PhysicsAsset': ('UNVERIFIED', 'not covered by any project run'),
    'WillowGame.WillowAnimTree': ('no', 'AnimTree is a node graph (data); needs our own decode, not a UModel export'),
    'Engine.AnimTree': ('no', 'AnimTree is a node graph (data); needs our own decode'),
    'Engine.FaceFXAsset': ('UNVERIFIED', 'not covered by any project run'),
}


def payload_gap(closure, subjects):
    """Payload nodes grouped by class with UModel support statements and which subject needs them."""
    rows = {}
    for k, n in closure.nodes.items():
        if n['kind'] != 'payload':
            continue
        cls = n['class']
        norm = cls if cls in UMODEL_SUPPORT else ('Engine.AnimTree' if 'AnimNode' in cls else cls)
        support = UMODEL_SUPPORT.get(norm, ('UNVERIFIED', 'no project evidence for this class'))
        rows.setdefault(cls, {'class': cls, 'umodel': support[0], 'basis': support[1], 'count': 0, 'objects': []})
        rows[cls]['count'] += 1
        rows[cls]['objects'].append({'key': k, 'path': n['path'], 'package': n['package'],
                                     'needed_by': sorted(s for s, keys in subjects.items() if k in keys)})
    for r in rows.values():
        r['objects'].sort(key=lambda o: o['key'])
    return sorted(rows.values(), key=lambda r: r['class'])


# ======================================================================================
# Self checks
# ======================================================================================
def spot_check(env, closure, count=8):
    """Re-decode evenly spaced decoded nodes with the reader directly (bypassing caches) and compare."""
    keys = [k for k in closure.order if closure.nodes[k].get('status') in ('decoded', 'partial')]
    if not keys:
        return []
    step = max(1, len(keys) // count)
    picks = keys[::step][:count]
    results = []
    for k in picks:
        n = closure.nodes[k]
        args = ['--properties', n['index'], '--property-offset', n['prefix']]
        tmp = None
        if n['array_schema']:
            tmp = env.cache / ('spot-%d.schema' % os.getpid())
            tmp.write_text(''.join('%s=%s\n' % kv for kv in sorted(n['array_schema'].items())), encoding='ascii')
            args += ['--array-schema', tmp]
        try:
            fresh = env.run(n['package'], *args)
        except Failure as e:
            results.append({'key': k, 'match': False, 'reason': str(e)})
            continue
        finally:
            if tmp and tmp.exists():
                tmp.unlink()
        cached = env.decode(n['package'], n['index'])
        results.append({'key': k, 'path': n['path'], 'class': n['class'], 'prefix': n['prefix'],
                        'match': fresh['properties'] == cached['properties'],
                        'consumed_bytes': fresh['consumed_bytes'], 'trailing_bytes': fresh['trailing_bytes'],
                        'export_size': n.get('size')})
    return results


def count_by(items, key):
    out = {}
    for i in items:
        v = key(i)
        out[v] = out.get(v, 0) + 1
    return dict(sorted(out.items()))


# ======================================================================================
# Main
# ======================================================================================
def git_commit():
    try:
        return subprocess.run(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], capture_output=True, text=True).stdout.strip()
    except OSError:
        return None


def find_mission(env, mission_path):
    hits = [c for c in env.find_path(mission_path) if c['class'] in ('WillowGame.MissionDefinition',)]
    if not hits:
        raise SystemExit('Mission not found as a MissionDefinition export: ' + mission_path)
    startup = [h for h in hits if h['package'] == 'Startup']
    chosen = startup[0] if startup else hits[0]
    return chosen, hits


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--game', default=os.environ.get('OPENWILLOW_BL2'), help='Borderlands 2 install directory')
    ap.add_argument('--mission', required=True, help='MissionDefinition path, e.g. GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire')
    ap.add_argument('--reader', default=str(ROOT / 'build' / 'Release' / 'ow-package.exe'))
    ap.add_argument('--out', default=None, help='output directory under local/ (default local/missions/<slug>)')
    ap.add_argument('--maps', default='Sanctuary_Dynamic,Sanctuary_P', help='level packages that hold placed actors and Kismet')
    ap.add_argument('--prefer', default='Sanctuary_Dynamic,Sanctuary_P,Startup',
                    help='packages preferred when an import has several candidate copies')
    ap.add_argument('--max-nodes', type=int, default=6000)
    ap.add_argument('--workers', type=int, default=8)
    ap.add_argument('--no-cache', action='store_true', help='ignore cached property decodes')
    ap.add_argument('--skip-classes', action='store_true', help='skip the (slower) script/native class reports')
    args = ap.parse_args()
    if not args.game:
        sys.exit('Set --game or OPENWILLOW_BL2')
    t0 = time.time()
    slug = 'rpg_fire' if 'RockPaperGenocide_Fire' in args.mission else args.mission.replace('.', '_').lower()
    out = Path(args.out) if args.out else ROOT / 'local' / 'missions' / slug
    out = out.resolve()
    if not out.is_relative_to(ROOT / 'local'):
        sys.exit('Output must stay under repository local/')
    out.mkdir(parents=True, exist_ok=True)
    env = Env(args.game, args.reader, out.parent / '_cache', use_cache=not args.no_cache)
    env.open_index()
    prefer = [p for p in args.prefer.split(',') if p]
    maps = [p for p in args.maps.split(',') if p]
    log = lambda *a: print('[%4ds]' % (time.time() - t0), *a, flush=True)

    provenance = {'tool': TOOL, 'tool_version': TOOL_VERSION, 'decode_rev': DECODE_REV,
                  'reader': str(Path(args.reader).name), 'reader_sha256': env.reader_sha,
                  'repo_commit': git_commit(), 'generated_utc': utc_now(), 'game_dir_name': Path(args.game).name,
                  'note': 'game-derived data; ignored local/ only; AI-assisted tool'}

    mission, mission_hits = find_mission(env, args.mission)
    mkey = '%s:%d' % (mission['package'], mission['index'])
    log('mission', mkey, 'copies', [h['package'] for h in mission_hits])
    mdec = env.decode(mission['package'], mission['index'])
    mf = fields(mdec['properties'])

    # -- Kismet sequence(s) that listen for this mission's remote events --------------------
    listeners = []
    for pkg in maps:
        for x in env.exports(pkg):
            if env.class_of(pkg, x) == 'WillowGame.WillowSeqEvent_MissionRemoteEvent':
                rec = env.decode(pkg, x['index'])
                if rec['status'] == 'failed':
                    continue
                a = fields(rec['properties']).get('AssociatedMissionDefinition')
                if isinstance(a, dict) and a['path'] == args.mission:
                    listeners.append((pkg, x['index'], x['path'].rsplit('.', 1)[0], x['outer_index']))
    seqs = sorted({(p, o, path) for p, i, path, o in listeners})
    log('kismet sequences with listeners for this mission:', [(p, path) for p, o, path in seqs])
    kismet_seqs = []
    for pkg, outer, path in seqs:
        kismet_seqs.append((pkg, outer, path))
    if not kismet_seqs:
        log('no Kismet remote-event listeners found for this mission in', maps)

    closure = Closure(env, prefer, kismet_seqs[0][2] if kismet_seqs else 'TheWorld.NoSequence',
                      max_nodes=args.max_nodes)
    closure.root = mkey
    closure.root_path = args.mission
    closure.add_seed(mission['package'], mission['index'], 'mission')
    for pkg, outer, path in kismet_seqs:
        closure.add_seed(pkg, outer, 'kismet', via='remote-event listeners')

    # -- NPC giver: placed pawn identified by archetype ---------------------------------------
    giver = mf.get('MissionGiver')
    giver_report = {'name': giver, 'archetype_candidates': [], 'placements': []}
    if giver:
        cands = [c for c in env.open_index().execute(
            "SELECT pkg,idx,path FROM ex WHERE lpath LIKE ? AND class='WillowGame.WillowAIPawn' ORDER BY pkg,idx",
            ('gd_' + giver.lower() + '.character.pawn_%',)).fetchall()]
        giver_report['archetype_candidates'] = ['%s:%d %s' % c for c in cands]
        for path in sorted({c[2] for c in cands}):
            giver_report['placements'] += placements_by_archetype(env, maps + ['Sanctuary_Side', 'Sanctuary_Combat'], path)
    for pl in giver_report['placements']:
        if pl['package'] in maps:
            closure.add_seed(pl['package'], pl['index'], 'giver')
    log('giver placements', [(p['package'], p['index'], p['path']) for p in giver_report['placements']])

    def progress(done, queued, total):
        log('decoded %d / queued %d / nodes %d' % (done, queued, total))
    closure.run(progress, workers=args.workers)
    log('closure nodes', len(closure.nodes), 'failures', len(closure.failures), 'reader calls', env.calls)

    # -- Kismet graph -----------------------------------------------------------------------
    graphs = []
    for pkg, outer, path in kismet_seqs:
        g = Kismet(env, closure, pkg, outer).build()
        graphs.append(g)
    log('kismet nodes', sum(len(g.nodes) for g in graphs))

    remote = remote_event_index(env, closure, maps)
    by_name = {}
    for e in remote:
        if e['event_name']:
            by_name.setdefault(e['event_name'], []).append(e)

    kismet_out = []
    for g in graphs:
        events = [n for n in g.nodes.values() if n['kind'] == 'event']
        interps = [n for n in g.nodes.values() if n['class'] == 'Engine.SeqAct_Interp']
        flow = g.flow_paths([n['id'] for n in events], [n['id'] for n in interps])
        used_classes = sorted({n['class'] for n in g.nodes.values()})
        kismet_out.append({
            'package': g.pkg, 'sequence_index': g.seq_idx, 'sequence_path': g.seq_path,
            'counts': {'nodes': len(g.nodes), 'flow_edges': len(g.edges), 'variable_edges': len(g.var_edges),
                       'by_class': count_by(g.nodes.values(), lambda n: n['class']),
                       'by_kind': count_by(g.nodes.values(), lambda n: n['kind'])},
            'nodes': list(g.nodes.values()), 'edges': g.edges, 'variable_edges': g.var_edges,
            'flow_paths_event_to_interp': flow, 'failures': g.failures,
            'used_classes': used_classes})

    # -- BehaviorProviders in the closure ----------------------------------------------------
    providers = {}
    for k, n in closure.nodes.items():
        if n['kind'] == 'data' and n.get('status') in ('decoded', 'partial') and n['class'].endswith('BehaviorProviderDefinition'):
            try:
                providers[k] = behavior_provider_graphs(env, closure, k)
            except Exception as e:  # report, never hide
                closure.fail('behavior-provider', k, '%s: %s' % (type(e).__name__, e))

    # -- class reports (script vs native) ----------------------------------------------------
    class_reports = {}
    if not args.skip_classes:
        want = set()
        for g in graphs:
            want.update(c for c in (n['class'] for n in g.nodes.values()))
        for n in closure.nodes.values():
            if n['class'].split('.')[-1].startswith(('Behavior_', 'PlayerBehavior_')):
                want.add(n['class'])
        kids = {}
        for cp in sorted(want):
            try:
                class_reports[cp] = class_report(env, cp, kids)
            except Failure as e:
                closure.fail('class-report', cp, str(e))
        log('class reports', len(class_reports))

    # -- subjects for reachability / payload gap ---------------------------------------------
    subjects = {}
    mw = mf.get('MissionWeapon')
    if isinstance(mw, dict):
        r = closure.resolve(mission['package'], mw)
        if r['status'] == 'resolved':
            subjects['mission_weapon'] = set(reachable(closure, [closure.key(r['package'], r['index'])]))
    giver_keys = [closure.key(p['package'], p['index']) for p in giver_report['placements'] if p['package'] in maps]
    if giver_keys:
        subjects['giver'] = set(reachable(closure, giver_keys))
    dens = [k for k, n in closure.nodes.items() if n['class'].endswith('PopulationOpportunityDen')]
    for k in dens:
        subjects['den:' + k] = set(reachable(closure, [k]))
    dg = mf.get('MissionDialogGroup')
    if isinstance(dg, dict):
        r = closure.resolve(mission['package'], dg)
        if r['status'] == 'resolved':
            subjects['dialog'] = set(reachable(closure, [closure.key(r['package'], r['index'])]))
    subject_summaries = {name: summarize_reachable(closure, sorted(keys)) for name, keys in subjects.items()}
    gap = payload_gap(closure, subjects)

    checks = spot_check(env, closure)
    log('spot checks', [(c['key'], c['match']) for c in checks])

    # -- write outputs -------------------------------------------------------------------------
    nodes_out = [closure.nodes[k] for k in closure.order]
    statuses = count_by(nodes_out, lambda n: n.get('status'))
    doc = {
        'provenance': provenance,
        'mission': {'key': mkey, 'path': args.mission, 'copies': ['%s:%d' % (h['package'], h['index']) for h in mission_hits],
                    'status': mdec['status'], 'prefix': mdec['prefix']},
        'counts': {'nodes': len(nodes_out), 'edges': len(closure.edges), 'by_kind': count_by(nodes_out, lambda n: n['kind']),
                   'by_status': statuses, 'by_class': count_by(nodes_out, lambda n: n['class']),
                   'by_package': count_by(nodes_out, lambda n: n['package']),
                   'by_seed': count_by(nodes_out, lambda n: n['seed']),
                   'type_refs': dict(sorted(closure.type_refs.items())), 'truncated': closure.truncated,
                   'failures': len(closure.failures), 'duplicates': len(closure.duplicates)},
        'nodes': nodes_out, 'edges': closure.edges, 'failures': closure.failures,
        'duplicates': sorted(closure.duplicates.values(), key=lambda d: d['path']),
        'unsupported_properties': sorted({'%s %s' % (n['key'], u) for n in nodes_out for u in n.get('unsupported', [])}),
        'giver': giver_report, 'subjects': subject_summaries, 'payload_gap': gap,
        'behavior_providers': providers, 'spot_checks': checks,
        'remote_events': {'entries': remote, 'supported_remote_events': mf.get('SupportedRemoteEvents'),
                          'supported_custom_events': mf.get('SupportedCustomEvents')},
    }
    (out / 'closure.json').write_text(json.dumps(doc, indent=1), encoding='utf-8')
    kdoc = {'provenance': provenance, 'sequences': kismet_out, 'remote_event_index': remote,
            'class_reports': class_reports}
    (out / 'kismet_graph.json').write_text(json.dumps(kdoc, indent=1), encoding='utf-8')
    write_summary(out / 'SUMMARY.txt', doc, kdoc, time.time() - t0)
    log('wrote', out)
    return 0 if not closure.failures else 3


def write_summary(path, doc, kdoc, seconds):
    lines = ['Mission closure summary (generated; game-derived; local only)', '',
             'mission: %s' % doc['mission']['path'], 'generated: %s  tool: %s v%s  repo commit: %s' % (
                 doc['provenance']['generated_utc'], TOOL, TOOL_VERSION, doc['provenance']['repo_commit']),
             'elapsed: %.0fs' % seconds, '']
    c = doc['counts']
    lines.append('closure nodes: %d  edges: %d  failures: %d  duplicates: %d  truncated: %s' % (
        c['nodes'], c['edges'], c['failures'], c['duplicates'], c['truncated']))
    lines.append('by kind: %s' % c['by_kind'])
    lines.append('by status: %s' % c['by_status'])
    lines.append('by seed: %s' % c['by_seed'])
    lines.append('')
    lines.append('classes (count):')
    for k, v in sorted(c['by_class'].items(), key=lambda kv: (-kv[1], kv[0])):
        lines.append('  %5d  %s' % (v, k))
    lines.append('')
    for s in kdoc['sequences']:
        lines.append('kismet %s:%s  nodes %d  flow edges %d  variable edges %d' % (
            s['package'], s['sequence_path'], s['counts']['nodes'], s['counts']['flow_edges'], s['counts']['variable_edges']))
        lines.append('  by kind: %s' % s['counts']['by_kind'])
        lines.append('  event -> interp paths: %s' % {k: len(v) for k, v in s['flow_paths_event_to_interp'].items()})
    lines.append('')
    lines.append('failures by kind: %s' % count_by(doc['failures'], lambda f: f['kind']))
    lines.append('spot checks: %d/%d match' % (sum(1 for x in doc['spot_checks'] if x['match']), len(doc['spot_checks'])))
    path.write_text('\n'.join(lines) + '\n', encoding='utf-8')


if __name__ == '__main__':
    sys.exit(main())
