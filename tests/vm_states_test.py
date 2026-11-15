"""Object.GotoState stand-in cases, moved out of tests/vm_test.py (lane W1-PINSPLIT, 2026-11-15).

These checks pin the C++ VM's reference stand-in for states (natives_core.cpp): GotoState runs the BeginState / EndState
events and sets the state name, an undeclared state only sets the name, a Label is logged and not run (latent code is not
run), and a GotoState that never settles is abandoned at a nesting bound of 8 levels. The game's real state machine
(NATIVE_ENGINE_CORE.md) replaces them later, so they live in their own file: the Rust binary runs this file only with
OW_PARITY=reference (architecture section 6.1), and tests/vm_test.py stays free of stand-ins. The checks are the ones
vm_test.py held, unchanged.

The synthetic package is the one tests/vm_test.py builds (its `Mach` class carries the states). Its fixture code is
reused, not copied: the part of vm_test.py above its test block (everything before the line `with tempfile...`) is run
here, which defines `make()`, `run()` and `reader` and runs no test. Nothing here comes from the game.
"""
from pathlib import Path

VM_TEST = Path(__file__).resolve().with_name('vm_test.py')
MARKER = '\nwith tempfile.TemporaryDirectory() as folder:\n'
source = VM_TEST.read_text(encoding='utf-8')
if source.count(MARKER) != 1:
    raise SystemExit(f'{VM_TEST.name}: expected exactly one top-level test block starting {MARKER.strip()!r}')
exec(compile(source.split(MARKER)[0], str(VM_TEST), 'exec'))

with tempfile.TemporaryDirectory() as folder:
    root = Path(folder)
    (root / 'Core.upk').write_bytes(make().build())
    # Object.GotoState state events: EndState(new state) on the old state, the name is set, BeginState(old state) on the new one; the same
    # state runs nothing unless bForceEvents; a GotoState inside an event nests; one inside EndState replaces the outer change; an undeclared
    # state only sets the name; a Label is logged and not run (latent code is not run). One object per scenario.
    states = root / 'states.txt'
    states.write_text("""new m Core.Mach
run Core.Mach.CallWhere $m
run Core.Mach.GoA $m
run Core.Mach.CallWhere $m
run Core.Mach.ReadTrace $m
run Core.Mach.ReadBeginArg $m
run Core.Mach.GoA $m
run Core.Mach.ReadTrace $m
run Core.Mach.GoB $m
run Core.Mach.ReadTrace $m
run Core.Mach.ReadEndArg $m
run Core.Mach.ReadBeginArg $m
run Core.Mach.GoBForce $m
run Core.Mach.ReadTrace $m
run Core.Mach.GoNone $m
run Core.Mach.CallWhere $m
run Core.Mach.ReadTrace $m
run Core.Mach.ReadEndArg $m
run Core.Mach.GoMissing $m
run Core.Mach.ReadTrace $m
new n Core.Mach
run Core.Mach.GoC $n
run Core.Mach.CallWhere $n
run Core.Mach.ReadTrace $n
run Core.Mach.ReadBeginArg $n
new d Core.Mach
run Core.Mach.GoD $d
run Core.Mach.GoA $d
run Core.Mach.CallWhere $d
run Core.Mach.ReadTrace $d
run Core.Mach.ReadEndArg $d
run Core.Mach.ReadBeginArg $d
new q Core.Mach
run Core.Mach.GoLoop $q
run Core.Mach.ReadTrace $q
new l Core.Mach
run Core.Mach.GoABegin $l
run Core.Mach.ReadTrace $l
""", encoding='utf-8')
    done = subprocess.run([reader, str(root / 'Core.upk'), '--native-census', str(states), '--cooked', str(root), '--no-static'],
                          capture_output=True, text=True, encoding='utf-8')
    assert done.returncode == 0, (done.stdout, done.stderr)
    entries = json.loads(done.stdout)['entries']
    assert all(entry['status'] == 'completed' for entry in entries), entries
    results = [entry.get('result') for entry in entries]
    assert results[0] == '"-"', results                                   # the class-level Where: no state yet
    assert [results[i] for i in (2, 3, 4, 6)] == ['"A"', '"bA;"', "'None'", '"bA;"'], results   # BeginState ran (previous None), EndState did not; same state: no events
    assert [results[i] for i in (8, 9, 10)] == ['"bA;eA;bB;"', "'B'", "'A'"], results             # A -> B: EndState(B), then BeginState(A)
    assert results[12] == '"bA;eA;bB;eB;bB;"', results                     # bForceEvents: both events although the state stays B
    assert [results[i] for i in (14, 15, 16)] == ['"-"', '"bA;eA;bB;eB;bB;eB;"', "'None'"], results   # to None: EndState(None) only, the default Where answers again
    assert results[18] == results[15], results                             # an undeclared state sets the name and runs no event
    assert [results[i] for i in (20, 21, 22)] == ['"A"', '"bC;eC;bA;"', "'C'"], results          # BeginState(C) went to A: C ended once, A began
    assert [results[i] for i in (25, 26, 27, 28)] == ['"B"', '"bD;eD;eD;bB;"', "'B'", "'D'"], results   # the redirect in EndState wins over the outer change
    assert results[30] == '"bLoop;' + 'eLoop;bLoop;' * 7 + '"', results     # nesting bound: 8 levels run, the 9th is abandoned
    assert results[32] == '"bA;"', results
    assert any('state Nope is not declared on Mach' in line for line in run(root, 'Core.Mach.GoMissing', self_class='Core.Mach')['log'])
    assert any('GotoState: Loop abandoned' in line for line in run(root, 'Core.Mach.GoLoop', self_class='Core.Mach')['log'])
    assert any('label Begin not run' in line for line in run(root, 'Core.Mach.GoABegin', self_class='Core.Mach')['log'])
print('vm states (stand-in) coverage passed.')
