"""umodel listing parser and export-table comparison on synthetic text."""
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from crosscheck_umodel import compare, parse_listing

LISTING = """Found 3 game files (0 skipped) in 1 folders at path "X"
Scanned game directory in 0.0 sec, 1 allocs, 0.00 MBytes serialized in 0 calls.
Loading package: Synthetic.upk Ver: 832/46 Engine: 1 [FullComp] Names: 10 Exports: 3 Imports: 2 Game: 800017
   0       10       20 Class Object
   1       30        C Object Default__Object
   2       3C      4232 Texture2D fog-of-war-blob t
"""


def records(overrides=None):
    overrides = overrides or {}
    base = [dict(index=1, name='Object', path='Object', class_='Class', size=0x20, offset=0x10),
            dict(index=2, name='Default__Object', path='Default__Object', class_='Core.Object', size=0xC, offset=0x30),
            dict(index=3, name='fog-of-war-blob t', path='Shared.fog-of-war-blob t', class_='Engine.Texture2D',
                 size=0x4232, offset=0x3C)]
    out = []
    for record in base:
        record = dict(record)
        record['class'] = record.pop('class_')
        record.update(overrides.get(record['index'], {}))
        out.append(record)
    return out


class ListingTest(unittest.TestCase):
    def test_parses_header_and_rows_with_spaces_in_names(self):
        header, rows = parse_listing(LISTING)
        self.assertEqual((header['names'], header['exports'], header['imports'], header['version'], header['licensee']),
                         (10, 3, 2, 832, 46))
        self.assertEqual([r['index'] for r in rows], [1, 2, 3])
        self.assertEqual(rows[2], dict(index=3, offset=0x3C, size=0x4232, cls='Texture2D', name='fog-of-war-blob t'))

    def test_rows_before_header_are_ignored(self):
        header, rows = parse_listing('   0   10   20 Class Object\n')
        self.assertIsNone(header)
        self.assertEqual(rows, [])


class CompareTest(unittest.TestCase):
    def test_agreement(self):
        header, rows = parse_listing(LISTING)
        report = compare(header, rows, records(), dict(exports=3))
        self.assertEqual(report['mismatches'], [])
        self.assertEqual(report['compared'], 3)
        self.assertIsNone(report['header_mismatch'])
        self.assertNotIn('error', report)

    def test_field_mismatches_are_reported_per_field(self):
        header, rows = parse_listing(LISTING)
        report = compare(header, rows, records({2: dict(size=0xD, name='Renamed')}))
        fields = sorted((m['index'], m['field']) for m in report['mismatches'])
        self.assertEqual(fields, [(2, 'name'), (2, 'size')])

    def test_class_compares_on_short_name(self):
        header, rows = parse_listing(LISTING)
        report = compare(header, rows, records({3: {'class': 'Other.Texture2D'}}))
        self.assertEqual(report['mismatches'], [])

    def test_missing_rows_on_either_side(self):
        header, rows = parse_listing(LISTING)
        report = compare(header, rows[:2], records())
        self.assertEqual([m['field'] for m in report['mismatches']], ['missing_in_umodel'])
        self.assertIn('error', report)  # header says 3 exports, only 2 rows parsed
        report = compare(header, rows, records()[:2])
        self.assertEqual([m['field'] for m in report['mismatches']], ['missing_in_reader'])

    def test_umodel_name_normalization_is_classified_separately(self):
        header, rows = parse_listing(LISTING.replace('fog-of-war-blob t', '__name_7__'))
        report = compare(header, rows, records({3: dict(name='fogwar')}))
        self.assertEqual(report['mismatches'], [])
        self.assertEqual([m['field'] for m in report['name_normalized']], ['name'])
        # A plain rename is still a real mismatch, and so is a __name_N__ for a clean ASCII name.
        report = compare(header, rows, records({3: dict(name='clean')}))
        self.assertEqual([m['field'] for m in report['mismatches']], ['name'])
        header, rows = parse_listing(LISTING)
        report = compare(header, rows, records({3: dict(name='fog-of-war-blob t ')}))
        self.assertEqual(report['mismatches'], [])
        self.assertEqual(len(report['name_normalized']), 1)

    def test_header_count_mismatch(self):
        header, rows = parse_listing(LISTING)
        report = compare(header, rows, records(), dict(exports=4))
        self.assertEqual(report['header_mismatch'], dict(exports=(3, 4)))


if __name__ == '__main__':
    unittest.main()
