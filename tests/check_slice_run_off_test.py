"""Synthetic tests for tools/check_slice_run_off.py (no game or binary needed; every row is invented here).

AI-assisted (Claude Code), lane SLICE-RUN-OFF. The rows follow the format crates/ow-cli/src/cmd/slice_gate.rs prints; the
link names come from the checker's own list, and the stops are made-up texts.
"""
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_slice_run_off as chk  # noqa: E402

NO_DRIVER = {'pick_up_gun', 'pick_up_aimed_not_nearest'}
OWN_DRIVER = {'equip_gun': 'equip-run', 'phaselock_enemy': 'phaselock-cast', 'reach_fight_area': 'sanctuary-walk'}


def stop(text='synthetic stop', native='', record=''):
    return {'step': 'step:1', 'native': native, 'record': record, 'text': text}


def row(n, result='ok', grading='strict', **fields):
    name = chk.NAMES[n - 1]
    out = {'n': n, 'link': name, 'grading': grading, 'result': result,
           'driver': 'none' if name in NO_DRIVER else OWN_DRIVER.get(name, 'combat-run'), 'steps': [] if name in NO_DRIVER else ['step:1'],
           'stop': None if result == 'ok' else stop(), 'blocked_by': None, 'after_errors_in': None, 'error_lines': [],
           'scope': 'synthetic scope'}
    out.update(fields)
    return out


def all_ok(grading='strict'):
    rows = [row(n, grading=grading) for n in range(1, 15)]
    for r in rows:
        if r['driver'] in ('none', 'phaselock-cast'):
            # A link without a driver, or link 9's cast with no evidence check yet, is never ok: a passing run has an
            # evidence-checked driver for every link (a synthetic one here).
            r['driver'] = 'combat-run'
    return rows


def lines(rows):
    return [json.dumps(r) for r in rows]


class Parse(unittest.TestCase):
    def test_fourteen_strict_ok_rows_pass(self):
        rows = chk.parse(lines(all_ok()))
        self.assertEqual(chk.counts(rows), {'ok': 14, 'notyet': 0, 'error': 0})
        self.assertTrue(chk.passes(rows))

    def test_past_errors_grading_never_passes(self):
        rows = chk.parse(lines(all_ok('past-errors')))
        self.assertFalse(chk.passes(rows))

    def test_a_notyet_or_error_row_fails_the_gate_and_is_counted(self):
        rows = all_ok()
        rows[0] = row(1, 'error', stop=stop('unbound native A.B()', 'A.B()'))
        rows[1] = row(2, 'notyet', stop=stop('no headless driver: synthetic'))
        rows[2] = row(3, 'error', blocked_by='spawn_maya')
        parsed = chk.parse(lines(rows))
        self.assertEqual(chk.counts(parsed), {'ok': 11, 'notyet': 1, 'error': 2})
        self.assertFalse(chk.passes(parsed))

    def test_order_names_and_count_are_checked(self):
        rows = all_ok()
        for broken in (rows[:13], rows[1:] + rows[:1], [dict(rows[0], link='kill_enemy')] + rows[1:]):
            with self.assertRaises(chk.Malformed):
                chk.parse(lines(broken))
        with self.assertRaises(chk.Malformed):
            chk.parse(['not json'] + lines(rows[1:]))

    def test_each_row_needs_every_field_with_its_type(self):
        for key, bad in (('result', 'pass'), ('grading', 'loose'), ('driver', 'host'), ('steps', 'x'),
                         ('scope', None), ('error_lines', [{'step': 'x'}])):
            rows = all_ok()
            rows[4] = dict(rows[4], **{key: bad})
            with self.assertRaises(chk.Malformed, msg=key):
                chk.parse(lines(rows))
        rows = all_ok()
        del rows[4]['blocked_by']
        with self.assertRaises(chk.Malformed):
            chk.parse(lines(rows))

    def test_a_stop_is_there_exactly_when_the_result_is_not_ok(self):
        rows = all_ok()
        rows[3] = dict(rows[3], stop=stop())
        with self.assertRaises(chk.Malformed):
            chk.parse(lines(rows))
        rows = all_ok()
        rows[3] = dict(rows[3], result='notyet', stop=None)
        with self.assertRaises(chk.Malformed):
            chk.parse(lines(rows))
        rows[3] = dict(rows[3], stop={'step': 'x', 'native': 'y', 'text': 'z'})
        with self.assertRaises(chk.Malformed):
            chk.parse(lines(rows))

    def test_one_grading_for_all_rows(self):
        rows = all_ok()
        rows[7] = dict(rows[7], grading='past-errors')
        with self.assertRaises(chk.Malformed):
            chk.parse(lines(rows))

    def test_blocked_by_names_an_earlier_link_that_is_not_ok_with_its_result(self):
        rows = all_ok()
        rows[0] = row(1, 'notyet')
        rows[2] = row(3, 'notyet', blocked_by='spawn_maya')
        chk.parse(lines(rows))
        for bad in ('kill_enemy', 'bleed_out_respawn', 'enemy_attacks_maya'):
            broken = list(rows)
            broken[2] = row(3, 'notyet', blocked_by=bad)
            with self.assertRaises(chk.Malformed, msg=bad):
                chk.parse(lines(broken))
        broken = list(rows)
        broken[2] = row(3, 'error', blocked_by='spawn_maya')
        with self.assertRaises(chk.Malformed):
            chk.parse(lines(broken))

    def test_error_lines_only_on_a_past_errors_row_that_is_not_ok(self):
        line = [{'step': 'use', 'text': 'synthetic error line'}]
        rows = all_ok('past-errors')
        rows[9] = row(10, 'error', grading='past-errors', error_lines=line)
        chk.parse(lines(rows))
        rows[9] = row(10, 'ok', grading='past-errors', error_lines=line)
        with self.assertRaises(chk.Malformed, msg='an ok row with error lines'):
            chk.parse(lines(rows))
        rows = all_ok()
        rows[9] = row(10, 'error', error_lines=line)
        with self.assertRaises(chk.Malformed, msg='a strict row with error lines'):
            chk.parse(lines(rows))

    def test_the_equip_and_cast_drivers_are_known_and_an_unknown_one_is_malformed(self):
        rows = all_ok()
        self.assertEqual(rows[7]['driver'], 'equip-run')
        rows[8] = row(9, result='error', driver='phaselock-cast', steps=['cast'])
        chk.parse(lines(rows))
        rows[7] = dict(rows[7], driver='equip')
        with self.assertRaises(chk.Malformed):
            chk.parse(lines(rows))

    def test_each_one_link_driver_drives_its_own_link_only(self):
        rows = all_ok()
        rows[0] = dict(rows[0], driver='equip-run')
        with self.assertRaises(chk.Malformed, msg='equip-run on link 1'):
            chk.parse(lines(rows))
        rows = all_ok()
        rows[7] = dict(rows[7], driver='phaselock-cast', result='error', stop=stop())
        with self.assertRaises(chk.Malformed, msg='phaselock-cast on link 8'):
            chk.parse(lines(rows))

    def test_the_walk_driver_drives_link_2_only(self):
        rows = all_ok()
        self.assertEqual(rows[1]['driver'], 'sanctuary-walk')
        rows[1] = row(2, result='notyet', steps=['load', 'spawn', 'walk', 'fight_area'],
                      stop=stop(native='the fight area of link 2', record='SLICE_RUN_PLAN.md section 3.2'))
        chk.parse(lines(rows))
        rows = all_ok()
        rows[0] = dict(rows[0], driver='sanctuary-walk')
        with self.assertRaises(chk.Malformed, msg='sanctuary-walk on link 1'):
            chk.parse(lines(rows))

    def test_a_phaselock_cast_row_is_never_ok_until_its_evidence_check_exists(self):
        rows = all_ok()
        rows[8] = dict(rows[8], driver='phaselock-cast')
        with self.assertRaises(chk.Malformed):
            chk.parse(lines(rows))
        rows[8] = dict(rows[8], result='notyet', stop=stop())
        chk.parse(lines(rows))

    def test_a_link_without_a_driver_is_never_ok(self):
        rows = all_ok()
        rows[1] = dict(rows[1], driver='none')
        with self.assertRaises(chk.Malformed):
            chk.parse(lines(rows))


class Main(unittest.TestCase):
    def run_main(self, rows, *extra):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / 'rows.jsonl'
            path.write_text('\n'.join(rows) + '\n', encoding='utf-8')
            out, err = io.StringIO(), io.StringIO()
            md, js = Path(d) / 't.md', Path(d) / 's.json'
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                code = chk.main([str(path), '--markdown', str(md), '--json', str(js), *extra])
            files = (md.read_text(encoding='utf-8'), json.loads(js.read_text(encoding='utf-8'))) if code != 2 else None
            return code, out.getvalue(), err.getvalue(), files

    def test_exit_codes_and_outputs(self):
        code, out, _, (md, summary) = self.run_main(lines(all_ok()))
        self.assertEqual(code, 0)
        self.assertIn('GATE PASS', out)
        self.assertIn('ok 14, notyet 0, error 0 of 14', md)
        self.assertEqual(summary['counts'], {'ok': 14, 'notyet': 0, 'error': 0})
        self.assertTrue(summary['pass'])
        rows = all_ok()
        rows[9] = row(10, 'error', stop=stop('unbound | native', 'A|B'))
        code, out, _, (md, summary) = self.run_main(lines(rows))
        self.assertEqual(code, 1)
        self.assertIn('GATE NOT PASSED', out)
        self.assertIn('A\\|B', md, 'a pipe in a cell is escaped')
        self.assertFalse(summary['pass'])
        code, _, err, _ = self.run_main(lines(all_ok()[:3]))
        self.assertEqual(code, 2)
        self.assertIn('3 rows', err)


if __name__ == '__main__':
    unittest.main()
