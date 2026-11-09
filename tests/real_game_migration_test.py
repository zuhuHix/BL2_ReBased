"""The seven older capture scripts stop only their own game and send input only through the guarded helpers. No game and no real input.

Run: python tests/real_game_migration_test.py
"""
from pathlib import Path
import os
import re
import subprocess
import realgame_test_env  # noqa: F401  (a fresh pending-record folder for every PowerShell run)
import tempfile

ROOT = Path(__file__).resolve().parents[1]
R = ROOT / 'tools' / 'real_game'
SEVEN = ['action_skill_hud_capture', 'hud_integrity_capture', 'hud_layout_capture', 'hud_parity_capture', 'weapon_part_session',
         'projectiles_capture', 'weapon_parity2_capture']


def text(name):
    return (R / f'{name}.ps1').read_text(encoding='utf-8').replace('\r\n', '\n')


def test_no_script_stops_a_process_or_sends_raw_input_itself():
    for name in SEVEN:
        s = text(name)
        assert 'Stop-Process' not in s and 'mouse_event' not in s and 'keybd_event' not in s and '.Kill(' not in s, name
        assert 'Focus-Game' not in s, name


def test_each_stops_through_stop_own_game_with_a_marker_on_the_launch():
    for name in SEVEN:
        s = text(name)
        assert 'Stop-OwnGame $proc $procStart $marker' in s, name
        assert '$procStart = $proc' in s and re.search(r'Start-Game [^\n]*', s), name
        launch = re.search(r'Start-Game (@\([^\n]*\)|\$gameArgs)', s)
        assert '$marker' in (launch[0] if '@(' in launch[0] else re.search(r'\$gameArgs = @\([^\n]*', s)[0]), name


def test_each_proves_saves_are_blocked_before_gameplay():
    for name in SEVEN:
        assert "Assert-SavesBlocked (Py 'print(block_saves())')" in text(name), name


def test_the_held_right_button_is_released_when_a_step_throws():
    body = text('projectiles_capture')
    block = body[body.index('function Right-Hold'):body.index('function Right-Release')]
    assert 'Assert-GameForeground' in block
    assert re.search(r'try \{\n\s*Right-Hold 800.*?\} finally \{ Right-Release \}', body, flags=re.S)
    parity = text('weapon_parity2_capture')
    assert re.search(r'try \{ Right-Hold 900;.*?\} finally \{ Right-Release \}', parity)
    # Run the pattern on stubs: the release goes even when the step throws, and a refused hold sends nothing.
    defs = re.search(r'function Right-Hold[^\n]*\nfunction Right-Release[^\n]*\n', body)[0]
    script = f"""$ErrorActionPreference = 'Stop'
$script:sent = @()
function Send-MouseEvent([int]$Flags) {{ $script:sent += $Flags }}
function Assert-GameForeground {{ }}
function Start-Sleep {{ }}
{defs}
try {{ try {{ Right-Hold 800; throw 'step failed' }} finally {{ Right-Release }} }} catch {{ }}
"held: $($script:sent -join ',')"
$script:sent = @()
function Assert-GameForeground {{ throw 'refused' }}
try {{ Right-Hold 800 }} catch {{ "refused: $($script:sent.Count)" }}
"""
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / 'hold.ps1'
        path.write_text(script, encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(path)], capture_output=True, text=True, timeout=120)
    assert done.returncode == 0 and 'held: 8,16' in done.stdout and 'refused: 0' in done.stdout, done.stdout + done.stderr


def test_each_script_parses():
    names = ', '.join(f"'{R / (n + '.ps1')}'" for n in SEVEN)
    script = f"""foreach ($f in {names}) {{ $e = $null; [void][Management.Automation.Language.Parser]::ParseFile($f, [ref]$null, [ref]$e); "$f $(@($e).Count)" }}"""
    done = subprocess.run(['powershell', '-NoProfile', '-Command', script], capture_output=True, text=True, timeout=120, env=dict(os.environ))
    lines = [l for l in done.stdout.splitlines() if l.strip()]
    assert done.returncode == 0 and len(lines) == 7 and all(l.endswith(' 0') for l in lines), done.stdout + done.stderr


def test_the_doc_lifts_all_seven_and_bars_none():
    doc = (ROOT / 'docs/TOOLING.md').read_text(encoding='utf-8')
    line = next(l for l in doc.splitlines() if l.startswith('- **Bar lifted (2026-11-09):**'))
    assert not any(l.startswith('- **Not to be run:**') for l in doc.splitlines())
    lifted = re.findall(r'`([^`]+)`', line.split(' are no longer barred', 1)[0])
    assert len(lifted) == 7 and set(lifted) == set(SEVEN), lifted
    # The lift of the first five rests on run Z41 (hud_layout_capture, shared cleanup); the two finalizers on PR #200 and run Z44.
    assert 'UNVERIFIED for its own capture' in line and 'commit 6da8d34' in line and 'PR #200' in line and 'Z44' in line, line
    for name in SEVEN[:5]:
        s = text(name)
        assert 'Complete-BackupRun $bak' in s, name
        assert s.index('Complete-BackupRun $bak') < s.index('Exit-RunLock }', s.index('} finally {')), name   # restore before the locks go
        assert 'added (left in place)' not in s, name   # the old compare-only block is gone
    for name in SEVEN[5:]:
        s = text(name)
        assert '$cleanup.pending = Complete-PendingRecord' in s and 'Install-Driver -ClaimedBy $runId' in s, name
        assert 'Restore-VerifiedRoots $bak' in s, name   # the re-checking restore (PR #200), not a bare Restore-Tree


def helper(name):
    """One function's text from the shared realgame.ps1."""
    body = text('realgame')
    start = body.index(f'function {name}(')
    return body[start:body.index('\n}\n', start) + 3]


def run_stubbed(script):
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / 'stub.ps1'
        path.write_text(script.replace('@TMP@', tmp), encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(path)], capture_output=True, text=True, timeout=120)
    assert done.returncode == 0, done.stdout + done.stderr
    return done.stdout


def test_the_marker_is_compared_literally():
    out = run_stubbed("""$ErrorActionPreference = 'Stop'
$script:Game = 'C:/G'
$script:start = Get-Date
function Get-CimInstance {{ [pscustomobject]@{{ ExecutablePath = 'C:\\G\\Binaries\\Win32\\Borderlands2.exe'; CreationDate = $script:start; CommandLine = $script:line }} }}
function Stop-Process {{ }}
{}
$me = Get-Process -Id $PID
$script:line = 'game -owrun=[a]-1'; "brackets: $(Stop-OwnGame $me $script:start '-owrun=[a]-1')"
$script:line = 'game -owrun=other'; "star: $(Stop-OwnGame $me $script:start '-owrun=*')"
$script:line = $null; "unreadable: $(Stop-OwnGame $me $script:start '-owrun=x')"
""".format(helper('Stop-OwnGame')))
    assert 'brackets: stopped' in out, out      # a pattern match would refuse this: [a] matches the letter a
    assert 'star: refused' in out, out          # and would stop this one: * matches anything
    assert 'unreadable: refused' in out, out


def test_an_old_reply_cannot_answer_a_new_command():
    out = run_stubbed("""$ErrorActionPreference = 'Stop'
$script:CmdDir = '@TMP@'
function New-Guid {{ [guid]'00000000-0000-0000-0000-000000000001' }}
function Get-Date {{ param($Format) if ($Format) {{ 'T' }} else {{ Microsoft.PowerShell.Utility\\Get-Date }} }}
# Stands for the driver: at the moment the command is published, says whether an old reply is there, then answers.
function Move-Item($From, $To) {{
    Microsoft.PowerShell.Management\\Move-Item -LiteralPath $From -Destination $To
    "old reply at publish: $(Test-Path -LiteralPath ($To -replace '[.]py$', '.out'))"
    [IO.File]::WriteAllText(($To -replace '[.]py$', '.out'), 'fresh')
}}
{}
$old = Join-Path $script:CmdDir 'cT-00000000000000000000000000000001.out'
[IO.File]::WriteAllText($old, 'stale')
"reply: $(Invoke-GamePy 'print(1)' 5)"
""".format(helper('Invoke-GamePy')))
    assert 'old reply at publish: False' in out and 'fresh' in out and 'stale' not in out, out
    # Two real GUID names in a row differ even inside one millisecond.
    assert '(New-Guid)' in helper('Invoke-GamePy')



def script_fn(script, name):
    """One function's text from a capture script (a multi-line function ending in a closing brace on its own line)."""
    body = text(script)
    start = body.index(f'function {name}(')
    return body[start:body.index('\n}\n', start) + 3]


def test_an_empty_reply_fails_with_a_clear_message_in_both_finalizers():
    # Run Z47: the parity2 quit's reply file was empty as the game exited, Get-Content -Raw gave $null, and .Trim() threw a null-method error.
    for name in ('projectiles_capture', 'weapon_parity2_capture'):
        out = run_stubbed("""$ErrorActionPreference = 'Stop'
function Invoke-GamePy($Code, $Timeout) {{ if ($Code -eq 'empty') {{ $null }} else {{ "  answer`n" }} }}
{}
"ok: [$(Py 'full')]"
try {{ Py 'empty' | Out-Null; 'no throw' }} catch {{ "threw: $_" }}
""".format(script_fn(name, 'Py')))
        assert 'ok: [answer]' in out and 'threw: empty reply from the game (it may have exited) to: empty' in out, name + out


def test_the_parity2_quit_expects_no_reply_and_fp_cleanup_is_not_overwritten():
    for name in ('projectiles_capture', 'weapon_parity2_capture'):
        s = text(name)
        quit_line = next(l for l in s.splitlines() if 'ConsoleCommand("quit"' in l)
        assert quit_line.lstrip().startswith('try { Invoke-GamePy ') and 'catch { "quit sent; no reply: $_" }' in quit_line, name
        # The finalizer writes $log.cleanup; the probe's own cleanup result goes under another key so it survives.
        assert "$log.fp_cleanup = PyJson 'fp_cleanup()' 60" in s and "$log.cleanup = PyJson" not in s, name


def test_projectiles_refuses_a_missing_status_recipe_before_any_lock():
    s = text('projectiles_capture')
    check = s.index('if (!(Find-LocalRecipe $StatusGun))')
    assert check < s.index('Assert-NoPendingRecords') < s.index('New-Item -ItemType File -Path $HubLock')
    # $PSScriptRoot is the real script's folder; the stub points the same function at a temp tree laid out like the repository.
    fn = script_fn('projectiles_capture', 'Find-LocalRecipe').replace('$PSScriptRoot', '$script:Root')
    out = run_stubbed("""$ErrorActionPreference = 'Stop'
$script:Root = '@TMP@/tools/real_game'
New-Item -ItemType Directory -Force '@TMP@/local/items/slice', '@TMP@/local/items/firing/coverage' | Out-Null
Set-Content '@TMP@/local/items/slice/a_gun.json' '{{}}'
Set-Content '@TMP@/local/items/firing/coverage/b_gun.json' '{{}}'
{}
"a: $([bool](Find-LocalRecipe 'a_gun')) b: $([bool](Find-LocalRecipe 'b_gun')) c: $([bool](Find-LocalRecipe 'c_gun'))"
""".format(fn))
    assert 'a: True b: True c: False' in out, out


def test_projectiles_stops_the_status_step_on_a_probe_traceback():
    # Run Z47: fp_spawn and fp_ready raised (no recipe), PyJson kept the traceback text, and slot 1 drew the save's own gun.
    s = text('projectiles_capture')
    assert "Assert-PyResult (PyJson \"fp_spawn($(Q $StatusGun))\" 60) 'fp_spawn'" in s
    assert "Assert-PyResult (PyJson \"fp_ready($(Q $StatusGun), 1)\" 60) 'fp_ready'" in s
    assert s.index("'fp_ready'") < s.rindex('draw quick slot 1 (fp_ready only fills it)')  # the status step's draw (the last one)
    out = run_stubbed("""$ErrorActionPreference = 'Stop'
{}
$tb = "Traceback (most recent call last):`r`n  File x, line 1`r`nFileNotFoundError: no local recipe for g"
try {{ Assert-PyResult $tb 'fp_spawn' | Out-Null; 'no throw' }} catch {{ "threw: $_" }}
$obj = [pscustomobject]@{{ weapon = 'W' }}
"kept: $((Assert-PyResult $obj 'fp_ready').weapon)"
""".format(script_fn('projectiles_capture', 'Assert-PyResult')))
    assert 'threw: fp_spawn failed: FileNotFoundError: no local recipe for g' in out and 'kept: W' in out, out


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
