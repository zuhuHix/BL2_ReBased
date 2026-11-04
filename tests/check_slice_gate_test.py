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


def build(results=None, stop_after=None, summary='auto', start=True, extra=(), fixtures=None, order=None):
    """Synthetic log lines: a start line, one link line per link (PASS unless results says otherwise, and only up to
    stop_after), the extra lines, then the SUMMARY line (counted from the results, unless summary is given or None).
    fixtures maps a link name to its fixtures text; order, a list of link names, replaces the expected order."""
    results = results or {}
    lines = [PREFIX + 'OWSLICEGATE start synthetic run'] if start else []
    effective = {}
    fixtures = fixtures or {}
    for index, name in enumerate(order or chk.NAMES, 1):
        if stop_after is not None and index > stop_after:
            break
        result, detail = results.get(name, ('PASS', 'synthetic detail'))
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
        self.assertIn('slice gate: 13 PASS, 0 FAIL, 0 NOT_AVAILABLE of 13', out)
        header = out.splitlines()[0]
        self.assertEqual(header.split(), ['#', 'link', 'result', 'owner', 'detail'])
        self.assertNotIn('error', out)

    def test_not_available_exits_zero_and_shows_owner(self):
        results = {'enemy_attacks_maya': ('NOT_AVAILABLE', 'synthetic gap')}
        code, out = self.run_checker(self.write_log(build(results)))
        self.assertEqual(code, 0)
        self.assertIn('slice gate: 12 PASS, 0 FAIL, 1 NOT_AVAILABLE of 13', out)
        row = self.row(out, 4)
        self.assertIn('NOT_AVAILABLE', row)
        self.assertIn('range', row)
        self.assertIn('synthetic gap', row)
        self.assertEqual(self.row(out, 1).split()[3], '-', 'a PASS row shows no owner')

    def test_fail_exits_one(self):
        results = {'kill_enemy': ('FAIL', 'synthetic miss')}
        code, out = self.run_checker(self.write_log(build(results)))
        self.assertEqual(code, 1)
        self.assertIn('slice gate: 12 PASS, 1 FAIL, 0 NOT_AVAILABLE of 13', out)
        self.assertIn('range', self.row(out, 3))

    def test_run_stopped_after_link_5_fails_the_rest(self):
        path = self.write_log(build(stop_after=5, summary=None))
        code, out = self.run_checker(path, '--json', str(self.dir / 'run.json'))
        self.assertEqual(code, 1)
        self.assertIn('slice gate: 5 PASS, 8 FAIL, 0 NOT_AVAILABLE of 13', out)
        self.assertIn('not reached: the run stopped before this link', self.row(out, 6))
        report = json.loads((self.dir / 'run.json').read_text(encoding='utf-8'))
        self.assertFalse(report['summary_seen'])
        self.assertIn('no SUMMARY line: the run did not finish (crash or timeout)', report['errors'])
        self.assertEqual(report['links'][5]['result'], 'FAIL')
        self.assertEqual(report['links'][5]['owner'], 'range')   # pick_up_gun, the sixth link
        self.assertEqual(report['links'][12]['owner'], 'maya')   # bleed_out_respawn, the last link

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
        self.assertIn('link phaselock_enemy out of order (expected kill_enemy next)', out)
        self.assertIn('slice gate: 13 PASS, 0 FAIL, 0 NOT_AVAILABLE of 13', out)

    def test_link_after_summary_fails(self):
        lines = build(stop_after=12, summary='pass=12 fail=0 na=0')
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
        self.assertIn('PASS (fixture: phaselock point granted)', self.row(out, 8))
        self.assertIn('PASS (fixture: lethal damage; teleport)', self.row(out, 12))
        self.assertEqual(self.row(out, 1).split()[2], 'PASS')
        report = json.loads(json_path.read_text(encoding='utf-8'))
        self.assertEqual(report['links'][11]['fixtures'], ['lethal damage', 'teleport'])
        self.assertEqual(report['links'][0]['fixtures'], [])
        self.assertIn('| 8 | phaselock_enemy | PASS (fixture: phaselock point granted) | - |', md_path.read_text(encoding='utf-8'))

    def test_link_line_without_fixtures_field_is_read(self):
        lines = build()
        lines[1] = PREFIX + 'OWSLICEGATE link=spawn_maya result=PASS detail=old format'
        code, out = self.run_checker(self.write_log(lines))
        self.assertEqual(code, 0)
        self.assertIn('old format', self.row(out, 1))

    def test_summary_mismatch_exits_one(self):
        code, out = self.run_checker(self.write_log(build(summary='pass=12 fail=1 na=0')))
        self.assertEqual(code, 1)
        self.assertIn('error: SUMMARY says pass=12 fail=1 na=0, but the link lines say pass=13 fail=0 na=0', out)

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
        self.assertEqual((report['pass'], report['fail'], report['not_available']), (12, 1, 0))
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
        self.assertIn('slice gate: 13 PASS, 0 FAIL, 0 NOT_AVAILABLE of 13', out)

    def test_command_line_exit_code(self):
        script = Path(chk.__file__)
        done = subprocess.run([sys.executable, str(script), str(self.write_log(build(start=False)))],
                              capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(done.returncode, 2)
        self.assertIn('no slice-gate run in this log', done.stdout)


if __name__ == '__main__':
    unittest.main()
