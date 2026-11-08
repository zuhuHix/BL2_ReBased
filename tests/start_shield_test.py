"""Checks the text of the start shield (Maya starts in the stand-in shield in normal play) with no editor: the walker's BeginPlay
order and its opt-out switch, and that each launcher which needs the no-start-shield baseline passes -ownostartshield.
Synthetic, source-text checks only: nothing here runs the host. The behaviour is UNVERIFIED in the game (docs/verification/COMBAT_DOWN_RESPAWN.md)."""
import re
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
SOURCE = REPO / 'host' / 'ue5' / 'OpenWillow' / 'Source' / 'OpenWillow'


def text(path):
    return path.read_text(encoding='utf-8')


class StartShieldWalkerTest(unittest.TestCase):
    def setUp(self):
        self.walker = text(SOURCE / 'OpenWillowWalker.cpp')

    def test_start_block_sits_between_the_gear_fixture_and_the_health_refresh(self):
        gear_fixture = self.walker.index('if (bCombatShotsRun)\n    {\n        // Test-only: wear whatever gear')
        start = self.walker.index('ownostartshield')
        refresh = self.walker.index('RefreshHealthForLevel();\n    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya level')
        self.assertLess(gear_fixture, start)
        self.assertLess(start, refresh)

    def test_start_block_only_fills_an_empty_shield_slot(self):
        self.assertIn('!Inventory->GearSlotItem(TEXT("shield"))', self.walker)

    def test_start_block_logs_one_line_when_the_rule_does_not_load(self):
        self.assertIn('ReadStandInShieldRule(RuleError)', self.walker)
        self.assertIn('OpenWillow Maya starts with no shield: %s', self.walker)
        # no stand-in capacity is invented: the start equip only runs once the rule has loaded
        self.assertNotRegex(self.walker, r'EquipShield\(\s*\d')

    def test_shield_check_and_stand_in_switches_skip_the_start_shield(self):
        # review r1 (P2): these switches add their own stand-in, so the start shield would be a second one
        block = self.walker[self.walker.index('ownostartshield'):self.walker.index('RefreshHealthForLevel();\n    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya level')]
        self.assertIn('FParse::Param(FCommandLine::Get(), TEXT("owshieldcheck"))', block)
        self.assertIn('FParse::Param(FCommandLine::Get(), TEXT("owstandinshield"))', block)
        self.assertIn('-owshieldcheck or -owstandinshield owns the shield', block)
        # the skip is decided before the rule is read or the start stand-in is equipped
        self.assertLess(block.index('owshieldcheck'), block.index('ReadStandInShieldRule(RuleError)'))

    def test_no_brace_less_UE_LOG_before_else(self):
        # C2181: an if with a UE_LOG and no braces, followed by else (project memory: UE_LOG before else)
        block = self.walker[self.walker.index('ownostartshield'):self.walker.index('RefreshHealthForLevel();\n    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya level')]
        self.assertNotRegex(block, r'UE_LOG\([^;]*\);\s*\n\s*else')

    def test_rule_is_read_once_and_logs_only_success(self):
        body = self.walker[self.walker.index('bool AOpenWillowWalker::ReadStandInShieldRule'):]
        body = body[:body.index('void AOpenWillowWalker::TickShield')]
        self.assertIn('if (!bStandInShieldRuleRead)', body)
        self.assertEqual(body.count('UE_LOG('), 1)


class LauncherOptOutTest(unittest.TestCase):
    def assertOptsOut(self, name):
        self.assertIn("'-ownostartshield'", text(REPO / 'tools' / name), name)

    def test_inventory_actions_starts_with_no_shield(self):
        self.assertOptsOut('test_inventory_actions.ps1')

    def test_range_starts_with_no_shield(self):
        self.assertOptsOut('test_range.ps1')

    def test_hud_captures_start_with_no_shield(self):
        self.assertOptsOut('run_hud_shots.ps1')

    def test_slice_gate_variants_opt_out_and_the_plain_run_does_not(self):
        source = text(REPO / 'tools' / 'test_slice_gate.ps1')
        # the switch is added only with -Shield or -DownAfterFight, never in the base argument list
        self.assertRegex(source, r"if \(\$Shield -or \$DownAfterFight\) \{ \$arguments \+= '-ownostartshield' \}")
        base = source[source.index('$arguments = @('):source.index("if ($Gun)")]
        self.assertNotIn('-ownostartshield', base)


if __name__ == '__main__':
    unittest.main()
