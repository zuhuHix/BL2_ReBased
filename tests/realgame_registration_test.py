"""Guard: every test of the real-game tools is either registered in ctest or named, with its reason, in OW_OUTSIDE_CTEST.

AI-assisted (Claude Code), 2026-11-13. The tools under tools/real_game/ drive the installed game with the maintainer's saves; their
safety is proven only by stubbed tests under tests/. On 2026-11-13 six of those test files ran in no ctest entry, so CI never ran
them and a change could break them unseen. This test fails when
- a tests/*_test.py that imports realgame_test_env, or names a tools/real_game/ path, is neither started by a ctest entry in
  tests/CMakeLists.txt (a name in one of its OW_*_TESTS lists, or an add_test naming tests/<file>) nor in OW_OUTSIDE_CTEST;
- an OW_OUTSIDE_CTEST entry has no reason (a comment on its own line), names no existing test file, or is also in an OW_*_TESTS
  list (registered whole, so it is not outside). A file split in two, a fast part in its own add_test and the rest left out, is in
  both an add_test and OW_OUTSIDE_CTEST; that is allowed.

The synthetic cases use invented file names and invented CMake text (rule 9); the last class reads this checkout.
"""
from pathlib import Path
import ast
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
CMAKE = ROOT / 'tests' / 'CMakeLists.txt'
OUTSIDE = 'OW_OUTSIDE_CTEST'

ENV_MODULE = 'realgame_test_env'
_TOOL_PATH = re.compile(r'\breal_game[/\\\'"]')


def imports_env(source):
    """True when the source imports realgame_test_env in any form: `import realgame_test_env`, `import os, realgame_test_env`, an
    aliased import, `from realgame_test_env import ...`, `from <package> import realgame_test_env`, also inside a block. Read with ast,
    so the name in a string or a comment does not count. A file ast cannot parse counts when it names the module at all (it fails on
    its own anyway, and the guard should not let it slip through)."""
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return ENV_MODULE in source
    for node in ast.walk(tree):
        if isinstance(node, ast.Import) and any(alias.name.split('.')[-1] == ENV_MODULE for alias in node.names):
            return True
        if isinstance(node, ast.ImportFrom):
            if (node.module or '').split('.')[-1] == ENV_MODULE or any(alias.name == ENV_MODULE for alias in node.names):
                return True
    return False


def is_realgame_test(source):
    """True for a test that imports the real-game test environment or names a tools/real_game/ path."""
    return imports_env(source) or bool(_TOOL_PATH.search(source))


def _set_blocks(cmake):
    """{variable: [(line, trailing comment or ''), ...]} for each set(OW_...) block, comments kept per line."""
    blocks, name, rows = {}, None, []
    for line in cmake.splitlines():
        code, _, comment = line.partition('#')   # a comment may hold brackets; only the code part ends a block
        if name is None:
            start = re.match(r'\s*set\((OW_\w+)', code)
            if not start:
                continue
            name, rows, code = start.group(1), [], code[start.end():]
        body, closed, _ = code.partition(')')
        rows.extend((word, comment.strip()) for word in body.split())
        if closed:
            blocks[name], name = rows, None
    return blocks


def registration(cmake):
    """(listed, named, outside): names in OW_*_TESTS lists, names an add_test starts by path, and {name: reason} of OW_OUTSIDE_CTEST."""
    blocks = _set_blocks(cmake)
    listed = {word for var, rows in blocks.items() if var.endswith('_TESTS') for word, _ in rows}
    code = '\n'.join(line.partition('#')[0] for line in cmake.splitlines())   # a commented-out add_test registers nothing
    named = set(re.findall(r'add_test\([^)]*?tests/(\w+)_test\.py', code, re.S))
    outside = {word: reason for word, reason in blocks.get(OUTSIDE, [])}
    return listed, named, outside


def problems(cmake, sources):
    """Every rule broken, as text. `sources` maps a test name (file name without _test.py) to its text."""
    listed, named, outside = registration(cmake)
    found = []
    for name in sorted(sources):
        if is_realgame_test(sources[name]) and name not in listed | named | set(outside):
            found.append(f'{name}_test.py tests the real-game tools but is in no ctest entry and not in {OUTSIDE}')
    for name, reason in sorted(outside.items()):
        if not reason:
            found.append(f'{OUTSIDE} entry {name} gives no reason (a # comment on its line)')
        if name not in sources:
            found.append(f'{OUTSIDE} entry {name} names no tests/{name}_test.py')
        if name in listed:
            found.append(f'{name} is in {OUTSIDE} and also registered whole in an OW_*_TESTS list')
    return found


def checkout_sources():
    return {p.name[:-len('_test.py')]: p.read_text(encoding='utf-8', errors='replace') for p in (ROOT / 'tests').glob('*_test.py')}


CMAKE_FIXTURE = """
add_test(NAME alpha-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/alpha_test.py)
add_test(NAME beta-fast COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/beta_test.py FastPart)
set(OW_STANDALONE_PY_TESTS
  gamma delta)
set(OW_OUTSIDE_CTEST
  beta     # the slow part: 300 s
  epsilon  # needs an editor
  )
"""
ENV = 'import realgame_test_env  # noqa\n'
TOOL = "SCRIPT = ROOT / 'tools/real_game/zeta_capture.ps1'\n"


class GuardRulesTest(unittest.TestCase):
    def test_a_registered_or_listed_realgame_test_passes(self):
        sources = {'alpha': ENV, 'beta': ENV, 'gamma': TOOL, 'delta': 'x = 1\n', 'epsilon': ENV}
        self.assertEqual(problems(CMAKE_FIXTURE, sources), [])

    def test_an_unregistered_realgame_test_fails(self):
        for text in (ENV, TOOL, "path = ROOT / 'tools' / 'real_game' / 'x.ps1'\n"):
            sources = {'alpha': ENV, 'beta': ENV, 'epsilon': ENV, 'omega': text}
            found = problems(CMAKE_FIXTURE, sources)
            self.assertEqual(len(found), 1, found)
            self.assertIn('omega_test.py', found[0])

    def test_every_import_form_of_the_environment_counts(self):
        for text in ('from realgame_test_env import remove_tree\n', 'import os, realgame_test_env\n',
                     'import realgame_test_env as rg_env\n', 'from tests import realgame_test_env\n',
                     'from tests.realgame_test_env import remove_tree\n', 'if True:\n    import realgame_test_env\n'):
            sources = {'alpha': ENV, 'beta': ENV, 'epsilon': ENV, 'omega': text}
            found = problems(CMAKE_FIXTURE, sources)
            self.assertEqual(len(found), 1, (text, found))
            self.assertIn('omega_test.py', found[0])

    def test_the_module_name_in_a_string_or_a_comment_is_not_an_import(self):
        for text in ("NOTE = 'import realgame_test_env'\n", '# import realgame_test_env\n'):
            self.assertFalse(is_realgame_test(text), text)

    def test_a_commented_out_add_test_registers_nothing(self):
        cmake = CMAKE_FIXTURE + '# add_test(NAME omega-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/omega_test.py)\n'
        found = problems(cmake, {'alpha': ENV, 'beta': ENV, 'epsilon': ENV, 'omega': ENV})
        self.assertEqual(len(found), 1, found)
        self.assertIn('omega_test.py', found[0])

    def test_an_unrelated_unregistered_test_is_not_this_guards_business(self):
        sources = {'alpha': ENV, 'beta': ENV, 'epsilon': ENV, 'omega': 'import json\nrealgame = 1\n'}
        self.assertEqual(problems(CMAKE_FIXTURE, sources), [])

    def test_an_outside_entry_needs_a_reason_and_a_file(self):
        cmake = CMAKE_FIXTURE.replace('epsilon  # needs an editor', 'epsilon')
        found = problems(cmake, {'alpha': ENV, 'beta': ENV})
        self.assertIn(f'{OUTSIDE} entry epsilon gives no reason (a # comment on its line)', found)
        self.assertIn(f'{OUTSIDE} entry epsilon names no tests/epsilon_test.py', found)

    def test_an_outside_entry_registered_whole_fails(self):
        cmake = CMAKE_FIXTURE.replace('gamma delta', 'gamma delta epsilon')
        found = problems(cmake, {'alpha': ENV, 'beta': ENV, 'epsilon': ENV})
        self.assertEqual(found, [f'epsilon is in {OUTSIDE} and also registered whole in an OW_*_TESTS list'])

    def test_a_list_that_is_only_a_comment_does_not_count(self):
        cmake = '# too slow: omega\nset(OW_STANDALONE_PY_TESTS\n  gamma)\n'
        found = problems(cmake, {'omega': ENV, 'gamma': ENV})
        self.assertEqual(len(found), 1, found)


class CheckoutTest(unittest.TestCase):
    def test_this_checkout(self):
        sources = checkout_sources()
        self.assertTrue(any(is_realgame_test(text) for text in sources.values()))
        _, _, outside = registration(CMAKE.read_text(encoding='utf-8'))
        self.assertTrue(outside, f'{OUTSIDE} not found in tests/CMakeLists.txt')
        self.assertEqual(problems(CMAKE.read_text(encoding='utf-8'), sources), [])


if __name__ == '__main__':
    unittest.main()
