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
- the ue_automation suite expects a different number of tests than the paths its names match, its three counts (the
  pass line's expected= in tools/suites.txt, the expected= the launcher prints and the count its -eq checks) disagree, or
  one of its names carries a ^ or $ anchor (not modelled here: in the engine a $ name that matches a folder selects all
  of the folder's children);
- the ue_automation row does not run tools/test_automation.ps1, or overrides its -Tests or -Expected. Since 2026-11-13
  the names and the count live in one place, the launcher's -Tests and -Expected defaults; the printed count and the
  -eq count are read from the launcher's own lines (a literal there is read as it stands, so it cannot hide). The
  launcher's -DryRun runs under each PowerShell present and must show the names it would pass, with no lock taken.

The synthetic cases use invented paths only (rule 9); the last class reads this checkout.
"""
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'host' / 'ue5' / 'OpenWillow' / 'Source'
SUITES = ROOT / 'tools' / 'suites.txt'
LAUNCHER = ROOT / 'tools' / 'test_automation.ps1'
# Both shells, each when present (the suites run under Windows PowerShell 5.1).
SHELLS = {n: p for n, p in (('powershell', shutil.which('powershell')), ('pwsh', shutil.which('pwsh'))) if p}

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


def _count(value, expected, what):
    """A count written in the launcher: the -Expected variable stands for its default, a literal for itself."""
    if value.lower() == '$expected':
        return expected
    if value.isdigit():
        return int(value)
    raise ValueError(f"the launcher's {what} is neither $Expected nor a number: {value}")


def launcher_counts(launcher):
    """(names, -Expected default, printed expected, -eq count) read from the launcher's text."""
    names_m = re.search(r"\[string\[\]\]\$Tests\s*=\s*@\(((?:\s*'[^']+'\s*,?)+)\)", launcher)
    if not names_m:
        raise ValueError('the launcher has no [string[]]$Tests = @(...) default list of test names')
    names = re.findall(r"'([^']+)'", names_m.group(1))
    expected_m = re.search(r'\[int\]\$Expected\s*=\s*(\d+)', launcher)
    printed = re.findall(r"' expected='\s*\+\s*(\$\w+|\d+)", launcher)
    eq = re.findall(r'\$ok\.Count\s+-eq\s+(\$\w+|\d+)', launcher)
    if not expected_m or len(printed) != 1 or len(eq) != 1:
        raise ValueError("the launcher lost its -Expected default, its printed expected= or its $ok.Count -eq check "
                         f'(found {bool(expected_m)}, {len(printed)}, {len(eq)})')
    expected = int(expected_m.group(1))
    return names, expected, _count(printed[0], expected, 'printed expected='), _count(eq[0], expected, '-eq count')


def suite_counts(text, launcher):
    """(names, expected on the pass line, printed expected, -eq count) of the ue_automation suite: the row and its pass line
    from a suite map, the names and the other two counts from its launcher's text."""
    row = [l for l in text.splitlines() if l.startswith('ue_automation |')]
    passes = [l for l in text.splitlines() if l.startswith('#pass ue_automation |')]
    if len(row) != 1 or len(passes) != 1:
        raise ValueError(f'expected one ue_automation row and one pass line, found {len(row)} and {len(passes)}')
    command = row[0].split('|', 3)[3]
    if not re.search(r'-File\s+tools/test_automation\.ps1', command):
        raise ValueError('the ue_automation row does not run tools/test_automation.ps1')
    # Nothing may follow the script path but the exit: PowerShell binds a parameter by a unique prefix (-Te, -Exp), so any
    # argument could override the defaults the count is read from.
    if not re.search(r'-File\s+tools/test_automation\.ps1;\s*exit \$LASTEXITCODE\s*$', command):
        raise ValueError("the ue_automation row passes the launcher an argument, which could override -Tests or -Expected: "
                         "change the defaults there instead")
    # The pass line must also want timed_out=0: a run stopped at the timeout fails even with every test completed.
    expected = re.search(r'expected=(\d+) timed_out=0\b', passes[0].split('|')[1])
    if not expected:
        raise ValueError('the ue_automation pass line lost its expected= count or its timed_out=0')
    names, _, printed, eq = launcher_counts(launcher)
    return names, int(expected.group(1)), printed, eq


def count_problems(names, expected, printed, eq, paths):
    """Messages when the suite's counts disagree with each other or with the tests its names match."""
    anchored = [n for n in names if '^' in n or '$' in n]
    if anchored:
        # In the engine a $ name that matches a folder selects all of the folder's children; the count below does not model it.
        return [f'ue_automation names carry an anchor, which this count does not model: {", ".join(anchored)}; use plain '
                'names (a substring match), or teach tests/automation_paths_test.py the engine\'s anchor rule first']
    problems = []
    if not expected == printed == eq:
        problems.append(f'ue_automation counts disagree: pass line expected={expected}, launcher prints expected={printed}, launcher checks -eq {eq}')
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


def launcher_text(names, expected, printed='$Expected', eq='$Expected'):
    """An invented launcher in the shape of tools/test_automation.ps1 (only the lines the guard reads)."""
    listed = ', '.join(f"'Fake.{n}'" for n in names)
    return ('param(\n'
            f'    [string[]]$Tests = @({listed}),\n'
            f'    [int]$Expected = {expected}\n'
            ')\n'
            f"Write-Output ('AUTOMATION SUMMARY success=' + $ok.Count + ' failed=' + $bad.Count + ' expected=' + {printed} + ' timed_out=' + $timedOut)\n"
            f'if ($ok.Count -eq {eq} -and $bad.Count -eq 0) {{ $code = 0 }} else {{ $code = 1 }}\n')


def suite_text(expected, command='powershell -NoProfile -File tools/test_automation.ps1; exit $LASTEXITCODE'):
    return ('# invented map\n'
            f'ue_automation | 1 | src/ tools/test_automation.ps1 | {command}\n'
            f'#pass ue_automation | SUMMARY success={expected} failed=0 expected={expected} timed_out=0 | SUMMARY failed=[1-9]\n')


def counts(names, expected, printed='$Expected', eq='$Expected', passline=None):
    """suite_counts of an invented map and launcher; passline, when given, replaces the pass line's count."""
    return suite_counts(suite_text(expected if passline is None else passline), launcher_text(names, expected, printed, eq))


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
        names, expected, printed, eq = counts(['state', 'Gun'], 4)
        self.assertEqual(count_problems(names, expected, printed, eq, self.PATHS), [])

    def test_anchored_names_are_refused(self):
        for name in ('^Gun', 'Gun$'):
            names, expected, printed, eq = counts(['State', name], 4)
            problems = count_problems(names, expected, printed, eq, self.PATHS)
            self.assertEqual(len(problems), 1, problems)
            self.assertIn('anchor', problems[0])

    def test_right_count_passes(self):
        names, expected, printed, eq = counts(['State', 'Gun'], 4)
        self.assertEqual(names, ['Fake.State', 'Fake.Gun'])
        self.assertEqual(count_problems(names, expected, printed, eq, self.PATHS), [])

    def test_wrong_count_fails_and_lists_the_matches(self):
        names, expected, printed, eq = counts(['State', 'Gun'], 3)
        problems = count_problems(names, expected, printed, eq, self.PATHS)
        self.assertEqual(len(problems), 1)
        self.assertIn('expects 3 tests but its names match 4', problems[0])
        self.assertIn('Fake.GunTable', problems[0])

    def test_counts_that_disagree_fail(self):
        # A literal in the launcher's print or -eq is read as it stands; the pass line's count is read from the map.
        for printed, eq, passline in (('3', '$Expected', None), ('$Expected', '3', None), ('$Expected', '$Expected', 3)):
            names, expected, p, e = counts(['State', 'Gun'], 4, printed, eq, passline)
            problems = count_problems(names, expected, p, e, self.PATHS)
            self.assertTrue(any('counts disagree' in m for m in problems), (printed, eq, passline, problems))

    def test_literal_counts_that_agree_pass(self):
        names, expected, printed, eq = counts(['State', 'Gun'], 4, '4', '4')
        self.assertEqual(count_problems(names, expected, printed, eq, self.PATHS), [])

    def test_a_complex_test_is_flagged(self):
        paths = self.PATHS + [('Fake.State.Many', 'complex', None)]
        names, expected, printed, eq = counts(['State', 'Gun'], 5)
        self.assertTrue(any('complex test' in m for m in count_problems(names, expected, printed, eq, paths)))

    def test_a_map_without_the_row_is_refused(self):
        with self.assertRaises(ValueError):
            suite_counts('# invented map\nother | 1 | src/ | x\n', launcher_text(['State'], 2))

    def test_a_launcher_that_lost_a_line_is_refused(self):
        whole = launcher_text(['State'], 2)
        for lost in ('[int]$Expected = 2', "' expected=' + $Expected", '$ok.Count -eq $Expected', "[string[]]$Tests = @('Fake.State')"):
            self.assertIn(lost, whole)
            with self.assertRaises(ValueError, msg=lost):
                suite_counts(suite_text(2), whole.replace(lost, 'x'))
        with self.assertRaises(ValueError):
            suite_counts(suite_text(2), launcher_text(['State'], 2, eq='$Other'))

    def test_the_row_must_run_the_launcher_without_overrides(self):
        launcher = launcher_text(['State'], 2)
        for command in ("$names = @(); foreach ($n in 'State') { $names += 'Fake.' + $n }; exit 0",
                        'powershell -File tools/test_automation.ps1 -Tests Fake.Gun -Expected 1; exit $LASTEXITCODE',
                        'powershell -File tools/test_automation.ps1 -expected 2; exit $LASTEXITCODE',
                        'powershell -File tools/test_automation.ps1 -Te Fake.Gun -Exp 1; exit $LASTEXITCODE',   # unique prefixes
                        'powershell -File tools/test_automation.ps1 Fake.Gun; exit $LASTEXITCODE',             # a positional value
                        'powershell -File tools/test_automation.ps1 -DryRun; exit $LASTEXITCODE',
                        'powershell -File tools/test_automation_old.ps1; exit $LASTEXITCODE'):
            with self.assertRaises(ValueError, msg=command):
                suite_counts(suite_text(2, command), launcher)
        self.assertEqual(suite_counts(suite_text(2), launcher)[:2], (['Fake.State'], 2))


class ThisCheckout(unittest.TestCase):
    def test_every_declaration_is_read_and_no_path_is_a_folder_of_another(self):
        paths, unread = scan(SOURCE)
        self.assertTrue(paths, f'no automation tests found under {SOURCE}')
        self.assertEqual(unread, [])
        self.assertEqual(path_problems(paths), [])

    def test_ue_automation_expects_what_its_names_match(self):
        names, expected, printed, eq = suite_counts(SUITES.read_text(encoding='utf-8'), LAUNCHER.read_text(encoding='utf-8-sig'))
        problems = count_problems(names, expected, printed, eq, test_paths(SOURCE))
        self.assertEqual(problems, [])
        print(f'ue_automation: {len(names)} names, expected={expected}', file=sys.stderr)

    def test_the_pass_line_refuses_a_timed_out_run(self):
        line = [l for l in SUITES.read_text(encoding='utf-8').splitlines() if l.startswith('#pass ue_automation |')][0]
        must = re.compile(line.split('|')[1].strip())
        names, expected, _, _ = suite_counts(SUITES.read_text(encoding='utf-8'), LAUNCHER.read_text(encoding='utf-8-sig'))
        good = f'AUTOMATION SUMMARY success={expected} failed=0 expected={expected} timed_out=0'
        self.assertTrue(must.search(good))
        self.assertFalse(must.search(good.replace('timed_out=0', 'timed_out=1')))
        with self.assertRaises(ValueError):   # a pass line that drops timed_out=0 is refused
            suite_counts(suite_text(2).replace(' timed_out=0', ''), launcher_text(['State'], 2))


class DryRunCases:
    """The launcher's -DryRun under one PowerShell: it shows the command with the default names and count and starts nothing
    (an invented engine folder, and a lock path in a temporary folder that must stay absent)."""
    POWERSHELL = None   # set per shell by the loop below

    def run_launcher(self, *args, env=None):
        return subprocess.run([self.POWERSHELL, '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', str(LAUNCHER),
                               *args], capture_output=True, text=True, env=env, timeout=120)

    def test_dry_run_shows_the_command_and_starts_nothing(self):
        names, expected, _, _ = launcher_counts(LAUNCHER.read_text(encoding='utf-8-sig'))
        with tempfile.TemporaryDirectory() as tmp:
            lock = Path(tmp) / 'ue_run.lock'
            done = self.run_launcher('-DryRun', '-Engine', str(Path(tmp) / 'Invented UE'),
                                     env=dict(os.environ, OPENWILLOW_UE_LOCK=str(lock)))
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertFalse(lock.exists())
        out = done.stdout
        self.assertIn('DRY RUN (nothing started):', out)
        self.assertIn('Automation RunTests ' + '+'.join(names) + '; Quit', out)
        for flag in ('-game', '-windowed', '-ResX=1280', '-ResY=720', '-d3d11', '-unattended', '-nosplash', '-abslog='):
            self.assertIn(' ' + flag, out)
        self.assertRegex(out, r'local[\\/]automation[\\/]run-\d{8}-\d{6}\.log')
        self.assertIn(f'expected={expected} ', out)
        self.assertIn('editor_found=False', out)

    def test_tests_without_expected_is_refused(self):
        done = self.run_launcher('-DryRun', '-Tests', 'Fake.Invented')
        self.assertNotEqual(done.returncode, 0)
        self.assertIn('-Expected', done.stdout + done.stderr)


    def test_a_positional_or_prefix_override_is_refused_by_the_launcher(self):
        # Positional binding is off, so a bare value is an error; a prefix still binds, and -Tests without -Expected is refused.
        done = self.run_launcher('-DryRun', 'Fake.Invented')
        self.assertNotEqual(done.returncode, 0, done.stdout)
        done = self.run_launcher('-DryRun', '-Te', 'Fake.Invented')
        self.assertNotEqual(done.returncode, 0, done.stdout)
        self.assertIn('-Expected', done.stdout + done.stderr)


if not SHELLS:
    # The dry-run cases tie the -ExecCmds line to the -Tests defaults. Without any PowerShell they are not made; this skip says so
    # in the run (it exists only on such a host, so a run with a shell shows no skip).
    class NoShellTest(unittest.TestCase):
        @unittest.skip('no powershell or pwsh on this host: the launcher dry-run cases (DryRunCases) did not run')
        def test_dry_run_cases_need_a_powershell(self):
            pass


for _name, _path in SHELLS.items():
    globals()[f'DryRun_{_name}'] = type(f'DryRun_{_name}', (DryRunCases, unittest.TestCase), {'POWERSHELL': _path})

if __name__ == '__main__':
    unittest.main()
