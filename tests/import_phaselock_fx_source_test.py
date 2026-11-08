"""Source checks for host/ue5/import_phaselock_fx.py, which needs the editor's ``unreal`` module and so cannot be imported here.

AI-assisted (Claude), 2026-11-08, from PR #181 review r1: the refraction's SceneColor node was left in its default input mode, so its
offset pin did not exist, and the unchecked connection let the material compile without the shift. These checks pin the fix's shape:
every connection goes through the checked wrappers, the SceneColor node is created in OffsetFraction mode before the offset is linked,
and the suction material is an exact material with scene=True. They read the text only; the editor run is the real check.
Run: python tests/import_phaselock_fx_source_test.py
"""
import ast
from pathlib import Path

SOURCE = (Path(__file__).resolve().parents[1] / 'host/ue5/import_phaselock_fx.py').read_text(encoding='utf-8')
TREE = ast.parse(SOURCE)


def calls(tree):
    for node in ast.walk(tree):
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute):
            yield node


def enclosing_function(target):
    for fn in ast.walk(TREE):
        if isinstance(fn, ast.FunctionDef) and any(n is target for n in ast.walk(fn)):
            inner = [f for f in ast.walk(fn) if isinstance(f, ast.FunctionDef) and f is not fn and any(n is target for n in ast.walk(f))]
            return inner[-1].name if inner else fn.name
    return None


def test_every_connection_is_checked():
    raw = [c for c in calls(TREE) if c.func.attr in ('connect_material_expressions', 'connect_material_property')]
    assert raw and all(enclosing_function(c) in ('connect', 'connect_property') for c in raw), \
        [(c.lineno, enclosing_function(c)) for c in raw]
    for wrapper in ('def connect(a, a_pin, b, b_pin):', 'def connect_property(a, a_pin, prop):'):
        body = SOURCE.split(wrapper, 1)[1].split('\ndef ', 1)[0]
        assert 'if not mel.connect_material_' in body and 'raise RuntimeError' in body


def test_scene_color_is_in_offset_mode_before_its_offset_is_linked():
    created = SOURCE.index('MaterialExpressionSceneColor')
    mode = SOURCE.index('input_mode=unreal.MaterialSceneAttributeInputMode.OFFSET_FRACTION', created)
    linked = SOURCE.index("link(shift, '', scene, 'OffsetFraction')", created)
    assert created < mode < linked
    readback = SOURCE.index('get_inputs_for_material_expression(material, scene)', linked)
    assert 'scene_offset_from' in SOURCE[readback:readback + 600]


def test_suction_is_an_exact_scene_material():
    found = [c for c in ast.walk(TREE) if isinstance(c, ast.Call) and isinstance(c.func, ast.Name) and c.func.id == 'exact'
             and c.args and isinstance(c.args[0], ast.Constant) and c.args[0].value == 'Mat_SirenSuction']
    assert len(found) == 1
    assert any(k.arg == 'scene' and isinstance(k.value, ast.Constant) and k.value.value is True for k in found[0].keywords)
    assert "'scene_offset_from' not in report['materials']" in SOURCE


if __name__ == '__main__':
    tests = [v for k, v in sorted(globals().items()) if k.startswith('test_')]
    for test in tests:
        test()
    print(f'import_phaselock_fx_source_test: {len(tests)} passed')
