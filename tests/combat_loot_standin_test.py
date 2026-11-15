"""Loot stand-in-inventory case, moved out of tests/combat_test.py (lane W1-PINSPLIT, 2026-11-15).

The C++ loot roll (src/combat_loot.cpp) makes stand-in inventories: an item is its inventory class and its bDropOnDeath
flag, with no parts and no stats. This check pins that stand-in (7 ItemPool.SpawnBalancedInventoryFromPool calls of
Pool_Coins at stage 4 make 21 of them), so it lives in its own file: the Rust binary runs this file only with
OW_PARITY=reference (architecture section 6.1), and tests/combat_test.py stays free of it. The pool roll through the item
registry, with items that carry definition data, replaces it later. The check is the one combat_test.py held, unchanged.

The toy packages are the ones tests/combat_test.py builds. Its fixture code is reused, not copied: the part of
combat_test.py above its test block (everything before the line `with tempfile...`) is run here, which defines the
package builders, `combat_run()`, `check()` and `failures` and runs no test. Only the setup this check needs is re-run:
the five toy packages and the `rollpool` run whose fourth step is checked. Nothing here comes from the game.
"""
from pathlib import Path

COMBAT_TEST = Path(__file__).resolve().with_name('combat_test.py')
MARKER = '\nwith tempfile.TemporaryDirectory() as folder:\n'
source = COMBAT_TEST.read_text(encoding='utf-8')
if source.count(MARKER) != 1:
    raise SystemExit(f'{COMBAT_TEST.name}: expected exactly one top-level test block starting {MARKER.strip()!r}')
exec(compile(source.split(MARKER)[0], str(COMBAT_TEST), 'exec'))

with tempfile.TemporaryDirectory() as folder:
    root = Path(folder)
    (root / 'Engine.upk').write_bytes(build_engine().build())
    (root / 'GearboxFramework.upk').write_bytes(build_gearbox().build())
    (root / 'WillowGame.upk').write_bytes(build_willowgame().build())
    (root / 'ToyPawns.upk').write_bytes(build_pawns().build())
    (root / 'Startup.upk').write_bytes(build_startup().build())

    # the same run as combat_test.py's rollpool case. Its exit and errors are checked here too: on the Rust binary this file runs
    # only with OW_PARITY=reference and combat_test.py only without it, so combat_test.py's check does not cover this run.
    code, got = combat_run(root, 'seed:3', 'spawn:ToyPawns/Pawn_Loot:4:6:Balance_Low', 'rollpool:0:Pool_Gated:4:5', 'rollpool:0:Pool_Gated:5:5', 'rollpool:0:Pool_Coins:4:7')
    assert code == 0 and got['errors'] == [], (code, got['errors'])
    result = got['steps'][4]['result']
    check('loot: 7 calls of Pool_Coins at stage 4 make 21 stand-in inventories, coins with bDropOnDeath set',
          got['rows'].get('loot.roll.Pool_Coins.stage4.items') == 21 and got['rows'].get('loot.roll.Pool_Coins.stage4.true') == 7
          and result['inventories'] == [{'class': 'WillowUsableItem', 'drop_on_death': True}] * 3, (got['rows'], result))

if failures:
    print(f'{len(failures)} loot stand-in check(s) failed:')
    for failure in failures: print('  - ' + failure)
    sys.exit(1)
print('combat loot stand-in coverage passed.')
