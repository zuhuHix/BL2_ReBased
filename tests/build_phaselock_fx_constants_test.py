"""Synthetic tests for tools/build_phaselock_fx_constants.py (no game data, no editor; the values are invented).

Run: python tests/build_phaselock_fx_constants_test.py
"""
import json
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import build_phaselock_fx_constants as b  # noqa: E402

BLUE = {'R': 0.1, 'G': 0.2, 'B': 0.3, 'A': 1}
GLOW = {'R': 1, 'G': 2, 'B': 3, 'A': 0}
VECTOR = 'MaterialExpressionVectorParameter'


def write(base, tint=(0.4, 0.5, 0.6), blue=BLUE, glow=GLOW):
    emitters = base / 'emitters'
    emitters.mkdir()
    (emitters / f'{b.ORB_TEMPLATE}.json').write_text(json.dumps({'materials': {f'X.Y.{b.ORB_MATERIAL}': {
        'parameter_expressions': [{'class': VECTOR, 'DefaultValue': blue}]}}}), encoding='utf-8')
    manifest = base / 'fx_manifest.json'
    manifest.write_text(json.dumps({'tattooGlow': {'materialColours': {b.GLOW_PARAMETER: glow}}}), encoding='utf-8')
    out = base / 'fx_constants.json'
    if tint is not None:
        out.write_text(json.dumps({'Mat_PowerUpTwirls': {'tint': list(tint)}, 'Mat_SirenHandInnerOrb': {'blue': [9, 9, 9]}}), encoding='utf-8')
    return emitters, manifest, out


def expect_error(text, *args):
    try:
        b.build(*args)
    except b.BuildError as error:
        assert text in str(error), str(error)
        return
    raise AssertionError(f'expected a BuildError about {text}')


def test_rebuilds_blue_and_glow_and_keeps_the_tint():
    with tempfile.TemporaryDirectory() as tmp:
        emitters, manifest, out = write(Path(tmp))
        b.build(emitters, manifest, out)
        data = json.loads(out.read_text(encoding='utf-8'))
        assert data['Mat_SirenHandInnerOrb']['blue'] == [0.1, 0.2, 0.3]     # replaced
        assert data['Master_Player']['power_emissive'] == [1, 2, 3]
        assert data['Mat_PowerUpTwirls']['tint'] == [0.4, 0.5, 0.6]         # kept
        assert '_source' in data
        assert sorted(p.name for p in Path(tmp).iterdir()) == ['emitters', 'fx_constants.json', 'fx_manifest.json']


def test_no_file_and_no_tint_stop_and_write_nothing():
    with tempfile.TemporaryDirectory() as tmp:
        emitters, manifest, out = write(Path(tmp), tint=None)
        expect_error('no Mat_PowerUpTwirls.tint', emitters, manifest, out)
        assert not out.exists()
        out.write_text('{"Mat_SirenHandInnerOrb": {"blue": [9, 9, 9]}}', encoding='utf-8')
        before = out.read_bytes()
        expect_error('no Mat_PowerUpTwirls.tint', emitters, manifest, out)
        assert out.read_bytes() == before


def test_a_malformed_tint_stops_and_leaves_the_file():
    for bad in ([0.4, 0.5], ['x', 0.5, 0.6]):
        with tempfile.TemporaryDirectory() as tmp:
            emitters, manifest, out = write(Path(tmp), tint=bad)
            before = out.read_bytes()
            expect_error('Mat_PowerUpTwirls.tint', emitters, manifest, out)
            assert out.read_bytes() == before
            assert not out.with_name(out.name + '.tmp').exists()


def test_missing_inputs_leave_the_file():
    with tempfile.TemporaryDirectory() as tmp:
        emitters, manifest, out = write(Path(tmp))
        before = out.read_bytes()
        (emitters / f'{b.ORB_TEMPLATE}.json').unlink()
        expect_error(b.ORB_TEMPLATE, emitters, manifest, out)
        assert out.read_bytes() == before
    with tempfile.TemporaryDirectory() as tmp:
        emitters, manifest, out = write(Path(tmp))
        manifest.unlink()
        expect_error('manifest', emitters, manifest, out)


def test_wrong_shapes_are_named():
    cases = {
        'array root': lambda e, m: (e / f'{b.ORB_TEMPLATE}.json').write_text('[1]', encoding='utf-8'),
        'no vector': lambda e, m: (e / f'{b.ORB_TEMPLATE}.json').write_text(json.dumps(
            {'materials': {f'X.Y.{b.ORB_MATERIAL}': {'parameter_expressions': []}}}), encoding='utf-8'),
        'two vectors': lambda e, m: (e / f'{b.ORB_TEMPLATE}.json').write_text(json.dumps(
            {'materials': {f'X.Y.{b.ORB_MATERIAL}': {'parameter_expressions': [{'class': VECTOR, 'DefaultValue': BLUE}] * 2}}}), encoding='utf-8'),
        'bad colour': lambda e, m: (e / f'{b.ORB_TEMPLATE}.json').write_text(json.dumps(
            {'materials': {f'X.Y.{b.ORB_MATERIAL}': {'parameter_expressions': [{'class': VECTOR, 'DefaultValue': {'R': 1}}]}}}), encoding='utf-8'),
        'manifest array': lambda e, m: m.write_text('[1]', encoding='utf-8'),
    }
    for label, damage in cases.items():
        with tempfile.TemporaryDirectory() as tmp:
            emitters, manifest, out = write(Path(tmp))
            damage(emitters, manifest)
            try:
                b.build(emitters, manifest, out)
            except b.BuildError:
                continue
            raise AssertionError(label)


def test_non_finite_colour_stops():
    with tempfile.TemporaryDirectory() as tmp:
        emitters, manifest, out = write(Path(tmp), glow={'R': float('nan'), 'G': 2, 'B': 3})
        before = out.read_bytes()
        expect_error('Master_Player.power_emissive', emitters, manifest, out)
        assert out.read_bytes() == before


def test_cli_refuses_a_path_outside_local():
    with tempfile.TemporaryDirectory() as tmp:
        assert b.main(['--out', str(Path(tmp) / 'x.json')]) == 2


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
