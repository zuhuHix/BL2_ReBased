"""Synthetic tests for host/ue5/phaselock_fx_inputs.py (no game data, no editor; names and files are invented).

Run: python tests/phaselock_fx_inputs_test.py
"""
import json
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'host' / 'ue5'))
import phaselock_fx_inputs as f  # noqa: E402

EXACT = {'Mat_Used': ['Tex_A'], 'Mat_Unused': ['Tex_Absent']}


def write_template(folder, name, material='X.Y.Mat_Used'):
    (folder / f'{name}.json').write_text(json.dumps({'digest': [{'required': {'Material': material}}]}), encoding='utf-8')


def complete(base):
    emitters, root = base / 'emitters', base / 'root'
    emitters.mkdir()
    (root / 'sub').mkdir(parents=True)
    (root / 'sub' / 'Tex_A.png').write_bytes(b'x')
    for name in f.REQUIRED_TEMPLATES:
        write_template(emitters, name)
    return emitters, root


def test_complete_inputs_pass_and_an_unused_material_needs_no_texture():
    with tempfile.TemporaryDirectory() as tmp:
        emitters, root = complete(Path(tmp))
        assert f.check_fx_inputs(emitters, [root], EXACT) == []


def test_missing_directory():
    with tempfile.TemporaryDirectory() as tmp:
        problems = f.check_fx_inputs(Path(tmp) / 'nope', [tmp], EXACT)
        assert len(problems) == 1 and 'does not exist' in problems[0]


def test_empty_directory_names_every_required_template():
    with tempfile.TemporaryDirectory() as tmp:
        emitters = Path(tmp) / 'emitters'
        emitters.mkdir()
        problems = f.check_fx_inputs(emitters, [tmp], EXACT)
        assert len(problems) == len(f.REQUIRED_TEMPLATES)
        assert all('is missing' in p for p in problems)


def test_one_missing_template():
    with tempfile.TemporaryDirectory() as tmp:
        emitters, root = complete(Path(tmp))
        (emitters / f'{f.REQUIRED_TEMPLATES[0]}.json').unlink()
        problems = f.check_fx_inputs(emitters, [root], EXACT)
        assert len(problems) == 1 and f.REQUIRED_TEMPLATES[0] in problems[0]


def test_bad_json_and_no_emitters():
    with tempfile.TemporaryDirectory() as tmp:
        emitters, root = complete(Path(tmp))
        (emitters / f'{f.REQUIRED_TEMPLATES[0]}.json').write_text('{not json', encoding='utf-8')
        (emitters / f'{f.REQUIRED_TEMPLATES[1]}.json').write_text('{"digest": []}', encoding='utf-8')
        problems = f.check_fx_inputs(emitters, [root], EXACT)
        assert len(problems) == 2
        assert any('unreadable' in p for p in problems) and any('no emitters' in p for p in problems)


def test_no_root_and_a_missing_texture():
    with tempfile.TemporaryDirectory() as tmp:
        emitters, root = complete(Path(tmp))
        problems = f.check_fx_inputs(emitters, [Path(tmp) / 'gone'], EXACT)
        assert len(problems) == 1 and 'export roots' in problems[0]
        (root / 'sub' / 'Tex_A.png').unlink()
        problems = f.check_fx_inputs(emitters, [root], EXACT)
        assert len(problems) == 1 and 'Tex_A' in problems[0]


def test_constants_complete_missing_and_malformed():
    good = {material: {name: [0.1, 0.2, 0.3]} for material, name in f.REQUIRED_CONSTANTS}
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / 'constants.json'
        assert 'is missing' in f.load_fx_constants(path)[1][0]
        path.write_text('{not json', encoding='utf-8')
        assert 'unreadable' in f.load_fx_constants(path)[1][0]
        path.write_text(json.dumps(good), encoding='utf-8')
        assert f.load_fx_constants(path) == (good, [])
        material, name = f.REQUIRED_CONSTANTS[0]
        for bad in ([0.1, 0.2], [0.1, 0.2, 'x'], [0.1, 0.2, True], 'x'):
            changed = dict(good, **{material: {name: bad}})
            path.write_text(json.dumps(changed), encoding='utf-8')
            problems = f.load_fx_constants(path)[1]
            assert len(problems) == 1 and f'{material}.{name}' in problems[0]
        path.write_text(json.dumps({'_source': 'note'}), encoding='utf-8')
        assert len(f.load_fx_constants(path)[1]) == len(f.REQUIRED_CONSTANTS)


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
