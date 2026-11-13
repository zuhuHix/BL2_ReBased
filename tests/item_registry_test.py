"""Synthetic test for tools/item_registry.py: invented packages read by the real ow-package, no game files or values.

AI-assisted (Claude Code). Builds a fake install (WillowGame/CookedPCConsole plus one DLC folder) of tiny packages with the
same header, name, import and export layout and the same tag() encoding as tests/properties_test.py and
tests/global_resolve_test.py (those helpers run at import, so they are restated here for many exports with classes). Every
object name and number is invented. Run: python tests/item_registry_test.py <ow-package>
"""
import contextlib
import io
import struct
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import item_registry as ir  # noqa: E402

READER = None


def ints(*values):
    return struct.pack('<' + 'i' * len(values), *values)


class Ref:
    def __init__(self, path):
        self.path = path


class Enum:
    def __init__(self, enum, value):
        self.enum, self.value = enum, value


class Struct:
    def __init__(self, name, fields):
        self.name, self.fields = name, fields


class Array(list):
    pass


class Package:
    """One synthetic package. Exports are outermost objects whose own name holds the dotted path (as global_resolve_test
    shows, the reader gives such an export that same path); a Ref to a path this package does not export becomes an import."""

    def __init__(self):
        self.names, self.imports, self.exports = ['None'], [], []
        self.export_index = {}

    def fname(self, text):
        if text not in self.names:
            self.names.append(text)
        return ints(self.names.index(text), 0)

    def import_ref(self, class_package, class_name, outer, name):
        key = (class_package, class_name, outer, name)
        if key not in self.imports:
            self.imports.append(key)
        return -(self.imports.index(key) + 1)

    def class_ref(self, qualified):
        package, name = qualified.split('.')
        return self.import_ref('Core', 'Class', self.import_ref('Core', 'Package', 0, package), name)

    def add(self, path, cls, props=()):
        self.export_index[path] = len(self.exports) + 1
        self.exports.append((path, cls, list(props)))

    def ref(self, path):
        if path is None:
            return 0
        if path in self.export_index:
            return self.export_index[path]
        return self.import_ref('Core', 'Object', 0, path)

    def tag(self, name, kind, data, extra=b'', index=0):
        return self.fname(name) + self.fname(kind) + ints(len(data), index) + extra + data

    def value(self, name, value):
        if isinstance(value, bool):
            return self.fname(name) + self.fname('BoolProperty') + ints(0, 0) + (b'\x01' if value else b'\x00')
        if isinstance(value, int):
            return self.tag(name, 'IntProperty', ints(value))
        if isinstance(value, float):
            return self.tag(name, 'FloatProperty', struct.pack('<f', value))
        if isinstance(value, Ref):
            return self.tag(name, 'ObjectProperty', ints(self.ref(value.path)))
        if isinstance(value, Enum):
            return self.tag(name, 'ByteProperty', self.fname(value.value), self.fname(value.enum))
        if isinstance(value, Struct):
            return self.tag(name, 'StructProperty', self.body(value.fields), self.fname(value.name))
        if isinstance(value, Array):
            data = ints(len(value))
            for element in value:
                data += self.body(element.fields) if isinstance(element, Struct) else ints(self.ref(element.path))
            return self.tag(name, 'ArrayProperty', data)
        raise TypeError(value)

    def body(self, fields):
        return b''.join(self.value(n, v) for n, v in fields) + self.fname('None')

    def save(self, path):
        payloads = [ints(0) + self.body(props) for _, _, props in self.exports]  # 4 bytes before the tags (offset 4)
        for name, cls, _ in self.exports:  # every name and import exists before the tables are laid out
            self.class_ref(cls)
            self.fname(name)
        import_data = b''.join(self.fname(cp) + self.fname(cn) + ints(outer) + self.fname(name)
                               for cp, cn, outer, name in self.imports)
        table = b''.join(ints(len(n) + 1) + n.encode() + b'\0' + bytes(8) for n in self.names)
        import_offset = 48 + len(table)
        eo = import_offset + len(import_data)
        offset = eo + 68 * len(self.exports)
        export_data = b''
        for (name, cls, _), payload in zip(self.exports, payloads):
            export_data += ints(self.class_ref(cls), 0, 0) + self.fname(name) + ints(0) + bytes(8)
            export_data += ints(len(payload), offset, 0, 0) + bytes(20)
            offset += len(payload)
        header = struct.pack('<II', 0x9e2a83c1, 832 | (46 << 16))
        header += ints(48, 0, 0, len(self.names), 48, len(self.exports), eo, len(self.imports), import_offset, 0)
        Path(path).parent.mkdir(parents=True, exist_ok=True)
        Path(path).write_bytes(header + table + import_data + export_data + b''.join(payloads))


def const(value):
    return Struct('AttributeInitializationData', [('BaseValueConstant', float(value)), ('BaseValueScaleConstant', 1.0)])


def weighted(part, low=0, high=1, weight=None, makers=()):
    """A WeightedPart: stage window from ConsolidatedAttributeInitData[low/high]; makers [(manufacturer, weight index)]."""
    fields = [('Part', Ref(part)),
              ('Manufacturers', Array(Struct('ManufacturerWeight', [('Manufacturer', Ref(m)), ('DefaultWeightIndex', i)])
                                      for m, i in makers)),
              ('MinGameStageIndex', low), ('MaxGameStageIndex', high)]
    if weight is not None:
        fields.append(('DefaultWeightIndex', weight))
    return Struct('WeightedPart', fields)


def part_data(rows, enabled=True):
    return Struct('PartData', [('bEnabled', enabled), ('WeightedParts', Array(rows))])


def grade(low, high, definition=None):
    return Struct('GradeData', [
        ('GradeModifiers', Struct('Mods', [('ExpLevel', 1), ('CustomInventoryDefinition', Ref(definition))])),
        ('GameStageRequirement', Struct('Need', [('MinGameStage', low), ('MaxGameStage', high)]))])


def makers(*rows):
    return Array(Struct('ManufacturerGradeData', [('Manufacturer', Ref(m)), ('Grades', Array(g))]) for m, g in rows)


CONSOLIDATED = ('ConsolidatedAttributeInitData', Array([const(1), const(100), const(5), const(30), const(7)]))
ALPHA, BETA = 'GD_Syn.Makers.Alpha', 'GD_Syn.Makers.Beta'
W = 'WillowGame.'


def base_package():
    p = Package()
    p.add('Default__ShieldDefinition', W + 'ShieldDefinition')  # a class default object: skipped and counted
    p.add(ALPHA, 'Engine.ManufacturerDefinition')
    p.add(BETA, 'Engine.ManufacturerDefinition')
    p.add('GD_Syn.Names.Title_Shield', W + 'ItemNamePartDefinition', [('PartName', 1)])
    p.add('GD_Syn.Names.Title_Gun', W + 'WeaponNamePartDefinition')
    p.add('GD_Syn.Gestalt.Shield', 'GearboxFramework.GestaltSkeletalMeshDefinition')
    # Shield: a definition list for Alpha, an additive collection on the base balance (Alpha + Beta, Beta names a part no
    # package exports), a selective collection on the leaf that replaces Alpha.
    for name in ('Body_A', 'Body_B', 'Cap_A', 'Cap_B'):
        p.add(f'GD_Syn.Shield.{name}', W + 'ShieldPartDefinition',
              [('TitleList', Array([Ref('GD_Syn.Names.Title_Shield')]))] if name == 'Body_A' else
              [('TitleList', Array([Ref(None)]))] if name == 'Cap_A' else [])  # an empty name entry: counted, left out
    p.add('GD_Syn.Shield.List_Alpha', W + 'ItemPartListDefinition', [
        ('WeightedParts', Array([weighted('GD_Syn.Shield.Body_A'),
                                 weighted('GD_Syn.Shield.Body_B', low=2, high=3, weight=0, makers=[(ALPHA, 4)])])),
        CONSOLIDATED])
    p.add('GD_Syn.Shield.Def', W + 'ShieldDefinition', [
        ('AlphaParts', Ref('GD_Syn.Shield.List_Alpha')), ('GestaltMesh', Ref('GD_Syn.Gestalt.Shield'))])
    p.add('GD_Syn.Shield.Coll_Add', W + 'ItemPartListCollectionDefinition', [
        ('AlphaPartData', part_data([weighted('GD_Syn.Shield.Cap_A')])),
        ('BetaPartData', part_data([weighted('GD_Syn.Shield.Cap_B'), weighted('GD_Syn.Shield.Ghost')])),
        ('GammaPartData', part_data([weighted('GD_Syn.Shield.Cap_A')], enabled=False)),
        ('IotaPartData', part_data([weighted('GD_Syn.Shield.Cap_A')])),  # a slot outside the nine: unlisted_classes
        ('PartReplacementMode', Enum('EPartReplacementMode', 'EPRM_Additive')), CONSOLIDATED])
    p.add('GD_Syn.Shield.Coll_Sel', W + 'ItemPartListCollectionDefinition', [
        ('AlphaPartData', part_data([weighted('GD_Syn.Shield.Body_B', low=2, high=3, weight=0, makers=[(ALPHA, 4)])])),
        ('PartReplacementMode', Enum('EPartReplacementMode', 'EPRM_Selective')), CONSOLIDATED])
    p.add('GD_Syn.Grades.Shield_Base', W + 'InventoryBalanceDefinition', [
        ('InventoryDefinition', Ref('GD_Syn.Shield.Def')),
        ('Manufacturers', makers((ALPHA, [grade(1, 4), grade(5, 50)]))),
        ('PartListCollection', Ref('GD_Syn.Shield.Coll_Add'))])
    p.add('GD_Syn.Grades.Shield_Leaf', W + 'InventoryBalanceDefinition', [
        ('BaseDefinition', Ref('GD_Syn.Grades.Shield_Base')), ('PartListCollection', Ref('GD_Syn.Shield.Coll_Sel'))])
    # One balance of every other item kind.
    p.add('GD_Syn.Grenade.Part', W + 'GrenadeModPartDefinition')
    p.add('GD_Syn.Grenade.Part2', W + 'GrenadeModPartDefinition')
    p.add('GD_Syn.Grenade.List', W + 'ItemPartListDefinition', [
        ('WeightedParts', Array([weighted('GD_Syn.Grenade.Part'), weighted(None)])), CONSOLIDATED])  # a row naming no part
    p.add('GD_Syn.Grenade.Coll', W + 'ItemPartListCollectionDefinition', [  # no PartReplacementMode: additive
        ('DeltaPartData', part_data([weighted('GD_Syn.Grenade.Part2')])), CONSOLIDATED])
    p.add('GD_Syn.Grenade.Def', W + 'GrenadeModDefinition', [('DeltaParts', Ref('GD_Syn.Grenade.List'))])
    p.add('GD_Syn.Grades.Grenade', W + 'InventoryBalanceDefinition', [
        ('InventoryDefinition', Ref('GD_Syn.Grenade.Def')), ('Manufacturers', makers((BETA, [grade(1, 100)]))),
        ('PartListCollection', Ref('GD_Syn.Grenade.Coll'))])
    p.add('GD_Syn.ClassMod.Part', W + 'ClassModPartDefinition')
    p.add('GD_Syn.ClassMod.Def_A', W + 'ClassModDefinition')
    p.add('GD_Syn.ClassMod.Def_B', W + 'ClassModDefinition')
    p.add('GD_Syn.ClassMod.Bal.PartList', W + 'ItemPartListCollectionDefinition', [
        ('BetaPartData', part_data([weighted('GD_Syn.ClassMod.Part')])),
        ('PartReplacementMode', Enum('EPartReplacementMode', 'EPRM_Selective')), CONSOLIDATED])
    p.add('GD_Syn.ClassMod.Bal', W + 'ClassModBalanceDefinition', [
        ('ClassModDefinitions', Array([Ref('GD_Syn.ClassMod.Def_A'), Ref('GD_Syn.ClassMod.Def_B')])),
        ('ItemPartListCollection', Ref('GD_Syn.ClassMod.Bal.PartList')),
        ('Manufacturers', makers((ALPHA, [grade(1, 100)]), (BETA, [grade(1, 100)])))])
    p.add('GD_Syn.Relic.Part', W + 'ArtifactPartDefinition')
    p.add('GD_Syn.Relic.List', W + 'ItemPartListDefinition', [('WeightedParts', Array([weighted('GD_Syn.Relic.Part')])),
                                                             CONSOLIDATED])
    p.add('GD_Syn.Relic.Def', W + 'ArtifactDefinition', [('EtaParts', Ref('GD_Syn.Relic.List'))])
    p.add('GD_Syn.Grades.Relic', W + 'InventoryBalanceDefinition', [
        ('InventoryDefinition', Ref('GD_Syn.Relic.Def')), ('Manufacturers', makers((ALPHA, [grade(1, 100)])))])
    p.add('GD_Syn.Usable.Def', W + 'UsableItemDefinition')
    p.add('GD_Syn.Grades.Usable', W + 'InventoryBalanceDefinition', [('InventoryDefinition', Ref('GD_Syn.Usable.Def'))])
    p.add('GD_Syn.Custom.Def', W + 'UsableCustomizationItemDefinition')
    p.add('GD_Syn.Grades.Custom', W + 'InventoryBalanceDefinition', [('InventoryDefinition', Ref('GD_Syn.Custom.Def'))])
    p.add('GD_Syn.Mission.Def', W + 'MissionItemDefinition')
    p.add('GD_Syn.Mission.Unused', W + 'MissionItemDefinition')  # no balance names it
    p.add('GD_Syn.Grades.Mission', W + 'InventoryBalanceDefinition', [('InventoryDefinition', Ref('GD_Syn.Mission.Def'))])
    # Weapon: one type, two barrels, one collection, one balance.
    p.add('GD_Syn.Gun.Type', W + 'WeaponTypeDefinition', [('TitleList', Array([Ref('GD_Syn.Names.Title_Gun')]))])
    p.add('GD_Syn.Gun.Barrel_A', W + 'WeaponPartDefinition')
    p.add('GD_Syn.Gun.Barrel_B', W + 'WeaponPartDefinition')
    p.add('GD_Syn.Gun.Coll', W + 'WeaponPartListCollectionDefinition', [
        ('BarrelPartData', part_data([weighted('GD_Syn.Gun.Barrel_A'),
                                      weighted('GD_Syn.Gun.Barrel_B', weight=0, makers=[(BETA, 4)])])),
        ('PartReplacementMode', Enum('EPartReplacementMode', 'EPRM_Selective')), CONSOLIDATED])
    p.add('GD_Syn.Gun.Grip_A', W + 'WeaponPartDefinition')
    p.add('GD_Syn.Gun.Coll_Complete', W + 'WeaponPartListCollectionDefinition', [
        ('GripPartData', part_data([weighted('GD_Syn.Gun.Grip_A')])),
        ('PartReplacementMode', Enum('EPartReplacementMode', 'EPRM_Complete')), CONSOLIDATED])
    p.add('GD_Syn.Gun.Complete', W + 'WeaponBalanceDefinition', [
        ('BaseDefinition', Ref('GD_Syn.Gun.Balance')), ('WeaponPartListCollection', Ref('GD_Syn.Gun.Coll_Complete'))])
    p.add('GD_Syn.Odd.Balance', W + 'SynthOddBalanceDefinition')
    p.add('GD_Syn.Gun.Sight_A', W + 'WeaponPartDefinition')
    p.add('GD_Syn.Gun.Coll_Turret', W + 'WeaponPartListCollectionDefinition', [
        ('SightPartData', part_data([weighted('GD_Syn.Gun.Sight_A')])),
        ('PartReplacementMode', Enum('EPartReplacementMode', 'EPRM_Selective')), CONSOLIDATED])
    p.add('GD_Syn.Grades.Turret', W + 'InventoryBalanceDefinition', [  # an inventory balance naming a weapon type
        ('InventoryDefinition', Ref('GD_Syn.Gun.Type')), ('PartListCollection', Ref('GD_Syn.Gun.Coll_Turret'))])  # a balance class the registry does not read
    p.add('GD_Syn.Gun.Balance', W + 'WeaponBalanceDefinition', [
        ('InventoryDefinition', Ref('GD_Syn.Gun.Type')), ('WeaponPartListCollection', Ref('GD_Syn.Gun.Coll')),
        ('Manufacturers', makers((BETA, [grade(1, 100)])))])
    # Chains that do not reach a root: a base no package holds, and a two-step cycle.
    p.add('GD_Syn.Grades.Orphan', W + 'InventoryBalanceDefinition', [('BaseDefinition', Ref('GD_Syn.Grades.Nowhere'))])
    p.add('GD_Syn.Grades.Loop_A', W + 'InventoryBalanceDefinition', [('BaseDefinition', Ref('GD_Syn.Grades.Loop_B'))])
    p.add('GD_Syn.Grades.Loop_B', W + 'InventoryBalanceDefinition', [('BaseDefinition', Ref('GD_Syn.Grades.Loop_A'))])
    return p


def dlc_package():
    p = Package()
    p.add(ALPHA, 'Engine.ManufacturerDefinition')  # the same path again: the base game package owns it
    p.add('GD_Syn.Grades.Usable', W + 'ItemPoolDefinition')  # the same path with another class
    p.add('GD_SynDLC.Grades.Shield', W + 'InventoryBalanceDefinition', [
        ('BaseDefinition', Ref('GD_Syn.Grades.Shield_Base')),  # an import: the base lives in the base game's package
        ('Manufacturers', makers((BETA, [grade(1, 100, definition='GD_Syn.Shield.Def')])))])
    return p


class RegistryTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.folder = tempfile.TemporaryDirectory()
        game = Path(cls.folder.name) / 'Game'
        base_package().save(game / 'WillowGame' / 'CookedPCConsole' / 'SynStartup.upk')
        dlc_package().save(game / 'DLC' / 'SynDLC' / 'Content' / 'SynDLC_Startup.upk')
        schema = Path(cls.folder.name) / 'registry.schema'
        schema.write_text('\n'.join(ir.SCHEMA_LINES) + '\n', encoding='utf-8')
        cls.game, cls.schema = game, schema
        cls.census, dlc_by_file, cls.errors = ir.open_census(READER, game, str(schema), jobs=2)
        cls.registry, cls.report = ir.build(cls.census, dlc_by_file)

    @classmethod
    def tearDownClass(cls):
        cls.folder.cleanup()

    def test_one_balance_per_kind(self):
        kinds = {path: b['kind'] for path, b in self.registry['balances'].items()}
        self.assertEqual(self.errors, {})
        for path, kind in [('GD_Syn.Grades.Shield_Leaf', 'shield'), ('GD_Syn.Grades.Grenade', 'grenade_mod'),
                           ('GD_Syn.ClassMod.Bal', 'class_mod'), ('GD_Syn.Grades.Relic', 'relic'),
                           ('GD_Syn.Grades.Usable', 'usable'), ('GD_Syn.Grades.Custom', 'customization'),
                           ('GD_Syn.Grades.Mission', 'mission_item'), ('GD_Syn.Gun.Balance', 'weapon')]:
            self.assertEqual(kinds[path], kind, path)
        self.assertEqual(self.registry['schema'], ir.SCHEMA_TAG)

    def test_base_chain_and_merge(self):
        leaf = self.registry['balances']['GD_Syn.Grades.Shield_Leaf']
        self.assertEqual(leaf['chain'], ['GD_Syn.Grades.Shield_Leaf', 'GD_Syn.Grades.Shield_Base'])
        self.assertEqual(leaf['chain_end'], 'root')
        self.assertEqual(leaf['inventory_definition'], 'GD_Syn.Shield.Def')
        self.assertEqual(leaf['gestalt'], 'GD_Syn.Gestalt.Shield')
        slots = leaf['variants'][0]['slots']
        # Definition list (Body_A, Body_B), the additive collection appends Cap_A, the selective one replaces Alpha.
        self.assertEqual([c['part'] for c in slots['Alpha']], ['GD_Syn.Shield.Body_B'])
        self.assertEqual([c['part'] for c in slots['Beta']], ['GD_Syn.Shield.Cap_B', 'GD_Syn.Shield.Ghost'])
        self.assertNotIn('Gamma', slots)  # a disabled slot adds nothing
        self.assertEqual([m['mode'] for m in leaf['variants'][0]['merge']],
                         ['definition', 'EPRM_Additive', 'EPRM_Selective'])
        base = self.registry['balances']['GD_Syn.Grades.Shield_Base']['variants'][0]['slots']
        self.assertEqual([c['part'] for c in base['Alpha']],
                         ['GD_Syn.Shield.Body_A', 'GD_Syn.Shield.Body_B', 'GD_Syn.Shield.Cap_A'])

    def test_weights_windows_and_manufacturers(self):
        leaf = self.registry['balances']['GD_Syn.Grades.Shield_Leaf']
        body_b = leaf['variants'][0]['slots']['Alpha'][0]
        self.assertEqual(body_b['weight'], {'source': 'manufacturer', 'by_manufacturer': {ALPHA: 7.0}, 'default': 1.0})
        self.assertEqual((body_b['min_stage'], body_b['max_stage']), (5.0, 30.0))
        flat = leaf['variants'][0]['slots']['Beta'][0]
        self.assertEqual(flat['weight'], {'source': 'flat', 'value': 100.0})
        self.assertEqual(leaf['manufacturers'][0]['manufacturer'], ALPHA)  # inherited from the base balance
        self.assertEqual([(g['min_stage'], g['max_stage']) for g in leaf['manufacturers'][0]['grades']], [(1, 4), (5, 50)])
        at = ir.candidates_at(self.census, leaf['variants'][0]['slots'], 10, ALPHA)
        self.assertEqual(at['Alpha'], {'GD_Syn.Shield.Body_B': 7.0})
        self.assertNotIn('Alpha', ir.candidates_at(self.census, leaf['variants'][0]['slots'], 3, ALPHA))  # below its window
        self.assertEqual(ir.candidates_at(self.census, leaf['variants'][0]['slots'], 10, BETA)['Alpha'],
                         {'GD_Syn.Shield.Body_B': 1.0})  # another maker falls back to DefaultWeight

    def test_class_mod_variants_and_names(self):
        record = self.registry['balances']['GD_Syn.ClassMod.Bal']
        self.assertEqual([v['definition'] for v in record['variants']], ['GD_Syn.ClassMod.Def_A', 'GD_Syn.ClassMod.Def_B'])
        self.assertEqual([m['manufacturer'] for m in record['manufacturers']], [ALPHA, BETA])
        self.assertEqual(self.registry['balances']['GD_Syn.Grades.Shield_Base']['name_parts']['title'],
                         ['GD_Syn.Names.Title_Shield'])
        self.assertEqual(self.registry['balances']['GD_Syn.Gun.Balance']['name_parts']['title'], ['GD_Syn.Names.Title_Gun'])

    def test_census_reports_missing_and_broken_chains(self):
        unresolved = self.report['unresolved']
        self.assertEqual([r['path'] for r in unresolved['part']], ['GD_Syn.Shield.Ghost'])
        self.assertEqual(unresolved['part'][0]['reason'], 'no installed package exports it')
        balances = self.registry['balances']
        self.assertEqual((balances['GD_Syn.Grades.Orphan']['chain_end'], balances['GD_Syn.Grades.Orphan']['chain_missing']),
                         ('missing', 'GD_Syn.Grades.Nowhere'))
        self.assertEqual(balances['GD_Syn.Grades.Loop_A']['chain_end'], 'cycle')
        self.assertEqual(self.report['definitions_without_balance_by_kind'], {'mission_item': 1})
        self.assertEqual(self.report['skipped'], {
            'ShieldDefinition: class default object (Default__), not a game object': 1})

    def test_duplicate_paths_and_dlc(self):
        self.assertEqual(self.report['paths_in_several_packages_by_class'],
                         {'InventoryBalanceDefinition': 1, 'ManufacturerDefinition': 1})
        self.assertEqual([c['path'] for c in self.report['paths_with_two_classes']], ['GD_Syn.Grades.Usable'])
        self.assertEqual(self.registry['manufacturers'][ALPHA]['dlc'], 'base')
        dlc = self.registry['balances']['GD_SynDLC.Grades.Shield']
        self.assertEqual(dlc['dlc'], 'SynDLC')
        self.assertEqual(dlc['chain'], ['GD_SynDLC.Grades.Shield', 'GD_Syn.Grades.Shield_Base'])
        self.assertEqual(dlc['chain_end'], 'root')
        self.assertEqual(dlc['kind'], 'shield')
        self.assertEqual(dlc['manufacturers'][0]['grades'][0]['definition'], 'GD_Syn.Shield.Def')
        self.assertEqual(self.report['by_dlc']['SynDLC'], {'balances_shield': 1})

    def test_agrees_with_the_existing_tools(self):
        weapons = ir.check_weapons(self.census, self.registry)
        self.assertEqual(weapons['counts'], {'equal': 6})
        items = ir.check_items(self.census, self.registry)
        self.assertEqual(items['different'], [])
        # Something with parts was compared, and the part-less balances are counted apart.
        self.assertGreater(items['counts'].get('equal', 0), 0)
        self.assertGreater(items['counts'].get('equal_both_empty', 0), 0)
        shield = ir.check_stock_shield(self.census, self.registry, balance='GD_Syn.Grades.Shield_Base', level=10)
        self.assertTrue(shield['equal'])
        # Alpha: Body_A, Body_B (stage 10 is inside 5..30), Cap_A; Beta: Cap_B (Ghost weighs 100 but is unresolved, kept).
        self.assertEqual(shield['parts_per_slot'], {'Alpha': 3, 'Beta': 2})
        self.assertEqual(shield['combinations'], 6)

    def test_modes_and_skipped_rows(self):
        balances = self.registry['balances']
        grenade = balances['GD_Syn.Grades.Grenade']['variants'][0]
        self.assertEqual([c['part'] for c in grenade['slots']['Delta']], ['GD_Syn.Grenade.Part', 'GD_Syn.Grenade.Part2'])
        self.assertEqual([m['mode'] for m in grenade['merge']], ['definition', 'EPRM_Additive'])
        complete = balances['GD_Syn.Gun.Complete']['variants'][0]
        self.assertEqual({s: [c['part'] for c in e] for s, e in complete['slots'].items()}, {'Grip': ['GD_Syn.Gun.Grip_A']})
        self.assertEqual(self.report['part_list_rows_naming_no_part']['rows'], 1)
        self.assertEqual(self.report['name_list_entries_empty']['entries'], 1)
        self.assertEqual(self.registry['parts']['GD_Syn.Shield.Cap_A']['title'], [])

    def test_weapon_list_under_an_inventory_balance(self):
        turret = self.registry['balances']['GD_Syn.Grades.Turret']
        self.assertEqual(turret['kind'], 'weapon_by_inventory_balance')
        self.assertEqual({s: [c['part'] for c in e] for s, e in turret['variants'][0]['slots'].items()},
                         {'Sight': ['GD_Syn.Gun.Sight_A']})

    def test_unlisted_classes(self):
        self.assertEqual(self.report['unlisted_classes'], {'export_classes': {'SynthOddBalanceDefinition': 1},
                                                           'part_data_properties': {'IotaPartData': 1}})
        self.assertNotIn('GD_Syn.Odd.Balance', self.registry['balances'])

    def test_same_bytes_twice(self):
        self.assertNotIn('generated', self.registry)
        census, dlc_by_file, _ = ir.open_census(READER, self.game, str(self.schema), jobs=2)
        again, _ = ir.build(census, dlc_by_file)
        first, second = Path(self.folder.name) / 'first.json', Path(self.folder.name) / 'second.json'
        ir.write_json(first, self.registry)
        ir.write_json(second, again)
        self.assertEqual(first.read_bytes(), second.read_bytes())

    def test_output_stays_in_local(self):
        outside = Path(self.folder.name) / 'out'
        with contextlib.redirect_stderr(io.StringIO()) as said:
            self.assertEqual(ir.main(['--reader', READER, '--game', self.folder.name, '--out', str(outside)]), 2)
        self.assertIn('output must be inside', said.getvalue())
        self.assertFalse(outside.exists())


if __name__ == '__main__':
    if len(sys.argv) < 2:
        sys.exit('usage: python tests/item_registry_test.py <ow-package>')
    READER = str(Path(sys.argv.pop(1)).resolve())
    unittest.main()
