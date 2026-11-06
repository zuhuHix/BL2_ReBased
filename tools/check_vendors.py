"""Grade one -owvendors run of the in-engine vendors suite (tools/test_vendors.ps1).

Reads the host's log lines (OpenWillowVendor.cpp): the spawn count, the open / buy / sell / restock OWVENDOR lines, the
OWVENDSTATE {json} reports, and the automation result of OpenWillow.Vendor.Stock. Prints one line per check:
    CHECK <name> PASS|FAIL|MISSING <detail>
then SUMMARY passed=N failed=M missing=K. Exit 0 when nothing FAILs and no row in REQUIRED_ROWS is MISSING, else 1; 2 on bad
arguments.

MISSING means the log or the state does not carry the value a check needs (for example a sell that was not run, or a
state row without "value"). It is reported, not counted as a failure.

--persist-log adds the persist_* checks: the second launch (same -owcurrencysave file, no -owmoney) must load the money,
shelf, featured item and reset clock that the first launch ended with (grade_persist).

Expected values:
- machines spawned = the supported machines in the vendors.json file (the host skips "supported": false ones);
- the shop price of an item = floor(value * (level * 0.14 + 4.1)), each step rounded to float32 as the host does
  (FOpenWillowVendorStock::BuyPrice). A machine's fixed_item_cost / fixed_featured_cost, when above 0, replaces the formula.
  The state's machine id names which machine's fixed costs apply.
- a non-gun row's value = trunc(part_total * scaler^level * base_cost) in float32 steps (FOpenWillowVendorStock::ItemValue),
  recomputed from the row's "valueTerms" (PR 2b; docs/verification/VENDORS.md section 6).
- buy: money after = money before - the first shelf price; sell: money after = money before + the sold item's value.
- ammo_buy: the first buy from the Ammo Dump's shelf (row 0, an ammo row) raises its pool by min(max, before + amount) and
  charges its price. heal_buy: the first buy from the Dr. Zed health shelf heals the row's healFraction of max
  health; "Health full" or "Ammo full" is a valid refusal and reports MISSING with that reason.
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
AUTOMATION_TESTS = ('OpenWillow.Vendor.Stock', 'OpenWillow.Vendor.Persist', 'OpenWillow.Vendor.Items')
AMMO_LINE = re.compile(r'OWVENDAMMO type=(\w+) before=(-?\d+) after=(-?\d+) max=(-?\d+)')
HEAL_LINE = re.compile(r'OWVENDHEAL before=(-?[\d.]+) after=(-?[\d.]+) max=(-?[\d.]+)')
CURRENCY_REFUSED = re.compile(r'OWCURRENCY load refused: (.*)$', re.M)
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


def item_value(terms, level):
    """The host's ItemValue for a non-gun row: trunc(part_total * scaler^level * base_cost), each step in float32.

    The scaler to the level is taken in double and then rounded, as the host does; the terms are the row's valueTerms."""
    scaler_to_level = f32(float(terms['scaler']) ** int(level))
    partial = f32(float(terms['part_total']) * scaler_to_level)
    return math.trunc(f32(partial * f32(float(terms['base_cost']))))


# The sequence always produces these, so their absence is a failure. Other rows may legitimately be MISSING.
REQUIRED_ROWS = ('stale_press',)


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
        return failed + sum(1 for name, s, _ in self.rows if s == 'MISSING' and name in REQUIRED_ROWS)


def supported_machines(vendors):
    return [m for m in vendors.get('machines', []) if m.get('supported', True)]


STALE_PRESS = re.compile(r'OWVENDTEST stale_press start=(-?\d+) after_refusals=(-?\d+) after_stale=(-?\d+) refusal=\[([^\]]*)\] kept=\[([^\]]*)\] alone=\[([^\]]*)\] opening=(\d+)')


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

    # Prices: every shelf and featured row in every report. Each report's own machine gives its fixed costs.
    by_id = {m.get('id'): m for m in machines}
    rows = []
    for index, state in enumerate(states):
        state_machine = by_id.get((state.get('machine') or {}).get('id'), machine)
        for row in state.get('shop') or []:
            rows.append((index, 'shop', row, state_machine))
        if state.get('featured'):
            rows.append((index, 'featured', state['featured'], state_machine))
    weapon_rows = [(i, w, r, m) for i, w, r, m in rows if is_gun_row(r)]
    if not weapon_rows:
        out.add('prices', 'MISSING', 'no gun rows in any state report')
    else:
        checked, mismatches, unknown = 0, [], 0
        for index, where, row, state_machine in weapon_rows:
            if row.get('price') is None or row.get('value') is None or row.get('level') is None:
                unknown += 1
                continue
            fixed = int((state_machine.get('fixed_featured_cost' if where == 'featured' else 'fixed_item_cost')) or 0)
            expected = fixed if fixed > 0 else buy_price(row['value'], row['level'])
            checked += 1
            if int(row['price']) != expected:
                mismatches.append(f'report {index} {where} {row.get("id")}: price {row["price"]} != {expected} '
                                  f'(value {row["value"]}, level {row["level"]})')
        if checked == 0:
            out.add('prices', 'MISSING', f'{unknown} gun rows without price/value/level')
        elif mismatches:
            out.add('prices', 'FAIL', f'{len(mismatches)} of {checked} prices differ; first: {mismatches[0]}')
        else:
            note = f'; {unknown} gun rows without value skipped' if unknown else ''
            out.add('prices', 'PASS', f'{checked} gun prices equal floor(value x (level x 0.14 + 4.1)){note}')
    out.rows.extend(grade_item_rows(rows).rows)

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

    restock = RESTOCK.search(log_text)
    if restock is None:
        out.add('restock', 'FAIL', 'no "OWVENDOR restock" line')
    elif restock.group(2) != '1':
        out.add('restock', 'FAIL', 'restock ok=0')
    else:
        # The last weapon report before the restock line against the first one after it. Shelf ids ("stock_<n>") repeat
        # across restocks, so compare the rolled guns' own ids ("gun", one per roll) and fall back to the shelf id.
        timed = timed_states(log_text)
        before = [s for pos, s in timed if pos < restock.start() and is_gun_state(s)]
        after = [s for pos, s in timed if pos > restock.start() and is_gun_state(s)]
        if not before or not after:
            out.add('restock', 'MISSING', f'{len(before)} report(s) before and {len(after)} after the restock; need one each')
        else:
            key = lambda row: row.get('gun') or row.get('id')
            before_ids = [key(row) for row in before[-1].get('shop') or []]
            after_ids = [key(row) for row in after[0].get('shop') or []]
            if not after_ids:
                out.add('restock', 'MISSING', 'no shelf in the report after the restock')
            elif before_ids == after_ids:
                out.add('restock', 'FAIL', 'shelf ids unchanged by the restock')
            else:
                out.add('restock', 'PASS', f'shelf ids changed ({len(before_ids)} -> {len(after_ids)} items)')

    # The non-gun sections: the Ammo Dump's ammo buy and the health machine's heal (PR 2b).
    out.rows.extend(grade_ammo_buy(log_text).rows)
    out.rows.extend(grade_heal_buy(log_text).rows)
    grade_stale_press(log_text, out)

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


def is_gun_row(row):
    """A shelf or buy-back row is a gun unless its kind says otherwise (rows from before PR 2b have no kind)."""
    return row.get('kind') in (None, 'weapon')


def is_gun_state(state):
    """A report of a weapons machine (the reports carry the machine's kind)."""
    return (state.get('machine') or {}).get('kind') in (None, 'weapons')


def timed_states(text):
    """Every OWVENDSTATE report in text with its position: (position, dict) pairs. A broken report ends the list."""
    found = []
    for match in STATE.finditer(text):
        try:
            found.append((match.start(), json.loads(match.group(1).strip())))
        except json.JSONDecodeError:
            break
    return found


def grade_item_rows(rows):
    """item_prices: every non-gun row carries its value terms; the value and the price must follow from them."""
    out = Results()
    checked, mismatches, skipped = 0, [], 0
    for index, where, row, state_machine in rows:
        if is_gun_row(row):
            continue
        terms = row.get('valueTerms')
        if not terms or row.get('value') is None or row.get('level') is None or row.get('price') is None:
            skipped += 1
            continue
        value = item_value(terms, row['level'])
        fixed = int(state_machine.get('fixed_featured_cost' if where == 'featured' else 'fixed_item_cost') or 0)
        expected_price = fixed if fixed > 0 else buy_price(value, row['level'])
        checked += 1
        if int(row['value']) != value:
            mismatches.append(f'report {index} {where} {row.get("id")}: value {row["value"]} != {value} (terms {terms})')
        elif int(row['price']) != expected_price:
            mismatches.append(f'report {index} {where} {row.get("id")}: price {row["price"]} != {expected_price} '
                              f'(value {value}, level {row["level"]})')
    if checked == 0:
        out.add('item_prices', 'FAIL', f'{skipped} non-gun rows without valueTerms (no items or health section)')
    elif mismatches:
        out.add('item_prices', 'FAIL', f'{len(mismatches)} of {checked} non-gun rows differ; first: {mismatches[0]}')
    else:
        note = f'; {skipped} rows without terms skipped' if skipped else ''
        out.add('item_prices', 'PASS', f'{checked} non-gun rows: value and price follow from valueTerms{note}')
    return out


def shop_buy(log_text, kind):
    """The first buy from a shop of this kind opened in the log: ((buy, report before it, report after it), None), or
    (None, reason) when there is no such shop or buy."""
    opened = next((m for m in OPEN.finditer(log_text) if m.group(2) == kind), None)
    if opened is None:
        return None, f'no "OWVENDOR open" line for a {kind} machine'
    buy = next((m for m in ACTION.finditer(log_text, opened.end()) if m.group(1) == 'buy'), None)
    if buy is None:
        return None, f'no buy from the {kind} machine'
    timed = timed_states(log_text)
    before = [state for pos, state in timed if pos < buy.start()]
    after = [state for pos, state in timed if pos > buy.start()]
    if not before:
        return None, f'no report before the {kind} buy'
    return (buy, before[-1], after[0] if after else None, opened), None


def effect_line(pattern, log_text, opened, buy):
    """The last line matching pattern between the shop's open line and its buy. The host logs the effect (OWVENDAMMO,
    OWVENDHEAL) inside the purchase, before the OWVENDOR buy line that reports the action."""
    found = list(pattern.finditer(log_text, opened.end(), buy.start()))
    return found[-1] if found else None


def grade_stale_press(log_text, out):
    """stale_press: the -owvendortest sequence's refused press, the same press again, and a press with an old serial, through the
    page's message path. A refusal leaves the serial, so the repeat gets the refusal; an old serial leaves it too and keeps the
    refusal, or says to try again; and the host counted the three openings the sequence makes."""
    found = STALE_PRESS.search(log_text)
    if found is None:
        out.add('stale_press', 'MISSING', 'no "OWVENDTEST stale_press" line')
        return
    start, after_refusals, after_stale, refusal, kept, alone, opening = found.groups()
    tail = log_text[:found.start()]
    refused = len(re.findall(r'OWVENDOR buy id=no_such_row ok=0', tail))
    stale = len(re.findall(r'OWVENDOR ignored stale buy', tail))
    if refused != 2:
        out.add('stale_press', 'FAIL', f'{refused} refused press(es) reached the shop, expected the press and its repeat')
    elif stale != 2:
        out.add('stale_press', 'FAIL', f'{stale} press(es) ignored as stale, expected 2')
    elif len({start, after_refusals, after_stale}) != 1:
        out.add('stale_press', 'FAIL', f'the serial moved: {start} -> {after_refusals} -> {after_stale}')
    elif not refusal or kept != refusal:
        out.add('stale_press', 'FAIL', f'the refusal [{refusal}] did not stay through a stale press: [{kept}]')
    elif alone != 'Try again':
        out.add('stale_press', 'FAIL', f'a stale press with no refusal showing said [{alone}]')
    elif opening != '3':
        out.add('stale_press', 'FAIL', f'{opening} openings counted, the sequence opens 3 machines')
    else:
        out.add('stale_press', 'PASS', f'serial stayed at {start}; the repeat was refused again, a stale press kept [{refusal}]; 3 openings counted')


def grade_ammo_buy(log_text):
    """ammo_buy: the first buy of the Ammo Dump (shelf row 0, an ammo row) raises its pool by min(max, before + amount)
    and charges the row's price. A full pool is a valid refusal (MISSING with its reason)."""
    out = Results()
    shop, reason = shop_buy(log_text, 'items')
    if shop is None:
        out.add('ammo_buy', 'FAIL', reason)   # expected: the -owvendortest sequence always buys here
        return out
    buy, before, after, opened = shop
    if buy.group(3) != '1':
        # The sequence half-empties the pool first, so even "Ammo full" is a failure here.
        out.add('ammo_buy', 'FAIL', f'ammo buy ok=0: {buy.group(5).strip()}')
        return out
    row = (before.get('shop') or [None])[0]
    if not row or row.get('kind') != 'ammo' or row.get('ammoType') is None or row.get('amount') is None or row.get('price') is None:
        out.add('ammo_buy', 'FAIL', 'shelf row 0 of the items machine is not an ammo row with a type, amount and price')
        return out
    if buy.group(2) != row.get('id'):
        out.add('ammo_buy', 'FAIL', f'bought {buy.group(2)}, the first shelf row was {row.get("id")}')
        return out
    found = effect_line(AMMO_LINE, log_text, opened, buy)
    if not found:
        out.add('ammo_buy', 'FAIL', 'no "OWVENDAMMO" line between the shop open and the buy')
        return out
    ammo_type, pool_before, pool_after, pool_max = found.group(1), int(found.group(2)), int(found.group(3)), int(found.group(4))
    expected = min(pool_max, pool_before + int(row['amount']))
    pool_report = (before.get('ammo') or {}).get(ammo_type)
    price, money_before, money_after = int(row['price']), int(before['money']), int(buy.group(4))
    if ammo_type != row['ammoType']:
        out.add('ammo_buy', 'FAIL', f'pool {ammo_type}, the row sells {row["ammoType"]}')
    elif pool_report is not None and int(pool_report['current']) != pool_before:
        out.add('ammo_buy', 'FAIL', f'pool was {pool_before} before the buy, the report said {pool_report["current"]}')
    elif pool_after != expected:
        out.add('ammo_buy', 'FAIL', f'pool {pool_before} + {row["amount"]} -> {pool_after}, expected {expected} (max {pool_max})')
    elif money_before - money_after != price:
        out.add('ammo_buy', 'FAIL', f'money {money_before} -> {money_after} (drop {money_before - money_after}), price {price}')
    elif after is None:
        out.add('ammo_buy', 'FAIL', 'no report after the ammo buy')
    elif int(((after.get('ammo') or {}).get(ammo_type) or {}).get('current', -1)) != pool_after or int(after.get('money', -1)) != money_after:
        out.add('ammo_buy', 'FAIL', f'the report after the buy shows pool {((after.get("ammo") or {}).get(ammo_type) or {}).get("current")} '
                f'and money {after.get("money")}, the buy logged {pool_after} and {money_after}')
    else:
        out.add('ammo_buy', 'PASS', f'{ammo_type} pool {pool_before} -> {pool_after} (max {pool_max}); bought {row["id"]} for {price}')
    return out


def grade_heal_buy(log_text):
    """heal_buy: the first buy of the health machine (shelf row 0) heals healFraction of max health, up to the max, and
    charges the price. "Health full" is a valid refusal (MISSING with its reason)."""
    out = Results()
    shop, reason = shop_buy(log_text, 'health')
    if shop is None:
        out.add('heal_buy', 'FAIL', reason)   # expected: the -owvendortest sequence always buys here
        return out
    buy, before, after, opened = shop
    if buy.group(3) != '1':
        # The sequence drops Maya to half health first, so even "Health full" is a failure here.
        out.add('heal_buy', 'FAIL', f'heal buy ok=0: {buy.group(5).strip()}')
        return out
    row = (before.get('shop') or [None])[0]
    if not row or row.get('kind') != 'health' or row.get('healFraction') is None or row.get('price') is None:
        out.add('heal_buy', 'FAIL', 'shelf row 0 of the health machine is not a health row with a fraction and a price')
        return out
    if buy.group(2) != row.get('id'):
        out.add('heal_buy', 'FAIL', f'bought {buy.group(2)}, the first shelf row was {row.get("id")}')
        return out
    found = effect_line(HEAL_LINE, log_text, opened, buy)
    if not found:
        out.add('heal_buy', 'FAIL', 'no "OWVENDHEAL" line between the shop open and the buy')
        return out
    health_before, health_after, health_max = (float(found.group(i)) for i in (1, 2, 3))
    expected = min(health_max, health_before + float(row['healFraction']) * health_max)
    price, money_before, money_after = int(row['price']), int(before['money']), int(buy.group(4))
    health_report = before.get('health') or {}
    if abs(float(health_report.get('current', -1)) - health_before) > 0.01:
        out.add('heal_buy', 'FAIL', f'health was {health_before:.2f} before the buy, the report said {health_report.get("current")}')
    elif abs(health_after - expected) > 0.01:
        out.add('heal_buy', 'FAIL', f'health {health_before:.2f} -> {health_after:.2f}, expected {expected:.2f} (max {health_max:.2f})')
    elif money_before - money_after != price:
        out.add('heal_buy', 'FAIL', f'money {money_before} -> {money_after} (drop {money_before - money_after}), price {price}')
    elif after is None:
        out.add('heal_buy', 'FAIL', 'no report after the heal buy')
    elif abs(float((after.get('health') or {}).get('current', -1)) - health_after) > 0.01 or int(after.get('money', -1)) != money_after:
        out.add('heal_buy', 'FAIL', f'the report after the buy shows health {(after.get("health") or {}).get("current")} '
                f'and money {after.get("money")}, the buy logged {health_after:.2f} and {money_after}')
    else:
        out.add('heal_buy', 'PASS', f'health {health_before:.1f} -> {health_after:.1f} of {health_max:.1f}; bought {row["id"]} for {price}')
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
    # Money and eridium: the last report of the first launch (any machine; the run ends on the health machine). The shelf,
    # featured item and clock: its last weapons report (the second launch opens the weapons machine first).
    before, after = first[-1], second[0]
    gun_before = next((state for state in reversed(first) if is_gun_state(state)), before)

    load = CURRENCY_LOAD.search(second_text)
    refused = CURRENCY_REFUSED.search(first_text) or CURRENCY_REFUSED.search(second_text)
    if refused:
        # A refused load turns saving off for that session, so name the cause rather than only its effects.
        out.add('persist_load', 'FAIL', f'a load was refused: {refused.group(1).strip()[:200]}')
    elif not load:
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

    if before.get('eridium') is None or after.get('eridium') is None or not load:
        out.add('persist_eridium', 'MISSING', 'no eridium in a state report or no load line')
    elif not (int(before['eridium']) == int(after['eridium']) == int(load.group(2))):
        out.add('persist_eridium', 'FAIL', f'eridium {before["eridium"]} at the end of the first launch, loaded '
                f'{load.group(2)}, {after["eridium"]} at the start of the second')
    elif int(after['eridium']) == 0:
        out.add('persist_eridium', 'MISSING', 'eridium is 0 in both launches (seed it with -owerid in the first)')
    else:
        out.add('persist_eridium', 'PASS', f'eridium {after["eridium"]} carried over')

    key = lambda row: row.get('gun') or row.get('id')
    before_shelf = [key(row) for row in gun_before.get('shop') or []]
    after_shelf = [key(row) for row in after.get('shop') or []]
    if not before_shelf:
        out.add('persist_shelf', 'MISSING', 'no shelf in the last report of the first launch')
    elif before_shelf == after_shelf:
        out.add('persist_shelf', 'PASS', f'{len(after_shelf)} shelf items carried over')
    else:
        out.add('persist_shelf', 'FAIL', f'shelf {before_shelf} became {after_shelf}')

    featured_before = key(gun_before['featured']) if gun_before.get('featured') else None
    featured_after = key(after['featured']) if after.get('featured') else None
    if featured_before is None or featured_before != featured_after:
        out.add('persist_featured', 'FAIL', f'featured {featured_before} became {featured_after}')
    else:
        out.add('persist_featured', 'PASS', f'featured {featured_after} carried over')

    if gun_before.get('resetSeconds') is None or after.get('resetSeconds') is None:
        out.add('persist_clock', 'MISSING', 'no resetSeconds in a state report')
    else:
        gap = int(gun_before['resetSeconds']) - int(after['resetSeconds'])
        if 0 <= gap <= RESET_TOLERANCE_SECONDS:
            out.add('persist_clock', 'PASS', f'reset clock {gun_before["resetSeconds"]} s -> {after["resetSeconds"]} s')
        else:
            out.add('persist_clock', 'FAIL', f'reset clock {gun_before["resetSeconds"]} s -> {after["resetSeconds"]} s '
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
