"""Differential gate for the Rust item registry: tools/item_registry.py against `ow-package --item-registry`.

AI-assisted (Claude Code). Lane W4-REGISTRY, review r1 of PR #310. Builds an invented install (the packages of
tests/item_registry_test.py, plus parts with titles in two slots whose sorted order differs from the slot order, for a
weapon and for an item, and one DLC file that is not a package), runs the tool on it with the C++ `ow-package`, and
compares its `registry.json` and the kept parts of its `census.json` byte for byte with:

- the same tool run with the Rust `ow-package` as its reader (the reader's parity under the tool), and
- the Rust binary's own `--item-registry --cooked <install> --census <file>` (the port's parity).

The census parts not kept are the run's timestamp and time (`generated`, `seconds`) and the cross-checks the Rust port
does not write (`check_*`); `decode_failures` is compared by its paths (the tool's text embeds its subprocess command
line). The failing DLC file also checks the `scan_errors` text against the C++ `--exports` stderr.

While `DefStore::census()` has no body (W3-DEFS), the Rust `--item-registry` answers NotYet (exit 3): that comparison is
reported as skipped, with the reason, and the reader comparison still runs. Every name and number is invented.
Run: python tests/item_registry_rust_test.py <C++ ow-package> <Rust ow-package>
"""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'tools'))
import item_registry as ir  # noqa: E402
import item_registry_test as fx  # noqa: E402

CXX = RUST = None
W = 'WillowGame.'
BROKEN = 'DLC/SynBroken/Content/SynBroken.upk'


def add_slot_titles(p):
    """Titles in Body and Barrel (sorted: Barrel first) and in Gamma and Delta (sorted: Delta first)."""
    for name, cls in (('Title_Body', 'WeaponNamePartDefinition'), ('Title_Barrel', 'WeaponNamePartDefinition'),
                      ('Title_Gamma', 'ItemNamePartDefinition'), ('Title_Delta', 'ItemNamePartDefinition')):
        p.add(f'GD_Syn.Names.{name}', W + cls)
    p.add('GD_Syn.Gun.Body_T', W + 'WeaponPartDefinition',
          [('TitleList', fx.Array([fx.Ref('GD_Syn.Names.Title_Body')]))])
    p.add('GD_Syn.Gun.Barrel_T', W + 'WeaponPartDefinition',
          [('TitleList', fx.Array([fx.Ref('GD_Syn.Names.Title_Barrel')]))])
    p.add('GD_Syn.Gun.Coll_Titles', W + 'WeaponPartListCollectionDefinition', [
        ('BodyPartData', fx.part_data([fx.weighted('GD_Syn.Gun.Body_T')])),
        ('BarrelPartData', fx.part_data([fx.weighted('GD_Syn.Gun.Barrel_T')])),
        ('PartReplacementMode', fx.Enum('EPartReplacementMode', 'EPRM_Selective')), fx.CONSOLIDATED])
    p.add('GD_Syn.Gun.Titles', W + 'WeaponBalanceDefinition', [
        ('InventoryDefinition', fx.Ref('GD_Syn.Gun.Type')),
        ('WeaponPartListCollection', fx.Ref('GD_Syn.Gun.Coll_Titles'))])
    p.add('GD_Syn.Grenade.Gamma_T', W + 'GrenadeModPartDefinition',
          [('TitleList', fx.Array([fx.Ref('GD_Syn.Names.Title_Gamma')]))])
    p.add('GD_Syn.Grenade.Delta_T', W + 'GrenadeModPartDefinition',
          [('TitleList', fx.Array([fx.Ref('GD_Syn.Names.Title_Delta')]))])
    p.add('GD_Syn.Grenade.Coll_Titles', W + 'ItemPartListCollectionDefinition', [
        ('GammaPartData', fx.part_data([fx.weighted('GD_Syn.Grenade.Gamma_T')])),
        ('DeltaPartData', fx.part_data([fx.weighted('GD_Syn.Grenade.Delta_T')])), fx.CONSOLIDATED])
    p.add('GD_Syn.Grades.Grenade_Titles', W + 'InventoryBalanceDefinition', [
        ('InventoryDefinition', fx.Ref('GD_Syn.Grenade.Def')),
        ('PartListCollection', fx.Ref('GD_Syn.Grenade.Coll_Titles'))])


def kept(census_bytes, scan_error_text=True):
    """The census parts both sides write, as sorted JSON text (decode_failures by path only; scan_errors by path only
    when `scan_error_text` is false)."""
    data = json.loads(census_bytes)
    if not scan_error_text:
        data['scan_errors'] = sorted(data.get('scan_errors', {}))
    for key in [k for k in data if k in ('generated', 'seconds') or k.startswith('check_')]:
        del data[key]
    data['decode_failures'] = sorted(data.get('decode_failures', {}))
    return json.dumps(data, ensure_ascii=False, separators=(',', ':'), sort_keys=True)


class RegistryRustTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.folder = tempfile.TemporaryDirectory()
        root = Path(cls.folder.name)
        game = (root / 'Game').absolute()
        base = fx.base_package()
        add_slot_titles(base)
        base.save(game / 'WillowGame' / 'CookedPCConsole' / 'SynStartup.upk')
        fx.dlc_package().save(game / 'DLC' / 'SynDLC' / 'Content' / 'SynDLC_Startup.upk')
        (game / BROKEN).parent.mkdir(parents=True)
        (game / BROKEN).write_bytes(b'not a package, invented bytes')
        cls.game = game
        cls.cxx_out = root / 'cxx'
        cls.cxx_report = ir.run(CXX, game, cls.cxx_out, jobs=2, checks=False)

    @classmethod
    def tearDownClass(cls):
        cls.folder.cleanup()

    def tool_files(self, out):
        return (out / 'registry.json').read_bytes(), (out / 'census.json').read_bytes()

    def test_fixture_has_the_multi_slot_titles(self):
        registry = json.loads((self.cxx_out / 'registry.json').read_text(encoding='utf-8'))
        self.assertEqual(registry['balances']['GD_Syn.Gun.Titles']['name_parts']['title'],
                         ['GD_Syn.Names.Title_Gun', 'GD_Syn.Names.Title_Body', 'GD_Syn.Names.Title_Barrel'])
        self.assertEqual(registry['balances']['GD_Syn.Grades.Grenade_Titles']['name_parts']['title'],
                         ['GD_Syn.Names.Title_Gamma', 'GD_Syn.Names.Title_Delta'])

    def test_the_tool_on_the_rust_reader_writes_the_same_bytes(self):
        out = Path(self.folder.name) / 'rust_reader'
        ir.run(RUST, self.game, out, jobs=2, checks=False)
        cxx_registry, cxx_census = self.tool_files(self.cxx_out)
        rust_registry, rust_census = self.tool_files(out)
        self.assertEqual(cxx_registry, rust_registry)
        # The scan error's text is the reader's stderr, so this also checks that both readers print the same line.
        self.assertEqual(kept(cxx_census), kept(rust_census))

    def exports(self, binary):
        """(exit code, stderr line) of `--exports` on the file that is not a package."""
        said = subprocess.run([binary, str(self.game / BROKEN), '--exports'], capture_output=True, text=True)
        return said.returncode, said.stderr.strip()

    def test_scan_errors_hold_the_cxx_readers_stderr_line(self):
        code, text = self.exports(CXX)
        self.assertEqual(code, 1)
        self.assertTrue(text.startswith('ow-package: '), text)
        self.assertEqual(self.cxx_report['scan_errors'], {BROKEN: text})

    def test_both_readers_refuse_a_file_that_is_not_a_package(self):
        self.assertEqual([self.exports(b)[0] for b in (CXX, RUST)], [1, 1])

    def test_the_rust_reader_says_what_the_cxx_reader_says_for_a_file_that_is_not_a_package(self):
        """Rule R7, found by this test: for a file with a bad magic both readers print `ow-package: invalid package
        magic` (the Rust reader printed `cannot decode the package container: not a UE3 package (bad tag)` until
        FOLLOWUPS-2). The tool keeps that line as the file's scan error. The exit code is checked above."""
        self.assertEqual(self.exports(RUST)[1], self.exports(CXX)[1])

    def rust_registry(self):
        """(stdout bytes, census bytes) of the Rust `--item-registry`; skips the test while the store's census answers
        NotYet (W3-DEFS)."""
        census_file = Path(self.folder.name) / 'rust_census.json'
        run = subprocess.run([RUST, '--item-registry', '--cooked', str(self.game), '--census', str(census_file)],
                             capture_output=True)
        if run.returncode == 3 and b'DefStore::census' in run.stderr:
            self.skipTest('the Rust --item-registry answers NotYet until DefStore::census() has a body (W3-DEFS): '
                          + run.stderr.decode('utf-8', 'replace').strip())
        self.assertEqual(run.returncode, 0, run.stderr.decode('utf-8', 'replace'))
        return run.stdout, census_file.read_bytes()

    def test_the_rust_registry_writes_the_tools_bytes(self):
        registry, census = self.rust_registry()
        cxx_registry, cxx_census = self.tool_files(self.cxx_out)
        self.assertEqual(registry, cxx_registry)
        # The scan error's text is the reader's line, compared in the test below.
        self.assertEqual(kept(census, False), kept(cxx_census, False))

    def test_the_rust_registrys_scan_error_text_is_the_cxx_readers(self):
        """The census `FileFailure.error` the Rust registry writes after `ow-package: ` must print as the C++
        `--exports` stderr does (decision 2026-11-15-15). Skipped while the store's census answers NotYet."""
        _, census = self.rust_registry()
        _, cxx_census = self.tool_files(self.cxx_out)
        self.assertEqual(json.loads(census)['scan_errors'], json.loads(cxx_census)['scan_errors'])

if __name__ == '__main__':
    if len(sys.argv) < 3:
        sys.exit('usage: python tests/item_registry_rust_test.py <C++ ow-package> <Rust ow-package>')
    RUST = str(Path(sys.argv.pop(2)).resolve())
    CXX = str(Path(sys.argv.pop(1)).resolve())
    unittest.main()
