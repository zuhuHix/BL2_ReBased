"""Synthetic tests for tools/check_vendors.py: invented ids, money and prices, no game data.

Run: python -m pytest tests/check_vendors_test.py
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_vendors as cv  # noqa: E402

LEVEL = 10          # markup 10 x 0.14 + 4.1 = 5.5 exactly in single precision, so the prices below are plain products
START_MONEY = 5000
SHOP_VALUES = [100, 51, 80, 60, 30, 20, 10]       # the first shelf item is worth 100 (price 550)
SHOP_PRICES = [550, 280, 440, 330, 165, 110, 55]  # floor(value x 5.5)
FEATURED = {'id': 'F1', 'name': 'Featured', 'kind': 'weapon', 'rarity': 'rare', 'level': LEVEL, 'price': 1100,
            'value': 200, 'affordable': True, 'featured': True, 'quantity': 1, 'card': {}}


MACHINE_KINDS = ['weapons', 'items', 'health']


def machines_doc(count=3, unsupported=1):
    machines = [{'id': f'Map:Point_{i}', 'kind': MACHINE_KINDS[i], 'title': f'Machine {i}',
                 'supported': True, 'fixed_item_cost': 0, 'fixed_featured_cost': 0} for i in range(count)]
    machines += [{'id': f'Map:Black_{i}', 'kind': 'blackmarket', 'supported': False} for i in range(unsupported)]
    return {'schema': 'openwillow-vendors/1', 'machines': machines}


def shelf(ids, values=SHOP_VALUES, prices=SHOP_PRICES, with_value=True):
    rows = []
    for item_id, value, price in zip(ids, values, prices):
        row = {'id': item_id, 'name': item_id, 'kind': 'weapon', 'rarity': 'common', 'level': LEVEL, 'price': price,
               'affordable': True, 'featured': False, 'quantity': 1, 'card': {}}
        if with_value:
            row['value'] = value
        rows.append(row)
    return rows


AMMO_TERMS = {'base_cost': 5.0, 'scaler': cv.f32(1.12), 'part_total': 1.0}
HEAL_TERMS = {'base_cost': 3.0, 'scaler': cv.f32(1.12), 'part_total': 1.0}
AMMO_VALUE = cv.item_value(AMMO_TERMS, LEVEL)
AMMO_PRICE = cv.buy_price(AMMO_VALUE, LEVEL)
HEAL_VALUE = cv.item_value(HEAL_TERMS, LEVEL)
HEAL_PRICE = cv.buy_price(HEAL_VALUE, LEVEL)


def item_row(item_id, kind, terms, **extra):
    """A non-gun shelf row with its value and price worked out from its terms (as the host sends them)."""
    value = cv.item_value(terms, LEVEL)
    row = {'id': item_id, 'name': item_id, 'kind': kind, 'rarity': 1, 'level': LEVEL, 'price': cv.buy_price(value, LEVEL),
           'value': value, 'valueTerms': terms, 'affordable': True, 'featured': False, 'quantity': 1}
    row.update(extra)
    return row


def section_state(machine_id, kind, shop, money, ammo=None, health=None):
    doc = {'serial': 1, 'open': True, 'machine': {'id': machine_id, 'kind': kind, 'title': machine_id}, 'money': money,
           'eridium': 0, 'capacity': 12, 'used': 0, 'shop': shop, 'featured': None, 'backpack': [], 'buyback': [],
           'resetSeconds': 600, 'message': ''}
    if ammo is not None:
        doc['ammo'] = ammo
    if health is not None:
        doc['health'] = health
    return doc


def state(shop, money=START_MONEY, featured=FEATURED):
    return {'serial': 1, 'open': True, 'machine': {'id': 'Map:Point_0', 'kind': 'weapons', 'title': 'Machine 0'},
            'money': money, 'eridium': 0, 'capacity': 12, 'used': 0, 'shop': shop, 'featured': featured,
            'backpack': [], 'buyback': [], 'resetSeconds': 1200, 'message': ''}


def line(text):
    return f'LogTemp: Display: {text}\n'


def state_line(doc):
    return line('OWVENDSTATE ' + json.dumps(doc, separators=(',', ':')))


def good_log(spawned=3, sell=True, restock=True, first_ids=None, second_ids=None, with_value=True, sections=True):
    """A complete run: open, report, buy the first item, report, sell it, restock, report.

    The report after the buy shows the shelf without the bought item plus one new item (S8)."""
    first_ids = first_ids or [f'S{i}' for i in range(1, 8)]
    second_ids = second_ids or [f'N{i}' for i in range(1, 8)]
    after_buy_ids = first_ids[1:] + ['S8']
    text = line(f'OpenWillow vendors: {spawned} machine(s) spawned from x.json')
    text += line('OWVENDOR open id=Map:Point_0 kind=weapons restocked=1')
    featured = FEATURED if with_value else {k: v for k, v in FEATURED.items() if k != 'value'}
    text += state_line(state(shelf(first_ids, with_value=with_value), featured=featured))
    text += line(f'OWVENDOR buy id={first_ids[0]} ok=1 money={START_MONEY - SHOP_PRICES[0]} error=')
    text += state_line(state(shelf(after_buy_ids, with_value=with_value), money=START_MONEY - SHOP_PRICES[0],
                             featured=featured))
    if sell:
        text += line(f'OWVENDOR sell id=G1 ok=1 money={START_MONEY - SHOP_PRICES[0] + SHOP_VALUES[0]} error=')
    if restock:
        text += line('OWVENDOR restock id=Map:Point_0 ok=1')
        text += state_line(state(shelf(second_ids, with_value=with_value),
                                 money=START_MONEY - SHOP_PRICES[0] + SHOP_VALUES[0], featured=featured))
    if sections:
        text += items_section(START_MONEY - SHOP_PRICES[0] + SHOP_VALUES[0])
        text += health_section(START_MONEY - SHOP_PRICES[0] + SHOP_VALUES[0] - AMMO_PRICE)
    return text


def items_section(money, ammo_before=0, ammo_after=54, pool_max=100, bought='A1', price=AMMO_PRICE):
    """The Ammo Dump: open, report, buy row 0 (ammo), the pool line, report."""
    shelf = [item_row('A1', 'ammo', AMMO_TERMS, ammoType='pistol', amount=54)]
    before = section_state('Map:Point_1', 'items', shelf, money, ammo={'pistol': {'current': ammo_before, 'max': pool_max}})
    after = section_state('Map:Point_1', 'items', shelf, money - price,
                          ammo={'pistol': {'current': ammo_after, 'max': pool_max}})
    text = line('OWVENDOR open id=Map:Point_1 kind=items restocked=1') + state_line(before)
    text += line(f'OWVENDAMMO type=pistol before={ammo_before} after={ammo_after} max={pool_max}')
    text += line(f'OWVENDOR buy id={bought} ok=1 money={money - price} error=')
    return text + state_line(after)


def health_section(money, health_before=100.0, health_after=200.0, health_max=400.0, refused=None):
    """Dr. Zed: open, report, buy row 0 (a heal of a quarter of max health), the heal line, report."""
    heal = item_row('H1', 'health', HEAL_TERMS, healFraction=0.25)
    before = section_state('Map:Point_2', 'health', [heal], money, health={'current': health_before, 'max': health_max})
    after = section_state('Map:Point_2', 'health', [heal], money - HEAL_PRICE,
                          health={'current': health_after, 'max': health_max})
    text = line('OWVENDOR open id=Map:Point_2 kind=health restocked=1') + state_line(before)
    if refused:
        return text + line(f'OWVENDOR buy id=H1 ok=0 money={money} error={refused}')
    text += line(f'OWVENDHEAL before={health_before:.3f} after={health_after:.3f} max={health_max:.3f}')
    text += line(f'OWVENDOR buy id=H1 ok=1 money={money - HEAL_PRICE} error=')
    return text + state_line(after)


GOOD_AUTOMATION = ('LogAutomationController: Display: Test Completed. Result={Passed}\tName={OpenWillow.Vendor.Stock}'
                   '\tPath={OpenWillow.Vendor.Stock}\n'
                   'LogAutomationController: Display: Test Completed. Result={Passed}\tName={OpenWillow.Vendor.Persist}'
                   '\tPath={OpenWillow.Vendor.Persist}\n'
                   'LogAutomationController: Display: Test Completed. Result={Passed}\tName={OpenWillow.Vendor.Items}'
                   '\tPath={OpenWillow.Vendor.Items}\n')


def run(log, automation=GOOD_AUTOMATION, doc=None, open_index=0):
    doc = doc if doc is not None else machines_doc()
    return cv.grade(log, automation, doc, open_index, 'weapons')


def by_name(results):
    return {name: (status, detail) for name, status, detail in results.rows}


class PriceFormulaTest(unittest.TestCase):
    def test_level_ten_is_a_plain_product(self):
        self.assertEqual(cv.buy_price(100, 10), 550)
        self.assertEqual(cv.buy_price(51, 10), 280)   # floor(280.5): the automation test's own number

    def test_single_precision_rounding_differs_from_double(self):
        # 30 x 4.1 is 122.99999... in double (floor 122) but rounds to 123.0 in single precision, which is what the host
        # computes. A double-precision checker would falsely FAIL this row.
        self.assertEqual(cv.buy_price(30, 0), 123)
        self.assertEqual(cv.buy_price(101, 0), 414)


class GradeTest(unittest.TestCase):
    def test_full_run_all_pass(self):
        results = run(good_log())
        self.assertEqual(sum(1 for _, s, _ in results.rows if s == 'FAIL'), 0)
        self.assertEqual(sum(1 for _, s, _ in results.rows if s == 'MISSING'), 0)
        checks = by_name(results)
        for name in ('machines_spawned', 'open_machine', 'open_shelf', 'prices', 'buy', 'sell', 'restock',
                     'item_prices', 'ammo_buy', 'heal_buy', 'automation_stock', 'automation_persist', 'automation_items'):
            self.assertEqual(checks[name][0], 'PASS', f'{name}: {checks[name][1]}')
        self.assertEqual(checks['buy'][1], 'bought S1 for 550; money 5000 -> 4450')
        self.assertEqual(checks['sell'][1], 'sold G1 for 100; money 4450 -> 4550')

    def test_wrong_spawn_count_fails(self):
        checks = by_name(run(good_log(spawned=2)))
        self.assertEqual(checks['machines_spawned'][0], 'FAIL')

    def test_missing_spawn_line_fails(self):
        checks = by_name(run(good_log().replace('machine(s) spawned', 'machine(s) lost')))
        self.assertEqual(checks['machines_spawned'][0], 'FAIL')

    def test_price_off_by_one_fails(self):
        bad = shelf([f'S{i}' for i in range(1, 8)], prices=[549] + SHOP_PRICES[1:])
        log = good_log().replace(json.dumps(state(shelf([f'S{i}' for i in range(1, 8)])), separators=(',', ':')),
                                 json.dumps(state(bad), separators=(',', ':')))
        checks = by_name(run(log))
        self.assertEqual(checks['prices'][0], 'FAIL')
        self.assertIn('price 549 != 550', checks['prices'][1])

    def test_fixed_cost_replaces_the_formula(self):
        doc = machines_doc()
        doc['machines'][0]['fixed_item_cost'] = 549
        shop = shelf([f'S{i}' for i in range(1, 8)], prices=[549] + SHOP_PRICES[1:])
        log = good_log().replace(json.dumps(state(shelf([f'S{i}' for i in range(1, 8)])), separators=(',', ':')),
                                 json.dumps(state(shop), separators=(',', ':')))
        checks = by_name(run(log, doc=doc))
        self.assertEqual(checks['prices'][0], 'FAIL')   # a fixed cost replaces the formula on every shelf row
        self.assertIn('price 280 != 549', checks['prices'][1])

    def test_buy_money_drop_wrong_fails(self):
        log = good_log().replace('money=4450 error=', 'money=4400 error=', 1)
        checks = by_name(run(log))
        self.assertEqual(checks['buy'][0], 'FAIL')
        self.assertIn('drop 600', checks['buy'][1])

    def test_refused_buy_fails_with_reason(self):
        log = good_log().replace('OWVENDOR buy id=S1 ok=1 money=4450 error=',
                                 'OWVENDOR buy id=S1 ok=0 money=5000 error=Insufficient funds')
        checks = by_name(run(log))
        self.assertEqual(checks['buy'][0], 'FAIL')
        self.assertIn('Insufficient funds', checks['buy'][1])

    def test_no_sell_line_is_missing_not_failed(self):
        results = run(good_log(sell=False))
        checks = by_name(results)
        self.assertEqual(checks['sell'][0], 'MISSING')
        self.assertIn('SellLast', checks['sell'][1])
        self.assertEqual(sum(1 for _, s, _ in results.rows if s == 'FAIL'), 0)

    def test_sell_adds_the_wrong_amount_fails(self):
        log = good_log().replace('money=4550 error=', 'money=4500 error=')
        checks = by_name(run(log))
        self.assertEqual(checks['sell'][0], 'FAIL')

    def test_rows_without_value_are_missing(self):
        checks = by_name(run(good_log(with_value=False)))
        self.assertEqual(checks['prices'][0], 'MISSING')
        self.assertEqual(checks['sell'][0], 'MISSING')

    def test_restock_with_new_ids_passes(self):
        checks = by_name(run(good_log(second_ids=[f'N{i}' for i in range(1, 8)])))
        self.assertEqual(checks['restock'][0], 'PASS')
        self.assertIn('7 items', checks['restock'][1])

    def test_restock_unchanged_shelf_fails(self):
        # The report after the buy shows S2..S8; a restock that leaves that same shelf is a FAIL.
        log = good_log(second_ids=[f'S{i}' for i in range(2, 9)])
        checks = by_name(run(log))
        self.assertEqual(checks['restock'][0], 'FAIL')
        self.assertIn('unchanged', checks['restock'][1])

    def test_restock_missing_line_fails(self):
        checks = by_name(run(good_log(restock=False)))
        self.assertEqual(checks['restock'][0], 'FAIL')

    def test_page_error_line_fails(self):
        log = good_log() + '\nLogTemp: Display: OpenWillow vending console: OWVENDERR {"error":"backpack[0].kind must be one of weapon"} (x:1)\n'
        checks = by_name(run(log))
        self.assertEqual(checks['page_state'][0], 'FAIL')
        self.assertIn('backpack[0].kind', checks['page_state'][1])

    def test_no_page_error_passes(self):
        self.assertEqual(by_name(run(good_log()))['page_state'][0], 'PASS')

    def test_ue58_success_result_passes(self):
        # UE 5.8 prints Result={Success} (seen in the batch-2 integration run).
        success = GOOD_AUTOMATION.replace('Result={Passed}', 'Result={Success}')
        checks = by_name(run(good_log(), automation=success))
        self.assertEqual(checks['automation_stock'][0], 'PASS')
        self.assertEqual(checks['automation_persist'][0], 'PASS')

    def test_failed_automation_fails(self):
        failed = GOOD_AUTOMATION.replace('Result={Passed}', 'Result={Failed}', 1)   # the Stock line only
        checks = by_name(run(good_log(), automation=failed))
        self.assertEqual(checks['automation_stock'][0], 'FAIL')
        self.assertIn('Result=Failed', checks['automation_stock'][1])
        self.assertEqual(checks['automation_persist'][0], 'PASS')

    def test_automation_without_result_line_fails(self):
        checks = by_name(run(good_log(), automation='LogAutomationController: nothing ran\n'))
        self.assertEqual(checks['automation_stock'][0], 'FAIL')
        self.assertEqual(checks['automation_persist'][0], 'FAIL')

    def test_no_automation_log_is_missing(self):
        checks = by_name(run(good_log(), automation=None))
        self.assertEqual(checks['automation'][0], 'MISSING')

    def test_no_state_lines_fails_and_skips_price_check(self):
        log = ''.join(l for l in good_log().splitlines(keepends=True) if 'OWVENDSTATE' not in l)
        checks = by_name(run(log))
        self.assertEqual(checks['state_json'][0], 'FAIL')
        self.assertEqual(checks['prices'][0], 'MISSING')

    def test_bad_state_json_fails(self):
        log = good_log() + line('OWVENDSTATE {"open": tru')
        checks = by_name(run(log))
        self.assertEqual(checks['state_json'][0], 'FAIL')

    def test_open_refused_fails_with_reason(self):
        log = line('ow.Vendor.Open refused: Index 9 is not a machine')
        checks = by_name(run(log))
        self.assertEqual(checks['open_machine'][0], 'FAIL')
        self.assertIn('Index 9', checks['open_machine'][1])

    def test_wrong_machine_opened_fails(self):
        log = good_log().replace('OWVENDOR open id=Map:Point_0', 'OWVENDOR open id=Map:Point_1')
        checks = by_name(run(log))
        self.assertEqual(checks['open_machine'][0], 'FAIL')

    def test_open_index_outside_the_machines_fails(self):
        results = run(good_log(), open_index=7)
        self.assertEqual(by_name(results)['open_machine'][0], 'FAIL')

    def test_shelf_with_wrong_count_fails(self):
        short = state(shelf([f'S{i}' for i in range(1, 6)], values=SHOP_VALUES[:5], prices=SHOP_PRICES[:5]))
        log = good_log().replace(json.dumps(state(shelf([f'S{i}' for i in range(1, 8)])), separators=(',', ':')),
                                 json.dumps(short, separators=(',', ':')))
        checks = by_name(run(log))
        self.assertEqual(checks['open_shelf'][0], 'FAIL')


class MainTest(unittest.TestCase):
    def write(self, folder, name, text):
        path = Path(folder) / name
        path.write_text(text, encoding='utf-8')
        return path

    def test_exit_zero_and_summary_on_pass(self):
        with tempfile.TemporaryDirectory() as folder:
            log = self.write(folder, 'run.log', good_log())
            auto = self.write(folder, 'auto.log', GOOD_AUTOMATION)
            vendors = self.write(folder, 'vendors.json', json.dumps(machines_doc()))
            import io
            from contextlib import redirect_stdout
            buffer = io.StringIO()
            with redirect_stdout(buffer):
                code = cv.main(['--log', str(log), '--automation-log', str(auto), '--vendors', str(vendors),
                                '--open-index', '0'])
            self.assertEqual(code, 0)
            self.assertIn('SUMMARY passed=15 failed=0 missing=0', buffer.getvalue())
            self.assertIn('CHECK prices PASS', buffer.getvalue())

    def test_exit_one_on_any_fail(self):
        with tempfile.TemporaryDirectory() as folder:
            log = self.write(folder, 'run.log', good_log(spawned=2))
            vendors = self.write(folder, 'vendors.json', json.dumps(machines_doc()))
            import io
            from contextlib import redirect_stdout
            with redirect_stdout(io.StringIO()):
                code = cv.main(['--log', str(log), '--vendors', str(vendors), '--open-index', '0'])
            self.assertEqual(code, 1)

    def test_exit_two_on_unreadable_input(self):
        with tempfile.TemporaryDirectory() as folder:
            vendors = self.write(folder, 'vendors.json', json.dumps(machines_doc()))
            import io
            from contextlib import redirect_stderr, redirect_stdout
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                code = cv.main(['--log', str(Path(folder) / 'absent.log'), '--vendors', str(vendors),
                                '--open-index', '0'])
            self.assertEqual(code, 2)


def second_launch(money, ids, reset=1195, load=True, featured=FEATURED, eridium=0, loaded_eridium=None):
    """The second launch: its OWCURRENCY load line and its first report (the saved purse, shelf and clock)."""
    loaded = eridium if loaded_eridium is None else loaded_eridium
    text = line(f'OWCURRENCY load money={money} eridium={loaded} machines=1') if load else ''
    doc = state(shelf(ids), money=money, featured=featured)
    doc['resetSeconds'] = reset
    doc['eridium'] = eridium
    return text + state_line(doc)


def seeded(log_text, eridium):
    """A first launch whose reports carry this invented eridium amount."""
    return log_text.replace('"eridium":0', f'"eridium":{eridium}')


class PersistTest(unittest.TestCase):
    """grade_persist: the first launch's last report against the second launch's first one."""
    # good_log() ends with the health section: the purse after the sale, the ammo and the heal.
    SAVED_MONEY = START_MONEY - SHOP_PRICES[0] + SHOP_VALUES[0] - AMMO_PRICE - HEAL_PRICE
    FIRST_IDS = [f'N{i}' for i in range(1, 8)]

    def check(self, second):
        # good_log() restocks, then runs the items and health sections: the shelf is N1..N7, money SAVED_MONEY, reset 1200.
        results = cv.grade_persist(good_log(), second)
        return {row[0]: (row[1], row[2]) for row in results.rows}

    def test_saved_state_carried_over_all_pass(self):
        checks = self.check(second_launch(self.SAVED_MONEY, self.FIRST_IDS))
        for name in ('persist_load', 'persist_money', 'persist_shelf', 'persist_featured', 'persist_clock'):
            self.assertEqual(checks[name][0], 'PASS', name)

    def test_seeded_eridium_carried_over_passes(self):
        first = cv.grade_persist(seeded(good_log(), 7), second_launch(self.SAVED_MONEY, self.FIRST_IDS, eridium=7))
        self.assertEqual({r[0]: r[1] for r in first.rows}['persist_eridium'], 'PASS')

    def test_eridium_lost_on_load_fails(self):
        first = cv.grade_persist(seeded(good_log(), 7),
                                 second_launch(self.SAVED_MONEY, self.FIRST_IDS, eridium=0, loaded_eridium=0))
        self.assertEqual({r[0]: r[1] for r in first.rows}['persist_eridium'], 'FAIL')

    def test_unseeded_eridium_is_missing(self):
        checks = self.check(second_launch(self.SAVED_MONEY, self.FIRST_IDS))
        self.assertEqual(checks['persist_eridium'][0], 'MISSING')

    def test_missing_load_line_fails(self):
        checks = self.check(second_launch(self.SAVED_MONEY, self.FIRST_IDS, load=False))
        self.assertEqual(checks['persist_load'][0], 'FAIL')

    def test_money_not_carried_over_fails(self):
        checks = self.check(second_launch(self.SAVED_MONEY - 1, self.FIRST_IDS))
        self.assertEqual(checks['persist_load'][0], 'FAIL')
        self.assertEqual(checks['persist_money'][0], 'FAIL')

    def test_different_shelf_fails(self):
        checks = self.check(second_launch(self.SAVED_MONEY, [f'S{i}' for i in range(1, 8)]))
        self.assertEqual(checks['persist_shelf'][0], 'FAIL')

    def test_different_featured_fails(self):
        checks = self.check(second_launch(self.SAVED_MONEY, self.FIRST_IDS, featured=dict(FEATURED, id='F2')))
        self.assertEqual(checks['persist_featured'][0], 'FAIL')

    def test_clock_that_went_up_fails(self):
        checks = self.check(second_launch(self.SAVED_MONEY, self.FIRST_IDS, reset=1300))
        self.assertEqual(checks['persist_clock'][0], 'FAIL')

    def test_clock_that_fell_too_far_fails(self):
        checks = self.check(second_launch(self.SAVED_MONEY, self.FIRST_IDS, reset=1000))
        self.assertEqual(checks['persist_clock'][0], 'FAIL')

    def test_broken_second_launch_state_fails(self):
        checks = self.check('OWVENDSTATE {"serial":1,\n')
        self.assertEqual(checks['persist_state'][0], 'FAIL')


class ItemSectionTest(unittest.TestCase):
    """The PR 2b checks: the value formula, the ammo and heal buys, and the non-gun rows' prices (synthetic numbers)."""

    def test_item_value_is_the_trunc_of_the_product(self):
        # 50 x 1.12^5 = 88.1 -> 88 (the same numbers as the automation test); the part total scales it: 0.96 x 28 -> 47.
        self.assertEqual(cv.item_value({'base_cost': 50.0, 'scaler': cv.f32(1.12), 'part_total': 1.0}, 5), 88)
        self.assertEqual(cv.item_value({'base_cost': 28.0, 'scaler': cv.f32(1.12), 'part_total': 0.96}, 5), 47)
        self.assertEqual(cv.item_value({'base_cost': 50.0, 'scaler': cv.f32(1.12), 'part_total': 1.0}, 0), 50)

    def test_ammo_buy_passes_with_the_numbers_in_the_detail(self):
        checks = by_name(cv.grade_ammo_buy(good_log()))
        self.assertEqual(checks['ammo_buy'][0], 'PASS')
        self.assertIn('pistol pool 0 -> 54 (max 100); bought A1 for 82', checks['ammo_buy'][1])

    def test_ammo_pool_that_does_not_rise_by_the_amount_fails(self):
        log = good_log().replace('OWVENDAMMO type=pistol before=0 after=54 max=100',
                                 'OWVENDAMMO type=pistol before=0 after=40 max=100')
        self.assertEqual(by_name(run(log))['ammo_buy'][0], 'FAIL')

    def test_ammo_pool_capped_at_the_maximum_passes(self):
        # The pool holds 20 at most: 0 + 54 is capped to 20, and the checker expects the cap.
        log = good_log(sections=False) + items_section(4550, ammo_before=0, ammo_after=20, pool_max=20)
        self.assertEqual(by_name(cv.grade_ammo_buy(log))['ammo_buy'][0], 'PASS')

    def test_ammo_money_drop_wrong_fails(self):
        log = good_log().replace(f'OWVENDOR buy id=A1 ok=1 money={4550 - AMMO_PRICE} error=',
                                 f'OWVENDOR buy id=A1 ok=1 money={4550 - AMMO_PRICE - 1} error=')
        self.assertEqual(by_name(run(log))['ammo_buy'][0], 'FAIL')

    def test_full_pool_refusal_is_missing_with_reason(self):
        log = good_log().replace(f'OWVENDOR buy id=A1 ok=1 money={4550 - AMMO_PRICE} error=',
                                 'OWVENDOR buy id=A1 ok=0 money=4550 error=Ammo full')
        checks = by_name(run(log))
        self.assertEqual(checks['ammo_buy'][0], 'MISSING')
        self.assertIn('Ammo full', checks['ammo_buy'][1])

    def test_other_ammo_refusal_fails(self):
        log = good_log().replace(f'OWVENDOR buy id=A1 ok=1 money={4550 - AMMO_PRICE} error=',
                                 'OWVENDOR buy id=A1 ok=0 money=4550 error=Insufficient funds')
        self.assertEqual(by_name(run(log))['ammo_buy'][0], 'FAIL')

    def test_no_items_section_is_missing(self):
        log = good_log(sections=False)
        checks = by_name(run(log))
        self.assertEqual(checks['ammo_buy'][0], 'MISSING')
        self.assertEqual(checks['heal_buy'][0], 'MISSING')
        self.assertEqual(checks['item_prices'][0], 'MISSING')

    def test_heal_passes_and_reports_the_heal(self):
        checks = by_name(run(good_log()))
        self.assertEqual(checks['heal_buy'][0], 'PASS')
        self.assertIn('health 100.0 -> 200.0 of 400.0; bought H1 for 49', checks['heal_buy'][1])

    def test_heal_that_does_not_add_the_fraction_fails(self):
        log = good_log().replace('OWVENDHEAL before=100.000 after=200.000 max=400.000',
                                 'OWVENDHEAL before=100.000 after=150.000 max=400.000')
        self.assertEqual(by_name(run(log))['heal_buy'][0], 'FAIL')

    def test_full_health_refusal_is_missing_with_reason(self):
        log = good_log(sections=False) + health_section(4000, refused='Health full')
        checks = by_name(run(log))
        self.assertEqual(checks['heal_buy'][0], 'MISSING')
        self.assertIn('Health full', checks['heal_buy'][1])

    def test_item_value_off_by_one_fails(self):
        log = good_log().replace(f'"value":{AMMO_VALUE},', f'"value":{AMMO_VALUE + 1},')
        checks = by_name(run(log))
        self.assertEqual(checks['item_prices'][0], 'FAIL')
        self.assertIn('value', checks['item_prices'][1])

    def test_item_price_not_from_the_formula_fails(self):
        log = good_log().replace(f'"price":{AMMO_PRICE},', f'"price":{AMMO_PRICE + 1},', 1)
        self.assertEqual(by_name(run(log))['item_prices'][0], 'FAIL')

    def test_item_rows_without_terms_are_skipped_not_failed(self):
        log = good_log().replace('"valueTerms":', '"valueTermsGone":')
        checks = by_name(run(log))
        self.assertEqual(checks['item_prices'][0], 'MISSING')
        self.assertEqual(checks['prices'][0], 'PASS')   # the gun rows are still checked

    def test_automation_items_missing_fails(self):
        automation = GOOD_AUTOMATION.split('LogAutomationController: Display: Test Completed. Result={Passed}\tName={OpenWillow.Vendor.Items}')[0]
        checks = by_name(run(good_log(), automation=automation))
        self.assertEqual(checks['automation_items'][0], 'FAIL')
        self.assertIn('did not run', checks['automation_items'][1])


if __name__ == '__main__':
    unittest.main()
