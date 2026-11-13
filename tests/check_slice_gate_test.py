"""Synthetic tests for tools/check_slice_gate.py (no game or engine needed; the logs are generated here).

Every line is invented: the link names come from the checker's own list, the details are "synthetic detail" or
short made-up phrases, and the logs are written to a temporary directory.
"""
import contextlib
import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_slice_gate as chk  # noqa: E402

PREFIX = '[2026.11.04-10.00.00:000][123]LogTemp: Display: '
TOTAL = len(chk.LINKS)


CARD_DETAIL = 'aimed_drop=item_x card_id=item_x picked_id=item_x accepted=1 synthetic detail'
DECOY_DETAIL = 'aimed_drop=item_x nearer_decoy=item_y card_id=item_x picked_id=item_x accepted=1 synthetic detail'
EQUIP_DETAIL = 'item_x equipped in slot 1 and drawn; firing=Bullet'


# The fixtures each link of the three older variants names in a real run (SLICE_GATE.md's tables), in one synthetic log.
OLD_VARIANT_FIXTURES = {
    'reach_fight_area': 'selected enemy on fallback shapes, no stock mesh',
    'kill_enemy': 'level rule overridden by the test: shield s1 of level 20 equipped on Maya of level 8; one shot at a time until the selected enemy was wounded; fire held until the selected enemy hit Maya',
    'enemy_attacks_maya': 'level rule overridden by the test: shield s1 of level 20 equipped on Maya of level 8; fire held until the selected enemy hit Maya',
    'loot_drop_on_kill': 'range set passive after the fight; lethal damage at the end of the fight; Maya revived to full health after the fight; guaranteed loot roll',
    'pick_up_gun': 'teleport; gun rolled and dropped by the test',
    'pick_up_aimed_not_nearest': 'two guns rolled and dropped by the test; teleport',
    'phaselock_enemy': 'phaselock point granted',
    'mission_accept': 'range cleared before the mission; teleport',
    'mission_objective': 'teleport',
    'mission_turn_in': 'teleport',
    'go_down': 'lethal damage applied by the test',
    'bleed_out_respawn': "down state from the test's lethal damage",
}


def build(results=None, stop_after=None, summary='auto', start=True, extra=(), fixtures=None, order=None, range_mode=False,
          unforced=False):
    """Synthetic log lines: a start line, one link line per link (PASS unless results says otherwise, and only up to
    stop_after), the extra lines, then the SUMMARY line (counted from the results, unless summary is given or None).
    fixtures maps a link name to its fixtures text; order, a list of link names, replaces the expected order;
    range_mode marks the run as one with the range on its start line; unforced marks it as the unforced variant, and its
    default details then carry each link's how= token."""
    results = results or {}
    lines = [PREFIX + f'OWSLICEGATE start map=Sanctuary_P quest=1 range_command={int(range_mode)}' + (' unforced=1' if unforced else '')] if start else []
    effective = {}
    fixtures = fixtures or {}
    for index, name in enumerate(order or chk.NAMES, 1):
        if stop_after is not None and index > stop_after:
            break
        default = {'pick_up_gun': CARD_DETAIL, chk.DECOY_LINK: DECOY_DETAIL, chk.EQUIP_LINK: EQUIP_DETAIL}.get(name, 'synthetic detail')
        if unforced and name in chk.UNFORCED_EVIDENCE:
            default = f'{chk.UNFORCED_EVIDENCE[name]} synthetic detail'
        result, detail = results.get(name, ('PASS', default))
        effective[name] = result
        lines.append(PREFIX + f'OWSLICEGATE link={name} result={result} fixtures=[{fixtures.get(name, "")}] detail={detail}')
    lines.append(PREFIX + 'OWSLICEGATE note synthetic note, ignored')
    lines.extend(extra)
    if summary == 'auto':
        values = list(effective.values())
        summary = (f'pass={values.count("PASS")} fail={values.count("FAIL")} '
                   f'na={values.count("NOT_AVAILABLE")}')
    if summary:
        lines.append(PREFIX + f'OWSLICEGATE SUMMARY {summary}')
    return lines


class SliceGateTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.dir = Path(self.tmp.name)

    def write_log(self, lines, bom=False, name='run.log'):
        data = ('\n'.join(lines) + '\n').encode('utf-8')
        if bom:
            data = b'\xef\xbb\xbf' + data
        path = self.dir / name
        path.write_bytes(data)
        return path

    def run_checker(self, path, *args):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = chk.main([str(path), *args])
        return code, out.getvalue()

    def row(self, output, index):
        """The table line for link number index (the row starts with the index)."""
        for line in output.splitlines():
            if line.startswith(f'{index:<2} '):
                return line
        self.fail(f'no table row for link {index}')

    def test_all_pass_exits_zero(self):
        code, out = self.run_checker(self.write_log(build()))
        self.assertEqual(code, 0)
        self.assertIn('slice gate: 14 PASS, 0 FAIL, 0 NOT_AVAILABLE of 14', out)
        header = out.splitlines()[0]
        self.assertEqual(header.split(), ['#', 'link', 'result', 'owner', 'detail'])
        self.assertNotIn('error', out)

    def test_not_available_exits_zero_and_shows_owner(self):
        results = {'enemy_attacks_maya': ('NOT_AVAILABLE', 'synthetic gap')}
        code, out = self.run_checker(self.write_log(build(results)))
        self.assertEqual(code, 0)
        self.assertIn('slice gate: 13 PASS, 0 FAIL, 1 NOT_AVAILABLE of 14', out)
        row = self.row(out, 4)
        self.assertIn('NOT_AVAILABLE', row)
        self.assertIn('range', row)
        self.assertIn('synthetic gap', row)
        self.assertEqual(self.row(out, 1).split()[3], '-', 'a PASS row shows no owner')

    def test_fail_exits_one(self):
        results = {'kill_enemy': ('FAIL', 'synthetic miss')}
        code, out = self.run_checker(self.write_log(build(results)))
        self.assertEqual(code, 1)
        self.assertIn('slice gate: 13 PASS, 1 FAIL, 0 NOT_AVAILABLE of 14', out)
        self.assertIn('range', self.row(out, 3))

    def test_a_refused_range_is_a_failure_that_keeps_its_reason(self):
        reason = 'range refused: kind made has no melee tables or swing range (an old or --no-vm manifest)'
        results = {'reach_fight_area': ('FAIL', reason)}
        code, out = self.run_checker(self.write_log(build(results, stop_after=chk.NAMES.index('reach_fight_area') + 1)))
        self.assertEqual(code, 1)
        self.assertIn(reason, out)

    def test_run_stopped_after_link_5_fails_the_rest(self):
        path = self.write_log(build(stop_after=5, summary=None))
        code, out = self.run_checker(path, '--json', str(self.dir / 'run.json'))
        self.assertEqual(code, 1)
        self.assertIn('slice gate: 5 PASS, 9 FAIL, 0 NOT_AVAILABLE of 14', out)
        self.assertIn('not reached: the run stopped before this link', self.row(out, 6))
        report = json.loads((self.dir / 'run.json').read_text(encoding='utf-8'))
        self.assertFalse(report['summary_seen'])
        self.assertIn('no SUMMARY line: the run did not finish (crash or timeout)', report['errors'])
        self.assertEqual(report['links'][5]['result'], 'FAIL')
        self.assertEqual(report['links'][5]['owner'], 'range')   # pick_up_gun, the sixth link
        self.assertEqual(report['links'][13]['owner'], 'maya')   # bleed_out_respawn, the last link

    def test_no_start_line_exits_two(self):
        code, out = self.run_checker(self.write_log(build(start=False)))
        self.assertEqual(code, 2)
        self.assertIn('no slice-gate run in this log', out)

    def test_duplicate_link_fails(self):
        extra = [PREFIX + 'OWSLICEGATE link=spawn_maya result=PASS detail=second try']
        code, out = self.run_checker(self.write_log(build(extra=extra)))
        self.assertEqual(code, 1)
        self.assertIn('error: line', out)
        self.assertIn('link spawn_maya logged a second time', out)

    def test_unknown_link_fails(self):
        extra = [PREFIX + 'OWSLICEGATE link=made_up_link result=PASS detail=synthetic']
        code, out = self.run_checker(self.write_log(build(extra=extra)))
        self.assertEqual(code, 1)
        self.assertIn('link made_up_link is not in the expected list', out)

    def test_shuffled_pass_lines_fail(self):
        order = list(chk.NAMES)
        order[2], order[7] = order[7], order[2]
        code, out = self.run_checker(self.write_log(build(order=order)))
        self.assertEqual(code, 1)
        self.assertIn('link equip_gun out of order (expected kill_enemy next)', out)
        self.assertIn('slice gate: 14 PASS, 0 FAIL, 0 NOT_AVAILABLE of 14', out)

    def test_link_after_summary_fails(self):
        lines = build(stop_after=13, summary='pass=13 fail=0 na=0')
        lines.append(PREFIX + 'OWSLICEGATE link=bleed_out_respawn result=PASS detail=late')
        code, out = self.run_checker(self.write_log(lines))
        self.assertEqual(code, 1)
        self.assertIn('link bleed_out_respawn after the SUMMARY line', out)

    def test_link_before_start_fails(self):
        lines = [PREFIX + 'OWSLICEGATE link=spawn_maya result=PASS detail=early'] + build(order=chk.NAMES[1:])
        code, out = self.run_checker(self.write_log(lines))
        self.assertEqual(code, 1)
        self.assertIn('link spawn_maya before the start line', out)

    def test_two_runs_in_one_log_fail(self):
        code, out = self.run_checker(self.write_log(build() + build()))
        self.assertEqual(code, 1)
        self.assertIn('a second start line', out)
        self.assertIn('a second SUMMARY line', out)

    def test_fixtures_reach_table_json_and_markdown(self):
        fixtures = {'phaselock_enemy': 'phaselock point granted', 'go_down': 'lethal damage; teleport'}
        json_path, md_path = self.dir / 'f.json', self.dir / 'f.md'
        code, out = self.run_checker(self.write_log(build(fixtures=fixtures)), '--json', str(json_path), '--markdown', str(md_path))
        self.assertEqual(code, 0)
        self.assertIn('PASS (fixture: phaselock point granted)', self.row(out, 9))
        self.assertIn('PASS (fixture: lethal damage; teleport)', self.row(out, 13))
        self.assertEqual(self.row(out, 1).split()[2], 'PASS')
        report = json.loads(json_path.read_text(encoding='utf-8'))
        self.assertEqual(report['links'][12]['fixtures'], ['lethal damage', 'teleport'])
        self.assertEqual(report['links'][0]['fixtures'], [])
        self.assertIn('| 9 | phaselock_enemy | PASS (fixture: phaselock point granted) | - |', md_path.read_text(encoding='utf-8'))

    def test_card_link_pass_with_different_ids_is_overruled(self):
        results = {'pick_up_gun': ('PASS', 'aimed_drop=item_a card_id=item_a picked_id=item_b accepted=1: synthetic')}
        code, out = self.run_checker(self.write_log(build(results)))
        self.assertEqual(code, 1)
        self.assertIn('FAIL', self.row(out, 6))
        self.assertIn('checker: card_id item_a is not picked_id item_b', self.row(out, 6))
        self.assertIn('error: link pick_up_gun: PASS overruled', out)

    def test_card_link_pass_without_ids_is_overruled(self):
        results = {'pick_up_aimed_not_nearest': ('PASS', 'synthetic, no ids')}
        code, out = self.run_checker(self.write_log(build(results)))
        self.assertEqual(code, 1)
        self.assertIn('checker: no card_id/picked_id in the detail', self.row(out, 7))

    def test_card_link_not_available_with_decoy_ids_is_overruled(self):
        detail = 'aimed_drop=item_a nearer_decoy=item_b card_id=item_a picked_id=item_b: E took the nearer decoy'
        results = {chk.DECOY_LINK: ('NOT_AVAILABLE', detail)}
        code, out = self.run_checker(self.write_log(build(results)))
        self.assertEqual(code, 1)
        self.assertIn('FAIL', self.row(out, 7))
        self.assertIn('checker: the ids show E took the nearer decoy', self.row(out, 7))
        self.assertIn('error: link pick_up_aimed_not_nearest: NOT_AVAILABLE overruled', out)

    def test_card_link_not_available_after_an_attempt_is_overruled(self):
        results = {'pick_up_gun': ('NOT_AVAILABLE', 'aimed_drop=item_a card_id=item_a picked_id=none accepted=0: synthetic')}
        code, out = self.run_checker(self.write_log(build(results)))
        self.assertEqual(code, 1)
        self.assertIn('checker: the ids show the pickup attempt was made', self.row(out, 6))

    def test_card_link_not_available_without_an_attempt_is_kept(self):
        results = {chk.DECOY_LINK: ('NOT_AVAILABLE', 'Maya down from a previous link, synthetic')}
        code, out = self.run_checker(self.write_log(build(results)))
        self.assertEqual(code, 0)
        self.assertIn('NOT_AVAILABLE', self.row(out, 7))
        self.assertIn('inventory', self.row(out, 7))
        self.assertNotIn('test setup gaps', out)

    def test_setup_gap_is_not_a_missing_host_capability(self):
        results = {chk.DECOY_LINK: ('NOT_AVAILABLE', chk.SETUP_GAP + ', synthetic: no source')}
        code, out = self.run_checker(self.write_log(build(results)))
        self.assertEqual(code, 0)
        self.assertIn('slice-gate', self.row(out, 7))
        self.assertNotIn('inventory', self.row(out, 7))
        self.assertIn('1 of those NOT_AVAILABLE are test setup gaps', out)

    def test_card_link_pass_needs_the_aimed_drop(self):
        for detail, message in (('card_id=item_b picked_id=item_b accepted=1: synthetic', 'checker: no aimed_drop in the detail'),
                                ('aimed_drop=item_a card_id=item_b picked_id=item_b accepted=1: synthetic',
                                 'checker: aimed_drop item_a is not picked_id item_b'),
                                ('aimed_drop=none card_id=item_b picked_id=item_b accepted=1: synthetic',
                                 'checker: aimed_drop none is not picked_id item_b')):
            code, out = self.run_checker(self.write_log(build({'pick_up_gun': ('PASS', detail)})))
            self.assertEqual(code, 1, detail)
            self.assertIn(message, self.row(out, 6))

    def test_decoy_link_pass_with_wrong_card_and_pick_is_overruled(self):
        detail = 'aimed_drop=item_a card_id=item_b picked_id=item_b accepted=1 nearer_decoy=item_c: synthetic'
        code, out = self.run_checker(self.write_log(build({chk.DECOY_LINK: ('PASS', detail)})))
        self.assertEqual(code, 1)
        self.assertIn('checker: aimed_drop item_a is not picked_id item_b', self.row(out, 7))

    def test_card_link_pass_with_all_ids_none_is_overruled(self):
        detail = 'aimed_drop=none card_id=none picked_id=none accepted=1: synthetic'
        code, out = self.run_checker(self.write_log(build({'pick_up_gun': ('PASS', detail)})))
        self.assertEqual(code, 1)
        self.assertIn('checker: card_id none is not picked_id none', self.row(out, 6))

    def test_decoy_link_pass_with_decoy_none_is_overruled(self):
        detail = 'aimed_drop=item_a nearer_decoy=none card_id=item_a picked_id=item_a accepted=1: synthetic'
        code, out = self.run_checker(self.write_log(build({chk.DECOY_LINK: ('PASS', detail)})))
        self.assertEqual(code, 1)
        self.assertIn('checker: no nearer_decoy id in the detail', self.row(out, 7))

    def test_link_line_without_fixtures_field_fails(self):
        lines = build()
        lines[1] = PREFIX + 'OWSLICEGATE link=spawn_maya result=PASS detail=no field'
        code, out = self.run_checker(self.write_log(lines))
        self.assertEqual(code, 1)
        self.assertIn('link spawn_maya has no fixtures field', out)

    def test_card_link_pass_not_accepted_is_overruled(self):
        for name, index, detail in (('pick_up_gun', 6, 'aimed_drop=item_a card_id=item_a picked_id=item_a accepted=0: synthetic'),
                                    (chk.DECOY_LINK, 7, 'aimed_drop=item_a nearer_decoy=item_b card_id=item_a picked_id=item_a: synthetic')):
            code, out = self.run_checker(self.write_log(build({name: ('PASS', detail)})))
            self.assertEqual(code, 1)
            self.assertIn('checker: the pickup was not accepted', self.row(out, index))

    def test_decoy_link_pass_with_shared_id_is_overruled(self):
        detail = 'aimed_drop=item_a nearer_decoy=item_a card_id=item_a picked_id=item_a accepted=1: synthetic'
        code, out = self.run_checker(self.write_log(build({chk.DECOY_LINK: ('PASS', detail)})))
        self.assertEqual(code, 1)
        self.assertIn('checker: nearer_decoy has the same id as the picked-up drop (item_a)', self.row(out, 7))

    def test_decoy_link_pass_without_decoy_id_is_overruled(self):
        detail = 'aimed_drop=item_a card_id=item_a picked_id=item_a accepted=1: synthetic'
        code, out = self.run_checker(self.write_log(build({chk.DECOY_LINK: ('PASS', detail)})))
        self.assertEqual(code, 1)
        self.assertIn('checker: no nearer_decoy id in the detail', self.row(out, 7))

    def test_equip_pass_names_each_loaded_firing_kind(self):
        for kind in ('Trace', 'Bullet', 'Rocket'):
            code, out = self.run_checker(self.write_log(build({'equip_gun': ('PASS', f'item_x drawn; firing={kind}')})))
            self.assertEqual(code, 0, out)

    def test_equip_pass_without_a_firing_kind_is_overruled(self):
        # A log from before link 8 graded the firing, a host that fell back to the plain trace, and a kind it cannot name.
        for detail, why in (('item_x equipped in slot 1 and drawn', 'no firing=<kind>'),
                            ('item_x drawn; firing=none: no block', 'firing=none'),
                            ('item_x drawn; refiring=Bullet', 'no firing=<kind>'),
                            ('item_x drawn; firing=Beam', 'firing=Beam')):
            code, out = self.run_checker(self.write_log(build({'equip_gun': ('PASS', detail)})))
            self.assertEqual(code, 1, (detail, out))
            self.assertIn(f'link equip_gun: PASS overruled, {why}', out)
            self.assertIn('checker: ', self.row(out, 8))

    def test_equip_fail_keeps_the_hosts_reason(self):
        code, out = self.run_checker(self.write_log(build({'equip_gun': ('FAIL', 'item_x drawn; firing=none: no firing block')})))
        self.assertEqual(code, 1, out)
        self.assertNotIn('overruled', out)

    def test_enemy_attack_not_available_fails_with_the_range(self):
        results = {'enemy_attacks_maya': ('NOT_AVAILABLE', 'synthetic gap')}
        code, out = self.run_checker(self.write_log(build(results, range_mode=True)))
        self.assertEqual(code, 1)
        self.assertIn('FAIL', self.row(out, 4))
        self.assertIn('error: link enemy_attacks_maya: NOT_AVAILABLE overruled', out)

    def test_enemy_attack_pass_after_a_hold_ended_by_down_is_overruled(self):
        results = {'enemy_attacks_maya': ('PASS', '1 hit(s), synthetic, landed after she was Up again')}
        fixtures = {'enemy_attacks_maya': chk.HOLD_ENDED_BY_DOWN}
        code, out = self.run_checker(self.write_log(build(results, fixtures=fixtures, range_mode=True)))
        self.assertEqual(code, 1)
        self.assertIn('FAIL', self.row(out, 4))
        self.assertIn('error: link enemy_attacks_maya: PASS overruled, the hold on fire ended with Maya down', out)

    def test_enemy_attack_pass_after_a_hold_ended_by_death_is_overruled(self):
        results = {'enemy_attacks_maya': ('PASS', '1 hit(s), synthetic')}
        fixtures = {'enemy_attacks_maya': chk.HOLD_ENDED_BY_DEATH}
        code, out = self.run_checker(self.write_log(build(results, fixtures=fixtures, range_mode=True)))
        self.assertEqual(code, 1)
        self.assertIn('error: link enemy_attacks_maya: PASS overruled, the hold on fire ended with the selected enemy dead', out)

    def test_enemy_attack_pass_after_other_hold_ends_is_kept(self):
        for fixture in ('fire held, ended by timeout', 'fire held until the selected enemy hit Maya'):
            results = {'enemy_attacks_maya': ('PASS', '1 hit(s), synthetic')}
            code, out = self.run_checker(self.write_log(build(results, fixtures={'enemy_attacks_maya': fixture}, range_mode=True)))
            self.assertEqual(code, 0, fixture)
            self.assertIn(f'PASS (fixture: {fixture})', self.row(out, 4))

    def test_enemy_attack_pass_with_no_hold_is_kept(self):
        code, out = self.run_checker(self.write_log(build(range_mode=True)))
        self.assertEqual(code, 0)
        self.assertEqual(self.row(out, 4).split()[2], 'PASS')

    def test_forced_loot_not_available_fails(self):
        results = {'loot_drop_on_kill': ('NOT_AVAILABLE', 'synthetic gap')}
        code, out = self.run_checker(self.write_log(build(results, fixtures={'loot_drop_on_kill': chk.FORCED_LOOT})))
        self.assertEqual(code, 1)
        self.assertIn('FAIL', self.row(out, 5))

    def test_loot_not_available_is_kept_without_the_forced_roll(self):
        results = {'loot_drop_on_kill': ('NOT_AVAILABLE', 'synthetic gap')}
        code, out = self.run_checker(self.write_log(build(results)))
        self.assertEqual(code, 0)
        self.assertIn('NOT_AVAILABLE', self.row(out, 5))

    def test_summary_mismatch_exits_one(self):
        code, out = self.run_checker(self.write_log(build(summary='pass=12 fail=1 na=0')))
        self.assertEqual(code, 1)
        self.assertIn('error: SUMMARY says pass=12 fail=1 na=0, but the link lines say pass=14 fail=0 na=0', out)

    def test_json_output(self):
        results = {'pick_up_gun': ('FAIL', 'synthetic | pipe')}
        out_path = self.dir / 'out.json'
        code, _ = self.run_checker(self.write_log(build(results)), '--json', str(out_path))
        self.assertEqual(code, 1)
        text = out_path.read_text(encoding='utf-8')
        self.assertTrue(text.startswith('{\n "links": ['), 'indent=1 layout')
        report = json.loads(text)
        self.assertEqual(sorted(report), ['errors', 'fail', 'links', 'not_available', 'pass', 'summary_seen'])
        self.assertEqual(len(report['links']), TOTAL)
        self.assertEqual(report['links'][0], {'index': 1, 'name': 'spawn_maya', 'result': 'PASS', 'fixtures': [],
                                              'owner': 'maya', 'detail': 'synthetic detail'})
        self.assertEqual(report['links'][5]['detail'], 'synthetic | pipe')
        self.assertEqual((report['pass'], report['fail'], report['not_available']), (13, 1, 0))
        self.assertTrue(report['summary_seen'])
        self.assertEqual(report['errors'], [])

    def test_markdown_output_escapes_pipes(self):
        results = {'pick_up_gun': ('FAIL', 'a | b')}
        out_path = self.dir / 'out.md'
        code, _ = self.run_checker(self.write_log(build(results)), '--markdown', str(out_path))
        self.assertEqual(code, 1)
        lines = out_path.read_text(encoding='utf-8').splitlines()
        self.assertEqual(lines[0], '| # | Link | Result | Owner (when not PASS) | Detail |')
        self.assertEqual(lines[1], '|---|---|---|---|---|')
        self.assertEqual(len(lines), 2 + TOTAL)
        self.assertEqual(lines[2], '| 1 | spawn_maya | PASS | - | synthetic detail |')
        self.assertEqual(lines[7], '| 6 | pick_up_gun | FAIL | range | a \\| b |')

    def test_utf8_bom_at_start_of_file(self):
        code, out = self.run_checker(self.write_log(build(), bom=True))
        self.assertEqual(code, 0)
        self.assertIn('slice gate: 14 PASS, 0 FAIL, 0 NOT_AVAILABLE of 14', out)

    # The older variants (default, -Shield, -DownAfterFight) are graded as before: their forcing fixtures are not graded.

    def test_older_variants_keep_their_fixtures_and_verdicts(self):
        for range_mode in (False, True):
            json_path = self.dir / f'old-{range_mode}.json'
            code, out = self.run_checker(self.write_log(build(fixtures=OLD_VARIANT_FIXTURES, range_mode=range_mode)),
                                         '--json', str(json_path))
            self.assertEqual(code, 0, out)
            self.assertIn('slice gate: 14 PASS, 0 FAIL, 0 NOT_AVAILABLE of 14', out)
            self.assertNotIn('unforced', out)
            self.assertNotIn('error', out)
            report = json.loads(json_path.read_text(encoding='utf-8'))
            self.assertEqual(sorted(report), ['errors', 'fail', 'links', 'not_available', 'pass', 'summary_seen'])
            self.assertEqual(sorted(report['links'][0]), ['detail', 'fixtures', 'index', 'name', 'owner', 'result'])
            self.assertEqual(report['links'][8]['fixtures'], ['phaselock point granted'])

    def test_older_variants_need_no_how_token(self):
        details = {name: ('PASS', 'synthetic detail, no token') for name in chk.UNFORCED_EVIDENCE}
        code, out = self.run_checker(self.write_log(build(details, range_mode=True)))
        self.assertEqual(code, 0, out)

    def test_older_variants_keep_the_old_not_available_rules(self):
        results = {'go_down': ('NOT_AVAILABLE', 'Maya down from a previous link before the lethal damage was due'),
                   'loot_drop_on_kill': ('NOT_AVAILABLE', 'no pickup appeared, and this build cannot force a gun roll')}
        code, out = self.run_checker(self.write_log(build(results, range_mode=True)))
        self.assertEqual(code, 0, out)
        self.assertIn('slice gate: 12 PASS, 0 FAIL, 2 NOT_AVAILABLE of 14', out)

    # The unforced variant (-Unforced; the start line says unforced=1).

    def test_unforced_run_with_every_link_unforced_passes(self):
        fixtures = {'pick_up_gun': 'teleport', chk.DECOY_LINK: 'two guns rolled and dropped by the test; teleport',
                    'kill_enemy': 'one shot at a time until the selected enemy was wounded; fire held until the selected enemy hit Maya',
                    'go_down': 'teleport'}
        json_path = self.dir / 'unforced.json'
        code, out = self.run_checker(self.write_log(build(fixtures=fixtures, range_mode=True, unforced=True)),
                                     '--expect-unforced', '--json', str(json_path))
        self.assertEqual(code, 0, out)
        self.assertIn('slice gate: 14 PASS, 0 FAIL, 0 NOT_AVAILABLE of 14', out)
        self.assertIn('unforced run: 14 of 14 links got there with no stand-in for play', out)
        report = json.loads(json_path.read_text(encoding='utf-8'))
        self.assertTrue(report['unforced'])
        self.assertEqual([link['forced'] for link in report['links']], [[]] * TOTAL)

    def test_unforced_run_fails_every_forcing_fixture(self):
        names = {chk.FORCED_LOOT: 'loot_drop_on_kill', 'gun rolled and dropped by the test': 'pick_up_gun',
                 "gun from the test's drop, not a kill": 'equip_gun', 'phaselock point granted': 'phaselock_enemy',
                 'range set passive after the fight': 'loot_drop_on_kill', 'range cleared before the mission': 'mission_accept',
                 'Maya healed to full after the fight': 'loot_drop_on_kill', 'Maya revived to full health after the fight': 'loot_drop_on_kill',
                 'lethal damage at the end of the fight': 'loot_drop_on_kill', 'lethal damage applied by the test': 'go_down',
                 "down state from the test's lethal damage": 'bleed_out_respawn',
                 'accepted by the use key without the mission page': 'mission_accept',
                 'accepted by a direct call after the mission page did not accept': 'mission_accept',
                 'turned in by the use key without the mission page': 'mission_turn_in'}
        self.assertEqual(sorted(names), sorted(chk.FORCING_FIXTURES), 'a test for every forcing fixture')
        for fixture, name in names.items():
            with self.subTest(fixture=fixture):
                json_path = self.dir / 'forced.json'
                code, out = self.run_checker(self.write_log(build(fixtures={name: f'teleport; {fixture}'}, range_mode=True, unforced=True)),
                                             '--json', str(json_path))
                self.assertEqual(code, 1, out)
                index = chk.NAMES.index(name) + 1
                self.assertIn(f'checker: forced in an unforced run ({fixture}); host said PASS', self.row(out, index))
                self.assertIn(f'error: link {name}: PASS overruled, forced in an unforced run', out)
                self.assertIn(f'not unforced: {name}', out)
                report = json.loads(json_path.read_text(encoding='utf-8'))
                self.assertEqual(report['links'][index - 1]['forced'], [fixture])

    def test_unforced_run_fails_a_shield_fixture(self):
        for fixture in ('shield s1 equipped from the gear manifest', 'level rule overridden by the test: shield s1 of level 20 equipped on Maya of level 8',
                        'host stand-in shield of 213 (no shield could be equipped from the gear manifest; not stock data)'):
            with self.subTest(fixture=fixture):
                code, out = self.run_checker(self.write_log(build(fixtures={'kill_enemy': fixture}, range_mode=True, unforced=True)))
                self.assertEqual(code, 1)
                self.assertIn('checker: forced in an unforced run', self.row(out, 3))

    def test_unforced_not_available_with_a_forcing_fixture_fails(self):
        results = {'bleed_out_respawn': ('NOT_AVAILABLE', 'synthetic')}
        fixtures = {'bleed_out_respawn': "down state from the test's lethal damage"}
        code, out = self.run_checker(self.write_log(build(results, fixtures=fixtures, range_mode=True, unforced=True)))
        self.assertEqual(code, 1)
        self.assertIn('host said NOT_AVAILABLE', self.row(out, 14))

    # Written out here, so that dropping a link from the checker's mapping fails this test instead of removing its case.
    EVIDENCE = {
        'loot_drop_on_kill': 'how=kill_roll',
        'phaselock_enemy': 'how=skills_page',
        'mission_accept': 'how=mission_page',
        'mission_turn_in': 'how=mission_page',
        'go_down': 'how=enemies',
        'bleed_out_respawn': 'how=after_enemy_down',
    }

    def test_unforced_evidence_mapping_is_the_six_links(self):
        self.assertEqual(chk.UNFORCED_EVIDENCE, self.EVIDENCE)

    def test_unforced_pass_without_its_how_token_fails(self):
        for name, token in self.EVIDENCE.items():
            with self.subTest(link=name):
                code, out = self.run_checker(self.write_log(build({name: ('PASS', 'synthetic, no token')}, range_mode=True, unforced=True)))
                self.assertEqual(code, 1)
                self.assertIn(f'checker: no sign that it got there unforced (no {token} in the detail)', self.row(out, chk.NAMES.index(name) + 1))

    def test_unforced_how_token_must_be_a_word_of_its_own(self):
        results = {'go_down': ('PASS', 'synthetic xhow=enemies')}
        code, out = self.run_checker(self.write_log(build(results, range_mode=True, unforced=True)))
        self.assertEqual(code, 1)
        self.assertIn('no how=enemies in the detail', self.row(out, 13))

    def test_unforced_down_not_available_fails_with_the_range(self):
        results = {'go_down': ('NOT_AVAILABLE', 'synthetic: no enemy downed her')}
        code, out = self.run_checker(self.write_log(build(results, range_mode=True, unforced=True)))
        self.assertEqual(code, 1)
        self.assertIn('checker: the range was there, so its enemies can down her', self.row(out, 13))
        code, out = self.run_checker(self.write_log(build(results, range_mode=False, unforced=True)))
        self.assertEqual(code, 1, out)
        self.assertIn("checker: a NOT_AVAILABLE is not a pass in an unforced run", self.row(out, 13))
        self.assertIn('not unforced: go_down', out)

    def test_unforced_mission_skipped_while_down_fails(self):
        """The review's case: Maya still down when the mission is due, links 10 to 12 skipped, 13 and 14 on enemy hits."""
        skipped = ('NOT_AVAILABLE', 'Maya down from a previous link (injured, health 0/213) when this link was due to start')
        results = {name: skipped for name in ('mission_accept', 'mission_objective', 'mission_turn_in')}
        code, out = self.run_checker(self.write_log(build(results, range_mode=True, unforced=True)), '--expect-unforced')
        self.assertEqual(code, 1, out)
        self.assertIn('slice gate: 11 PASS, 3 FAIL, 0 NOT_AVAILABLE of 14', out)
        for index in (10, 11, 12):
            self.assertIn('checker: a NOT_AVAILABLE is not a pass in an unforced run', self.row(out, index))
        self.assertIn('not unforced: mission_accept, mission_objective, mission_turn_in', out)

    def test_unforced_setup_gap_is_not_a_pass_either(self):
        results = {chk.DECOY_LINK: ('NOT_AVAILABLE', chk.SETUP_GAP + ', synthetic: no source')}
        code, out = self.run_checker(self.write_log(build(results, range_mode=True, unforced=True)))
        self.assertEqual(code, 1)
        self.assertIn('FAIL', self.row(out, 7))

    def test_unforced_loot_not_available_fails_with_the_range(self):
        results = {'loot_drop_on_kill': ('NOT_AVAILABLE', 'synthetic')}
        code, out = self.run_checker(self.write_log(build(results, range_mode=True, unforced=True)))
        self.assertEqual(code, 1)
        self.assertIn('checker: the range was there, so its enemies can drop a gun', self.row(out, 5))

    def test_unforced_fail_keeps_the_hosts_reason(self):
        reason = 'no gun from 100 kill(s) (budget 100)'
        code, out = self.run_checker(self.write_log(build({'go_down': ('FAIL', reason)}, range_mode=True, unforced=True)))
        self.assertEqual(code, 1)
        self.assertIn(reason, self.row(out, 13))
        self.assertNotIn('checker:', self.row(out, 13))
        self.assertIn('unforced run: 13 of 14 links got there with no stand-in for play; not unforced: go_down', out)

    # The start record is parsed strictly: only the forms the host prints.

    def start_line(self, rest):
        return PREFIX + 'OWSLICEGATE start map=Sanctuary_P quest=1 ' + rest

    def test_host_start_lines_are_accepted(self):
        for rest, unforced in (('range_command=1', False), ('range_command=0', False), ('range_command=1 unforced=1', True)):
            with self.subTest(rest=rest):
                lines = build(range_mode=True, unforced=unforced)
                lines[0] = self.start_line(rest)
                code, out = self.run_checker(self.write_log(lines))
                self.assertEqual(code, 0, out)
                self.assertEqual('unforced run:' in out, unforced)

    def test_malformed_unforced_marker_fails_the_log(self):
        for rest in ('range_command=1 unforced=1x', 'range_command=1 unforced=1.0', 'range_command=1 unforced=0',
                     'range_command=1 unforced=1 unforced=1', 'unforced=1 range_command=1', 'range_command=1 xunforced=1'):
            with self.subTest(rest=rest):
                lines = build(range_mode=True)
                lines[0] = self.start_line(rest)
                code, out = self.run_checker(self.write_log(lines))
                self.assertEqual(code, 1, out)
                self.assertIn('error: malformed start line', out)
                self.assertIn('unforced run:', out)   # graded strictly as well

    def test_malformed_range_command_fails_the_log(self):
        for rest in ('range_command=1.0', 'range_command=1x', 'quest=1', 'range_command=1 range_command=1'):
            with self.subTest(rest=rest):
                lines = build(range_mode=True)
                lines[0] = self.start_line(rest)
                code, out = self.run_checker(self.write_log(lines))
                self.assertEqual(code, 1, out)
                self.assertIn('error: malformed start line', out)

    MARKER_MAPS = ('Invented_unforced_P', 'Invented_unforced=1_P', 'range_command=1_P', 'unforced=1')

    def test_map_token_with_marker_text_is_not_searched(self):
        for map_name in self.MARKER_MAPS:
            for unforced in (False, True):
                with self.subTest(map=map_name, unforced=unforced):
                    lines = build(range_mode=True, unforced=unforced)
                    lines[0] = PREFIX + f'OWSLICEGATE start map={map_name} quest=1 range_command=1' + (' unforced=1' if unforced else '')
                    code, out = self.run_checker(self.write_log(lines))
                    self.assertEqual(code, 0, out)
                    self.assertEqual('unforced run:' in out, unforced)
                    self.assertNotIn('error', out)

    def test_truncated_start_line_fails(self):
        for rest in ('range_command=1 unforc', 'range_command=1 unforced', 'range_command=1 unforced=', 'range_command='):
            with self.subTest(rest=rest):
                lines = build(range_mode=True)
                lines[0] = self.start_line(rest)
                code, out = self.run_checker(self.write_log(lines))
                self.assertEqual(code, 1, out)
                self.assertIn('error: malformed start line', out)
                self.assertIn('unforced run:', out)   # graded strictly, not as a default run

    def test_extra_words_after_the_marker_fail(self):
        lines = build(range_mode=True, unforced=True)
        lines[0] = self.start_line('range_command=1 unforced=1 extra')
        code, out = self.run_checker(self.write_log(lines))
        self.assertEqual(code, 1)
        self.assertIn('error: malformed start line', out)

    def test_trailing_spaces_and_carriage_return_are_accepted(self):
        for tail in ('   ', '\r', ' \r'):
            with self.subTest(tail=repr(tail)):
                self.assertEqual(chk.start_mode(self.start_line('range_command=1 unforced=1') + tail), (True, True, []))
                self.assertEqual(chk.start_mode(self.start_line('range_command=0') + tail), (False, False, []))
                lines = build(range_mode=True, unforced=True)
                lines[0] += tail
                code, out = self.run_checker(self.write_log(lines))
                self.assertEqual(code, 0, out)
                self.assertIn('unforced run:', out)

    def test_two_start_lines_that_disagree_fail(self):
        lines = build(range_mode=True, unforced=True)
        lines.insert(1, self.start_line('range_command=1'))
        code, out = self.run_checker(self.write_log(lines))
        self.assertEqual(code, 1)
        self.assertIn('a second start line', out)
        self.assertIn('error: the start lines disagree on the variant (range_command or unforced)', out)

    def test_two_start_lines_that_agree_still_fail(self):
        lines = build(range_mode=True)
        lines.insert(1, lines[0])
        code, out = self.run_checker(self.write_log(lines))
        self.assertEqual(code, 1)
        self.assertIn('a second start line', out)
        self.assertNotIn('disagree', out)

    def test_expect_unforced_fails_a_run_without_the_marker(self):
        code, out = self.run_checker(self.write_log(build(range_mode=True)), '--expect-unforced')
        self.assertEqual(code, 1)
        self.assertIn('error: the unforced variant was asked for, but the start line has no unforced=1', out)

    def test_command_line_exit_code(self):
        script = Path(chk.__file__)
        done = subprocess.run([sys.executable, str(script), str(self.write_log(build(start=False)))],
                              capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(done.returncode, 2)
        self.assertIn('no slice-gate run in this log', done.stdout)


class RunnerSourceTest(unittest.TestCase):
    """tools/test_slice_gate.ps1: -Unforced changes nothing the older variants launch (read from the script's text)."""

    def setUp(self):
        self.source = (Path(__file__).resolve().parents[1] / 'tools' / 'test_slice_gate.ps1').read_text(encoding='utf-8')

    def test_base_arguments_have_no_unforced_switch_or_page(self):
        base = self.source[self.source.index('$arguments = @('):self.source.index("if ($Gun)")]
        for switch in ('-owslicegateunforced', '-owflashskills', '-owflashmissionscreen', '-owflashhud'):
            self.assertNotIn(switch, base)
        self.assertIn("'-owcanvashud'", base)

    def test_unforced_switches_are_added_only_under_the_switch(self):
        code = [line.split('#')[0].strip() for line in self.source.splitlines()]
        lines = [line for line in code if '-owslicegateunforced' in line or '--expect-unforced' in line or 'Start-OwMovieHud -Port' in line]
        self.assertEqual(len(lines), 3, lines)
        for line in lines:
            self.assertTrue(line.startswith('if ($Unforced)'), line)

    def test_unforced_refuses_the_fixture_variants(self):
        self.assertIn("if ($Unforced -and ($Shield -or $DownAfterFight)) { throw", self.source)


if __name__ == '__main__':
    unittest.main()
