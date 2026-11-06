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


def build(results=None, stop_after=None, summary='auto', start=True, extra=(), fixtures=None, order=None, range_mode=False):
    """Synthetic log lines: a start line, one link line per link (PASS unless results says otherwise, and only up to
    stop_after), the extra lines, then the SUMMARY line (counted from the results, unless summary is given or None).
    fixtures maps a link name to its fixtures text; order, a list of link names, replaces the expected order;
    range_mode marks the run as one with the range on its start line."""
    results = results or {}
    lines = [PREFIX + f'OWSLICEGATE start synthetic run range_command={int(range_mode)}'] if start else []
    effective = {}
    fixtures = fixtures or {}
    for index, name in enumerate(order or chk.NAMES, 1):
        if stop_after is not None and index > stop_after:
            break
        default = {'pick_up_gun': CARD_DETAIL, chk.DECOY_LINK: DECOY_DETAIL}.get(name, 'synthetic detail')
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

    def test_enemy_attack_not_available_fails_with_the_range(self):
        results = {'enemy_attacks_maya': ('NOT_AVAILABLE', 'synthetic gap')}
        code, out = self.run_checker(self.write_log(build(results, range_mode=True)))
        self.assertEqual(code, 1)
        self.assertIn('FAIL', self.row(out, 4))
        self.assertIn('error: link enemy_attacks_maya: NOT_AVAILABLE overruled', out)

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

    def test_command_line_exit_code(self):
        script = Path(chk.__file__)
        done = subprocess.run([sys.executable, str(script), str(self.write_log(build(start=False)))],
                              capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(done.returncode, 2)
        self.assertIn('no slice-gate run in this log', done.stdout)


if __name__ == '__main__':
    unittest.main()
