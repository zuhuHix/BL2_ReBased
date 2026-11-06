"""Synthetic tests for tools/check_interactives.py: invented ids, levels, money and pickups, no game data.

Run: python -m pytest tests/check_interactives_test.py
"""
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_interactives as ci  # noqa: E402

# Invented constants: calc(L) = 10 x 2^L; the amount at grade g is calc x (1 + 0.5 x g), grades 0..4.
CASH_CONSTANTS = {'multiplier': 10.0, 'level': 2.0, 'base': 1.0, 'per_grade': 0.5, 'grade_max': 4}


def document(supported=3, unsupported=1):
    objects = []
    for i in range(supported):
        objects.append({'id': f'Map:Point_{i}', 'kind': 'lootable', 'balance': f'Balance.{i}', 'supported': True,
                        'game_stage': {'region': 'Zone', 'min': 2, 'max': 4}})
    for i in range(unsupported):
        objects.append({'id': f'Map:Chest_{i}', 'kind': 'chest', 'supported': False, 'game_stage': {'min': 2, 'max': 4}})
    items = {'Item.Cash': {'kind': 'usable', 'pickup': {'kind': 'cash', 'cash': {
        'constants': {'Multiplier': 10.0, 'Level': 2.0}, 'base_scale': 1.0, 'per_grade_scale': 0.5,
        'grade_range': [0, 4]}}}}
    return {'schema': 'openwillow-interactives/1', 'objects': objects, 'balances': {}, 'items': items}


def line(text):
    return f'LogTemp: Display: {text}\n'


QUICK_RUN = ['OWQUICKCHANGE prompt id=S cost=200 money=5000',
             'OWQCTEST walked away',
             'OWQUICKCHANGE cancel id=S reason=not the selected target',
             'OWQUICKCHANGE prompt id=S cost=200 money=5000',
             'OWQUICKCHANGE reset id=S ok=0 cost=200 before=0 after=0 reason=respec costs 200 credits, 0 held',
             'OWQCTEST refusal money=0 grades_same=1',
             'OWQUICKCHANGE prompt id=S cost=200 money=5000',
             'OWQUICKCHANGE reset id=S ok=1 cost=200 before=5000 after=4800 reason=',
             'OWQCTEST reset grades_changed=1']


def good_log(spawned=3, unsupported=1):
    text = line(f'OpenWillow interactives: {spawned} lootable(s) spawned, {unsupported} unsupported not spawned')
    text += line('OWLOOT open id=Map:Point_0 config=Cash_Box drops=2')
    text += line('OWLOOT drop id=Map:Point_0 kind=cash balance=Item.Cash level=3 value=80')
    text += line('OWLOOT drop id=Map:Point_0 kind=ammo balance=Item.Ammo level=3 value=18')
    text += line('OWLOOT open id=Map:Point_1 config=Empty_Box drops=0')
    text += line('OWLOOT empty id=Map:Point_1 reason=no attachment dropped')
    text += line('OWLOOT open id=Map:Point_2 config=Empty_Box drops=0')
    text += line('OWLOOT empty id=Map:Point_2 reason=no attachment dropped')
    for i in range(3):
        text += line(f'OWINTERACTIVETEST test_open id=Map:Point_{i} balance=Balance.{i}')
    text += line('OWINTERACTIVETEST opened 3 lootable(s) of different balances')
    text += line('OWPICKUP kind=cash amount=80.00 before=100.00 after=180.00 cap=2147483647.00 ok=1 reason=')
    text += line('OWPICKUP kind=ammo amount=18.00 before=10.00 after=20.00 cap=20.00 ok=1 reason=')
    text += line('OWPICKUP kind=ammo amount=18.00 before=20.00 after=20.00 cap=20.00 ok=0 reason=ammo pool full')
    text += line('OWPICKUP kind=health amount=0.25 before=50.00 after=75.00 cap=100.00 ok=1 reason=')
    text += line('OWPICKUP kind=eridium amount=4.00 before=1.00 after=5.00 cap=2147483647.00 ok=1 reason=')
    text += line('OWLOOTLID queue=1')
    text += line('OWLOOTLID view balance=Synth clear=1 yaw=45')
    for phase in ('closed', 'mid', 'opened'):
        text += line(f'OWLOOTLID shot balance=Synth phase={phase}')
    text += ''.join(line(row) for row in QUICK_RUN)
    text += line('OWINTERACTIVETEST done spawned=3 opened=3')
    return text


AUTOMATION_OK = 'LogAutomationController: Test Completed. Result={Success} Name={OpenWillow.Interactive.Loot}\n'


def grade(log_text, automation=AUTOMATION_OK, doc=None):
    return ci.grade(log_text, automation, doc or document())


def status_of(results, name):
    return next(s for n, s, _ in results.rows if n == name)


class GoodRunTest(unittest.TestCase):
    def test_a_complete_run_has_no_failures(self):
        results = grade(good_log())
        failed = [(n, d) for n, s, d in results.rows if s == 'FAIL']
        self.assertEqual(failed, [])
        self.assertEqual(status_of(results, 'lootables_spawned'), 'PASS')
        self.assertEqual(status_of(results, 'opened_lootables'), 'PASS')
        self.assertEqual(status_of(results, 'open_drops'), 'PASS')
        self.assertEqual(status_of(results, 'cash_drops'), 'PASS')
        self.assertEqual(status_of(results, 'pickups'), 'PASS')
        self.assertEqual(status_of(results, 'test_sequence'), 'PASS')
        self.assertEqual(status_of(results, 'automation'), 'PASS')


class SpawnTest(unittest.TestCase):
    def test_wrong_spawn_count_fails(self):
        self.assertEqual(status_of(grade(good_log(spawned=2)), 'lootables_spawned'), 'FAIL')

    def test_no_spawn_line_fails(self):
        self.assertEqual(status_of(grade('nothing here\n'), 'lootables_spawned'), 'FAIL')

    def test_unsupported_objects_are_not_counted(self):
        # Two supported objects in the file: spawning three is a failure even though three is in the file's objects.
        results = grade(good_log(spawned=3), doc=document(supported=2, unsupported=1))
        self.assertEqual(status_of(results, 'lootables_spawned'), 'FAIL')


class OpenTest(unittest.TestCase):
    def test_open_of_an_unknown_id_fails(self):
        log = good_log() + line('OWLOOT open id=Map:Ghost config=X drops=1')
        self.assertEqual(status_of(grade(log), 'opened_lootables'), 'FAIL')

    def test_empty_open_without_a_reason_fails(self):
        log = good_log().replace('OWLOOT empty id=Map:Point_1 reason=no attachment dropped\n', '')
        self.assertEqual(status_of(grade(log), 'open_drops'), 'FAIL')

    def test_a_partial_run_fails_on_the_reported_count(self):
        log = good_log().replace('OWINTERACTIVETEST opened 3 lootable(s)', 'OWINTERACTIVETEST opened 1 lootable(s)')
        self.assertEqual(status_of(grade(log), 'opened_balances'), 'FAIL')

    def test_opens_covering_fewer_balances_than_reported_fail(self):
        doc = document()
        doc['objects'][1]['balance'] = doc['objects'][2]['balance'] = doc['objects'][0]['balance']
        # All three share a balance, so the sequence should aim at one: reporting three is wrong.
        self.assertEqual(status_of(ci.grade(good_log(), AUTOMATION_OK, doc), 'opened_balances'), 'FAIL')

    def test_no_reported_count_is_missing(self):
        log = good_log().replace('OWINTERACTIVETEST opened 3 lootable(s) of different balances\n', '')
        self.assertEqual(status_of(grade(log), 'opened_balances'), 'MISSING')

    def test_a_done_line_with_the_wrong_open_count_fails(self):
        log = good_log().replace('done spawned=3 opened=3', 'done spawned=3 opened=2')
        self.assertEqual(status_of(grade(log), 'test_sequence'), 'FAIL')

    def test_a_done_line_with_the_wrong_spawn_count_fails(self):
        log = good_log().replace('done spawned=3 opened=3', 'done spawned=2 opened=3')
        self.assertEqual(status_of(grade(log), 'test_sequence'), 'FAIL')

    def test_no_open_is_missing(self):
        results = grade('OpenWillow interactives: 3 lootable(s) spawned, 1 unsupported not spawned\n')
        self.assertEqual(status_of(results, 'opened_lootables'), 'MISSING')

    def test_the_same_lootable_opened_twice_fails(self):
        first = next(line_ for line_ in good_log().splitlines() if 'OWLOOT open' in line_)
        self.assertEqual(status_of(grade(good_log() + first + '\n'), 'opened_lootables'), 'FAIL')

    def test_a_run_where_nothing_happened_fails_at_exit(self):
        # One correct spawn line and nothing else must not exit 0 (review of PR #105).
        with tempfile.TemporaryDirectory() as tmp:
            log = Path(tmp) / 'run.log'
            log.write_text('OpenWillow interactives: 3 lootable(s) spawned, 1 unsupported not spawned\n', encoding='utf-8')
            interactives = Path(tmp) / 'i.json'
            interactives.write_text(json.dumps(document()), encoding='utf-8')
            with contextlib.redirect_stdout(io.StringIO()) as printed:
                code = ci.main(['--log', str(log), '--interactives', str(interactives), '--skip-automation'])
        self.assertEqual(code, 1)
        self.assertIn('missing=', printed.getvalue())

    def test_skipped_checks_are_counted_apart_and_do_not_fail(self):
        results = ci.Results()
        results.add('x', 'PASS', '')
        results.add('y', 'SKIPPED', 'optional')
        with contextlib.redirect_stdout(io.StringIO()) as printed:
            self.assertEqual(results.print(), 0)
        self.assertIn('passed=1 failed=0 missing=0 skipped=1', printed.getvalue())


class CashRangeTest(unittest.TestCase):
    def test_cash_range_is_rounded_down(self):
        self.assertEqual(ci.cash_range(CASH_CONSTANTS, 3), (80, 240))

    def test_cash_outside_the_range_fails(self):
        log = good_log().replace('kind=cash balance=Item.Cash level=3 value=80', 'kind=cash balance=Item.Cash level=3 value=999')
        self.assertEqual(status_of(grade(log), 'cash_drops'), 'FAIL')

    def test_level_outside_the_stage_fails(self):
        log = good_log().replace('level=3 value=80', 'level=9 value=80')
        self.assertEqual(status_of(grade(log), 'cash_drops'), 'FAIL')

    def test_no_cash_drop_is_skipped(self):
        # Loot is rolled, so a run may legitimately have no cash.
        log = good_log().replace('kind=cash', 'kind=ammo')
        self.assertEqual(status_of(grade(log), 'cash_drops'), 'SKIPPED')


class PickupTest(unittest.TestCase):
    def test_ammo_taken_past_the_cap_fails(self):
        log = good_log().replace('before=10.00 after=20.00 cap=20.00', 'before=10.00 after=28.00 cap=20.00')
        self.assertEqual(status_of(grade(log), 'pickups'), 'FAIL')

    def test_health_past_the_max_fails(self):
        log = good_log().replace('before=50.00 after=75.00 cap=100.00', 'before=50.00 after=95.00 cap=100.00')
        self.assertEqual(status_of(grade(log), 'pickups'), 'FAIL')

    def test_refused_pickup_that_moved_fails(self):
        log = good_log().replace('before=20.00 after=20.00 cap=20.00 ok=0', 'before=20.00 after=21.00 cap=20.00 ok=0')
        self.assertEqual(status_of(grade(log), 'pickups'), 'FAIL')

    def test_cash_that_does_not_add_up_fails(self):
        log = good_log().replace('before=100.00 after=180.00', 'before=100.00 after=170.00')
        self.assertEqual(status_of(grade(log), 'pickups'), 'FAIL')

    def test_no_pickup_line_is_missing(self):
        log = '\n'.join(l for l in good_log().splitlines() if 'OWPICKUP' not in l) + '\n'
        self.assertEqual(status_of(grade(log), 'pickups'), 'MISSING')


class SequenceAndAutomationTest(unittest.TestCase):
    def test_test_fail_line_fails_the_sequence(self):
        log = good_log() + line('OWINTERACTIVETEST FAIL no walker after 60 s')
        self.assertEqual(status_of(grade(log), 'test_sequence'), 'FAIL')

    def test_unfinished_sequence_is_missing(self):
        log = good_log().replace('OWINTERACTIVETEST done spawned=3 opened=3\n', '')
        self.assertEqual(status_of(grade(log), 'test_sequence'), 'MISSING')

    def test_failed_automation_fails(self):
        automation = 'Result={Fail} Name={OpenWillow.Interactive.Loot}\n'
        self.assertEqual(status_of(grade(good_log(), automation), 'automation'), 'FAIL')

    def test_automation_absent_from_the_log_fails(self):
        self.assertEqual(status_of(grade(good_log(), 'Result={Success} Name={OpenWillow.Vendor.Stock}\n'), 'automation'), 'FAIL')

    def test_no_automation_log_is_missing(self):
        self.assertEqual(status_of(grade(good_log(), None), 'automation'), 'MISSING')


class CommandLineTest(unittest.TestCase):
    def test_main_returns_one_on_a_failing_run(self):
        with tempfile.TemporaryDirectory() as tmp:
            log = Path(tmp) / 'run.log'
            log.write_text(good_log(spawned=2), encoding='utf-8')
            interactives = Path(tmp) / 'interactives.json'
            interactives.write_text(json.dumps(document()), encoding='utf-8')
            self.assertEqual(ci.main(['--log', str(log), '--interactives', str(interactives)]), 1)

    def test_main_returns_two_on_a_missing_input(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(ci.main(['--log', str(Path(tmp) / 'none.log'), '--interactives', str(Path(tmp) / 'x.json')]), 2)


class QuickChangeTest(unittest.TestCase):
    def grade(self, rows=None):
        out = ci.Results()
        ci.grade_quick_change('\n'.join(QUICK_RUN if rows is None else rows) + '\n', out)
        return out.rows[0]

    def changed(self, index, old, new):
        rows = list(QUICK_RUN)
        self.assertIn(old, rows[index])
        rows[index] = rows[index].replace(old, new)
        return self.grade(rows)

    def test_the_full_sequence_passes(self):
        self.assertEqual(self.grade()[1], 'PASS')

    def test_no_lines_fail(self):
        self.assertEqual(self.grade([])[1], 'FAIL')

    def test_an_extra_reset_after_the_sequence_fails(self):
        extra = 'OWQUICKCHANGE reset id=S ok=1 cost=200 before=4800 after=4600 reason='
        self.assertEqual(self.grade(QUICK_RUN + [extra])[1], 'FAIL')

    def test_a_reset_with_no_cancel_after_walking_away_fails(self):
        # The stale-confirm bug: one press after coming back resets.
        rows = [r for r in QUICK_RUN if 'cancel' not in r]
        self.assertEqual(self.grade(rows)[1], 'FAIL')

    def test_a_prompt_cancelled_for_another_reason_fails(self):
        row = self.changed(2, 'not the selected target', 'expired')
        self.assertEqual(row[1], 'FAIL')
        self.assertIn('expired', row[2])

    def test_a_charged_price_other_than_the_prompted_one_fails(self):
        row = self.changed(6, 'cost=200', 'cost=150')
        self.assertEqual(row[1], 'FAIL')
        self.assertIn('the prompt showed 150', row[2])

    def test_a_refused_price_other_than_the_prompted_one_fails(self):
        self.assertEqual(self.changed(3, 'cost=200', 'cost=150')[1], 'FAIL')

    def test_a_reset_on_another_stations_prompt_fails(self):
        row = self.changed(7, 'id=S', 'id=T')
        self.assertEqual(row[1], 'FAIL')
        self.assertIn('prompt of S', row[2])

    def test_a_wrong_charge_fails(self):
        self.assertEqual(self.changed(7, 'after=4800', 'after=4700')[1], 'FAIL')

    def test_money_changed_by_the_prompt_fails(self):
        self.assertEqual(self.changed(7, 'before=5000 after=4800', 'before=4900 after=4700')[1], 'FAIL')

    def test_a_low_money_confirm_that_took_money_fails(self):
        self.assertEqual(self.changed(4, 'before=0 after=0', 'before=10 after=0')[1], 'FAIL')

    def test_grades_changed_by_the_refusal_fail(self):
        self.assertEqual(self.changed(5, 'grades_same=1', 'grades_same=0')[1], 'FAIL')

    def test_a_refused_final_reset_fails(self):
        self.assertEqual(self.changed(7, 'ok=1', 'ok=0')[1], 'FAIL')

    def test_a_reset_that_left_the_grades_fails(self):
        self.assertEqual(self.changed(8, 'grades_changed=1', 'grades_changed=0')[1], 'FAIL')


class LidCaptureTest(unittest.TestCase):
    def grade(self, text):
        out = ci.Results()
        ci.grade_lid_capture(text, out)
        return out.rows[0]

    def test_all_three_frames_of_each_type_pass(self):
        text = 'OWLOOTLID queue=1\nOWLOOTLID view balance=Synth clear=1 yaw=0\n' + ''.join(f'OWLOOTLID shot balance=Synth phase={p}\n' for p in ('closed', 'mid', 'opened'))
        row = self.grade(text)
        self.assertEqual(row[1], 'PASS')
        self.assertIn('UNVERIFIED', row[2])

    FRAMES = ''.join(f'OWLOOTLID shot balance=Synth phase={p}\n' for p in ('closed', 'mid', 'opened'))

    def test_no_clear_view_is_incomplete_and_fails_the_run(self):
        row = self.grade('OWLOOTLID queue=1\nOWLOOTLID view balance=Synth clear=0 yaw=-1\n' + self.FRAMES)
        self.assertEqual(row[1], 'INCOMPLETE')
        self.assertIn('Synth', row[2])
        out = ci.Results()
        out.add('lid_capture', *row[1:])
        self.assertEqual(out.print(), 1)

    def test_frames_without_a_view_line_fail(self):
        self.assertEqual(self.grade('OWLOOTLID queue=1\n' + self.FRAMES)[1], 'FAIL')

    def test_a_missing_frame_fails(self):
        self.assertEqual(self.grade('OWLOOTLID queue=1\nOWLOOTLID shot balance=Synth phase=closed\n')[1], 'FAIL')

    def with_content(self, folder, meshes):
        """A checkout's Content folder holding each named mesh and its open clip, and a manifest of those plus Bare (no assets)."""
        root = Path(folder) / 'OpenWillow/Lootables'
        balances = {}
        objects = []
        for name in (*meshes, 'Bare'):
            balances[f'Grades.{name}'] = {'visual': {'components': [{'skeletal_mesh': f'Pkg.Meshes.Skel_{name}', 'anim_sets': [f'Pkg.Anims.Anim_{name}']}]}}
            objects.append({'id': f'Map:{name}', 'balance': f'Grades.{name}', 'supported': True})
        for name in meshes:
            mesh = root / f'Skel_{name}/Skel_{name}/SkeletalMeshes/Skel_{name}.uasset'
            mesh.parent.mkdir(parents=True)
            mesh.write_bytes(b'')
            clip = root / f'Anims/Anim_{name}_Open.uasset'
            clip.parent.mkdir(parents=True, exist_ok=True)
            clip.write_bytes(b'')
        return {'objects': objects, 'balances': balances}

    def test_expected_types_are_read_from_the_content_on_disk(self):
        with tempfile.TemporaryDirectory() as folder:
            doc = self.with_content(folder, ['A', 'B'])
            self.assertEqual(ci.expected_lid_types(doc, folder), ['A', 'B'])
            self.assertIsNone(ci.expected_lid_types(doc, None))

    def test_a_mesh_without_its_clip_is_not_expected(self):
        with tempfile.TemporaryDirectory() as folder:
            doc = self.with_content(folder, ['A'])
            (Path(folder) / 'OpenWillow/Lootables/Anims/Anim_A_Open.uasset').unlink()
            self.assertEqual(ci.expected_lid_types(doc, folder), [])

    def test_a_host_load_failure_fails_when_the_content_is_there(self):
        # Not SKIPPED: the assets are on disk now, and the host did not load them.
        out = ci.Results()
        ci.grade_lid_capture('OWLOOTLID queue=0\n', out, ['A', 'B'])
        self.assertEqual(out.rows[0][1], 'FAIL')

    def test_a_partial_load_fails(self):
        out = ci.Results()
        ci.grade_lid_capture('OWLOOTLID queue=1\nOWLOOTLID view balance=A clear=1 yaw=0\n' + ''.join(
            f'OWLOOTLID shot balance=A phase={p}\n' for p in ('closed', 'mid', 'opened')), out, ['A', 'B'])
        self.assertEqual(out.rows[0][1], 'FAIL')

    def test_a_checkout_without_content_is_skipped_whatever_an_old_import_said(self):
        out = ci.Results()
        ci.grade_lid_capture('OWLOOTLID queue=0\n', out, [])
        self.assertEqual(out.rows[0][1], 'SKIPPED')

    def test_no_content_folder_given_cannot_skip(self):
        out = ci.Results()
        ci.grade_lid_capture('OWLOOTLID queue=0\n', out, None)
        self.assertEqual(out.rows[0][1], 'MISSING')

    def test_a_lid_capture_open_does_not_count_for_the_test_step(self):
        # Three balances in the log, but only one came from the test step.
        text = good_log().replace('OWINTERACTIVETEST test_open id=Map:Point_1 balance=Balance.1\n', 'x\n').replace(
            'OWINTERACTIVETEST test_open id=Map:Point_2 balance=Balance.2\n', 'x\n')
        self.assertEqual(status_of(grade(text), 'opened_balances'), 'FAIL')

    def test_a_test_open_with_the_wrong_balance_fails(self):
        text = good_log().replace('test_open id=Map:Point_0 balance=Balance.0', 'test_open id=Map:Point_0 balance=Balance.9')
        self.assertEqual(status_of(grade(text), 'opened_balances'), 'FAIL')

    def test_no_queue_line_is_missing(self):
        self.assertEqual(self.grade('')[1], 'MISSING')

    def test_phases_are_checked_per_type_not_only_in_total(self):
        # Six frames for two types, but one type has closed twice and no opened.
        text = 'OWLOOTLID queue=2\n' + ''.join(f'OWLOOTLID shot balance=A phase={p}\n' for p in ('closed', 'closed', 'mid'))
        text += ''.join(f'OWLOOTLID shot balance=B phase={p}\n' for p in ('closed', 'mid', 'opened'))
        self.assertEqual(self.grade(text)[1], 'FAIL')

    def test_a_missing_type_fails(self):
        text = 'OWLOOTLID queue=2\n' + ''.join(f'OWLOOTLID shot balance=A phase={p}\n' for p in ('closed', 'mid', 'opened'))
        self.assertEqual(self.grade(text)[1], 'FAIL')


if __name__ == '__main__':
    unittest.main()
