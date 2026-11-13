"""Synthetic tests for block_saves() and the reply publishing of run_pending() in tools/real_game/openwillow_realgame/__init__.py
(no game: the SDK calls are stubs).

Run: python tests/block_saves_test.py
"""
import ast
import contextlib
import io
from pathlib import Path
import tempfile
import traceback
import types

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'tools' / 'real_game' / 'openwillow_realgame' / '__init__.py'
# Written out here, not read from the driver, so a wrong name there that happens to exist in the game still fails.
EXPECTED = {'WillowGame.WillowSaveGameManager:' + name for name in ('SaveGame', 'Save', 'SaveRawData', 'SaveGraveyard')}
# Still ignoring the reply on this branch. These seven capture scripts (all unbarred on 2026-11-09; docs/TOOLING.md) get it from #157 and vendors from #160; an entry can go once that has
# landed, and the test stays green either way.
PENDING = {'action_skill_hud_capture', 'hud_integrity_capture', 'hud_layout_capture', 'hud_parity_capture', 'weapon_part_session',
           'projectiles_capture', 'weapon_parity2_capture', 'vendors_capture'}


def load(existing):
    """The driver's save-block code alone, as importing the module needs a running game. existing: the functions the stub game has."""
    tree = ast.parse(SOURCE.read_text(encoding='utf-8'))
    wanted = [n for n in tree.body if (isinstance(n, ast.FunctionDef) and n.name in ('_add_save_hooks', 'block_saves', '_block_save'))
              or (isinstance(n, ast.Assign) and getattr(n.targets[0], 'id', '') == 'SAVE_FUNCTIONS')]
    hooks = []

    def find_object(cls, path):
        assert cls == 'Function'
        if path not in existing:
            raise ValueError(f"Couldn't find object '{path}'")
        return path
    namespace = {'unrealsdk': types.SimpleNamespace(find_object=find_object), 'Type': types.SimpleNamespace(PRE=1), 'Block': object(),
                 'add_hook': lambda path, kind, name, fn: hooks.append((path, name))}
    exec(compile(ast.Module(wanted, []), str(SOURCE), 'exec'), namespace)
    return namespace, hooks


def test_all_found_hooks_exactly_the_expected_functions_and_answers_their_number():
    namespace, hooks = load(EXPECTED)
    assert namespace['block_saves']() == len(EXPECTED)
    assert {path for path, _ in hooks} == EXPECTED and len(hooks) == len(EXPECTED)


def test_a_missing_function_stops_before_any_hook_and_names_it():
    for missing in EXPECTED:
        namespace, hooks = load(EXPECTED - {missing})
        try:
            namespace['block_saves']()
        except RuntimeError as error:
            assert missing in str(error)
        else:
            raise AssertionError('a missing save function was accepted')
        assert hooks == []


def test_a_start_up_hook_needs_no_proof():
    # The flag path runs while the mod loads, perhaps before the class is; the capture's block_saves() call later is the proof.
    namespace, hooks = load([])
    namespace['_add_save_hooks']()
    assert {path for path, _ in hooks} == EXPECTED


def test_the_flag_path_hooks_without_the_proof_and_the_capture_gets_the_proof():
    text = SOURCE.read_text(encoding='utf-8')
    assert 'autoblock.flag").is_file():\n    _add_save_hooks()' in text.replace('\r\n', '\n')


def ignores_the_answer(line):
    code = line.split('#', 1)[0]
    return 'block_saves()' in code and 'throw' not in code and 'Assert-SavesBlocked (' not in code


def calls_without_the_check():
    """Every script line that calls block_saves() and does not stop on its answer, as (script, line number)."""
    found = []
    for script in (ROOT / 'tools').rglob('*.ps1'):
        for number, line in enumerate(script.read_text(encoding='utf-8').splitlines(), 1):
            if ignores_the_answer(line):
                found.append((script.stem, number))
    return found


def test_every_capture_stops_on_a_failed_block_saves():
    unchecked = [row for row in calls_without_the_check() if row[0] not in PENDING]
    assert not unchecked, unchecked


def test_the_scan_tells_a_checked_call_from_an_unchecked_one():
    # On synthetic lines, so it still proves the scan works once no real script is left to find.
    for line in ("$log.block_saves = Py 'print(block_saves())'", "Invoke-GamePy 'print(block_saves())' | Out-Null"):
        assert ignores_the_answer(line), line
    for line in ("$log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')", "# block_saves() answers the count", "throw 'block_saves() failed'"):
        assert not ignores_the_answer(line), line


def test_the_scan_reaches_the_captures():
    texts = [script.read_text(encoding='utf-8') for script in (ROOT / 'tools').rglob('*.ps1')]
    assert sum('Assert-SavesBlocked (' in text for text in texts) >= 3


def load_channel(path_class):
    """The driver's command loop alone (cmd_dir and run_pending), over a stand-in command folder. path_class stands in for Path, so a
    test can watch or break the reply's write and rename."""
    tree = ast.parse(SOURCE.read_text(encoding='utf-8'))
    wanted = [n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name in ('cmd_dir', 'run_pending')]
    tmp = Path(tempfile.mkdtemp(prefix='ow-driver-'))
    folder = tmp / 'cmd'
    folder.mkdir()
    (tmp / 'cmd_dir.txt').write_text(str(folder), encoding='utf-8')
    namespace = {'Path': path_class, '__file__': str(tmp / '__init__.py'), 'io': io, 'contextlib': contextlib, 'traceback': traceback,
                 'scope': {'runs': []}}
    exec(compile(ast.Module(wanted, []), str(SOURCE), 'exec'), namespace)
    return namespace, folder


def test_a_reply_appears_only_complete_and_the_script_is_done_before_it_runs():
    # Review of #263: the reply was written in place, so a reader could see it empty or cut off. It is written under <name>.out.tmp and
    # renamed onto <name>.out; the script is renamed to .py.done before it runs, so it never runs twice.
    seen = []

    class Watched(type(Path())):
        def write_text(self, *args, **kwargs):
            seen.append((self.name, self.with_name(self.name.replace('.out.tmp', '.out')).exists()))
            return super().write_text(*args, **kwargs)
    ns, folder = load_channel(Watched)
    script = folder / 'c1-invented.py'
    script.write_text("import os\nruns.append(os.path.exists(r'%s'))\nprint('invented reply ' * 50)" % script, encoding='utf-8')
    ns['run_pending']()
    assert ns['scope']['runs'] == [False]                       # already .py.done when it ran
    assert seen == [('c1-invented.out.tmp', False)], seen       # written under the other name, with no .out yet
    assert (folder / 'c1-invented.out').read_text(encoding='utf-8') == 'invented reply ' * 50 + '\n'
    assert sorted(p.name for p in folder.iterdir()) == ['c1-invented.out', 'c1-invented.py.done']


def test_a_failed_rename_leaves_no_reply_and_the_script_does_not_run_again():
    class Unrenamable(type(Path())):
        def replace(self, target):
            if self.name.endswith('.out.tmp'):   # the reply's rename only; the script's own .py.done rename goes through
                raise PermissionError('invented: the rename was refused')
            return super().replace(target)
    ns, folder = load_channel(Unrenamable)
    (folder / 'c2-invented.py').write_text("runs.append(1)\nprint('ok')", encoding='utf-8')
    (folder / 'c3-invented.py').write_text("runs.append(2)\nprint('ok')", encoding='utf-8')
    ns['run_pending']()
    ns['run_pending']()
    assert ns['scope']['runs'] == [1, 2]                        # each ran once; the failure did not stop the next one
    assert sorted(p.name for p in folder.iterdir()) == ['c2-invented.py.done', 'c3-invented.py.done']   # no reply, no .tmp left


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
