"""
particle_system.py - read-only decoder for cooked BL2 ParticleSystem templates (Cascade data).

AI-assisted (Claude). Clean-room: no game data in this file. Output goes to stdout or the ignored
``local/`` tree (default ``local/phaselock/emitters/``). Written to give a later host pass the numbers it
needs to rebuild a few effects (Phaselock's bubble and hand orb); not a converter.

What a cooked template looks like (read off the packages; see docs/verification/PHASELOCK_STOCK_DATA.md):

ParticleSystem -> Emitters[] -> ParticleSpriteEmitter -> LODLevels[] -> ParticleLODLevel ->
RequiredModule, SpawnModule, TypeDataModule, Modules[]. Every one of those is an export whose payload is a
4-byte prefix and then a tagged-property stream. Tags are delta-serialized: a property equal to the
archetype's value is omitted. For these exports the archetype is the class default object (CDO) in
Engine.upk, itself delta-serialized against its parent class's CDO, so a value is the own tag, else the
CDO chain, else zero. Struct properties are merged field by field the same way; arrays are replaced
whole. Struct defaults of a ScriptStruct itself are read where the struct_defaults_census layout applies.

RawDistributionFloat / RawDistributionVector (the cooked form of a Cascade distribution) is a tagged struct:
    Distribution            object, None once cooked (the curve object is stripped)
    Type, Op, LookupTableNumElements, LookupTableChunkSize    plain bytes, omitted when equal to the CDO's
    LookupTable             array<float>
    LookupTableTimeScale, LookupTableStartTime                 floats
The table layout this module assumes, and checks on every table it reads (``check_table``):
    LookupTable = [range_a, range_b] + entries, each entry ``LookupTableChunkSize`` floats,
    ChunkSize = NumElements x width (width 1 for float, 3 for vector), NumElements 1 (one value) or 2 (a
    low/high pair for a random or extreme distribution), and entries are spaced 1/TimeScale apart
    starting at StartTime (one entry when TimeScale is 0).
How Op selects between the low/high pair (uniform random versus "extreme") is FITTED from which modules
carry which Op and is UNVERIFIED; the header semantics are recorded in the verification note together with
the oracle counts (exact sizes, entry arithmetic, and agreement with uncooked distribution objects in
Engine.upk/Startup.upk evaluated at the table's sample times).

BurstList (array of ParticleBurst) and DynamicParams (array of EmitterDynamicParameter) are tagged structs
per element; their field names come from the tags themselves.

Usage:
    python research/particle_system.py [Part_SirenASHandOrb | FX_CHAR_Siren.Particles.Part_SirenASHandOrb ...]
        [--package GD_Siren_Streaming_SF] [--out local/phaselock/emitters] [--oracle [PACKAGE ...]]
With no template names it decodes the Phaselock set (DEFAULT_TEMPLATES).
Set OPENWILLOW_BL2 (game folder) or pass --game <CookedPCConsole dir>.
"""
import argparse
import json
import os
import struct
import sys

# ---------------------------------------------------------------- tagged properties

# Atomic structs serialized natively inside a tag (field names, struct format). Observed 832/46 layouts;
# the same table the C++ reader uses (src/package.cpp). Color is stored B,G,R,A.
NATIVE_STRUCTS = {
    'Vector': (('X', 'Y', 'Z'), '<3f'),
    'Vector2D': (('X', 'Y'), '<2f'),
    'Rotator': (('Pitch', 'Yaw', 'Roll'), '<3i'),
    'LinearColor': (('R', 'G', 'B', 'A'), '<4f'),
    'Color': (('B', 'G', 'R', 'A'), '<4B'),
    'Guid': (('A', 'B', 'C', 'D'), '<4i'),
    'Quat': (('X', 'Y', 'Z', 'W'), '<4f'),
    'IntPoint': (('X', 'Y'), '<2i'),
}

# Element kind of each array this decoder reads. 'f' float, 'i' int, 'o' object reference, 'n' FName,
# 'b' byte, ('s', StructName) a tagged struct per element, ('v', StructName) a native struct per element.
# Arrays not listed are kept as raw bytes and reported, never guessed.
ARRAYS = {
    'LookupTable': 'f',
    'Emitters': 'o', 'LODLevels': 'o', 'Modules': 'o', 'SpawnModules': 'o', 'UpdateModules': 'o',
    'OrbitModules': 'o', 'EventReceiverModules': 'o', 'SpawningModules': 'o',
    'LODDistances': 'f',
    'BurstList': ('s', 'ParticleBurst'),
    'DynamicParams': ('s', 'EmitterDynamicParameter'),
    'LODSettings': ('s', 'ParticleSystemLOD'),
    'Points': ('s', 'InterpCurvePoint'),
    'ScalarParameterValues': ('s', 'ScalarParameterValue'),
    'VectorParameterValues': ('s', 'VectorParameterValue'),
    'TextureParameterValues': ('s', 'TextureParameterValue'),
    'StaticSwitchParameters': ('s', 'StaticSwitchParameter'),
    'StaticComponentMaskParameters': ('s', 'StaticComponentMaskParameter'),
    'InstanceParameters': ('s', 'ParticleSysParam'),
    'Materials': 'o', 'Expressions': 'o',
}


class DecodeError(ValueError):
    pass


class Context:
    """Names and object paths of one package. ``ref_path(ref)`` turns an import/export index into a path."""

    def __init__(self, names, ref_path=None):
        self.names = names
        self.ref_path = ref_path or (lambda ref: ref)

    def fname(self, b, p):
        if p + 8 > len(b):
            raise DecodeError('name past end')
        i, n = struct.unpack_from('<ii', b, p)
        if not 0 <= i < len(self.names) or n < 0:
            raise DecodeError('name index out of range at %d' % p)
        return self.names[i] + ('_%d' % (n - 1) if n else '')


def _array(ctx, name, b, p, stop, depth):
    count = struct.unpack_from('<i', b, p)[0]
    p += 4
    if count < 0 or count > 1000000:
        raise DecodeError('bad array count for ' + name)
    kind = ARRAYS.get(name)
    body = stop - p
    if kind is None:
        return {'_raw_array': b[p:stop].hex(), '_count': count}, stop
    if kind in ('f', 'i', 'o'):
        if body != 4 * count:
            raise DecodeError('%s: %d bytes for %d words' % (name, body, count))
        fmt = {'f': '<%df', 'i': '<%di', 'o': '<%di'}[kind] % count
        values = list(struct.unpack_from(fmt, b, p))
        if kind == 'o':
            values = [ctx.ref_path(v) if v else None for v in values]
        return values, stop
    if kind == 'n':
        if body != 8 * count:
            raise DecodeError(name + ': name array size')
        return [ctx.fname(b, p + 8 * k) for k in range(count)], stop
    if kind == 'b':
        if body != count:
            raise DecodeError(name + ': byte array size')
        return list(b[p:stop]), stop
    if kind[0] == 'v':
        names, fmt = NATIVE_STRUCTS[kind[1]]
        width = struct.calcsize(fmt)
        if body != width * count:
            raise DecodeError(name + ': native struct array size')
        return [dict(zip(names, struct.unpack_from(fmt, b, p + width * k))) for k in range(count)], stop
    items = []
    for _ in range(count):
        item, p = walk(ctx, b, p, stop, depth + 1)
        item['_type'] = kind[1]
        items.append(item)
    if p != stop:
        raise DecodeError('%s: struct array ends at %d, tag says %d' % (name, p, stop))
    return items, stop


def walk(ctx, b, p, end, depth=0, sizes=None):
    """Decode one tagged-property stream. Returns (dict name -> value, position after the None tag).

    Static-array elements past index 0 are keyed ``Name[i]``. Every tag's value must consume exactly the
    size the tag declares (DecodeError otherwise): that is the per-property structural oracle. ``sizes``,
    if given, receives (name, kind, detail, size) for every tag at this level.
    """
    if depth > 24:
        raise DecodeError('nesting')
    out = {}
    while True:
        name = ctx.fname(b, p)
        p += 8
        if name == 'None':
            return out, p
        kind = ctx.fname(b, p)
        p += 8
        if p + 8 > end:
            raise DecodeError('tag past end')
        size, index = struct.unpack_from('<ii', b, p)
        p += 8
        if size < 0 or index < 0:
            raise DecodeError('negative tag')
        detail = None
        if kind in ('StructProperty', 'ByteProperty'):
            detail = ctx.fname(b, p)
            p += 8
        boolean = None
        if kind == 'BoolProperty':
            boolean = b[p]
            p += 1
            if boolean > 1:
                raise DecodeError('bool byte %d' % boolean)
        stop = p + size
        if stop > end:
            raise DecodeError('tag body past end: ' + name)
        if kind == 'IntProperty':
            value, q = struct.unpack_from('<i', b, p)[0], p + 4
        elif kind == 'FloatProperty':
            value, q = struct.unpack_from('<f', b, p)[0], p + 4
        elif kind == 'BoolProperty':
            value, q = bool(boolean), p
        elif kind == 'NameProperty':
            value, q = ctx.fname(b, p), p + 8
        elif kind in ('ObjectProperty', 'ClassProperty', 'ComponentProperty', 'InterfaceProperty'):
            ref = struct.unpack_from('<i', b, p)[0]
            value, q = (ctx.ref_path(ref) if ref else None), p + 4
        elif kind == 'ByteProperty':
            if detail == 'None':
                value, q = b[p], p + 1
            else:
                value, q = ctx.fname(b, p), p + 8
        elif kind == 'StrProperty':
            n = struct.unpack_from('<i', b, p)[0]
            if n < 0:
                value = b[p + 4:p + 4 - 2 * n].decode('utf-16le', 'replace').rstrip('\0')
                q = p + 4 - 2 * n
            else:
                value = b[p + 4:p + 4 + n].decode('latin-1').rstrip('\0')
                q = p + 4 + n
        elif kind == 'StructProperty':
            if detail == 'Box':  # Min, Max (Vector), IsValid byte: native, as in src/package.cpp
                mn, mx = struct.unpack_from('<3f', b, p), struct.unpack_from('<3f', b, p + 12)
                value, q = {'Min': dict(zip('XYZ', mn)), 'Max': dict(zip('XYZ', mx)), 'IsValid': b[p + 24]}, p + 25
            elif detail in NATIVE_STRUCTS:
                fields, fmt = NATIVE_STRUCTS[detail]
                value, q = dict(zip(fields, struct.unpack_from(fmt, b, p))), p + struct.calcsize(fmt)
            else:
                value, q = walk(ctx, b, p, stop, depth + 1)
                value['_type'] = detail
        elif kind == 'ArrayProperty':
            value, q = _array(ctx, name, b, p, stop, depth)
        else:
            raise DecodeError('unknown property kind ' + kind)
        if q != stop:
            raise DecodeError('%s (%s %s): value used %d of %d bytes' % (name, kind, detail, q - p, size))
        if sizes is not None:
            sizes.append((name, kind, detail, size))
        out[name if index == 0 else '%s[%d]' % (name, index)] = value
        p = stop


def merge(default, own):
    """Own tags over defaults. Tagged structs (dicts without native-only keys) merge field by field."""
    if not isinstance(default, dict) or not isinstance(own, dict):
        return own
    out = dict(default)
    for k, v in own.items():
        out[k] = merge(default.get(k), v) if isinstance(v, dict) and isinstance(default.get(k), dict) else v
    return out


# ---------------------------------------------------------------- baked distributions

FLOAT_WIDTH = {'RawDistributionFloat': 1, 'RawDistributionVector': 3}


def raw_table(d, width):
    """Header + entries of a merged RawDistribution dict. No interpretation beyond the layout."""
    table = d.get('LookupTable') or []
    return {
        'type': d.get('Type', 0), 'op': d.get('Op', 0),
        'elements': d.get('LookupTableNumElements', 0), 'chunk': d.get('LookupTableChunkSize', 0),
        'time_scale': d.get('LookupTableTimeScale', 0.0), 'start_time': d.get('LookupTableStartTime', 0.0),
        'width': width, 'values': table,
    }


def check_table(t):
    """Structural oracle for one baked table. Returns a list of failure strings (empty = consistent).

    Checks: chunk = elements x width; values = 2 + whole entries; more than one entry only with a time
    scale, except the two-entry form constants and pairs are baked in; the two leading values bound the
    entries (one-value tables) or equal (min of the low halves, max of the high halves) for pair tables.
    """
    fails = []
    v, chunk, elems, width = t['values'], t['chunk'], t['elements'], t['width']
    if not v:
        return ['empty table']
    if chunk <= 0:
        return ['chunk size %d' % chunk]
    if chunk != elems * width:
        fails.append('chunk %d != elements %d x width %d' % (chunk, elems, width))
    if len(v) < 2 + chunk or (len(v) - 2) % chunk:
        fails.append('%d values do not split into 2 + n x %d' % (len(v), chunk))
        return fails
    count = (len(v) - 2) // chunk
    if not t['time_scale'] and count != 2:
        fails.append('%d entries without a time scale' % count)
    body = [v[2 + k * chunk:2 + (k + 1) * chunk] for k in range(count)]
    eps = 1e-4 * max(1.0, max(abs(x) for x in v))
    if elems == 2:
        lo = min(x for e in body for x in e[:width])
        hi = max(x for e in body for x in e[width:])
        alo, ahi = min(min(e) for e in body), max(max(e) for e in body)
        # Two observed forms: (min of lows, max of highs), or the range over both halves (seen with Type
        # 0x80, e.g. mirrored vector pairs; meaning of Type UNVERIFIED).
        if (abs(v[0] - lo) > eps or abs(v[1] - hi) > eps) and (abs(v[0] - alo) > eps or abs(v[1] - ahi) > eps):
            fails.append('pair range (%r, %r) != (min low %r, max high %r)' % (v[0], v[1], lo, hi))
    else:
        lo, hi = min(min(e) for e in body), max(max(e) for e in body)
        if v[0] > lo + eps or v[1] < hi - eps:
            fails.append('range (%r, %r) does not bound entries (%r, %r)' % (v[0], v[1], lo, hi))
    return fails


def entries(t):
    """[(time, [values of one entry])] using the fitted layout (2 range floats, then chunks)."""
    v, chunk = t['values'], t['chunk']
    n = (len(v) - 2) // chunk if chunk else 0
    step = 1.0 / t['time_scale'] if t['time_scale'] else 0.0
    return [(t['start_time'] + k * step, v[2 + k * chunk:2 + (k + 1) * chunk]) for k in range(n)]


def sample(t, time):
    """Value of a baked table at ``time``: linear between neighbouring entries, clamped at the ends.
    Returns the entry-sized list (low/high pairs stay pairs). Lerp between entries is UNVERIFIED as the
    engine's exact rule; it is what the entry spacing implies."""
    es = entries(t)
    if not es:
        return None
    if len(es) == 1 or not t['time_scale']:
        return list(es[0][1])
    x = max(0.0, (time - t['start_time']) * t['time_scale'])
    i = min(int(x), len(es) - 1)
    if i >= len(es) - 1:
        return list(es[-1][1])
    a = x - i
    return [p + (q - p) * a for p, q in zip(es[i][1], es[i + 1][1])]


def summarize(t):
    """Compact human/host description of a baked table: constant, pair (low/high), or a curve."""
    es = entries(t)
    if not es:
        return None
    pair = t['elements'] == 2
    w = t['width']

    def split(vals):
        if not pair:
            return vals[:w] if w > 1 else vals[0]
        lo, hi = vals[:w], vals[w:2 * w]
        return {'low': lo if w > 1 else lo[0], 'high': hi if w > 1 else hi[0]}

    if len(es) == 1 or all(e[1] == es[0][1] for e in es):
        return {'kind': 'pair' if pair else 'constant', 'value': split(es[0][1])}
    return {'kind': 'curve_pair' if pair else 'curve', 'samples': [[round(tm, 6), split(vals)] for tm, vals in es]}


# ---------------------------------------------------------------- uncooked distributions (oracle)

def eval_curve(points, x, width):
    """Evaluate an InterpCurve (list of point dicts) at x. Linear, constant and Hermite segments, with the
    stored tangents. Mode names are the tags' enum names."""
    if not points:
        return [0.0] * width

    def vec(v):
        if isinstance(v, dict):
            return [v.get('X', 0.0), v.get('Y', 0.0), v.get('Z', 0.0)][:width]
        return [v if v is not None else 0.0]

    pts = [(p.get('InVal', 0.0), vec(p.get('OutVal', 0.0 if width == 1 else {})),
            vec(p.get('ArriveTangent', 0.0 if width == 1 else {})), vec(p.get('LeaveTangent', 0.0 if width == 1 else {})),
            p.get('InterpMode', 'CIM_Linear')) for p in points]
    if x <= pts[0][0]:
        return pts[0][1]
    if x >= pts[-1][0]:
        return pts[-1][1]
    for k in range(len(pts) - 1):
        x0, y0, _, l0, mode = pts[k]
        x1, y1, a1, _, _ = pts[k + 1]
        if x0 <= x < x1:
            d = x1 - x0
            if d <= 0:
                return y1
            a = (x - x0) / d
            if mode == 'CIM_Constant':
                return y0
            if mode == 'CIM_Linear':
                return [p + (q - p) * a for p, q in zip(y0, y1)]
            a2, a3 = a * a, a * a * a
            h00, h10, h01, h11 = 2 * a3 - 3 * a2 + 1, a3 - 2 * a2 + a, -2 * a3 + 3 * a2, a3 - a2
            return [h00 * p + h10 * d * t0 + h01 * q + h11 * d * t1 for p, q, t0, t1 in zip(y0, y1, l0, a1)]
    return pts[-1][1]


def eval_distribution(cls, props, x, width):
    """Value of an uncooked distribution object at x as the entry the baker would store: [v] / [lo, hi]
    (floats) or [x,y,z] / [lox,loy,loz,hix,hiy,hiz] (vectors). None for classes this oracle does not model."""
    def vec(v):
        return [v.get('X', 0.0), v.get('Y', 0.0), v.get('Z', 0.0)] if isinstance(v, dict) else [0.0] * 3
    if cls == 'DistributionFloatConstant':
        return [props.get('Constant', 0.0)]
    if cls == 'DistributionFloatUniform':
        return [props.get('Min', 0.0), props.get('Max', 0.0)]
    if cls == 'DistributionVectorConstant':
        return vec(props.get('Constant'))
    if cls == 'DistributionVectorUniform':
        return vec(props.get('Min')) + vec(props.get('Max'))
    if cls in ('DistributionFloatConstantCurve', 'DistributionVectorConstantCurve'):
        curve = props.get('ConstantCurve') or {}
        return eval_curve(curve.get('Points') or [], x, width)
    return None


# ---------------------------------------------------------------- package access

def _default_game():
    root = os.environ.get('OPENWILLOW_BL2', r'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2')
    return os.path.join(root, 'WillowGame', 'CookedPCConsole')


PREFIXES = (4, 8, 16, 12)


def _export_archetypes(data):
    """ArchetypeIndex of every export, read from the same export-table layout research/native_count.py
    walks (which does not keep this field)."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import native_count as nc  # noqa: E402
    r = nc.Reader(data)
    r.u32(); r.u16(); r.u16(); r.i32(); r.fstring(); r.u32()
    r.i32(); r.i32()
    count, offset = r.i32(), r.i32()
    r.p = offset
    out = []
    for _ in range(count):
        r.i32(); r.i32(); r.i32(); r.fname()
        out.append(r.i32())
        r.u64(); r.i32(); r.i32(); r.i32()
        nets = r.i32()
        r.p += 4 * nets + 16 + 4
    return out


class Package:
    """One decompressed package (via research/behavior_census.py's loader) plus a property cache."""

    def __init__(self, path):
        sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
        import behavior_census as bc  # noqa: E402 - pure-Python LZO + tables
        self.pkg = bc.Pkg(path)
        self.name = self.pkg.name
        self.ctx = Context(self.pkg.names, self.pkg.path)
        self.by_path = {self.pkg.path(i).lower(): i for i in range(1, len(self.pkg.exports) + 1)}
        self.archetypes = _export_archetypes(self.pkg.data)
        self.cache = {}

    def index(self, path):
        return self.by_path.get(path.lower())

    def class_name(self, ref):
        return self.pkg.class_name(ref)

    def own(self, ref, sizes=None):
        """Own tags of export ``ref``. The payload starts with a short prefix before the tag stream: 4 bytes
        for most exports, 8 or 16 for distribution subobjects (the extra words carry what looks like a
        template FName; meaning UNVERIFIED). The prefix is the first of PREFIXES whose stream parses and ends
        exactly at the export end (``_trailing`` 0); if none does, the first that parses is kept and
        ``_trailing`` reports the leftover bytes."""
        if ref in self.cache and sizes is None:
            return self.cache[ref]
        e = self.pkg.exports[ref - 1]
        start, end = e['off'], e['off'] + e['size']
        best, best_sizes, error = None, None, None
        for prefix in PREFIXES:
            if start + prefix + 8 > end:
                continue
            got = [] if sizes is not None else None
            try:
                props, q = walk(self.ctx, self.pkg.data, start + prefix, end, sizes=got)
            except (DecodeError, struct.error, IndexError) as x:
                error = x
                continue
            props['_prefix'], props['_trailing'] = prefix, end - q
            if best is None or (best['_trailing'] and not props['_trailing']):
                best, best_sizes = props, got
            if not props['_trailing']:
                break
        if best is None:
            raise DecodeError('%s: no prefix parses (%s)' % (self.pkg.path(ref), error))
        if sizes is not None:
            sizes.extend(best_sizes)
        self.cache[ref] = best
        return best


def _plain(props):
    return {k: v for k, v in props.items() if not k.startswith('_')}


class Resolver:
    """Effective property values: own tags merged over the archetype chain.

    Archetype of an export: its ArchetypeIndex if set (an import is looked up in Engine.upk); otherwise,
    for a subobject whose outer has an archetype, the same-named child of that archetype (how Engine.upk's
    class-default subobjects inherit; FITTED, it is what makes their baked tables match their distribution
    objects); otherwise the class default object ``Engine.Default__<Class>``.
    """

    def __init__(self, game):
        self.game = game
        self.packages = {}
        self.engine = self.package('Engine')
        self.memo = {}

    def package(self, name):
        if name not in self.packages:
            self.packages[name] = Package(os.path.join(self.game, name + '.upk'))
        return self.packages[name]

    def archetype(self, P, ref):
        """(package, export index) of the archetype, or None for the root."""
        arch = P.archetypes[ref - 1]
        if arch > 0:
            return P, arch
        if arch < 0:
            path = P.pkg.path(arch)
            head, _, rest = path.partition('.')
            if head.lower() == 'engine':
                i = self.engine.index(rest)
                return (self.engine, i) if i else None
            return None
        outer = P.pkg.exports[ref - 1]['outer']
        if outer > 0:
            parent = self.archetype(P, outer)
            if parent:
                Q, q = parent
                i = Q.index(Q.pkg.path(q) + '.' + P.pkg.exports[ref - 1]['name'])
                if i:
                    return Q, i
        cls = P.class_name(ref)
        name = P.pkg.exports[ref - 1]['name']
        if name.startswith('Default__'):
            return None
        i = self.engine.index('Default__' + cls)
        return (self.engine, i) if i else None

    def effective(self, P, ref, depth=0):
        key = (P.name, ref)
        if key in self.memo:
            return self.memo[key]
        if depth > 32:
            raise DecodeError('archetype chain too deep')
        base = {}
        a = self.archetype(P, ref)
        if a:
            base = self.effective(a[0], a[1], depth + 1)
        result = merge(base, _plain(P.own(ref)))
        self.memo[key] = result
        return result


# ---------------------------------------------------------------- template description

EDITOR_ONLY = {'ModuleEditorColor', 'EmitterEditorColor', 'bEditable', 'ThumbnailAngle', 'ThumbnailDistance',
               'ThumbnailWarmup', 'ThumbnailImage', 'ThumbnailImageOutOfDate', 'CurveEdSetup', 'PreviewLightRadius',
               'PreviewLightBrightness', 'FloorMesh', 'FloorPosition', 'FloorRotation', 'FloorScale', 'FloorScale3D',
               'BackgroundColor', 'LightingGuid', 'ReferencedTextureGuids', 'bShouldResetPeakCounts', 'PeakActiveParticles',
               'ConvertedModules', 'bIsDirty', 'LODDuplicate', 'Expressions'}
PARAMETER_CLASSES = ('DistributionFloatParticleParameter', 'DistributionVectorParticleParameter')
DEFAULT_TEMPLATES = ['Part_SirenASHandOrb', 'Part_SirenASHandFizzle', 'Part_SirenASEnemyOrbBegin', 'Part_SirenASEnemyOrb',
                     'Part_SirenASEnemyOrbEnd', 'Part_PhaseLockScreenEffect', 'Part_PhaseLock_Miss_Impact',
                     'Part_PhaseLock_EnemyCannotBeLocked']


# Enum-typed properties that matter to a host, and the Engine.upk enum that types them. An absent tag
# means the archetype's value; where no archetype sets it, the enum's first value. The enum value names
# are read from Engine.upk, not written here.
ENUM_PROPERTIES = {
    'ScreenAlignment': 'ParticleSpriteEmitter.EParticleScreenAlignment',
    'InterpolationMethod': 'ParticleEmitter.EParticleSubUVInterpMethod',
    'SortMode': 'ParticleModuleRequired.EParticleSortMode',
    'ParticleBurstMethod': 'ParticleEmitter.EParticleBurstMethod',
    'SystemUpdateMode': 'ParticleSystem.EParticleSystemUpdateMode',
    'LODMethod': 'ParticleSystem.ParticleSystemLODMethod',
    'MeshAlignment': 'ParticleModuleTypeDataMesh.EMeshScreenAlignment',
}


def enum_names(P, path):
    """Value names of an Enum export: a count followed by that many FNames, ending exactly at the export
    end (the start is searched within the first 32 bytes; layout FITTED on Engine.upk enums)."""
    i = P.index(path)
    if not i:
        return None
    e = P.pkg.exports[i - 1]
    b = P.pkg.data[e['off']:e['off'] + e['size']]
    for start in range(0, 32, 4):
        if start + 4 > len(b):
            break
        count = struct.unpack_from('<i', b, start)[0]
        if 0 < count < 256 and start + 4 + 8 * count == len(b):
            return [P.ctx.fname(b, start + 4 + 8 * k) for k in range(count)]
    return None


class Describer:
    """Turns one template into a JSON-ready description. ``issues`` collects every check failure."""

    def __init__(self, resolver, package):
        self.R, self.P = resolver, package
        self.issues = []
        self.parameters = {}
        self.materials = {}

    def lookup(self, path):
        """(package, index) of an object path: this package, then Startup.upk (FX shared by streaming
        packages are exported there), then a cooked package named after the path's first segment."""
        i = self.P.index(path)
        if i:
            return self.P, i
        for name in ('Startup', path.split('.')[0]):
            if not os.path.exists(os.path.join(self.R.game, name + '.upk')):
                continue
            Q = self.R.package(name)
            i = Q.index(path)
            if i:
                return Q, i
        return None

    def distribution(self, value, where):
        width = FLOAT_WIDTH[value['_type']]
        t = raw_table(value, width)
        out = {'header': {k: t[k] for k in ('type', 'op', 'elements', 'chunk', 'time_scale', 'start_time')}}
        if t['values']:
            fails = check_table(t)
            out['table'] = summarize(t) if not fails else None
            out['raw'] = t['values']
            out['check'] = fails or 'ok'
            if fails:
                self.issues.append('%s: %s' % (where, fails))
        dist = value.get('Distribution')
        if dist:
            found = self.lookup(dist)
            if not found:
                out['distribution'] = {'path': dist, 'resolved': False}
                self.issues.append('%s: distribution %s not found' % (where, dist))
            else:
                Q, i = found
                cls = Q.class_name(i)
                props = {k: v for k, v in self.R.effective(Q, i).items() if k not in EDITOR_ONLY}
                out['distribution'] = {'class': cls, 'path': dist, **self.render(props, where + '.Distribution')}
                if cls in PARAMETER_CLASSES:
                    param = {
                        'name': props.get('ParameterName', 'None'), 'kind': 'vector' if 'Vector' in cls else 'float',
                        'mode': props.get('ParamMode', 'DPM_Normal (default 0)'),
                        'input': [props.get('MinInput', 0.0), props.get('MaxInput', 0.0)],
                        'output': [props.get('MinOutput', 0.0), props.get('MaxOutput', 0.0)],
                        'constant': props.get('Constant', 0.0),
                    }
                    out['parameter'] = param
                    self.parameters.setdefault(param['name'], []).append({'at': where, **param})
                elif not t['values']:
                    sample_at = [0.0, 0.25, 0.5, 0.75, 1.0]
                    vals = [eval_distribution(cls, props, x, width) for x in sample_at]
                    if vals[0] is not None:
                        out['evaluated'] = [[x, v] for x, v in zip(sample_at, vals)]
        return out

    def render(self, value, where):
        if isinstance(value, dict):
            if value.get('_type') in FLOAT_WIDTH:
                return self.distribution(value, where)
            return {k: self.render(v, '%s.%s' % (where, k)) for k, v in value.items()
                    if not k.startswith('_') and k not in EDITOR_ONLY}
        if isinstance(value, list):
            return [self.render(v, '%s[%d]' % (where, k)) for k, v in enumerate(value)]
        return value

    def obj(self, path):
        found = self.lookup(path)
        if not found:
            self.issues.append('object not found: ' + path)
            return None, {}
        Q, i = found
        own = Q.own(i)
        if own.get('_trailing'):
            self.issues.append('%s: %d trailing bytes after the tags' % (path, own['_trailing']))
        return Q.class_name(i), self.R.effective(Q, i)

    def module(self, path):
        cls, props = self.obj(path)
        return {'class': cls, 'path': path, 'properties': self.render(props, path.split('.')[-1])}

    def material(self, path):
        """Blend mode, lighting model and parameters of a material as far as its tagged properties say.
        Cooked materials keep their expression list only as object slots (mostly zero); the parameter
        expressions that survive are listed. Graph wiring is not in the tags."""
        if not isinstance(path, str) or path in self.materials:
            return
        found = self.lookup(path)
        if not found:
            self.materials[path] = {'resolved': False}
            return
        Q, i = found
        cls = Q.class_name(i)
        props = self.R.effective(Q, i)
        own = Q.own(i)
        keep = {k: v for k, v in props.items() if k not in EDITOR_ONLY and not k.startswith('_')}
        params = []
        prefix = Q.pkg.path(i) + '.'
        for k in range(1, len(Q.pkg.exports) + 1):
            p = Q.pkg.path(k)
            if p.startswith(prefix) and 'Parameter' in Q.class_name(k):
                params.append({'class': Q.class_name(k), **self.render(self.R.effective(Q, k), p)})
        self.materials[path] = {'class': cls, 'package': Q.name, 'properties': self.render(keep, path),
                                'parameter_expressions': params,
                                'bytes_after_tags': own.get('_trailing', 0)}
        parent = props.get('Parent')
        if isinstance(parent, str):
            self.material(parent)

    def template(self, path):
        cls, sysprops = self.obj(path)
        if cls != 'ParticleSystem':
            raise DecodeError('%s is a %s, not a ParticleSystem' % (path, cls))
        out = {'template': path, 'package': self.P.name, 'class': cls,
               'system': self.render({k: v for k, v in sysprops.items() if k != 'Emitters'}, 'system'),
               'emitters': [], 'modules': {}}
        for ep in sysprops.get('Emitters') or []:
            if not ep:
                continue
            ecls, eprops = self.obj(ep)
            em = {'name': eprops.get('EmitterName', 'None'), 'path': ep, 'class': ecls,
                  'properties': self.render({k: v for k, v in eprops.items() if k != 'LODLevels'}, ep.split('.')[-1]),
                  'lods': []}
            for lp in eprops.get('LODLevels') or []:
                _, lprops = self.obj(lp)
                lod = {'path': lp, 'level': lprops.get('Level', 0), 'enabled': lprops.get('bEnabled', False)}
                for key in ('RequiredModule', 'SpawnModule', 'TypeDataModule'):
                    if lprops.get(key):
                        lod[key] = lprops[key]
                lod['Modules'] = [m for m in (lprops.get('Modules') or []) if m]
                for m in [lod.get('RequiredModule'), lod.get('SpawnModule'), lod.get('TypeDataModule')] + lod['Modules']:
                    if m and m not in out['modules']:
                        out['modules'][m] = self.module(m)
                em['lods'].append(lod)
            em['lods'].sort(key=lambda d: d['level'])
            out['emitters'].append(em)
            if em['lods']:
                req = out['modules'].get(em['lods'][0].get('RequiredModule'), {}).get('properties', {})
                self.material(req.get('Material'))
        out['enum_defaults'] = {}
        for prop, enum in ENUM_PROPERTIES.items():
            names = enum_names(self.R.engine, enum)
            if names:
                out['enum_defaults'][prop] = names[0]
        out['parameters'] = self.parameters
        out['materials'] = self.materials
        out['digest'] = digest(out)
        out['issues'] = self.issues
        return out


def compact(value):
    """Rendered value -> the shortest host-facing form (table summary, parameter or evaluated curve)."""
    if isinstance(value, dict):
        if 'header' in value:
            if 'parameter' in value:
                return {'parameter': value['parameter']}
            if value.get('table') is not None:
                return value['table']
            if 'evaluated' in value:
                return {'evaluated': value['evaluated']}
            return None
        return {k: compact(v) for k, v in value.items()}
    if isinstance(value, list):
        return [compact(v) for v in value]
    return value


REQUIRED_KEYS = ('Material', 'ScreenAlignment', 'bUseLocalSpace', 'bKillOnDeactivate', 'bKillOnCompleted',
                 'EmitterDuration', 'EmitterDurationLow', 'bEmitterDurationUseRange', 'EmitterLoops', 'EmitterDelay',
                 'SubImages_Horizontal', 'SubImages_Vertical', 'InterpolationMethod', 'RandomImageTime',
                 'bDurationRecalcEachLoop', 'bUseLegacyEmitterTime', 'SortMode', 'bOrientZAxisTowardCamera',
                 'bScaleUV', 'SpawnRate', 'bRequiresSorting')


def digest(desc):
    """Per emitter, LOD 0 only: required settings, spawn, and every module's values in compact form."""
    rows = []
    mods = desc['modules']
    for em in desc['emitters']:
        if not em['lods']:
            rows.append({'name': em['name'], 'lods': 0})
            continue
        lod = em['lods'][0]
        req = compact(mods.get(lod.get('RequiredModule'), {}).get('properties', {}))
        spawn = compact(mods.get(lod.get('SpawnModule'), {}).get('properties', {}))
        row = {'name': em['name'], 'enabled': lod['enabled'], 'lod_count': len(em['lods']),
               'required': {k: req[k] if k in req else '%s (default)' % desc['enum_defaults'][k]
                            for k in REQUIRED_KEYS if k in req or k in desc.get('enum_defaults', {})},
               'spawn': {k: spawn[k] for k in ('Rate', 'RateScale', 'bProcessSpawnRate', 'bProcessBurstList',
                                               'BurstList', 'ParticleBurstMethod') if k in spawn},
               'modules': []}
        if lod.get('TypeDataModule'):
            row['type_data'] = compact(mods[lod['TypeDataModule']])
        for m in lod['Modules']:
            d = mods.get(m, {})
            props = compact(d.get('properties', {}))
            row['modules'].append({'class': (d.get('class') or '?').replace('ParticleModule', ''),
                                   **{k: v for k, v in props.items()
                                      if k not in ('bSpawnModule', 'bUpdateModule', 'bCurvesAsColor', 'LODValidity')}})
        rows.append(row)
    return rows


# ---------------------------------------------------------------- package-wide oracles

def oracle(resolver, names):
    """Structural oracles over every particle/distribution export of the named packages. Returns counts."""
    counts = {}

    def bump(*key):
        k = ' / '.join(map(str, key))
        counts[k] = counts.get(k, 0) + 1

    for name in names:
        P = resolver.package(name)
        for i in range(1, len(P.pkg.exports) + 1):
            cls = P.class_name(i)
            if not (cls.startswith('Particle') or cls.startswith('Distribution')):
                continue
            try:
                own = P.own(i)
                props = resolver.effective(P, i)
            except DecodeError:
                bump(name, 'tag stream does not parse', cls)
                continue
            bump(name, 'exports parsed', 'exact end' if not own['_trailing'] else 'trailing bytes')
            for k, v in props.items():
                if isinstance(v, list) and k in ('BurstList', 'DynamicParams'):
                    for s in {tuple(sorted(x for x in e if not x.startswith('_'))) for e in v}:
                        bump(name, k + ' element fields', ','.join(s))
                if not (isinstance(v, dict) and v.get('_type') in FLOAT_WIDTH):
                    continue
                width = FLOAT_WIDTH[v['_type']]
                t = raw_table(v, width)
                if t['values']:
                    fails = check_table(t)
                    bump(name, 'table layout', 'ok' if not fails else fails[0].split(' (')[0])
                    if not fails and t['time_scale']:
                        end = entries(t)[-1][0]
                        bump(name, 'curve table span', 'ends at 1.0' if abs(end - 1.0) < 1e-3 else 'ends elsewhere')
                dist = v.get('Distribution')
                if not dist:
                    continue
                di = P.index(dist)
                if not di:
                    bump(name, 'distribution object', 'not an export')
                    continue
                dc = P.class_name(di)
                if not t['values']:
                    bump(name, 'distribution object without a table', dc)
                    continue
                dp = resolver.effective(P, di)
                worst = 0.0
                for tm, vals in entries(t):
                    ref = eval_distribution(dc, dp, tm, width)
                    if ref is None:
                        worst = None
                        break
                    if len(ref) == 2 * len(vals) and ref[:len(vals)] == ref[len(vals):]:
                        ref = ref[:len(vals)]  # a uniform with low == high is baked as one value
                    if len(ref) != len(vals):
                        worst = float('inf')
                        break
                    worst = max(worst, max(abs(a - b) for a, b in zip(ref, vals)))
                if worst is None:
                    bump(name, 'table vs distribution object', dc, 'not modelled')
                else:
                    scale = max(1.0, max(abs(x) for x in t['values']))
                    bump(name, 'table vs distribution object', dc, 'op %d' % t['op'],
                         'match' if worst <= 1e-4 * scale else 'MISMATCH')
    return counts


# ---------------------------------------------------------------- command line

def main(argv=None):
    ap = argparse.ArgumentParser(description='Decode cooked ParticleSystem templates to JSON (read-only).')
    ap.add_argument('--package', default='GD_Siren_Streaming_SF', help='package name or .upk file name')
    ap.add_argument('templates', nargs='*', help='full object paths, or names under FX_CHAR_Siren.Particles')
    ap.add_argument('--game', default=_default_game(), help='CookedPCConsole directory')
    ap.add_argument('--out', default=os.path.join('local', 'phaselock', 'emitters'))
    ap.add_argument('--oracle', nargs='*', metavar='PACKAGE',
                    help='also run the package-wide oracles (default: Engine Startup WillowGame and the package)')
    args = ap.parse_args(argv)
    R = Resolver(args.game)
    pname = os.path.splitext(os.path.basename(args.package))[0]
    P = R.package(pname)
    os.makedirs(args.out, exist_ok=True)
    total_issues = 0
    for t in args.templates or DEFAULT_TEMPLATES:
        path = t if '.' in t else 'FX_CHAR_Siren.Particles.' + t
        desc = Describer(R, P).template(path)
        out = os.path.join(args.out, path.split('.')[-1] + '.json')
        with open(out, 'w', encoding='utf-8') as f:
            json.dump(desc, f, indent=1)
        print('%-36s emitters %2d  modules %3d  parameters %s  issues %d -> %s' % (
            path.split('.')[-1], len(desc['emitters']), len(desc['modules']),
            ','.join(sorted(desc['parameters'])) or '-', len(desc['issues']), out))
        total_issues += len(desc['issues'])
        for issue in desc['issues'][:10]:
            print('   issue:', issue)
    if args.oracle is not None:
        counts = oracle(R, args.oracle or ['Engine', 'Startup', 'WillowGame', pname])
        with open(os.path.join(args.out, 'oracles.json'), 'w', encoding='utf-8') as f:
            json.dump(counts, f, indent=1, sort_keys=True)
        for k in sorted(counts):
            print('%7d  %s' % (counts[k], k))
    return 1 if total_issues else 0


if __name__ == '__main__':
    sys.exit(main())
