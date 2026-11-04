"""Grade one -owvendors run of the in-engine vendors suite (tools/test_vendors.ps1).

Reads the host's log lines (OpenWillowVendor.cpp): the spawn count, the open / buy / sell / restock OWVENDOR lines, the
OWVENDSTATE {json} reports, and the automation result of OpenWillow.Vendor.Stock. Prints one line per check:
    CHECK <name> PASS|FAIL|MISSING <detail>
then SUMMARY passed=N failed=M missing=K. Exit 0 when nothing FAILs, 1 on any FAIL, 2 on bad arguments.

MISSING means the log or the state does not carry the value a check needs (for example a sell that was not run, or a
state row without "value"). It is reported, not counted as a failure.

--persist-log adds the persist_* checks: the second launch (same -owcurrencysave file, no -owmoney) must load the money,
shelf, featured item and reset clock that the first launch ended with (grade_persist).

Expected values:
- machines spawned = the supported machines in the vendors.json file (the host skips "supported": false ones);
- the shop price of an item = floor(value * (level * 0.14 + 4.1)), each step rounded to float32 as the host does
  (FOpenWillowVendorStock::BuyPrice). A machine's fixed_item_cost / fixed_featured_cost, when above 0, replaces the formula.
- buy: money after = money before - the first shelf price; sell: money after = money before + the sold item's value.
Rules are the host's (UNVERIFIED in game beyond what its own comments say); this script only compares the logged numbers.
"""
import argparse
import json
import math
import re
import struct
import sys
from pathlib import Path

SHOP_ITEM_COUNT = 7                 # StockWeaponCount on a weapon machine (docs/verification/VENDORS.md section 2)
SELL_EXPONENTIALIZER = 0.14         # FOpenWillowVendorStock::SellExponentializer
BASE_MARKUP_INCREASE = 4.1          # FOpenWillowVendorStock::BaseMarkupIncrease

SPAWNED = re.compile(r'OpenWillow vendors: (\d+) machine\(s\) spawned')
OPEN = re.compile(r'OWVENDOR open id=(\S*) kind=(\S*) restocked=(\d)')
ACTION = re.compile(r'OWVENDOR (buy|sell|buyback) id=(\S*) ok=(\d) money=(-?\d+) error=(.*)$', re.M)
RESTOCK = re.compile(r'OWVENDOR restock id=(\S*) ok=(\d)')
REFUSED = re.compile(r'ow\.Vendor\.(\w+) refused: (.*)$', re.M)
STATE = re.compile(r'OWVENDSTATE (\{.*)$', re.M)   # the whole rest of the line: a broken JSON is reported, not skipped
PAGE_ERROR = re.compile(r'OWVENDERR (.*)$', re.M)   # the page refused a host state
AUTOMATION_TESTS = ('OpenWillow.Vendor.Stock', 'OpenWillow.Vendor.Persist')
CURRENCY_LOAD = re.compile(r'OWCURRENCY load money=(-?\d+) eridium=(-?\d+) machines=(\d+)')
# The second run starts its clock from the saved seconds; a few seconds of world time pass before its first report.
RESET_TOLERANCE_SECONDS = 120


def f32(value):
    """Round a Python float to the nearest IEEE single, as a C++ float operation does."""
    return struct.unpack('<f', struct.pack('<f', value))[0]


def buy_price(value, level):
    """The host's BuyPrice in single precision: floor(value * (level * 0.14 + 4.1))."""
    markup = f32(f32(float(level) * f32(SELL_EXPONENTIALIZER)) + f32(BASE_MARKUP_INCREASE))
    return math.floor(f32(float(value) * markup))


class Results:
    def __init__(self):
        self.rows = []

    def add(self, name, status, detail):
        self.rows.append((name, status, detail))

    def print(self):
        passed = sum(1 for _, s, _ in self.rows if s == 'PASS')
        failed = sum(1 for _, s, _ in self.rows if s == 'FAIL')
        missing = sum(1 for _, s, _ in self.rows if s == 'MISSING')
        for name, status, detail in self.rows:
            print(f'CHECK {name} {status} {detail}')
        print(f'SUMMARY passed={passed} failed={failed} missing={missing}')
        return failed


def supported_machines(vendors):
    return [m for m in vendors.get('machines', []) if m.get('supported', True)]


def grade(log_text, automation_text, vendors, open_index, open_kind='weapons'):
    """Return a Results object with one row per check. vendors is the parsed vendors.json document."""
    out = Results()
    machines = supported_machines(vendors)
    expected_count = len(machines)

    spawn = SPAWNED.findall(log_text)
    if not spawn:
        out.add('machines_spawned', 'FAIL', 'no "machine(s) spawned" line: the subsystem did not run (-owvendors?)')
    elif int(spawn[-1]) == expected_count:
        out.add('machines_spawned', 'PASS', f'{spawn[-1]} of {expected_count} supported machines')
    else:
        out.add('machines_spawned', 'FAIL', f'spawned {spawn[-1]}, vendors.json has {expected_count} supported')

    if not 0 <= open_index < expected_count:
        out.add('open_machine', 'FAIL', f'open index {open_index} is outside 0..{expected_count - 1}')
        return out
    machine = machines[open_index]
    opened = OPEN.search(log_text)
    if not opened:
        refused = REFUSED.search(log_text)
        out.add('open_machine', 'FAIL', 'no "OWVENDOR open" line' + (f' ({refused.group(2)})' if refused else ''))
    elif opened.group(1) != machine.get('id') or opened.group(2) != open_kind or machine.get('kind') != open_kind:
        out.add('open_machine', 'FAIL', f'opened {opened.group(1)} kind={opened.group(2)}, expected '
                f'{machine.get("id")} kind={open_kind}')
    else:
        out.add('open_machine', 'PASS', f'{opened.group(1)} kind={opened.group(2)} restocked={opened.group(3)}')

    states, state_error = [], None
    for raw in STATE.findall(log_text):
        try:
            states.append(json.loads(raw.strip()))
        except json.JSONDecodeError as error:
            state_error = str(error)
            break
    if state_error:
        out.add('state_json', 'FAIL', f'an OWVENDSTATE line is not JSON: {state_error}')
    elif not states:
        out.add('state_json', 'FAIL', 'no OWVENDSTATE line (ow.Vendor.Report never ran, or no shop was open)')
    else:
        out.add('state_json', 'PASS', f'{len(states)} state report(s) parsed')
    # The page refuses a host state it cannot validate and logs why (vending.js); one such line means the page and host
    # disagree on the state's shape, even when the host's own report parses.
    page_errors = PAGE_ERROR.findall(log_text)
    if page_errors:
        out.add('page_state', 'FAIL', f'{len(page_errors)} OWVENDERR line(s), first: {page_errors[0].strip()[:160]}')
    else:
        out.add('page_state', 'PASS', 'the page accepted every state (no OWVENDERR)')
    first = states[0] if states else None

    if first is None:
        out.add('open_shelf', 'MISSING', 'no state report to read the shelf from')
    elif not first.get('open') or len(first.get('shop') or []) != SHOP_ITEM_COUNT or not first.get('featured'):
        out.add('open_shelf', 'FAIL', f'open={first.get("open")} shop={len(first.get("shop") or [])} '
                f'featured={"yes" if first.get("featured") else "none"}; expected open, {SHOP_ITEM_COUNT} shop items '
                'and one featured item')
    else:
        out.add('open_shelf', 'PASS', f'{SHOP_ITEM_COUNT} shop items and one featured item')

    # Prices: every shelf and featured row in every report.
    rows = []
    for index, state in enumerate(states):
        for row in state.get('shop') or []:
            rows.append((index, 'shop', row, int(machine.get('fixed_item_cost') or 0)))
        if state.get('featured'):
            rows.append((index, 'featured', state['featured'], int(machine.get('fixed_featured_cost') or 0)))
    if not rows:
        out.add('prices', 'MISSING', 'no shelf rows in any state report')
    else:
        checked, mismatches, unknown = 0, [], 0
        for index, where, row, fixed in rows:
            if row.get('price') is None or row.get('value') is None or row.get('level') is None:
                unknown += 1
                continue
            expected = fixed if fixed > 0 else buy_price(row['value'], row['level'])
            checked += 1
            if int(row['price']) != expected:
                mismatches.append(f'report {index} {where} {row.get("id")}: price {row["price"]} != {expected} '
                                  f'(value {row["value"]}, level {row["level"]})')
        if checked == 0:
            out.add('prices', 'MISSING', f'{unknown} rows without price/value/level')
        elif mismatches:
            out.add('prices', 'FAIL', f'{len(mismatches)} of {checked} prices differ; first: {mismatches[0]}')
        else:
            note = f'; {unknown} rows without value skipped' if unknown else ''
            out.add('prices', 'PASS', f'{checked} prices equal floor(value x (level x 0.14 + 4.1)){note}')

    buys = [m for m in ACTION.finditer(log_text) if m.group(1) == 'buy']
    buy = buys[0] if buys else None
    if buy is None:
        refused = [m for m in REFUSED.finditer(log_text) if m.group(1) == 'Buy']
        out.add('buy', 'FAIL', 'no "OWVENDOR buy" line' + (f' ({refused[0].group(2)})' if refused else ''))
    elif buy.group(3) != '1':
        out.add('buy', 'FAIL', f'buy ok=0: {buy.group(5).strip()}')
    elif first is None or not first.get('shop') or first.get('money') is None:
        out.add('buy', 'MISSING', 'no shelf or money in the state before the buy')
    else:
        before = int(first['money'])
        price = int(first['shop'][0]['price'])
        after = int(buy.group(4))
        if buy.group(2) != first['shop'][0].get('id'):
            out.add('buy', 'FAIL', f'bought {buy.group(2)}, the first shelf item was {first["shop"][0].get("id")}')
        elif before - after != price:
            out.add('buy', 'FAIL', f'money {before} -> {after} (drop {before - after}), shelf price {price}')
        else:
            out.add('buy', 'PASS', f'bought {buy.group(2)} for {price}; money {before} -> {after}')

    sells = [m for m in ACTION.finditer(log_text) if m.group(1) == 'sell']
    if not sells:
        out.add('sell', 'MISSING', 'sell not run (the -owvendortest sequence sells the bought gun with ow.Vendor.SellLast)')
    elif sells[0].group(3) != '1':
        out.add('sell', 'FAIL', f'sell ok=0: {sells[0].group(5).strip()}')
    elif buy is None or first is None or not first.get('shop') or first['shop'][0].get('value') is None:
        out.add('sell', 'MISSING', 'no bought item value to compare the sale with')
    else:
        before = int(buy.group(4))
        after = int(sells[0].group(4))
        value = int(first['shop'][0]['value'])
        if after - before != value:
            out.add('sell', 'FAIL', f'money {before} -> {after} (gain {after - before}), sale value {value}')
        else:
            out.add('sell', 'PASS', f'sold {sells[0].group(2)} for {value}; money {before} -> {after}')

    restocks = RESTOCK.findall(log_text)
    if not restocks:
        out.add('restock', 'FAIL', 'no "OWVENDOR restock" line')
    elif restocks[-1][1] != '1':
        out.add('restock', 'FAIL', 'restock ok=0')
    elif len(states) < 2:
        out.add('restock', 'MISSING', f'{len(states)} state reports; the restock needs one before and one after it')
    else:
        # The last report before the restock against the one after it. Shelf ids ("stock_<n>") repeat across restocks, so
        # compare the rolled guns' own ids ("gun", one per roll) and fall back to the shelf id only when it is absent.
        key = lambda row: row.get('gun') or row.get('id')
        before_ids = [key(row) for row in states[-2].get('shop') or []]
        after_ids = [key(row) for row in states[-1].get('shop') or []]
        if not after_ids:
            out.add('restock', 'MISSING', 'no shelf in the report after the restock')
        elif before_ids == after_ids:
            out.add('restock', 'FAIL', 'shelf ids unchanged by the restock')
        else:
            out.add('restock', 'PASS', f'shelf ids changed ({len(before_ids)} -> {len(after_ids)} items)')

    if automation_text is None:
        out.add('automation', 'MISSING', 'no automation log given')
    else:
        for name in AUTOMATION_TESTS:
            row = 'automation_' + name.rsplit('.', 1)[1].lower()     # automation_stock, automation_persist
            status = automation_status(automation_text, name)
            if status is None:
                out.add(row, 'FAIL', f'no result line for {name} (the test did not run)')
            else:
                # UE 5.8 prints Result={Success}; older builds printed Passed.
                out.add(row, 'PASS' if status in ('Success', 'Passed') else 'FAIL', f'{name} Result={status}')
    return out


def automation_status(text, name):
    """The Result={...} of one automation test from its log line, or None when the line is absent."""
    pattern = (rf'Result=\{{(\w+)\}}[^\n]*{re.escape(name)}(?![\w.])|'
               rf'{re.escape(name)}(?![\w.])[^\n]*Result=\{{(\w+)\}}')
    found = re.search(pattern, text)
    return (found.group(1) or found.group(2)) if found else None


def parse_states(text):
    """Every OWVENDSTATE report in text, and the JSON error text of the first one that is broken (else None)."""
    states = []
    for raw in STATE.findall(text):
        try:
            states.append(json.loads(raw.strip()))
        except json.JSONDecodeError as error:
            return states, str(error)
    return states, None


def grade_persist(first_text, second_text):
    """The currency save across two launches: run 2 must start with run 1's money, shelf, featured item and reset clock.

    The second launch must load the same -owcurrencysave file (its OWCURRENCY load line), and its first report is
    compared with the last report of the first launch. Only the shelf guns (ids rolled per restock) are compared, not
    the buy-back list, which is not saved (UNVERIFIED whether the game keeps it)."""
    out = Results()
    first, first_error = parse_states(first_text)
    second, second_error = parse_states(second_text)
    if first_error or second_error or not first or not second:
        out.add('persist_state', 'FAIL', 'the two launches need a readable OWVENDSTATE report each'
                + (f' (broken JSON: {first_error or second_error})' if first_error or second_error else ''))
        return out
    before, after = first[-1], second[0]

    load = CURRENCY_LOAD.search(second_text)
    if not load:
        out.add('persist_load', 'FAIL', 'no "OWCURRENCY load" line in the second launch (-owcurrencysave did not load)')
    elif before.get('money') is None or int(load.group(1)) != int(before['money']):
        out.add('persist_load', 'FAIL', f'loaded money {load.group(1)}, the first launch ended with {before.get("money")}')
    else:
        out.add('persist_load', 'PASS', f'loaded money={load.group(1)} machines={load.group(3)}')

    if before.get('money') is None or after.get('money') is None:
        out.add('persist_money', 'MISSING', 'no money in a state report')
    elif int(before['money']) != int(after['money']):
        out.add('persist_money', 'FAIL', f'money {before["money"]} at the end of the first launch, {after["money"]} at the start of the second')
    else:
        out.add('persist_money', 'PASS', f'money {after["money"]} carried over')

    key = lambda row: row.get('gun') or row.get('id')
    before_shelf = [key(row) for row in before.get('shop') or []]
    after_shelf = [key(row) for row in after.get('shop') or []]
    if not before_shelf:
        out.add('persist_shelf', 'MISSING', 'no shelf in the last report of the first launch')
    elif before_shelf == after_shelf:
        out.add('persist_shelf', 'PASS', f'{len(after_shelf)} shelf items carried over')
    else:
        out.add('persist_shelf', 'FAIL', f'shelf {before_shelf} became {after_shelf}')

    featured_before = key(before['featured']) if before.get('featured') else None
    featured_after = key(after['featured']) if after.get('featured') else None
    if featured_before is None or featured_before != featured_after:
        out.add('persist_featured', 'FAIL', f'featured {featured_before} became {featured_after}')
    else:
        out.add('persist_featured', 'PASS', f'featured {featured_after} carried over')

    if before.get('resetSeconds') is None or after.get('resetSeconds') is None:
        out.add('persist_clock', 'MISSING', 'no resetSeconds in a state report')
    else:
        gap = int(before['resetSeconds']) - int(after['resetSeconds'])
        if 0 <= gap <= RESET_TOLERANCE_SECONDS:
            out.add('persist_clock', 'PASS', f'reset clock {before["resetSeconds"]} s -> {after["resetSeconds"]} s')
        else:
            out.add('persist_clock', 'FAIL', f'reset clock {before["resetSeconds"]} s -> {after["resetSeconds"]} s '
                    f'(allowed to fall by at most {RESET_TOLERANCE_SECONDS} s)')
    return out


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('--log', required=True, type=Path, help='the in-game run log (-abslog)')
    parser.add_argument('--automation-log', type=Path, help='the -nullrhi automation run log')
    parser.add_argument('--vendors', required=True, type=Path, help='local/slice/vendors.json')
    parser.add_argument('--open-index', required=True, type=int, help='spawn index of the machine the suite opens')
    parser.add_argument('--open-kind', default='weapons')
    parser.add_argument('--persist-log', type=Path,
                        help='the second launch, with the same -owcurrencysave file (checks the save carried over)')
    args = parser.parse_args(argv)
    try:
        log_text = args.log.read_text(encoding='utf-8', errors='replace')
        vendors = json.loads(args.vendors.read_text(encoding='utf-8'))
        automation_text = (args.automation_log.read_text(encoding='utf-8', errors='replace')
                           if args.automation_log else None)
        persist_text = args.persist_log.read_text(encoding='utf-8', errors='replace') if args.persist_log else None
    except (OSError, json.JSONDecodeError) as error:
        print(f'check_vendors: cannot read input: {error}', file=sys.stderr)
        return 2
    results = grade(log_text, automation_text, vendors, args.open_index, args.open_kind)
    if persist_text is not None:
        results.rows.extend(grade_persist(log_text, persist_text).rows)
    return 1 if results.print() else 0


if __name__ == '__main__':
    sys.exit(main())
