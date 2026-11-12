"""Guard on the UE automation test paths and the ue_automation suite count.

AI-assisted (Claude Code), 2026-11-12. Two things went wrong silently before this test: a plain test whose path is also
the folder of other tests (OpenWillow.WeaponAssembly, then OpenWillow.MayaState) is never run by Unreal's automation
framework, and the suite's pass line still matched because the count stayed the same. Read in the UE 5.8 source (not
seen in a run): the report tree keys its nodes by full test path, so a test and a folder of the same path share one
node, and a node with children runs only its children. Each '+' part of `Automation RunTests` is a substring match on
the full path (a leading ^ or trailing $ anchors it). The matches and the path keys ignore case, so this test compares
lower-cased paths.

This test fails when
- one test path under host/ue5/OpenWillow/Source is a dot-prefix of another (both are named), or a path is used twice,
  ignoring case;
- a test declaration is one this test cannot read (a spec, a networked test, a path that is not a string literal), so
  it would go uncounted; the file and line are named;
- the ue_automation row of tools/suites.txt expects a different number of tests than the paths its names match, its
  three counts (the pass line's expected=, the row's printed expected= and its -eq) disagree, or one of its names carries
  a ^ or $ anchor (not modelled here: in the engine a $ name that matches a folder selects all of the folder's children).

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
# Every automation test or spec declaration, read or not: IMPLEMENT_*_AUTOMATION_TEST(, BEGIN_DEFINE_SPEC(, DEFINE_SPEC(.
_DECLARATION = re.compile(r'\b\w*(?:AUTOMATION_TEST|DEFINE_SPEC)\s*\(')


def strip_comments(text):
    return _TOKENS.sub(lambda m: m.group(0) if m.group(0)[0] in 'R"' else ' ', text)


def scan(source_dir):
    """([(path, kind, file)] for every macro read, [message] for every declaration that could not be read)."""
    found, unread = [], []
    for f in sorted(Path(source_dir).rglob('*')):
        if f.suffix.lower() not in ('.cpp', '.h') or not f.is_file():
            continue
        text = strip_comments(f.read_text(encoding='utf-8', errors='replace'))
        for d in _DECLARATION.finditer(text):
            m = _MACRO.match(text, d.start())
            if m:
                found.append((m.group(2), m.group(1).lower(), f))
            else:
                line = text.count('\n', 0, d.start()) + 1
                unread.append(f'{f}:{line}: test declaration {d.group(0).rstrip("( ")} not read (a spec, a networked test or '
                              'a path that is not a string literal), so it would not be counted')
    return found, unread


def test_paths(source_dir):
    """[(path, kind, file)] for every automation test macro under source_dir (.cpp and .h)."""
    return scan(source_dir)[0]


def path_problems(paths):
    """Messages for paths used twice and for a path that is a dot-prefix of another (that test would never run).
    The engine's path keys ignore case, so the comparison does too."""
    problems = []
    seen = {}
    for n in sorted(p for p, _, _ in paths):
        same = seen.get(n.lower())
        if same is None:
            seen[n.lower()] = n
        elif same == n:
            problems.append(f'test path {n} is used twice')
        else:
            problems.append(f'test paths {same} and {n} differ only in case')
    unique = sorted(seen.values())
    for a in unique:
        for b in unique:
            if b.lower().startswith(a.lower() + '.'):
                problems.append(f'test {a} is never run: its path is the folder of test {b}; rename it (for example {a}.Core)')
    return problems


def matches(name, path):
    """One unanchored '+' part of Automation RunTests against a full test path: a substring match ignoring case, as the
    UE 5.8 filter does. Anchored names are refused by count_problems, not matched here."""
    return name.lower() in path.lower()


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
    anchored = [n for n in names if '^' in n or '$' in n]
    if anchored:
        # In the engine a $ name that matches a folder selects all of the folder's children; the count below does not model it.
        return [f'ue_automation names carry an anchor, which this count does not model: {", ".join(anchored)}; use plain '
                'names (a substring match), or teach tests/automation_paths_test.py the engine\'s anchor rule first']
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
    def scan(self, files):
        with tempfile.TemporaryDirectory() as tmp:
            write_tree(tmp, files)
            found, unread = scan(tmp)
            return [(p, k, f.name) for p, k, f in found], unread

    def test_reads_simple_complex_and_custom(self):
        got, unread = self.scan({'A/One.cpp': macro('FOne', 'Fake.Alpha') + macro('FTwo', 'Fake.Beta.Two', 'COMPLEX'),
                                 'B/Two.h': 'IMPLEMENT_CUSTOM_SIMPLE_AUTOMATION_TEST(FThree, FBase, "Fake.Gamma", 0)\n',
                                 'B/notes.txt': macro('FNo', 'Fake.NotSource')})
        self.assertEqual(sorted((p, k) for p, k, _ in got), [('Fake.Alpha', 'simple'), ('Fake.Beta.Two', 'complex'), ('Fake.Gamma', 'simple')])
        self.assertEqual(unread, [])

    def test_comments_are_not_read_but_strings_are_kept(self):
        got, unread = self.scan({'C.cpp': '// ' + macro('FOld', 'Fake.Old') + '/* ' + macro('FBlock', 'Fake.Block') + ' */\n'
                                 'const TCHAR* Url = TEXT("http://x"); const TCHAR* Raw = TEXT(R"({"k": "//"})");\n'
                                 + macro('FNew', 'Fake.New')})
        self.assertEqual([p for p, _, _ in got], ['Fake.New'])
        self.assertEqual(unread, [])

    def test_a_spec_is_named_with_file_and_line(self):
        got, unread = self.scan({'Spec.cpp': '#include "Fake.h"\nBEGIN_DEFINE_SPEC(FFakeSpec, "Fake.Spec", 0)\nEND_DEFINE_SPEC(FFakeSpec)\n'})
        self.assertEqual(got, [])
        self.assertTrue(unread and all('Spec.cpp:' in m for m in unread), unread)
        self.assertTrue(any('Spec.cpp:2:' in m and 'BEGIN_DEFINE_SPEC' in m for m in unread), unread)

    def test_a_path_that_is_not_a_literal_is_named(self):
        got, unread = self.scan({'Built.cpp': '#define FAKE_PATH "Fake.Built"\n' + macro('FOne', 'Fake.Read')
                                 + 'IMPLEMENT_SIMPLE_AUTOMATION_TEST(FTwo, FAKE_PATH, 0)\n'
                                 + 'IMPLEMENT_NETWORKED_AUTOMATION_TEST(FNet, "Fake.Net", 0)\n'})
        self.assertEqual([p for p, _, _ in got], ['Fake.Read'])
        self.assertEqual(len(unread), 2, unread)
        self.assertIn('Built.cpp:4:', unread[0])
        self.assertIn('IMPLEMENT_SIMPLE_AUTOMATION_TEST', unread[0])
        self.assertIn('Built.cpp:5:', unread[1])

    def test_a_folder_path_names_both_tests(self):
        problems = path_problems([('Fake.State', 'simple', None), ('Fake.State.Split', 'simple', None), ('Fake.Other', 'simple', None)])
        self.assertEqual(len(problems), 1)
        self.assertIn('Fake.State is never run', problems[0])
        self.assertIn('Fake.State.Split', problems[0])

    def test_case_is_ignored(self):
        problems = path_problems([('Fake.State', 'simple', None), ('fake.state.Split', 'simple', None)])
        self.assertEqual(len(problems), 1)
        self.assertIn('Fake.State is never run', problems[0])
        self.assertEqual(path_problems([('Fake.State.X', 'simple', None), ('Fake.state.X', 'simple', None)]),
                         ['test paths Fake.State.X and Fake.state.X differ only in case'])

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

    def test_substring_ignoring_case(self):
        self.assertTrue(matches('Fake.State', 'Fake.State.Core'))
        self.assertTrue(matches('Fake.Gun', 'Fake.GunTable'))   # a substring, as the engine matches
        self.assertTrue(matches('State', 'Fake.State.Core'))
        self.assertTrue(matches('fake.STATE', 'Fake.State.Core'))
        self.assertFalse(matches('Fake.Other', 'Fake.State.Core'))

    def test_a_name_in_another_case_still_counts(self):
        names, expected, printed, eq = suite_counts(suite_text(['state', 'Gun'], 4))
        self.assertEqual(count_problems(names, expected, printed, eq, self.PATHS), [])

    def test_anchored_names_are_refused(self):
        for name in ('^Fake.Gun', 'Gun$'):
            names, expected, printed, eq = suite_counts(suite_text(['State', name], 4))
            problems = count_problems(names, expected, printed, eq, self.PATHS)
            self.assertEqual(len(problems), 1, problems)
            self.assertIn('anchor', problems[0])

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
    def test_every_declaration_is_read_and_no_path_is_a_folder_of_another(self):
        paths, unread = scan(SOURCE)
        self.assertTrue(paths, f'no automation tests found under {SOURCE}')
        self.assertEqual(unread, [])
        self.assertEqual(path_problems(paths), [])

    def test_ue_automation_expects_what_its_names_match(self):
        names, expected, printed, eq = suite_counts(SUITES.read_text(encoding='utf-8'))
        problems = count_problems(names, expected, printed, eq, test_paths(SOURCE))
        self.assertEqual(problems, [])
        print(f'ue_automation: {len(names)} names, expected={expected}', file=sys.stderr)


if __name__ == '__main__':
    unittest.main()
