"""Synthetic tests for block_saves() and the reply publishing of run_pending() in tools/real_game/openwillow_realgame/__init__.py
(no game: the SDK calls are stubs).

Run: python tests/block_saves_test.py
"""
import ast
import contextlib
import io
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
import traceback
import types
import realgame_test_env  # noqa: F401  (the PowerShell child of the format test gets the sanitised environment)

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'tools' / 'real_game' / 'openwillow_realgame' / '__init__.py'
# Written out here, not read from the driver, so a wrong name there that happens to exist in the game still fails.
EXPECTED = {'WillowGame.WillowSaveGameManager:' + name for name in ('SaveGame', 'Save', 'SaveRawData', 'SaveGraveyard')}
# Still ignoring the reply on this branch. These seven capture scripts (all unbarred on 2026-11-09; docs/TOOLING.md) get it from #157 and vendors from #160; an entry can go once that has
# landed, and the test stays green either way.
PENDING = {'action_skill_hud_capture', 'hud_integrity_capture', 'hud_layout_capture', 'hud_parity_capture', 'weapon_part_session',
           'projectiles_capture', 'weapon_parity2_capture', 'vendors_capture'}


def load(existing):
    """The driver's save-block code alone, as importing the module needs a running game. existing: the functions the stub game has."""
    tree = ast.parse(SOURCE.read_text(encoding='utf-8'))
    wanted = [n for n in tree.body if (isinstance(n, ast.FunctionDef) and n.name in ('_add_save_hooks', 'block_saves', '_block_save'))
              or (isinstance(n, ast.Assign) and getattr(n.targets[0], 'id', '') == 'SAVE_FUNCTIONS')]
    hooks = []

    def find_object(cls, path):
        assert cls == 'Function'
        if path not in existing:
            raise ValueError(f"Couldn't find object '{path}'")
        return path
    namespace = {'unrealsdk': types.SimpleNamespace(find_object=find_object), 'Type': types.SimpleNamespace(PRE=1), 'Block': object(),
                 'add_hook': lambda path, kind, name, fn: hooks.append((path, name))}
    exec(compile(ast.Module(wanted, []), str(SOURCE), 'exec'), namespace)
    return namespace, hooks


def test_all_found_hooks_exactly_the_expected_functions_and_answers_their_number():
    namespace, hooks = load(EXPECTED)
    assert namespace['block_saves']() == len(EXPECTED)
    assert {path for path, _ in hooks} == EXPECTED and len(hooks) == len(EXPECTED)


def test_a_missing_function_stops_before_any_hook_and_names_it():
    for missing in EXPECTED:
        namespace, hooks = load(EXPECTED - {missing})
        try:
            namespace['block_saves']()
        except RuntimeError as error:
            assert missing in str(error)
        else:
            raise AssertionError('a missing save function was accepted')
        assert hooks == []


def test_a_start_up_hook_needs_no_proof():
    # The flag path runs while the mod loads, perhaps before the class is; the capture's block_saves() call later is the proof.
    namespace, hooks = load([])
    namespace['_add_save_hooks']()
    assert {path for path, _ in hooks} == EXPECTED


def test_the_flag_path_hooks_without_the_proof_and_the_capture_gets_the_proof():
    text = SOURCE.read_text(encoding='utf-8')
    assert 'autoblock.flag").is_file():\n    _add_save_hooks()' in text.replace('\r\n', '\n')


def ignores_the_answer(line):
    code = line.split('#', 1)[0]
    return 'block_saves()' in code and 'throw' not in code and 'Assert-SavesBlocked (' not in code


def calls_without_the_check():
    """Every script line that calls block_saves() and does not stop on its answer, as (script, line number)."""
    found = []
    for script in (ROOT / 'tools').rglob('*.ps1'):
        for number, line in enumerate(script.read_text(encoding='utf-8').splitlines(), 1):
            if ignores_the_answer(line):
                found.append((script.stem, number))
    return found


def test_every_capture_stops_on_a_failed_block_saves():
    unchecked = [row for row in calls_without_the_check() if row[0] not in PENDING]
    assert not unchecked, unchecked


def test_the_scan_tells_a_checked_call_from_an_unchecked_one():
    # On synthetic lines, so it still proves the scan works once no real script is left to find.
    for line in ("$log.block_saves = Py 'print(block_saves())'", "Invoke-GamePy 'print(block_saves())' | Out-Null"):
        assert ignores_the_answer(line), line
    for line in ("$log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')", "# block_saves() answers the count", "throw 'block_saves() failed'"):
        assert not ignores_the_answer(line), line


def test_the_scan_reaches_the_captures():
    texts = [script.read_text(encoding='utf-8') for script in (ROOT / 'tools').rglob('*.ps1')]
    assert sum('Assert-SavesBlocked (' in text for text in texts) >= 3


def load_channel(path_class, now=None, wall=None):
    """The driver's command loop alone (cmd_dir, refusal, run_pending and the prefixes), over a stand-in command folder. path_class
    stands in for Path, so a test can watch or break the reply's write and rename. now, when given, is the driver's command clock (the
    performance counter, microseconds); wall, when given, is what time.time() answers there (a wall clock a test can step)."""
    tree = ast.parse(SOURCE.read_text(encoding='utf-8'))
    wanted = [n for n in tree.body if (isinstance(n, ast.FunctionDef) and n.name in ('cmd_dir', 'refusal', 'run_pending'))
              or (isinstance(n, ast.Assign) and getattr(n.targets[0], 'id', '') in ('DEADLINE_PREFIX', 'WALL_CLOCK_PREFIX', 'REFUSED_PREFIX'))]
    tmp = Path(tempfile.mkdtemp(prefix='ow-driver-'))
    folder = tmp / 'cmd'
    folder.mkdir()
    (tmp / 'cmd_dir.txt').write_text(str(folder), encoding='utf-8')
    wall_clock = types.SimpleNamespace(time=(lambda: wall) if wall is not None else time.time, perf_counter=time.perf_counter)
    namespace = {'Path': path_class, '__file__': str(tmp / '__init__.py'), 'io': io, 'contextlib': contextlib, 'traceback': traceback,
                 'time': wall_clock, 'command_clock_us': (lambda: now) if now is not None else qpc_us, 'scope': {'runs': []}}
    exec(compile(ast.Module(wanted, []), str(SOURCE), 'exec'), namespace)
    return namespace, folder


def qpc_us():
    """The command clock as this test reads it: the performance counter through the Windows API, in microseconds."""
    import ctypes
    frequency, count = ctypes.c_int64(), ctypes.c_int64()
    ctypes.windll.kernel32.QueryPerformanceFrequency(ctypes.byref(frequency))
    ctypes.windll.kernel32.QueryPerformanceCounter(ctypes.byref(count))
    return count.value * 1_000_000 // frequency.value


def command(code, deadline_us=None):
    """A command as the sender publishes it: its deadline line first (an hour ahead on the command clock unless given), then the code."""
    if deadline_us is None:
        deadline_us = qpc_us() + 3600 * 1_000_000
    return f'# ow-deadline-qpc-us: {deadline_us}\n{code}'


def test_a_reply_appears_only_complete_and_the_script_is_done_before_it_runs():
    # Review of #263: the reply was written in place, so a reader could see it empty or cut off. It is written under <name>.out.tmp and
    # renamed onto <name>.out; the script is renamed to .py.done before it runs, so it never runs twice.
    seen = []

    class Watched(type(Path())):
        def write_text(self, *args, **kwargs):
            seen.append((self.name, self.with_name(self.name.replace('.out.tmp', '.out')).exists()))
            return super().write_text(*args, **kwargs)
    ns, folder = load_channel(Watched)
    script = folder / 'c1-invented.py'
    script.write_text(command("import os\nruns.append(os.path.exists(r'%s'))\nprint('invented reply ' * 50)" % script), encoding='utf-8')
    ns['run_pending']()
    assert ns['scope']['runs'] == [False]                       # already .py.done when it ran
    assert seen == [('c1-invented.out.tmp', False)], seen       # written under the other name, with no .out yet
    assert (folder / 'c1-invented.out').read_text(encoding='utf-8') == 'invented reply ' * 50 + '\n'
    assert sorted(p.name for p in folder.iterdir()) == ['c1-invented.out', 'c1-invented.py.done']


def test_a_failed_rename_leaves_no_reply_and_the_script_does_not_run_again():
    class Unrenamable(type(Path())):
        def replace(self, target):
            if self.name.endswith('.out.tmp'):   # the reply's rename only; the script's own .py.done rename goes through
                raise PermissionError('invented: the rename was refused')
            return super().replace(target)
    ns, folder = load_channel(Unrenamable)
    (folder / 'c2-invented.py').write_text(command("runs.append(1)\nprint('ok')"), encoding='utf-8')
    (folder / 'c3-invented.py').write_text(command("runs.append(2)\nprint('ok')"), encoding='utf-8')
    ns['run_pending']()
    ns['run_pending']()
    assert ns['scope']['runs'] == [1, 2]                        # each ran once; the failure did not stop the next one
    assert sorted(p.name for p in folder.iterdir()) == ['c2-invented.py.done', 'c3-invented.py.done']   # no reply, no .tmp left


# ---------------------------------------------------------------- the deadline the receiver enforces (session gate's review, B1, and round 2)
def test_a_command_taken_at_or_after_its_deadline_is_refused_and_never_run():
    """B1: the driver ran whatever was queued, so a command its caller gave up on could run later (a slow acceptance, a hitch in the
    game). It refuses a command taken at or after its deadline (round 2: equality too; it compared floored milliseconds with '>'): not run,
    renamed .py.expired, and a reply that starts 'OW-REFUSED: ' (the sender's no-reply form). One taken in time still runs. The clock is
    the driver's command clock, here a fixed invented reading."""
    now = 1_900_000_000_000
    ns, folder = load_channel(type(Path()), now=now)
    (folder / 'c1-late.py').write_text(command("runs.append('late')\nprint('ran')", now - 1), encoding='utf-8')
    (folder / 'c2-equal.py').write_text(command("runs.append('equal')\nprint('ran')", now), encoding='utf-8')
    (folder / 'c3-in-time.py').write_text(command("runs.append('in time')\nprint('ran')", now + 1), encoding='utf-8')
    ns['run_pending']()
    ns['run_pending']()
    assert ns['scope']['runs'] == ['in time'], ns['scope']['runs']   # the late and the equal one never ran, not even on the next pass
    assert sorted(p.name for p in folder.iterdir()) == ['c1-late.out', 'c1-late.py.expired', 'c2-equal.out', 'c2-equal.py.expired',
                                                         'c3-in-time.out', 'c3-in-time.py.done']
    assert (folder / 'c1-late.out').read_text(encoding='utf-8') == 'OW-REFUSED: not run: its deadline was reached 1 us before the driver took it\n'
    assert (folder / 'c2-equal.out').read_text(encoding='utf-8') == 'OW-REFUSED: not run: its deadline was reached 0 us before the driver took it\n'
    assert (folder / 'c3-in-time.out').read_text(encoding='utf-8') == 'ran\n'


def test_a_command_with_no_deadline_an_unreadable_one_or_one_on_the_wall_clock_is_refused():
    ns, folder = load_channel(type(Path()))
    (folder / 'c1-bare.py').write_text("runs.append('bare')", encoding='utf-8')
    (folder / 'c2-bad.py').write_text("# ow-deadline-qpc-us: soon\nruns.append('bad')", encoding='utf-8')
    # The form before round 2, with a deadline far ahead on the wall clock: still refused, as a wall clock can be stepped back.
    (folder / 'c3-wall.py').write_text(f"# ow-deadline-unix-ms: {int(time.time() * 1000) + 3600_000}\nruns.append('wall')", encoding='utf-8')
    ns['run_pending']()
    assert ns['scope']['runs'] == []
    assert (folder / 'c1-bare.out').read_text(encoding='utf-8') == 'OW-REFUSED: not run: the command names no deadline\n'
    assert (folder / 'c2-bad.out').read_text(encoding='utf-8') == "OW-REFUSED: not run: the command's deadline line is unreadable\n"
    assert (folder / 'c3-wall.out').read_text(encoding='utf-8').startswith('OW-REFUSED: not run: its deadline is on the wall clock')


def test_a_command_withdrawn_between_the_listing_and_the_rename_is_skipped_and_the_next_still_runs():
    """Item 6: the sender withdraws a command it gave up on by renaming it away. If that lands after the driver listed the folder, the
    driver's own rename finds no file: the command is not run, and the next command in the same pass still is."""
    class Withdrawn(type(Path())):
        def read_text(self, *args, **kwargs):
            text = super().read_text(*args, **kwargs)
            if self.name == 'c1-gave-up.py':
                super().replace(self.with_name(self.name + '.withdrawn'))   # the sender's rename, between the driver's read and its own
            return text
    ns, folder = load_channel(Withdrawn)
    (folder / 'c1-gave-up.py').write_text(command("runs.append('gave up')"), encoding='utf-8')
    (folder / 'c2-next.py').write_text(command("runs.append('next')\nprint('ran')"), encoding='utf-8')
    ns['run_pending']()
    assert ns['scope']['runs'] == ['next'], ns['scope']['runs']
    assert sorted(p.name for p in folder.iterdir()) == ['c1-gave-up.py.withdrawn', 'c2-next.out', 'c2-next.py.done']


def publish_with_the_sender(timeout):
    """The real sender (Invoke-GameReply, realgame.ps1) publishes one command; its driver stand-in only copies the published file, then
    the call fails. Returns the file's text and the sender's command clock and wall clock just before the call."""
    realgame = ROOT / 'tools' / 'real_game' / 'realgame.ps1'
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'send.ps1'
        script.write_text(f""". '{realgame}'
$script:CmdDir = '{tmp}'
function Start-Sleep {{ foreach ($py in @(Get-ChildItem -LiteralPath $script:CmdDir -Filter '*.py')) {{ Copy-Item -LiteralPath $py.FullName (Join-Path '{tmp}' 'published.txt') }}; throw 'invented: stop waiting' }}
$clock = if (Get-Command Get-CommandClockUs -ErrorAction SilentlyContinue) {{ Get-CommandClockUs }} else {{ 0 }}
"before: $clock $([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())"
try {{ Invoke-GameReply 'print(1)' {timeout} | Out-Null }} catch {{ "error: $_" }}
""", encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True,
                              timeout=120, cwd=tmp, env=dict(os.environ))
        published = (Path(tmp) / 'published.txt').read_text(encoding='utf-8')
    clock, wall = re.search(r'before: (\d+) (\d+)', done.stdout).groups()
    return published, int(clock), int(wall), done.stdout + done.stderr


def test_the_sender_and_the_driver_agree_on_the_deadline_line_and_the_refusal():
    """The real sender writes a command; the driver's own parser reads that file's deadline as the call's timeout from the sender's
    reading of the command clock, refuses it at the deadline and runs it a second before. The prefixes the sender writes and checks are
    the driver's."""
    realgame = (ROOT / 'tools' / 'real_game' / 'realgame.ps1').read_text(encoding='utf-8')
    assert "StartsWith('OW-REFUSED: '" in realgame and '"# ow-deadline-qpc-us: $deadline`n"' in realgame
    published, before, _, out = publish_with_the_sender(7)
    ns, _ = load_channel(type(Path()))
    first, code = published.split('\n', 1)
    deadline = int(first.split(':', 1)[1])
    assert first.startswith(ns['DEADLINE_PREFIX']) and code == 'print(1)', published
    # The call's own 7 s from the sender's reading; up to 10 s more for a runner that stalls between the reading and the call (item 9).
    assert 7_000_000 <= deadline - before <= 17_000_000, (deadline - before, out)
    assert ns['refusal'](published, deadline) is not None and ns['refusal'](published, deadline - 1_000_000) is None
    assert ns['REFUSED_PREFIX'] == 'OW-REFUSED: ' and ns['WALL_CLOCK_PREFIX'] == '# ow-deadline-unix-ms:'


def test_a_wall_clock_stepped_back_does_not_revive_a_command_its_caller_gave_up_on():
    """Round 2, blocker 1: both sides read the wall clock, so after the sender gave up a clock stepped back put the queued command inside
    its deadline again, and the driver ran it. The real sender publishes a command with a 1 s timeout; the driver takes it 10 s later on
    the command clock while its wall clock reads an hour earlier than the sender's did. It is refused and never runs."""
    published, before, wall_ms, _ = publish_with_the_sender(1)
    ns, folder = load_channel(type(Path()), now=qpc_us() + 10_000_000, wall=wall_ms / 1000.0 - 3600.0)
    (folder / 'c1-gave-up.py').write_text(published.replace('print(1)', "runs.append('revived')"), encoding='utf-8')
    ns['run_pending']()
    assert ns['scope']['runs'] == [], 'a command its caller gave up on ran after the wall clock stepped back'
    assert (folder / 'c1-gave-up.out').read_text(encoding='utf-8').startswith('OW-REFUSED: not run: ')


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
