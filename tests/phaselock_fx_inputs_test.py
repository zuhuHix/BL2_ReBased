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


def template(material='X.Y.Mat_Used', **emitter):
    return {'digest': [{'required': {'Material': material}, **emitter}]}


def write_template(folder, name, data=None):
    (folder / f'{name}.json').write_text(json.dumps(template() if data is None else data), encoding='utf-8')


def complete(base):
    emitters, root = base / 'emitters', base / 'root'
    emitters.mkdir()
    (root / 'sub').mkdir(parents=True)
    (root / 'sub' / 'Tex_A.png').write_bytes(b'x')
    for name in f.REQUIRED_TEMPLATES:
        write_template(emitters, name)
    return emitters, root


def generic(material='X.Y.Mat_Generic', blend='BLEND_Additive', **emitter):
    data = template(material, **emitter)
    data['materials'] = {material: {'properties': {'BlendMode': blend}}}
    return data


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


def test_wrong_shapes_are_named_not_raised():
    bad = {'root array': [1, 2], 'digest object': {'digest': {'a': 1}}, 'emitter not object': {'digest': [3]},
           'required not object': {'digest': [{'required': 'x'}]}, 'materials array': {'materials': [1], **template()},
           'type_data not object': template(type_data='x'), 'properties not object': template(type_data={'properties': 'x'})}
    for label, data in bad.items():
        with tempfile.TemporaryDirectory() as tmp:
            emitters, root = complete(Path(tmp))
            write_template(emitters, f.REQUIRED_TEMPLATES[0], data)
            problems = f.check_fx_inputs(emitters, [root], EXACT)
            assert len(problems) == 1 and f.REQUIRED_TEMPLATES[0] in problems[0], (label, problems)


def test_no_root_and_a_missing_exact_texture():
    with tempfile.TemporaryDirectory() as tmp:
        emitters, root = complete(Path(tmp))
        problems = f.check_fx_inputs(emitters, [Path(tmp) / 'gone'], EXACT)
        assert len(problems) == 1 and 'export roots' in problems[0]
        (root / 'sub' / 'Tex_A.png').unlink()
        problems = f.check_fx_inputs(emitters, [root], EXACT)
        assert len(problems) == 1 and 'Tex_A' in problems[0]
        assert f.check_fx_inputs(emitters, [root], EXACT, not_drawn=('Mat_Used',)) == []


def test_generic_material_textures():
    with tempfile.TemporaryDirectory() as tmp:
        emitters, root = complete(Path(tmp))
        write_template(emitters, f.REQUIRED_TEMPLATES[0], generic())
        no_mat = f.check_fx_inputs(emitters, [root], EXACT)
        assert len(no_mat) == 1 and 'Mat_Generic' in no_mat[0] and 'no texture' in no_mat[0]
        (root / 'sub' / 'Mat_Generic.mat').write_text('Diffuse=Tex_G\nOther[0]=Soft_Mirror\n', encoding='utf-8')
        problems = f.check_fx_inputs(emitters, [root], EXACT)
        assert len(problems) == 2 and any('Tex_G' in p for p in problems) and any('mask Soft_Mirror' in p for p in problems)
        (root / 'sub' / 'Tex_G.png').write_bytes(b'x')
        (root / 'sub' / 'Soft_Mirror.png').write_bytes(b'x')
        assert f.check_fx_inputs(emitters, [root], EXACT) == []


def test_generic_material_modes():
    with tempfile.TemporaryDirectory() as tmp:
        emitters, root = complete(Path(tmp))
        write_template(emitters, f.REQUIRED_TEMPLATES[0], generic(blend='BLEND_Masked'))
        assert 'not hosted' in f.check_fx_inputs(emitters, [root], EXACT)[0]
        write_template(emitters, f.REQUIRED_TEMPLATES[0], generic(blend='BLEND_Modulate'))
        assert f.check_fx_inputs(emitters, [root], EXACT) == []    # a modulate needs no texture
        (root / 'sub' / 'Mat_Generic.mat').write_text('Diffuse=Tex_Nrm\n', encoding='utf-8')
        assert 'normal map' in f.check_fx_inputs(emitters, [root], EXACT)[0]


def test_optional_materials_are_the_only_skips():
    optional = next(iter(f.OPTIONAL_MATERIALS))
    with tempfile.TemporaryDirectory() as tmp:
        emitters, root = complete(Path(tmp))
        write_template(emitters, f.REQUIRED_TEMPLATES[0], generic(f'X.Y.{optional}'))
        assert f.check_fx_inputs(emitters, [root], EXACT) == []
        write_template(emitters, f.REQUIRED_TEMPLATES[0], generic('X.Y.Mat_NotListed'))
        assert 'Mat_NotListed' in f.check_fx_inputs(emitters, [root], EXACT)[0]


def test_meshes():
    with tempfile.TemporaryDirectory() as tmp:
        emitters, root = complete(Path(tmp))
        write_template(emitters, f.REQUIRED_TEMPLATES[0], template(type_data={'properties': {'Mesh': 'X.Y.Mesh_A'}}))
        problems = f.check_fx_inputs(emitters, [root], EXACT)
        assert len(problems) == 1 and 'Mesh_A.gltf' in problems[0]
        (root / 'sub' / 'Mesh_A.gltf').write_text('{}', encoding='utf-8')
        assert f.check_fx_inputs(emitters, [root], EXACT) == []


def test_choose_texture():
    slots = {'Diffuse': 'Tex_D', 'Other[0]': 'Sprite_SubUV', 'Other[1]': 'Soft_Mirror'}
    assert f.choose_texture('M', slots, False) == ('Tex_D', 'Soft_Mirror')
    assert f.choose_texture('M', slots, True) == ('Sprite_SubUV', 'Soft_Mirror')
    forced = next(iter(f.TEXTURE_CHOICE))
    assert f.choose_texture(forced, {'Diffuse': 'Tex_D'}, False)[0] == f.TEXTURE_CHOICE[forced]


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
        path.write_text('[1, 2, 3]', encoding='utf-8')
        assert 'not a JSON object' in f.load_fx_constants(path)[1][0]
        for bad in ([0.1, 0.2], [0.1, 0.2, 'x'], [0.1, 0.2, True], 'x', [0.1, 0.2, float('nan')]):
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
