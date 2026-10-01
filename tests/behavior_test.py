"""Synthetic behavior-provider coverage: builds toy `GearboxFramework`, `WillowGame`, `Things` packages and a toy
provider package with hand-assembled tagged data plus the untagged variable value block, runs them through
`ow-package --behavior-dump` / `--behavior-run` / `--object-dump`, and checks results computed by hand.

Nothing here comes from the game: the class, struct, property and enum names are the vocabulary the executor reads
by name (BehaviorProviderDefinition, BehaviorSequenceData, BVAR_Object...); objects and values are invented. The toy
EBehaviorVariableType deliberately orders its values differently from the game, so a decoder that hard-codes enum
positions instead of reading the names fails here.

Pinned here:
  * the value block after a provider's tags: one entry per variable, sizes by type name, exact consumption, or
    nothing is trusted (short / long / unknown type);
  * Behavior_CompareObject reads ObjectA from an event output and ObjectB from a constant, follows link id 0 when
    they are the same object and 1 otherwise;
  * enum bytes whose enum is declared in another package decode through the declaring property (was 0);
  * struct fields that a tagged struct omits take the struct's own default tags (was 0), and only when that default
    stream is consumed exactly.
"""
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

reader = str(Path(sys.argv[1]).resolve())


def w32(*values): return b''.join(struct.pack('<i', v) for v in values)
def u32(v): return struct.pack('<I', v)
def u64(v): return struct.pack('<Q', v)
def f32(v): return struct.pack('<f', v)


class Package:
    """Same toy package writer as tests/kismet_test.py."""
    def __init__(self):
        self.names = []
        self.imports = []
        self.exports = []

    def name(self, text):
        if text not in self.names: self.names.append(text)
        return self.names.index(text)

    def fname(self, text, number=0): return w32(self.name(text), number)

    def add_import_full(self, class_name, outer, name):
        self.imports.append((self.fname('Core'), self.fname(class_name), outer, self.fname(name)))
        return -len(self.imports)

    def add_import(self, class_name, name): return self.add_import_full(class_name, 0, name)

    def add_export(self, cls, name, payload, outer=0, super_ref=0):
        self.exports.append((cls, super_ref, outer, self.fname(name), payload))
        return len(self.exports)

    def build(self):
        self.name('None')
        table = b''.join(w32(len(n) + 1) + n.encode() + b'\0' + bytes(8) for n in self.names)
        io = 48 + len(table)
        import_data = b''.join(cp + cn + w32(outer) + nm for cp, cn, outer, nm in self.imports)
        eo = io + len(import_data)
        payload_at = eo + 68 * len(self.exports)
        export_data = b''
        payloads = b''
        for cls, sup, outer, name, payload in self.exports:
            export_data += w32(cls, sup, outer) + name + w32(0) + bytes(8) + w32(len(payload), payload_at + len(payloads)) \
                + w32(0) + w32(0) + bytes(20)
            payloads += payload
        header = w32(0x9e2a83c1 - (1 << 32), 832 | (46 << 16), 48, 0, 0, len(self.names), 48, len(self.exports), eo,
                     len(self.imports), io, 0)
        return header + table + import_data + export_data + payloads


class Tags:
    """Tagged-property writer for one package (names are package-relative)."""
    def __init__(self, p): self.p = p

    def tag(self, name, kind, size, body, detail=b''):
        return self.p.fname(name) + self.p.fname(kind) + w32(size, 0) + detail + body

    def int(self, name, v): return self.tag(name, 'IntProperty', 4, w32(v))
    def float(self, name, v): return self.tag(name, 'FloatProperty', 4, f32(v))
    def bool(self, name, v): return self.tag(name, 'BoolProperty', 0, bytes([1 if v else 0]))
    def name_(self, name, v): return self.tag(name, 'NameProperty', 8, self.p.fname(v))
    def obj(self, name, ref): return self.tag(name, 'ObjectProperty', 4, w32(ref))
    def enum(self, name, enum, value): return self.tag(name, 'ByteProperty', 8, self.p.fname(value), self.p.fname(enum))
    def byte(self, name, v): return self.tag(name, 'ByteProperty', 1, bytes([v]), self.p.fname('None'))
    def struct_(self, name, type_, body): return self.tag(name, 'StructProperty', len(body), body, self.p.fname(type_))
    def array(self, name, count, body): return self.tag(name, 'ArrayProperty', 4 + len(body), w32(count) + body)
    def none(self): return self.p.fname('None')


def declarations(p, kinds):
    """Toy reflection helpers over package `p` (same 40-byte property header as tests/kismet_test.py)."""
    imp = {n: p.add_import('Class', n) for n in ('Class', 'ScriptStruct', 'Enum') + tuple(k + 'Property' for k in kinds)}
    none = p.fname('None')

    def prop(kind, owner, name, type_ref=0):
        payload = w32(0) + none + w32(0) + u32(1) + u64(0) + none + w32(0) + w32(type_ref)
        return p.add_export(imp[kind + 'Property'], name, payload, outer=owner)

    def array_of(owner, name, inner_kind, inner_ref=0):
        index = len(p.exports) + 1
        prop('Array', owner, name, index + 1)
        prop(inner_kind, index, name, inner_ref)
        return index

    def struct_(name, defaults=None):
        # A ScriptStruct export: 52-byte header (word at +44 = 0), then its default tags (see vm.cpp applyStructDefaults).
        payload = w32(0) * 4 if defaults is None else bytes(52) + defaults
        return p.add_export(imp['ScriptStruct'], name, payload)

    def cls(name, super_ref=0): return p.add_export(imp['Class'], name, w32(0) * 4, super_ref=super_ref)

    def enum(owner, name, values):
        return p.add_export(imp['Enum'], name, w32(0) + none + w32(0) + w32(len(values)) + b''.join(p.fname(v) for v in values),
                            outer=owner)
    return prop, array_of, struct_, cls, enum


# Toy enum orders (not the game's): BVAR_Object is 1 here.
VARIABLE_TYPES = ['BVAR_None', 'BVAR_Object', 'BVAR_Int', 'BVAR_Float', 'BVAR_InstanceData', 'BVAR_NamedVariable', 'BVAR_Mystery']
LINK_TYPES = ['BVARLINK_Unknown', 'BVARLINK_Input', 'BVARLINK_Output']


def build_gearbox():
    p = Package()
    prop, array_of, struct_, cls, enum = declarations(p, ('Int', 'Float', 'Bool', 'Name', 'Object', 'Array', 'Struct', 'Byte'))
    bpd = cls('BehaviorProviderDefinition')
    var_types = enum(bpd, 'EBehaviorVariableType', VARIABLE_TYPES)
    link_types = enum(bpd, 'EBehaviorVariableLinkType', LINK_TYPES)
    sub = struct_('SubarrayData')
    prop('Int', sub, 'ArrayIndexAndLength')
    user = struct_('BehaviorEventUserData')
    prop('Name', user, 'EventName')
    event = struct_('BehaviorEventData2')
    prop('Struct', event, 'UserData', user)
    prop('Struct', event, 'OutputVariables', sub)
    prop('Struct', event, 'OutputLinks', sub)
    data = struct_('BehaviorData2')
    prop('Object', data, 'Behavior')
    prop('Struct', data, 'LinkedVariables', sub)
    prop('Struct', data, 'OutputLinks', sub)
    link = struct_('BehaviorOutputLinkData')
    prop('Int', link, 'LinkIdAndLinkedBehavior')
    prop('Float', link, 'ActivateDelay')
    var = struct_('BehaviorVariableData')
    prop('Name', var, 'Name')
    prop('Byte', var, 'Type', var_types)
    vlink = struct_('BehaviorVariableLinkData2')
    prop('Name', vlink, 'PropertyName')
    prop('Byte', vlink, 'VariableLinkType', link_types)
    prop('Byte', vlink, 'ConnectionIndex')
    prop('Struct', vlink, 'LinkedVariables', sub)
    seq = struct_('BehaviorSequenceData')
    prop('Name', seq, 'BehaviorSequenceName')
    prop('Bool', seq, 'bEnabledOnSpawn')
    prop('Object', seq, 'CustomEnableCondition')
    array_of(seq, 'EventData2', 'Struct', event)
    array_of(seq, 'BehaviorData2', 'Struct', data)
    array_of(seq, 'VariableData', 'Struct', var)
    array_of(seq, 'ConsolidatedOutputLinkData', 'Struct', link)
    array_of(seq, 'ConsolidatedVariableLinkData', 'Struct', vlink)
    array_of(seq, 'ConsolidatedLinkedVariables', 'Int')
    array_of(bpd, 'BehaviorSequences', 'Struct', seq)
    return p


def build_willowgame(broken_defaults=False):
    p = Package()
    prop, array_of, struct_, cls, enum = declarations(p, ('Float', 'Object', 'Struct'))
    t = Tags(p)
    compare = cls('Behavior_CompareObject')
    prop('Object', compare, 'ObjectA')
    prop('Object', compare, 'ObjectB')
    cls('Behavior_ToySame')
    cls('Behavior_ToyDifferent')
    # A struct whose own defaults set Scale = 1.0; the broken variant leaves 4 stray bytes after its None.
    defaults = t.float('Scale', 1.0) + t.none() + (w32(7) if broken_defaults else b'')
    scaled = struct_('ToyScaled', defaults)
    prop('Float', scaled, 'Base')
    prop('Float', scaled, 'Scale')
    holder = cls('ToyHolder')
    prop('Struct', holder, 'Holder', scaled)
    prop('Struct', holder, 'Other', scaled)
    return p


def build_things():
    p = Package()
    imp = p.add_import('Class', 'Object')
    none = p.fname('None')
    group = p.add_export(imp, 'DamageType', w32(0) + none)
    for leaf in ('ToyFire', 'ToyIce'): p.add_export(imp, leaf, w32(0) + none, outer=group)
    return p


def packed(index, length): return (index << 16) | length


def build_provider(block_variant='exact', extra_type='BVAR_InstanceData', a_link='BVARLINK_Input'):
    """ToyProvider: sequence Main (OnHit -> Cmp -> Same/Diff) and sequence Extra (disabled, two variables)."""
    p = Package()
    t = Tags(p)
    chains = {}

    def chain(*path):
        if path in chains: return chains[path]
        outer = chain(*path[:-1]) if len(path) > 1 else 0
        chains[path] = p.add_import_full('Package' if len(path) == 1 else 'Class', outer, path[-1])
        return chains[path]

    fire = chain('Things', 'DamageType', 'ToyFire')
    provider = p.add_export(chain('GearboxFramework', 'BehaviorProviderDefinition'), 'ToyProvider', b'')   # payload set below
    none = t.none()
    cmp_ = p.add_export(chain('WillowGame', 'Behavior_CompareObject'), 'Cmp', w32(0) + none, outer=provider)
    same = p.add_export(chain('WillowGame', 'Behavior_ToySame'), 'Same', w32(0) + none, outer=provider)
    diff = p.add_export(chain('WillowGame', 'Behavior_ToyDifferent'), 'Diff', w32(0) + none, outer=provider)
    holder = p.add_export(chain('WillowGame', 'ToyHolder'), 'Holder1',
                          w32(0) + t.struct_('Holder', 'ToyScaled', t.float('Base', 2.0) + none) + none)

    def sub(name, index, length): return t.struct_(name, 'SubarrayData', t.int('ArrayIndexAndLength', packed(index, length)) + none)

    def var(name, kind): return t.name_('Name', name) + t.enum('Type', 'EBehaviorVariableType', kind) + none

    def vlink(prop_name, kind, first, count):
        return (t.name_('PropertyName', prop_name) + t.enum('VariableLinkType', 'EBehaviorVariableLinkType', kind)
                + t.byte('ConnectionIndex', 0) + sub('LinkedVariables', first, count) + none)

    def link(behavior, link_id): return t.int('LinkIdAndLinkedBehavior', behavior | (link_id << 24) if link_id < 128 else
                                              behavior - (1 << 32) + (link_id << 24)) + t.float('ActivateDelay', 0.0) + none

    main = (t.name_('BehaviorSequenceName', 'Main') + t.bool('bEnabledOnSpawn', True)
            + t.array('EventData2', 1, t.struct_('UserData', 'BehaviorEventUserData', t.name_('EventName', 'OnHit') + none)
                      + sub('OutputVariables', 0, 1) + sub('OutputLinks', 0, 1) + none)
            + t.array('BehaviorData2', 3,
                      t.obj('Behavior', cmp_) + sub('LinkedVariables', 1, 2) + sub('OutputLinks', 1, 2) + none
                      + t.obj('Behavior', same) + sub('LinkedVariables', 0, 0) + sub('OutputLinks', 0, 0) + none
                      + t.obj('Behavior', diff) + sub('LinkedVariables', 0, 0) + sub('OutputLinks', 0, 0) + none)
            + t.array('VariableData', 5, var('None', 'BVAR_Object') + var('None', 'BVAR_Object') + var('Count', 'BVAR_Int')
                      + var('Ref', 'BVAR_NamedVariable') + var('None', 'BVAR_Float'))
            + t.array('ConsolidatedOutputLinkData', 3, link(0, 255) + link(1, 0) + link(2, 1))
            + t.array('ConsolidatedVariableLinkData', 3, vlink('DamageType', 'BVARLINK_Output', 0, 1)
                      + vlink('ObjectA', a_link, 1, 1) + vlink('ObjectB', 'BVARLINK_Input', 2, 1))
            + t.array('ConsolidatedLinkedVariables', 3, w32(0, 0, 1))
            + none)
    extra = (t.name_('BehaviorSequenceName', 'Extra') + t.bool('bEnabledOnSpawn', False)
             + t.array('VariableData', 2, var('None', extra_type) + var('None', 'BVAR_Object'))
             + none)
    tags = w32(0) + t.array('BehaviorSequences', 2, main + extra) + none
    # Value block: Main = Object 0, Object ToyFire, Int 7, NamedVariable (no bytes), Float 1.5; Extra = InstanceData
    # (word, FName ToySwitch) and Object 0.
    block = w32(0, fire, 7) + f32(1.5) + w32(0) + p.fname('ToySwitch') + w32(0)
    if block_variant == 'short': block = block[:-4]
    if block_variant == 'long': block += w32(0)
    cls, sup, outer, name, _ = p.exports[provider - 1]
    p.exports[provider - 1] = (cls, sup, outer, name, tags + block)
    return p, fire


# ------------------------------------------------------------------------------------------------- harness
failures = []


def check(label, condition, detail=''):
    if not condition: failures.append(f'{label}: {detail}')


def run(root, mode, target, *steps):
    if mode == '--object-dump':   # --object-dump <index> <prefix> --cooked <dir> --all
        args = [mode, target, '4', '--cooked', str(root), '--all']
    else:                         # --behavior-dump / --behavior-run <provider> --cooked <dir> <step>...
        args = [mode, target, '--cooked', str(root), *steps]
    proc = subprocess.run([reader, str(root / 'TestBpd.upk'), *args], capture_output=True, text=True, encoding='utf-8')
    assert proc.stdout.strip(), ('no JSON output', args, proc.returncode, proc.stderr)
    return proc.returncode, json.loads(proc.stdout)


def write(root, **variant):
    package, _ = build_provider(**variant)
    (root / 'TestBpd.upk').write_bytes(package.build())


with tempfile.TemporaryDirectory() as folder:
    root = Path(folder)
    (root / 'GearboxFramework.upk').write_bytes(build_gearbox().build())
    (root / 'WillowGame.upk').write_bytes(build_willowgame().build())
    (root / 'Things.upk').write_bytes(build_things().build())
    FIRE, ICE = 'Things.DamageType.ToyFire', 'Things.DamageType.ToyIce'

    # (1) the value block decodes exactly; enum bytes declared in GearboxFramework name the right values.
    write(root)
    code, got = run(root, '--behavior-dump', 'ToyProvider')
    check('1 dump exit', code == 0, (code, got))
    check('1 decoded', got['values_decoded'] is True and got['diagnostics'] == [], got)
    main, extra = got['sequences']
    check('1 main variables', [(v['type'], v['name'], v['object']) for v in main['variables']] == [
        ('BVAR_Object', 'None', ''), ('BVAR_Object', 'None', FIRE), ('BVAR_Int', 'Count', ''),
        ('BVAR_NamedVariable', 'Ref', ''), ('BVAR_Float', 'None', '')], main['variables'])
    check('1 int and float words', main['variables'][2]['word'] == 7 and
          main['variables'][4]['word'] == struct.unpack('<i', f32(1.5))[0], main['variables'])
    check('1 extra variables', [v['type'] for v in extra['variables']] == ['BVAR_InstanceData', 'BVAR_Object'] and
          extra['enabled_on_spawn'] is False, extra)
    check('1 variable links', main['behavior_inputs'] == [
        dict(behavior='Cmp', property='ObjectA', link='BVARLINK_Input', variables=[0]),
        dict(behavior='Cmp', property='ObjectB', link='BVARLINK_Input', variables=[1])], main['behavior_inputs'])

    # (2) CompareObject: ObjectA = the OnHit DamageType output, ObjectB = the ToyFire constant.
    SAME = ['OnHit -> Cmp', 'OnHit -> Same']
    DIFF = ['OnHit -> Cmp', 'OnHit -> Diff']
    code, got = run(root, '--behavior-run', 'ToyProvider', f'event:OnHit:DamageType={FIRE}')
    check('2a same object -> link id 0', code == 0 and got['trace'] == SAME and
          got['boundary'] == ['OnHit -> WillowGame.Behavior_ToySame:Same'] and got['errors'] == [], got)
    code, got = run(root, '--behavior-run', 'ToyProvider', f'event:OnHit:DamageType={ICE}')
    check('2b other object -> link id 1', code == 0 and got['trace'] == DIFF, got)
    code, got = run(root, '--behavior-run', 'ToyProvider', 'event:OnHit')
    check('2c no output supplied (None) -> link id 1', code == 0 and got['trace'] == DIFF, got)
    code, got = run(root, '--behavior-run', 'ToyProvider', f'event:OnHit:DamageType={ICE}', f'event:OnHit:DamageType={FIRE}')
    check('2d outputs are rewritten per event', code == 0 and got['trace'] == DIFF + SAME, got)
    code, got = run(root, '--behavior-run', 'ToyProvider', 'disable:Main', f'event:OnHit:DamageType={FIRE}')
    check('2e disabled sequence does not run', code == 0 and got['trace'] == [], got)

    # (3) a block that does not decode exactly is not trusted, and reading an input from it is an error.
    for variant, message in (('short', 'variable value block shorter than its variables in ToyProvider'),
                             ('long', 'variable value block size mismatch in ToyProvider')):
        write(root, block_variant=variant)
        code, got = run(root, '--behavior-dump', 'ToyProvider')
        check(f'3 {variant} dump', code == 1 and got['values_decoded'] is False and got['diagnostics'] == [message], got)
        code, got = run(root, '--behavior-run', 'ToyProvider', f'event:OnHit:DamageType={FIRE}')
        check(f'3 {variant} run', code == 1 and got['trace'] == ['OnHit -> Cmp'] and
              got['errors'] == ['variable values not decoded for Cmp.ObjectA in ToyProvider',
                                'variable values not decoded for Cmp.ObjectB in ToyProvider'], got)
    write(root, extra_type='BVAR_Mystery')
    code, got = run(root, '--behavior-dump', 'ToyProvider')
    check('3 unknown type', code == 1 and got['values_decoded'] is False and
          got['diagnostics'] == ['unknown variable type BVAR_Mystery in ToyProvider'], got)

    # A link type that is neither Input nor Output is an error, never a fall back to the behavior's own (None) property.
    write(root, a_link='BVARLINK_Unknown')
    code, got = run(root, '--behavior-run', 'ToyProvider', f'event:OnHit:DamageType={ICE}')
    check('3 unexpected link type', code == 1 and got['trace'] == ['OnHit -> Cmp'] and
          got['errors'] == ['unexpected variable link type BVARLINK_Unknown for Cmp.ObjectA'], got)

    # (4) struct defaults: Holder sets Base only, so Scale comes from ToyScaled's default tags; Other is absent.
    write(root)
    holder_index = 5
    code, got = run(root, '--object-dump', str(holder_index))
    props = got['properties']
    check('4a omitted field takes the struct default', code == 0 and props['holder'] == {'Base': 2, 'Scale': 1}, props)
    check('4b absent struct property takes the struct default', props['other'] == {'Base': 0, 'Scale': 1}, props)
    (root / 'WillowGame.upk').write_bytes(build_willowgame(broken_defaults=True).build())
    code, got = run(root, '--object-dump', str(holder_index))
    props = got['properties']
    check('4c defaults not consumed exactly are ignored', code == 0 and props['holder'] == {'Base': 2, 'Scale': 0} and
          props['other'] == {'Base': 0, 'Scale': 0}, props)

if failures:
    print(f'{len(failures)} behavior check(s) failed:')
    for failure in failures: print('  - ' + failure)
    sys.exit(1)
print('behavior synthetic coverage passed.')
