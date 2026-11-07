"""Synthetic tests for block_saves() in tools/real_game/openwillow_realgame/__init__.py (no game: the SDK calls are stubs).

Run: python tests/block_saves_test.py
"""
import ast
from pathlib import Path
import types

SOURCE = Path(__file__).resolve().parents[1] / 'tools' / 'real_game' / 'openwillow_realgame' / '__init__.py'


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


def paths(namespace):
    return ['WillowGame.WillowSaveGameManager:' + name for name in namespace['SAVE_FUNCTIONS']]


def test_all_found_hooks_all_and_answers_the_count():
    namespace, _ = load([])
    names = paths(namespace)
    namespace, hooks = load(names)
    assert namespace['block_saves']() == len(names) == 4
    assert [path for path, _ in hooks] == names


def test_a_missing_function_stops_before_any_hook_and_names_it():
    namespace, _ = load([])
    names = paths(namespace)
    for missing in names:
        namespace, hooks = load([n for n in names if n != missing])
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
    assert len(hooks) == 4


def test_the_flag_path_hooks_without_the_proof_and_the_capture_gets_the_proof():
    text = SOURCE.read_text(encoding='utf-8')
    assert 'autoblock.flag").is_file():\n    _add_save_hooks()' in text.replace('\r\n', '\n')


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
