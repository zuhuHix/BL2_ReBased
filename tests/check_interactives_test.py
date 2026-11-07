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


PLANNED_PATH = '/Game/OpenWillow/Lootables/Materials/Mat_1/Mat_1.Mat_1'
MATERIAL_PLAN = {'meshes': [{'mesh': 'Skel_Synth', 'instance': 'Mat_1'}], 'pickups': []}


def document_with_visual():
    """document(), with one balance that shows an imported mesh and clip."""
    doc = document()
    doc['balances']['Balance.0'] = {'visual': {'components': [{'skeletal_mesh': 'Pkg.Meshes.Skel_Synth', 'anim_sets': ['Pkg.Anims.Anim_Synth']}]}}
    return doc


def write_frames(folder, balance='Balance.0'):
    for phase in ('closed', 'mid', 'opened'):
        (Path(folder) / ci.lid_frame_name(balance, phase)).write_bytes(b'')


def write_content(folder):
    root = Path(folder) / 'OpenWillow/Lootables'
    for path in (root / 'Skel_Synth/Skel_Synth/SkeletalMeshes/Skel_Synth.uasset', root / 'Anims/Anim_Synth_Open.uasset'):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b'')


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


QUICK_LOOKED = 'OWQCTEST looked away selected=0 target=none'
VIEW_BLOCKED = 'OWQCTEST blocked selected=0 target=none fixture=1'
VIEW_CLEAR = 'OWQCTEST clear selected=1 target=interactive'
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
    text += line('OWLOOT slot id=Map:Point_0 point=Ammo1 at=socket x=1.0 y=-2.0 z=3.0 from_container=40.5')
    text += line('OWLOOT slot id=Map:Point_0 point=Ammo2 at=front x=1.0 y=-2.0 z=3.0 from_container=110.0 reason=placeholder')
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
    text += line('OWLOOTLID view balance=Balance.0 clear=1 yaw=45')
    text += line(f'OWLOOTLID material balance=Balance.0 id=Map:Point_2 path={PLANNED_PATH}')
    for phase in ('closed', 'mid', 'opened'):
        text += line(f'OWLOOTLID shot balance=Balance.0 phase={phase}')
    text += ''.join(line(row) for row in [QUICK_LOOKED, VIEW_BLOCKED, VIEW_CLEAR, *QUICK_RUN])
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
        self.assertEqual(status_of(results, 'drop_slots'), 'PASS')
        self.assertEqual(status_of(results, 'pickups'), 'PASS')
        self.assertEqual(status_of(results, 'test_sequence'), 'PASS')
        self.assertEqual(status_of(results, 'automation'), 'PASS')


class DropSlotsTest(unittest.TestCase):
    def test_a_drop_on_an_imported_mesh_with_no_socket_fails(self):
        results = grade(good_log() + line('OWLOOT slot id=Map:Point_2 point=Gun9 at=front x=1.0 y=1.0 z=1.0 from_container=5.0 reason=no socket'))
        self.assertEqual(status_of(results, 'drop_slots'), 'FAIL')

    def test_a_socket_drop_far_from_its_lootable_fails(self):
        text = good_log().replace('from_container=40.5', 'from_container=9000.0')
        self.assertEqual(status_of(grade(text), 'drop_slots'), 'FAIL')

    def test_a_drop_with_no_placement_line_fails(self):
        # A spawn that failed used to be counted: its line came first. Now an open's drops and lines must agree.
        text = ''.join(row for row in good_log().splitlines(True) if 'point=Ammo2' not in row)
        self.assertEqual(status_of(grade(text), 'drop_slots'), 'FAIL')

    def test_a_placement_line_with_no_drop_fails(self):
        text = good_log() + line('OWLOOT slot id=Map:Point_1 point=Gun at=socket x=1.0 y=1.0 z=1.0 from_container=5.0')
        self.assertEqual(status_of(grade(text), 'drop_slots'), 'FAIL')

    def test_a_placement_line_without_a_position_is_not_a_placement(self):
        text = good_log().replace(' x=1.0 y=-2.0 z=3.0 from_container=40.5', '')
        self.assertEqual(status_of(grade(text), 'drop_slots'), 'FAIL')

    def test_drops_with_no_slot_line_are_missing(self):
        text = ''.join(row for row in good_log().splitlines(True) if 'OWLOOT slot' not in row)
        self.assertEqual(status_of(grade(text), 'drop_slots'), 'MISSING')

    def test_an_unknown_reason_is_not_excused(self):
        results = grade(good_log() + line('OWLOOT slot id=Map:Point_2 point=Gun9 at=front x=1.0 y=1.0 z=1.0 from_container=5.0 reason=later'))
        self.assertEqual(status_of(results, 'drop_slots'), 'FAIL')


class FullRunTest(unittest.TestCase):
    def test_a_good_run_with_both_sources_passes_every_row(self):
        with tempfile.TemporaryDirectory() as folder:
            write_content(folder)
            write_frames(folder)
            results = ci.grade(good_log(), AUTOMATION_OK, document_with_visual(), False, folder, MATERIAL_PLAN, folder)
        self.assertEqual([(n, s) for n, s, _ in results.rows if s != 'PASS'], [])

    def test_the_same_run_from_the_command_line(self):
        with tempfile.TemporaryDirectory() as folder:
            write_content(folder)
            write_frames(folder)
            base = Path(folder)
            (base / 'run.log').write_text(good_log(), encoding='utf-8')
            (base / 'i.json').write_text(json.dumps(document_with_visual()), encoding='utf-8')
            (base / 'plan.json').write_text(json.dumps(MATERIAL_PLAN), encoding='utf-8')
            with contextlib.redirect_stdout(io.StringIO()):
                code = ci.main(['--log', str(base / 'run.log'), '--interactives', str(base / 'i.json'), '--content-root', folder,
                                '--materials-plan', str(base / 'plan.json'), '--frames', folder, '--skip-automation'])
            self.assertEqual(code, 0)

    def test_no_content_folder_or_no_plan_cannot_pass(self):
        # A direct run that names no source grades only what the log says, so it must not read as a pass.
        results = ci.grade(good_log(), AUTOMATION_OK, document_with_visual())
        self.assertEqual(status_of(results, 'lid_capture'), 'MISSING')
        self.assertEqual(status_of(results, 'lootable_materials'), 'MISSING')
        with tempfile.TemporaryDirectory() as folder:
            write_content(folder)
            write_frames(folder)
            results = ci.grade(good_log(), AUTOMATION_OK, document_with_visual(), False, folder, None, folder)
            self.assertEqual(status_of(results, 'lootable_materials'), 'MISSING')
            self.assertEqual(status_of(results, 'lid_capture'), 'PASS')

    def test_no_lootables_fails_the_run_unless_the_run_allows_it(self):
        # document() has no imported lootable, as on a machine without the import. The rest of the run is good.
        log = good_log().replace('OWLOOTLID queue=1', 'OWLOOTLID queue=0')
        lines = [row for row in log.splitlines(True) if 'OWLOOTLID shot' not in row and 'OWLOOTLID view' not in row and 'OWLOOTLID material' not in row]
        with tempfile.TemporaryDirectory() as folder:
            strict = ci.grade(''.join(lines), AUTOMATION_OK, document(), False, folder, MATERIAL_PLAN, folder)
            allowed = ci.grade(''.join(lines), AUTOMATION_OK, document(), False, folder, MATERIAL_PLAN, folder, True)
        self.assertEqual([status_of(strict, n) for n in ('lid_capture', 'lootable_materials')], ['FAIL', 'FAIL'])
        with contextlib.redirect_stdout(io.StringIO()) as printed:
            self.assertEqual(strict.print(), 2)
        self.assertNotIn('allow_no_lootables', printed.getvalue())
        self.assertEqual([status_of(allowed, n) for n in ('lid_capture', 'lootable_materials')], ['SKIPPED', 'SKIPPED'])
        with contextlib.redirect_stdout(io.StringIO()) as printed:
            self.assertEqual(allowed.print(), 0)
        self.assertIn('allow_no_lootables=1', printed.getvalue())


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


class ViewBlockerTest(unittest.TestCase):
    def grade(self, *rows):
        out = ci.Results()
        ci.grade_view_blocker('\n'.join([*rows, QUICK_RUN[0]]) + '\n', out)
        return out.rows[0][1]

    def test_blocked_then_clear_passes(self):
        self.assertEqual(self.grade(VIEW_BLOCKED, VIEW_CLEAR), 'PASS')

    def test_the_target_kept_behind_the_blocker_fails(self):
        self.assertEqual(self.grade('OWQCTEST blocked selected=1 target=interactive fixture=1', VIEW_CLEAR), 'FAIL')

    def test_another_target_behind_the_blocker_fails(self):
        self.assertEqual(self.grade('OWQCTEST blocked selected=0 target=door fixture=1', VIEW_CLEAR), 'FAIL')

    def test_no_fixture_shows_nothing(self):
        self.assertEqual(self.grade('OWQCTEST blocked selected=0 target=none fixture=0', VIEW_CLEAR), 'FAIL')

    def test_the_clear_ray_must_select_it(self):
        self.assertEqual(self.grade(VIEW_BLOCKED, 'OWQCTEST clear selected=0 target=none'), 'FAIL')

    def test_a_clear_ray_taken_by_another_target_fails(self):
        self.assertEqual(self.grade(VIEW_BLOCKED, 'OWQCTEST clear selected=0 target=vendor'), 'FAIL')

    def test_a_lost_line_fails(self):
        self.assertEqual(self.grade(VIEW_BLOCKED), 'FAIL')
        self.assertEqual(self.grade(VIEW_CLEAR), 'FAIL')

    def test_the_wrong_order_fails(self):
        self.assertEqual(self.grade(VIEW_CLEAR, VIEW_BLOCKED), 'FAIL')


class QuickChangeTest(unittest.TestCase):
    def grade(self, rows=None):
        out = ci.Results()
        ci.grade_quick_change('\n'.join([QUICK_LOOKED, *(QUICK_RUN if rows is None else rows)]) + '\n', out)
        return out.rows[0]

    def graded_as_is(self, rows):
        out = ci.Results()
        ci.grade_quick_change('\n'.join(rows) + '\n', out)
        return out.rows[0]

    def test_no_look_away_line_fails(self):
        self.assertEqual(self.graded_as_is(QUICK_RUN)[1], 'FAIL')

    def test_a_station_selected_while_looking_away_fails(self):
        self.assertEqual(self.graded_as_is(['OWQCTEST looked away selected=1', *QUICK_RUN])[1], 'FAIL')

    def test_another_target_while_looking_away_fails(self):
        # The station is not selected, but the press would still go to something.
        self.assertEqual(self.graded_as_is(['OWQCTEST looked away selected=0 target=vendor', *QUICK_RUN])[1], 'FAIL')

    def test_a_prompt_before_the_look_away_line_fails(self):
        self.assertEqual(self.graded_as_is([QUICK_RUN[0], QUICK_LOOKED, *QUICK_RUN[1:]])[1], 'FAIL')

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

    def test_an_unknown_station_event_fails(self):
        for row in ('OWQUICKCHANGE refund id=S cost=200', 'OWQUICKCHANGE prompt id=S cost=x money=5000'):
            with self.subTest(row=row):
                self.assertEqual(self.grade([*QUICK_RUN, row])[1], 'FAIL')
                self.assertIn('unknown station event', self.grade([*QUICK_RUN, row])[2])

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


class LootableMaterialsTest(unittest.TestCase):
    A = '/Game/OpenWillow/Lootables/Materials/Mat_a/Mat_a.Mat_a'
    B = '/Game/OpenWillow/Lootables/Materials/Mat_b/Mat_b.Mat_b'

    def grade(self, text, expected):
        out = ci.Results()
        ci.grade_lootable_materials(text, out, expected)
        return out.rows[0]

    def row(self, balance, path):
        return f'OWLOOTLID material balance={balance} id=Map:x path={path}\n'

    def test_every_type_with_its_planned_instance_passes(self):
        row = self.grade(self.row('G.A', self.A) + self.row('G.B', self.B), {'G.A': self.A, 'G.B': self.B})
        self.assertEqual(row[1], 'PASS')
        self.assertIn('UNVERIFIED', row[2])

    def test_another_bound_instance_is_not_enough(self):
        # Under the materials folder, but not the instance planned for this type.
        row = self.grade(self.row('G.A', self.B) + self.row('G.B', self.B), {'G.A': self.A, 'G.B': self.B})
        self.assertEqual(row[1], 'INCOMPLETE')
        self.assertIn('G.A', row[2])
        self.assertNotIn('G.B', row[2])

    def test_the_importers_default_slot_fails_the_run(self):
        # The false pass this replaces: the bind failed, and the suite was green with grey lootables.
        row = self.grade(self.row('G.A', '/Game/OpenWillow/Lootables/Skel_A/Skel_A/Materials/Mat.Mat'), {'G.A': self.A})
        self.assertEqual(row[1], 'INCOMPLETE')
        results = ci.Results()
        results.add(*row)
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(results.print(), 1)

    def test_no_material_at_all_is_incomplete(self):
        self.assertEqual(self.grade(self.row('G.A', ''), {'G.A': self.A})[1], 'INCOMPLETE')

    def test_a_type_the_plan_has_no_instance_for_is_incomplete(self):
        self.assertEqual(self.grade(self.row('G.A', self.A), {'G.A': None})[1], 'INCOMPLETE')

    def test_a_type_without_a_line_is_missing(self):
        self.assertEqual(self.grade(self.row('G.A', self.A), {'G.A': self.A, 'G.B': self.B})[1], 'MISSING')

    def test_balances_sharing_a_short_name_are_not_one_line(self):
        self.assertEqual(self.grade(self.row('One.Leaf', self.A), {'One.Leaf': self.A, 'Two.Leaf': self.A})[1], 'MISSING')

    def test_a_checkout_without_lootable_content_fails_unless_allowed(self):
        self.assertEqual(self.grade('', {})[1], 'FAIL')
        out = ci.Results()
        ci.grade_lootable_materials('', out, {}, True)
        self.assertEqual(out.rows[0][1], 'SKIPPED')

    def test_a_missing_source_cannot_pass(self):
        self.assertEqual(self.grade(self.row('G.A', self.A), None)[1], 'MISSING')

    def test_expected_paths_come_from_the_plan_and_the_content(self):
        with tempfile.TemporaryDirectory() as folder:
            write_content(folder)
            doc = document_with_visual()
            self.assertEqual(ci.expected_lid_materials(doc, folder, MATERIAL_PLAN), {'Balance.0': PLANNED_PATH})
            self.assertEqual(ci.expected_lid_materials(doc, folder, {'meshes': []}), {'Balance.0': None})
            self.assertIsNone(ci.expected_lid_materials(doc, folder, None))
            self.assertIsNone(ci.expected_lid_materials(doc, None, MATERIAL_PLAN))
            self.assertEqual(ci.expected_lid_materials(document(), folder, None), {})   # no content for its balances


class LidCaptureTest(unittest.TestCase):
    def grade(self, text, expected=('Synth',), frames_dir=None):
        out = ci.Results()
        ci.grade_lid_capture(text, out, expected, frames_dir)
        return out.rows[0]

    def test_all_three_frames_of_each_type_pass(self):
        text = 'OWLOOTLID queue=1\nOWLOOTLID view balance=Synth clear=1 yaw=0\n' + ''.join(f'OWLOOTLID shot balance=Synth phase={p}\n' for p in ('closed', 'mid', 'opened'))
        with tempfile.TemporaryDirectory() as folder:
            write_frames(folder, 'Synth')
            row = self.grade(text, ('Synth',), folder)
            self.assertEqual(row[1], 'PASS')
            self.assertIn('UNVERIFIED', row[2])
            self.assertEqual(self.grade(text, None, folder)[1], 'MISSING')   # no source says which types had to be captured
            self.assertEqual(self.grade(text, ('Synth',), None)[1], 'MISSING')   # nor that the frames were written
            (Path(folder) / ci.lid_frame_name('Synth', 'mid')).unlink()
            row = self.grade(text, ('Synth',), folder)
            self.assertEqual(row[1], 'FAIL')
            self.assertIn('Synth_mid', row[2])

    def test_two_balances_with_one_frame_name_fail(self):
        balances = ('Pkg_A.Obj', 'Pkg.A_Obj')
        self.assertEqual(ci.lid_frame_name(balances[0], 'mid'), ci.lid_frame_name(balances[1], 'mid'))
        text = 'OWLOOTLID queue=2\n'
        for balance in balances:
            text += f'OWLOOTLID view balance={balance} clear=1 yaw=0\n'
            text += ''.join(f'OWLOOTLID shot balance={balance} phase={p}\n' for p in ('closed', 'mid', 'opened'))
        with tempfile.TemporaryDirectory() as folder:
            for balance in balances:
                write_frames(folder, balance)
            row = self.grade(text, balances, folder)
        self.assertEqual(row[1], 'FAIL')
        self.assertIn('share one frame name', row[2])

    def test_each_run_has_its_own_log_and_frames_folder(self):
        script = (Path(__file__).resolve().parents[1] / 'tools/test_interactives.ps1').read_text(encoding='utf-8')
        stamp = next(row for row in script.splitlines() if row.startswith('$stamp = '))
        self.assertIn('$PID', stamp)
        self.assertIn('NewGuid', stamp)
        self.assertIn('"shots-$stamp"', script)

    def test_a_frame_name_is_unique_per_full_balance(self):
        self.assertNotEqual(ci.lid_frame_name('One.Leaf', 'mid'), ci.lid_frame_name('Two.Leaf', 'mid'))
        self.assertEqual(ci.lid_frame_name('Pkg.Grades.Leaf', 'mid'), 'OWLootLid_Pkg_Grades_Leaf_mid.png')

    def test_frames_of_another_balance_with_the_same_short_name_fail(self):
        frames = ''.join(f'OWLOOTLID shot balance=Other.Leaf phase={p}\n' for p in ('closed', 'mid', 'opened'))
        row = self.grade('OWLOOTLID queue=1\nOWLOOTLID view balance=Other.Leaf clear=1 yaw=0\n' + frames, ['Grades.Leaf'])
        self.assertEqual(row[1], 'FAIL')

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
            self.assertEqual(ci.expected_lid_types(doc, folder), ['Grades.A', 'Grades.B'])
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

    def test_a_checkout_without_content_fails_unless_allowed(self):
        out = ci.Results()
        ci.grade_lid_capture('OWLOOTLID queue=0\n', out, [])
        self.assertEqual(out.rows[0][1], 'FAIL')
        out = ci.Results()
        ci.grade_lid_capture('OWLOOTLID queue=0\n', out, [], None, True)
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
