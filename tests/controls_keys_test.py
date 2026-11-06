"""The host's key bindings hang together: every bound action has a key and every mapped action is bound.

UE silently ignores a BindAction whose name has no mapping, so a typo in either file leaves a dead key that no build
reports. Reads only our own config and source, no game data.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parent.parent
INI = ROOT / 'host/ue5/OpenWillow/Config/DefaultInput.ini'
SOURCE = ROOT / 'host/ue5/OpenWillow/Source/OpenWillow/OpenWillowWalker.cpp'


def mapped_actions(text):
    """The action mappings of the OW actions as (action, key) pairs."""
    return re.findall(r'^\+ActionMappings=\(ActionName="(OW\w+)",Key=(\w+)\)', text, re.M)


def bound_actions(text):
    return set(re.findall(r'BindAction\(TEXT\("(OW\w+)"\)', text))


class ControlsKeysTest(unittest.TestCase):
    def test_parsers_on_a_synthetic_sample(self):
        self.assertEqual(mapped_actions('+ActionMappings=(ActionName="OWA",Key=M)\n+ActionMappings=(ActionName="Other",Key=N)\n'), [('OWA', 'M')])
        self.assertEqual(bound_actions('Input->BindAction(TEXT("OWA"), IE_Pressed, this, &X::Y);'), {'OWA'})

    def setUp(self):
        self.mapped = mapped_actions(INI.read_text(encoding='utf-8'))
        self.bound = bound_actions(SOURCE.read_text(encoding='utf-8'))

    def test_every_bound_action_has_a_key_and_every_key_a_binding(self):
        self.assertEqual({action for action, _ in self.mapped}, self.bound)

    def test_no_key_runs_two_actions(self):
        keys = [key for _, key in self.mapped]
        self.assertEqual(sorted(keys), sorted(set(keys)))

    def test_the_map_and_missions_keys(self):
        self.assertIn(('OWMap', 'M'), self.mapped)
        self.assertIn(('OWMissions', 'L'), self.mapped)


if __name__ == '__main__':
    unittest.main()
