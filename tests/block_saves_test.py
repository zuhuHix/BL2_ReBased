"""Synthetic tests for block_saves() in tools/real_game/openwillow_realgame/__init__.py (no game: the SDK calls are stubs).

Run: python tests/block_saves_test.py
"""
import ast
from pathlib import Path
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


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
