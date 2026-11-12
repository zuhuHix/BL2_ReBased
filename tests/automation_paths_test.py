"""Guard on the UE automation test paths and the ue_automation suite count.

AI-assisted (Claude Code), 2026-11-12. Two things went wrong silently before this test: a plain test whose path is also
the folder of other tests (OpenWillow.WeaponAssembly, then OpenWillow.MayaState) is never run by Unreal's automation
framework, and the suite's pass line still matched because the count stayed the same. Read in the UE 5.8 source (not
seen in a run): the report tree keys its nodes by full test path, so a test and a folder of the same path share one
node, and a node with children runs only its children. Each '+' part of `Automation RunTests` is a substring match on
the full path (a leading ^ or trailing $ anchors it).

This test fails when
- one test path under host/ue5/OpenWillow/Source is a dot-prefix of another (both are named), or a path is used twice;
- the ue_automation row of tools/suites.txt expects a different number of tests than the paths its names match, or
  its three counts (the pass line's expected=, the row's printed expected= and its -eq) disagree.

The synthetic cases use invented paths only (rule 9); the last class reads this checkout.
"""
from pathlib import Path
import re
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'host' / 'ue5' / 'OpenWillow' / 'Source'
SUITES = ROOT / 'tools' / 'suites.txt'

# Raw strings, plain strings, then comments; strings are kept and comments blanked, so a commented-out macro is not read.
_TOKENS = re.compile(r'R"([^(\s]{0,16})\(.*?\)\1"|"(?:\\.|[^"\\\n])*"|//[^\n]*|/\*.*?\*/', re.S)
# IMPLEMENT_SIMPLE / IMPLEMENT_COMPLEX (and the CUSTOM_ variants, whose second argument is the base class).
_MACRO = re.compile(r'\bIMPLEMENT_(?:CUSTOM_)?(SIMPLE|COMPLEX)_AUTOMATION_TEST\s*\(\s*\w+\s*,\s*(?:\w+\s*,\s*)?"([^"]+)"')


def strip_comments(text):
    return _TOKENS.sub(lambda m: m.group(0) if m.group(0)[0] in 'R"' else ' ', text)


def test_paths(source_dir):
    """[(path, kind, file)] for every automation test macro under source_dir (.cpp and .h)."""
    found = []
    for f in sorted(Path(source_dir).rglob('*')):
        if f.suffix.lower() not in ('.cpp', '.h') or not f.is_file():
            continue
        text = strip_comments(f.read_text(encoding='utf-8', errors='replace'))
        for m in _MACRO.finditer(text):
            found.append((m.group(2), m.group(1).lower(), f))
    return found


def path_problems(paths):
    """Messages for paths used twice and for a path that is a dot-prefix of another (that test would never run)."""
    problems = []
    names = sorted(p for p, _, _ in paths)
    seen = set()
    for n in names:
        if n in seen:
            problems.append(f'test path {n} is used twice')
        seen.add(n)
    unique = sorted(seen)
    for a in unique:
        for b in unique:
            if b.startswith(a + '.'):
                problems.append(f'test {a} is never run: its path is the folder of test {b}; rename it (for example {a}.Core)')
    return problems


def matches(name, path):
    """One '+' part of Automation RunTests against a full test path, as the UE 5.8 filter does."""
    start, end = name.startswith('^'), name.endswith('$') and len(name) > 1
    core = name[1 if start else 0:len(name) - (1 if end else 0)]
    if start or end:
        return (not start or path.startswith(core)) and (not end or path.endswith(core))
    return core in path


def suite_counts(text):
    """(names, expected on the pass line, printed expected, -eq count) of the ue_automation row of a suite map."""
    row = [l for l in text.splitlines() if l.startswith('ue_automation |')]
    passes = [l for l in text.splitlines() if l.startswith('#pass ue_automation |')]
    if len(row) != 1 or len(passes) != 1:
        raise ValueError(f'expected one ue_automation row and one pass line, found {len(row)} and {len(passes)}')
    names_m = re.search(r"foreach \(\$n in ((?:'[^']+',?\s*)+)\)\s*\{\s*\$names \+= '([^']*)' \+ \$n", row[0])
    if not names_m:
        raise ValueError('the ue_automation row has no foreach list of test names')
    names = [names_m.group(2) + n for n in re.findall(r"'([^']+)'", names_m.group(1))]
    expected = re.search(r'expected=(\d+)', passes[0].split('|')[1])
    printed = re.search(r"' expected=(\d+)'", row[0])
    eq = re.search(r'\$ok\.Count -eq (\d+)', row[0])
    if not (expected and printed and eq):
        raise ValueError('the ue_automation row or pass line lost one of its three counts')
    return names, int(expected.group(1)), int(printed.group(1)), int(eq.group(1))


def count_problems(names, expected, printed, eq, paths):
    """Messages when the suite's counts disagree with each other or with the tests its names match."""
    problems = []
    if not expected == printed == eq:
        problems.append(f'ue_automation counts disagree: pass line expected={expected}, row prints expected={printed}, row checks -eq {eq}')
    hit = sorted({p for p, kind, _ in paths if any(matches(n, p) for n in names)})
    complex_hit = sorted({p for p, kind, _ in paths if kind == 'complex' and p in hit})
    if complex_hit:
        # A complex test with parameters is a folder of one test per parameter; its count is not known from the source.
        problems.append(f'complex test(s) under the ue_automation names, count them by hand: {", ".join(complex_hit)}')
    if len(hit) != expected:
        problems.append(f'ue_automation expects {expected} tests but its names match {len(hit)}: {", ".join(hit)}')
    return problems


def write_tree(root, files):
    for rel, text in files.items():
        p = Path(root) / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding='utf-8')


def macro(cls, path, kind='SIMPLE'):
    return f'IMPLEMENT_{kind}_AUTOMATION_TEST({cls}, "{path}",\n    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)\n'


def suite_text(names, expected, printed=None, eq=None):
    printed = expected if printed is None else printed
    eq = expected if eq is None else eq
    listed = ','.join(f"'{n}'" for n in names)
    return ('# invented map\n'
            f"ue_automation | 1 | src/ | $names = @(); foreach ($n in {listed}) {{ $names += 'Fake.' + $n }}; "
            f"Write-Output ('SUMMARY success=' + $ok.Count + ' expected={printed}'); if ($ok.Count -eq {eq} -and $bad.Count -eq 0) {{ $code = 0 }}\n"
            f'#pass ue_automation | SUMMARY success={expected} failed=0 expected={expected} | SUMMARY failed=[1-9]\n')


class SyntheticPaths(unittest.TestCase):
    def paths(self, files):
        with tempfile.TemporaryDirectory() as tmp:
            write_tree(tmp, files)
            return [(p, k, f.name) for p, k, f in test_paths(tmp)]

    def test_reads_simple_complex_and_custom(self):
        got = self.paths({'A/One.cpp': macro('FOne', 'Fake.Alpha') + macro('FTwo', 'Fake.Beta.Two', 'COMPLEX'),
                          'B/Two.h': 'IMPLEMENT_CUSTOM_SIMPLE_AUTOMATION_TEST(FThree, FBase, "Fake.Gamma", 0)\n',
                          'B/notes.txt': macro('FNo', 'Fake.NotSource')})
        self.assertEqual(sorted((p, k) for p, k, _ in got), [('Fake.Alpha', 'simple'), ('Fake.Beta.Two', 'complex'), ('Fake.Gamma', 'simple')])

    def test_comments_are_not_read_but_strings_are_kept(self):
        got = self.paths({'C.cpp': '// ' + macro('FOld', 'Fake.Old') + '/* ' + macro('FBlock', 'Fake.Block') + ' */\n'
                          'const TCHAR* Url = TEXT("http://x"); const TCHAR* Raw = TEXT(R"({"k": "//"})");\n'
                          + macro('FNew', 'Fake.New')})
        self.assertEqual([p for p, _, _ in got], ['Fake.New'])

    def test_a_folder_path_names_both_tests(self):
        problems = path_problems([('Fake.State', 'simple', None), ('Fake.State.Split', 'simple', None), ('Fake.Other', 'simple', None)])
        self.assertEqual(len(problems), 1)
        self.assertIn('Fake.State is never run', problems[0])
        self.assertIn('Fake.State.Split', problems[0])

    def test_a_shared_prefix_without_a_dot_is_fine(self):
        self.assertEqual(path_problems([('Fake.Skill', 'simple', None), ('Fake.Skills', 'simple', None),
                                        ('Fake.SkillEffects', 'simple', None), ('Fake.State.Core', 'simple', None),
                                        ('Fake.State.Split', 'simple', None)]), [])

    def test_a_path_used_twice(self):
        self.assertEqual(path_problems([('Fake.A', 'simple', None), ('Fake.A', 'simple', None)]), ['test path Fake.A is used twice'])

    def test_deep_prefix(self):
        problems = path_problems([('Fake.A', 'simple', None), ('Fake.A.B.C', 'simple', None)])
        self.assertEqual(len(problems), 1)
        self.assertIn('Fake.A.B.C', problems[0])


class SyntheticCounts(unittest.TestCase):
    PATHS = [('Fake.State.Core', 'simple', None), ('Fake.State.Split', 'simple', None), ('Fake.Gun', 'simple', None),
             ('Fake.GunTable', 'simple', None), ('Fake.Unlisted', 'simple', None)]

    def test_substring_and_anchors(self):
        self.assertTrue(matches('Fake.State', 'Fake.State.Core'))
        self.assertTrue(matches('Fake.Gun', 'Fake.GunTable'))   # a substring, as the engine matches
        self.assertTrue(matches('State', 'Fake.State.Core'))
        self.assertFalse(matches('^State', 'Fake.State.Core'))
        self.assertTrue(matches('Fake.Gun$', 'Fake.Gun'))
        self.assertFalse(matches('Fake.Gun$', 'Fake.GunTable'))

    def test_right_count_passes(self):
        names, expected, printed, eq = suite_counts(suite_text(['State', 'Gun'], 4))
        self.assertEqual(names, ['Fake.State', 'Fake.Gun'])
        self.assertEqual(count_problems(names, expected, printed, eq, self.PATHS), [])

    def test_wrong_count_fails_and_lists_the_matches(self):
        names, expected, printed, eq = suite_counts(suite_text(['State', 'Gun'], 3))
        problems = count_problems(names, expected, printed, eq, self.PATHS)
        self.assertEqual(len(problems), 1)
        self.assertIn('expects 3 tests but its names match 4', problems[0])
        self.assertIn('Fake.GunTable', problems[0])

    def test_counts_that_disagree_fail(self):
        for printed, eq in ((3, 4), (4, 3)):
            names, expected, p, e = suite_counts(suite_text(['State', 'Gun'], 4, printed, eq))
            problems = count_problems(names, expected, p, e, self.PATHS)
            self.assertTrue(any('counts disagree' in m for m in problems), problems)

    def test_a_complex_test_is_flagged(self):
        paths = self.PATHS + [('Fake.State.Many', 'complex', None)]
        names, expected, printed, eq = suite_counts(suite_text(['State', 'Gun'], 5))
        self.assertTrue(any('complex test' in m for m in count_problems(names, expected, printed, eq, paths)))

    def test_a_map_without_the_row_is_refused(self):
        with self.assertRaises(ValueError):
            suite_counts('# invented map\nother | 1 | src/ | x\n')


class ThisCheckout(unittest.TestCase):
    def test_no_test_path_is_a_folder_of_another(self):
        paths = test_paths(SOURCE)
        self.assertTrue(paths, f'no automation tests found under {SOURCE}')
        self.assertEqual(path_problems(paths), [])

    def test_ue_automation_expects_what_its_names_match(self):
        names, expected, printed, eq = suite_counts(SUITES.read_text(encoding='utf-8'))
        problems = count_problems(names, expected, printed, eq, test_paths(SOURCE))
        self.assertEqual(problems, [])
        print(f'ue_automation: {len(names)} names, expected={expected}', file=sys.stderr)


if __name__ == '__main__':
    unittest.main()
