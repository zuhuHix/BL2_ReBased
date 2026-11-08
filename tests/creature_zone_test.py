"""Synthetic tests for tools/creature_zone.py (no game data; names follow the material's parameter names, values are invented).

Run: python tests/creature_zone_test.py
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import creature_zone as z  # noqa: E402

SLOT = {'vectors': {'p_AColorMidtone': [0.5, 0.25, 1.0, 1.0], 'p_ColorD': [0.2, 0.2, 0.2, 1.0],
                    'p_PatternColor': [1.0, 0.0, 0.0, 1.0], 'p_EmissiveScale': [1.0, 1.0, 1.0, 1.0], 'p_BColorShadow': None},
        'scalars': {}}


def test_vectors_are_kept_as_stored():
    out = z.zone_parameters(SLOT, 'Master_Creature')
    assert out['vectors'] == {'p_AColorMidtone': [0.5, 0.25, 1.0, 1.0], 'p_ColorD': [0.2, 0.2, 0.2, 1.0]}


def test_only_zone_vectors_pass():
    names = set(z.zone_parameters(SLOT)['vectors'])
    assert 'p_PatternColor' not in names and 'p_EmissiveScale' not in names and 'p_BColorShadow' not in names


def test_gamma_option_raises_rgb_only():
    out = z.zone_parameters(SLOT, gamma=True)['vectors']['p_AColorMidtone']
    assert abs(out[0] - 0.5 ** 2.2) < 1e-9 and abs(out[1] - 0.25 ** 2.2) < 1e-9 and out[2] == 1.0 and out[3] == 1.0


def test_master_defaults_when_not_overridden():
    assert z.zone_parameters(SLOT, 'Master_Creature')['scalars'] == {'p_HighlightsIntensity': 2.0, 'p_ShadowsIntensity': 2.0}


def test_other_or_unknown_master_keeps_the_earlier_scale():
    for master in (None, 'Master_SomethingElse'):
        assert z.zone_parameters(SLOT, master)['scalars'] == {'p_HighlightsIntensity': 1.0, 'p_ShadowsIntensity': 1.0}
    slot = {'scalars': {'p_ShadowsIntensity': 3.0}}
    assert z.zone_parameters(slot, None)['scalars'] == {'p_HighlightsIntensity': 1.0, 'p_ShadowsIntensity': 3.0}


def test_instance_overrides_win():
    slot = {'vectors': {}, 'scalars': {'p_HighlightsIntensity': 3.0, 'p_PatternIntensity': 7.0}}
    assert z.zone_parameters(slot, 'Master_Creature')['scalars'] == {'p_HighlightsIntensity': 3.0, 'p_ShadowsIntensity': 2.0}


def test_missing_fields():
    assert z.zone_parameters({}, 'Master_Creature') == {'vectors': {}, 'scalars': dict(z.MASTER_CREATURE_SCALARS)}


if __name__ == '__main__':
    tests = [v for k, v in sorted(globals().items()) if k.startswith('test_')]
    for test in tests:
        test()
    print(f'creature_zone_test: {len(tests)} passed')
