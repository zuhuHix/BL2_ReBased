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
function New-CommandName {{ 'cT-00000000000000000000000000000001' }}   # the name fixed, so an old reply under it can be planted
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
    # Two real GUID names in a row differ even inside one millisecond; plain .NET, so no module has to load (CI on #263).
    assert '[guid]::NewGuid()' in one_line('New-CommandName') and 'New-CommandName' in helper('Invoke-GamePy')



def one_line_of(script, name):
    """A one-line function's text from a capture script."""
    return next(line for line in text(script).split('\n') if line.startswith(f'function {name}('))


def script_fn(script, name):
    """One function's text from a capture script (a multi-line function ending in a closing brace on its own line)."""
    body = text(script)
    start = body.index(f'function {name}(')
    return body[start:body.index('\n}\n', start) + 3]


def test_an_empty_reply_fails_with_a_clear_message_in_both_finalizers():
    # Run Z47: the parity2 quit's reply file was empty as the game exited, Get-Content -Raw gave $null, and .Trim() threw a null-method error.
    # Review of #263, R3: the transport now returns '' for an empty reply; both wrappers still refuse it (null OR empty), and an empty
    # fp_ready answer does not reach the slot key that follows it.
    for name in ('projectiles_capture', 'weapon_parity2_capture'):
        out = run_stubbed("""$ErrorActionPreference = 'Stop'
$script:keys = @()
function Send-Key {{ $script:keys += "$args" }}
function Assert-GameForeground {{ }}
function Invoke-GamePy($Code, $Timeout) {{ if ($Code -eq 'empty') {{ '' }} elseif ($Code -eq 'null') {{ $null }} elseif ($Code -like '*fp_ready*') {{ '' }} else {{ "  answer`n" }} }}
{}
{}
{}
"ok: [$(Py 'full')]"
try {{ Py 'empty' | Out-Null; 'no throw' }} catch {{ "threw: $_" }}
try {{ Py 'null' | Out-Null; 'no throw' }} catch {{ "threw null: $_" }}
try {{ $ready = PyJson 'fp_ready("g", 1)' 60; Assert-GameForeground; Send-Key '1' }} catch {{ "ready: $_" }}
"keys: [$($script:keys -join ',')]"
""".format(wrapper(), one_line_of(name, 'Py'), script_fn(name, 'PyJson')))
        assert 'ok: [answer]' in out and 'threw: empty reply from the game (it may have exited) to: empty' in out, name + out
        assert 'threw null: empty reply from the game (it may have exited) to: null' in out, name + out
        assert 'ready: empty reply' in out and 'keys: []' in out, name + out   # no input after an empty fp_ready


def test_the_parity2_quit_expects_no_reply_and_fp_cleanup_is_not_overwritten():
    for name in ('projectiles_capture', 'weapon_parity2_capture'):
        s = text(name)
        quit_line = next(l for l in s.splitlines() if 'ConsoleCommand("quit"' in l)
        # Through the shared wrapper (session gate, B2): an empty reply is taken, and nothing is sent to a lost driver.
        assert quit_line.lstrip().startswith('try { Invoke-GameReply ') and ' -AllowEmpty | Out-Null }' in quit_line, name
        assert 'catch { "quit: no reply, or not sent to a lost driver: $_" }' in quit_line, name
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


def one_line(name):
    """A one-line function's text from the shared realgame.ps1."""
    return next(line for line in text('realgame').split('\n') if line.startswith(f'function {name} '))


def wrapper():
    """The shared reply wrapper with the driver-lost condition it holds (realgame.ps1), as text."""
    return '\n'.join(["$script:SliceWatchDriverLost = ''"] + [one_line(n) for n in ('Get-DriverLostReason', 'Assert-DriverAnswers')]
                     + [one_line_fn('Set-SliceWatchDriverLost')] + [helper(n) for n in ('ConvertTo-ReplyText', 'Invoke-GameReply', 'Invoke-GameReplyFile')])


def one_line_fn(name):
    """A one-line function with a parameter list from the shared realgame.ps1."""
    return next(line for line in text('realgame').split('\n') if line.startswith(f'function {name}('))


TRANSPORT_STUBS = r"""$ErrorActionPreference = 'Stop'
$script:CmdDir = '@TMP@'; $script:RG = '@TMP@'
$script:SliceWatchDriverLost = ''
function Set-SliceWatchDriverLost($Err) { if (!$script:SliceWatchDriverLost -and "$Err" -match 'no reply') { $script:SliceWatchDriverLost = "$Err" } }
function Sent { @(Get-ChildItem -LiteralPath $script:CmdDir -Filter '*.py').Count }
"""


def transport():
    return one_line('New-CommandName') + '\n' + helper('Invoke-GamePy') + helper('Invoke-GamePyFile')


def test_any_failure_inside_the_transport_leaves_in_the_no_reply_form():
    # Review of #263, S1: filesystem errors outside the transport's own catches left with their own text, which no latch matched: the
    # removal of a stale reply, the reply-existence check of the wait, a probe source that cannot be read, and anything else.
    sites = {
        'stale reply removal': "function Test-Path { $true }; function Remove-Item { throw 'invented: the stale reply stays' }",
        'reply check': "$script:tp = 0; function Test-Path { $script:tp++; if ($script:tp -ge 2) { throw 'invented: the check failed' }; $false }",
        'arbitrary': "function Join-Path { throw 'invented: anything at all, with no keyword' }",
    }
    for site, stub in sites.items():
        out = run_stubbed(TRANSPORT_STUBS + stub + '\n' + transport() + """
try { Invoke-GamePy 'print(1)' 5 | Out-Null; $e = 'returned' } catch { $e = "$_"; Set-SliceWatchDriverLost $_ }
"error: $e"
"latched: $script:SliceWatchDriverLost"
""")
        error = re.search(r'error: (.*)', out).group(1)
        assert error.startswith('no reply to ') and 'invented' in error, (site, out)
        assert f'latched: {error}' in out, (site, out)
    # A probe source that cannot be read (held open with no sharing): nothing is sent, and the latch takes it.
    out = run_stubbed(TRANSPORT_STUBS + transport() + r"""
$probe = Join-Path $script:CmdDir 'probe.txt'; [IO.File]::WriteAllText($probe, 'print(1)')
$held = [IO.File]::Open($probe, 'Open', 'Read', 'None')
try { Invoke-GamePyFile $probe 'x' 5 | Out-Null; $e = 'returned' } catch { $e = "$_"; Set-SliceWatchDriverLost $_ }
$held.Dispose()
"error: $e"
"latched: $script:SliceWatchDriverLost"
"sent: $(Sent)"
""")
    error = re.search(r'error: (.*)', out).group(1)
    assert error.startswith('no reply to probe file') and 'its source could not be read' in error, out
    assert f'latched: {error}' in out and 'sent: 0' in out, out


def test_one_deadline_covers_publishing_waiting_and_reading():
    # Review of #263, S2: the reply wait started after the publish retries, so publish retries, the wait and the read retries together could
    # run past a caller's window. A virtual clock: Get-Date reads it, Start-Sleep moves it; no real time passes. The worst case together:
    # the publish refused four times, then a reply that comes late, then a reply file that cannot be read.
    out = run_stubbed(TRANSPORT_STUBS + r"""
$script:t0 = [datetime]'2026-01-01T00:00:00'; $script:now = $script:t0
function Get-Date { param($Format) if ($Format) { 'T' } else { $script:now } }
function Start-Sleep { param($Milliseconds)
    $script:now = $script:now.AddMilliseconds($Milliseconds)
    $py = @(Get-ChildItem -LiteralPath $script:CmdDir -Filter '*.py')
    if ($py.Count -and !$script:held -and ($script:now - $script:t0).TotalSeconds -ge 2.5) {   # the late reply, held unreadable
        $script:held = [IO.File]::Open([IO.Path]::ChangeExtension($py[0].FullName, '.out'), 'Create', 'ReadWrite', 'None') } }
$script:moves = 0
function Move-Item($From, $To) { $script:moves++; if ($script:moves -le 4) { throw 'invented: being used by another process' }; [IO.File]::Move($From, $To) }
""" + transport() + r"""
try { Invoke-GamePy 'print(1)' 3 | Out-Null; $e = 'returned' } catch { $e = "$_" }
"error: $e"
"elapsed: $((($script:now - $script:t0).TotalSeconds).ToString([Globalization.CultureInfo]::InvariantCulture))"
"moves: $script:moves"
""")
    error = re.search(r'error: (.*)', out).group(1)
    elapsed = float(re.search(r'elapsed: ([\d.]+)', out).group(1))
    assert error.startswith('no reply to ') and 'could not be read' in error, out
    assert 'moves: 5' in out, out
    assert elapsed <= 3.0, out   # the caller's 3 s bound holds for the whole call


def test_a_complete_empty_reply_is_the_empty_string():
    # Review of #263, S3: an empty reply read as null, and a caller's .Trim() then threw an error no latch matched.
    out = run_stubbed(TRANSPORT_STUBS + r"""
function Start-Sleep { foreach ($py in @(Get-ChildItem -LiteralPath $script:CmdDir -Filter '*.py')) { Rename-Item -LiteralPath $py.FullName ($py.Name + '.done'); [IO.File]::WriteAllText([IO.Path]::ChangeExtension($py.FullName, '.out'), '') } }
""" + transport() + r"""
$r = Invoke-GamePy 'pass' 5
"null: $($null -eq $r)"
"trimmed: [$($r.Trim())]"
""")
    assert 'null: False' in out and 'trimmed: []' in out, out


def test_the_command_name_needs_no_module_auto_loading():
    # CI on #263: a Windows PowerShell child started with PowerShell 7's module path cannot load New-Guid (a script function of its
    # Utility module), and the name failed before anything was sent. The name is plain .NET; checked under pwsh's module path when there is one.
    assert 'New-Guid' not in transport() and '[guid]::NewGuid()' in one_line('New-CommandName')
    try:
        ps7 = subprocess.run(['pwsh', '-NoProfile', '-Command', '$env:PSModulePath'], capture_output=True, text=True, timeout=60).stdout.strip()
    except OSError:
        ps7 = ''
    if not ps7:
        print('  (no pwsh here: the module-path case is checked where PowerShell 7 is installed)')
        return
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / 'name.ps1'
        path.write_text("$ErrorActionPreference = 'Stop'\n" + one_line('New-CommandName') + "\n\"name: $(New-CommandName)\"\n", encoding='utf-8-sig')
        env = dict(os.environ, PSModulePath=ps7, OW_TEST_KEEP_PSMODULEPATH='1')   # this test sets the runner's module path on purpose
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(path)], capture_output=True, text=True, env=env, timeout=60)
    assert re.search(r'name: c\d{9}-[0-9a-f]{32}', done.stdout), done.stdout + done.stderr


def test_no_publication_and_no_reply_taken_after_the_deadline_when_operations_are_slow():
    # Review of #263, R1: the deadline was checked only on a failure or while the reply was absent. A virtual clock where the stale-reply
    # check, the existence test and the read each take time (they succeed): nothing is published after the deadline, a reply read after
    # it is not taken, and no call returns later than the operation under way when the deadline passed, plus one pause (200 ms).
    def run(stale, exists, read):
        out = run_stubbed(TRANSPORT_STUBS + r"""
$script:t0 = [datetime]'2026-01-01T00:00:00'; $script:now = $script:t0
function Get-Date { param($Format) if ($Format) { 'T' } else { $script:now } }
function Start-Sleep { param($Milliseconds) $script:now = $script:now.AddMilliseconds($Milliseconds) }
function Test-Path {
    if ($args[0] -eq '-LiteralPath') { $script:now = $script:now.AddSeconds(STALE) } else { $script:now = $script:now.AddSeconds(EXISTS) }
    Microsoft.PowerShell.Management\Test-Path -LiteralPath "$($args[-1])" }
function Get-Content { $script:now = $script:now.AddSeconds(READ); [IO.File]::ReadAllText("$($args[$args.IndexOf('-LiteralPath') + 1])") }
function Move-Item($From, $To) { [IO.File]::Move($From, $To); $script:published = ($script:now - $script:t0).TotalSeconds; [IO.File]::WriteAllText(($To -replace '[.]py$', '.out'), 'answer') }
""".replace('STALE', str(stale)).replace('EXISTS', str(exists)).replace('READ', str(read)) + transport() + r"""
try { $r = Invoke-GamePy 'print(1)' 3; $e = "returned $r" } catch { $e = "$_" }
"result: $e"
"published: $(if ($null -ne $script:published) { $script:published.ToString([Globalization.CultureInfo]::InvariantCulture) } else { 'never' })"
"ended: $((($script:now - $script:t0).TotalSeconds).ToString([Globalization.CultureInfo]::InvariantCulture))"
""")
        result = re.search(r'result: (.*)', out).group(1)
        published = re.search(r'published: (\S+)', out).group(1)
        ended = float(re.search(r'ended: ([\d.]+)', out).group(1))
        return result, published, ended, out
    result, published, ended, out = run(0, 0, 0)
    assert result == 'returned answer' and published == '0', out   # in time: taken
    result, published, ended, out = run(4, 0, 0)   # the stale-reply check alone outlasts the 3 s
    assert result.startswith('no reply to ') and published == 'never' and ended <= 4.2, out
    result, published, ended, out = run(0, 1.5, 2)   # found at 1.5 s, read until 3.5 s: after the deadline, so not taken
    assert result.startswith('no reply to ') and published == '0' and ended <= 3.5 + 0.2, out
    result, published, ended, out = run(0, 0, 4)   # a read that ends at 4 s
    assert result.startswith('no reply to ') and ended <= 4.2, out


def test_a_fault_at_the_first_statement_of_either_transport_function_is_no_reply():
    # Review of #263, R2: the deadline's first clock read sat before Invoke-GamePy's boundary, and Invoke-GamePyFile's delegation after its
    # own. Every statement of both bodies is now inside; only the parameter binding is outside.
    cases = {
        'Invoke-GamePy, its first clock read': ("function Get-Date { throw 'invented: the clock failed' }", "Invoke-GamePy 'print(1)' 3"),
        'Invoke-GamePyFile, its first statement': ("function Join-Path { throw 'invented: no path' }", "Invoke-GamePyFile 'x.py' 'out' 3"),
        'Invoke-GamePyFile, its delegation': ("function Invoke-GamePy { throw 'invented: the delegation failed' }",
                                              "Invoke-GamePyFile (Join-Path $script:CmdDir 'p.py') 'out' 3"),
    }
    for site, (stub, call) in cases.items():
        out = run_stubbed(TRANSPORT_STUBS + r"[IO.File]::WriteAllText((Join-Path '@TMP@' 'p.py'), 'print(1)')" + '\n' + transport() + stub + '\n' + f"""
try {{ {call} | Out-Null; $e = 'returned' }} catch {{ $e = "$_"; Set-SliceWatchDriverLost $_ }}
"error: $e"
"latched: $script:SliceWatchDriverLost"
""")
        error = re.search(r'error: (.*)', out).group(1)
        assert error.startswith('no reply to ') and 'invented' in error, (site, out)
        assert f'latched: {error}' in out, (site, out)


def test_a_failed_test_cleanup_never_hides_the_tests_own_error():
    # Review of #263, R4: with raising=False an OSError other than a permission error escaped and replaced the test's own exception.
    import io
    import shutil
    import sys as _sys
    calls = []

    def failing(error):
        def rmtree(path):
            calls.append(path)
            raise error
        return rmtree
    keep, keep_err = shutil.rmtree, _sys.stderr
    try:
        for error in (OSError(22, 'invented: not a sharing failure'), PermissionError(13, 'invented: still held')):
            shutil.rmtree, _sys.stderr = failing(error), io.StringIO()
            calls.clear()
            realgame_test_env.remove_tree('x', tries=3, pause=0, raising=False)   # reported, not raised
            assert 'not removed' in _sys.stderr.getvalue(), error
            assert len(calls) == (3 if isinstance(error, PermissionError) else 1), (error, calls)   # only a sharing failure is retried
            try:
                realgame_test_env.remove_tree('x', tries=3, pause=0)
                raised = False
            except OSError:
                raised = True
            assert raised, error   # with raising, a cleanup that keeps failing is reported as an error
    finally:
        shutil.rmtree, _sys.stderr = keep, keep_err


# ---------------------------------------------------------------- the session gate's review (B1, B2, item 2), 2026-11-13
VIRTUAL_CLOCK = r"""
$script:t0 = [datetime]'2026-01-01T00:00:00'; $script:now = $script:t0
function Get-Date { param($Format) if ($Format) { 'T' } else { $script:now } }
function Start-Sleep { param($Milliseconds) $script:now = $script:now.AddMilliseconds($Milliseconds) }
"""


def test_a_command_whose_file_is_written_after_the_deadline_is_not_published():
    """B1: the rename that publishes a command ran even when writing its file outlasted the deadline. A virtual clock that jumps 5 s while
    the command's temporary file is there (a slow write); the caller's timeout is 3 s. Nothing is renamed, the temporary file is removed, and
    the call fails in the no-reply form."""
    out = run_stubbed(TRANSPORT_STUBS + VIRTUAL_CLOCK + r"""
function Get-Date { param($Format)
    if ($Format) { return 'T' }
    if (!$script:jumped -and @(Get-ChildItem -LiteralPath $script:CmdDir -Filter '*.py.tmp').Count) { $script:jumped = $true; $script:now = $script:now.AddSeconds(5) }
    $script:now }
$script:moves = 0
function Move-Item($From, $To) { $script:moves++; [IO.File]::Move($From, $To) }
""" + transport() + r"""
try { Invoke-GamePy 'print(1)' 3 | Out-Null; $e = 'returned' } catch { $e = "$_" }
"error: $e"
"moves: $script:moves"
"files: $(@(Get-ChildItem -LiteralPath $script:CmdDir -Filter 'c*').Count)"
""")
    assert re.search(r'error: no reply to c\S+ \(not sent: the deadline passed while its file was written\)', out), out
    assert 'moves: 0' in out and 'files: 0' in out, out   # never renamed onto <name>.py, and no .tmp left


def test_every_published_command_carries_its_deadline_and_a_refusal_is_no_reply():
    """B1: the receiver enforces the deadline, so the sender puts it in the command: the first line names the call's deadline in Unix
    milliseconds of this machine's clock. A stand-in driver that refuses the command (as openwillow_realgame does with an expired one)
    answers 'OW-REFUSED: ...'; the transport fails in the no-reply form, which the shared wrapper latches."""
    out = run_stubbed(TRANSPORT_STUBS + r"""
function Start-Sleep { foreach ($py in @(Get-ChildItem -LiteralPath $script:CmdDir -Filter '*.py')) {
    $script:first = @(Get-Content -LiteralPath $py.FullName)[0]
    Rename-Item -LiteralPath $py.FullName ($py.Name + '.expired')
    [IO.File]::WriteAllText([IO.Path]::ChangeExtension($py.FullName, '.out'), "OW-REFUSED: not run: its deadline passed 7 ms before the driver took it`n") } }
""" + transport() + wrapper() + r"""
$before = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
try { Invoke-GameReply 'print(1)' 5 | Out-Null; $e = 'returned' } catch { $e = "$_" }
"first: $script:first"
"window: $before"
"error: $e"
"latched: $(Get-DriverLostReason)"
""")
    first = re.search(r'first: # ow-deadline-unix-ms: (\d+)', out)
    assert first, out
    ahead = int(first.group(1)) - int(re.search(r'window: (\d+)', out).group(1))
    assert 4000 <= ahead <= 6000, (ahead, out)   # the call's own 5 s from now
    assert re.search(r'error: no reply to c\S+ \(the driver refused it: not run: its deadline passed 7 ms', out), out
    assert re.search(r'latched: no reply to c\S+ \(the driver refused it', out), out


def test_the_file_call_counts_the_read_of_its_source():
    """B1: Invoke-GamePyFile read its source before the call's deadline existed, so a slow read lengthened the call past its timeout. A
    virtual clock where resolving the source's path takes 5 s; the timeout is 3 s. Nothing is published."""
    out = run_stubbed(TRANSPORT_STUBS + VIRTUAL_CLOCK + r"""
function Resolve-Path { $script:now = $script:now.AddSeconds(5); Microsoft.PowerShell.Management\Resolve-Path @args }
$script:moves = 0
function Move-Item($From, $To) { $script:moves++; [IO.File]::Move($From, $To); [IO.File]::WriteAllText(($To -replace '[.]py$', '.out'), 'answer') }
[IO.File]::WriteAllText((Join-Path $script:CmdDir 'probe.txt'), 'print(1)')
""" + transport() + r"""
try { $r = Invoke-GamePyFile (Join-Path $script:CmdDir 'probe.txt') 'x' 3; $e = "returned $r" } catch { $e = "$_" }
"error: $e"
"moves: $script:moves"
""")
    assert 'error: no reply to probe file' in out and 'the deadline passed while its source was read' in out, out
    assert 'moves: 0' in out, out


def test_a_late_reply_is_never_taken_for_the_next_commands():
    """B1: a command that timed out may still get its reply later (a driver that took it before its deadline). That reply goes to that
    command's own name and is not read by the next call, whose name is its own. A stand-in driver that answers the first command only
    while the second is waiting, then the second."""
    out = run_stubbed(TRANSPORT_STUBS + r"""
$script:names = @()
function Start-Sleep {
    $py = @(Get-ChildItem -LiteralPath $script:CmdDir -Filter '*.py' | Sort-Object Name)
    foreach ($p in $py) { if ($script:names -notcontains $p.Name) { $script:names += $p.Name } }
    if ($script:phase -eq 'second' -and $py.Count) {
        foreach ($p in $py) {
            $answer = if ($p.Name -eq $script:names[0]) { 'the first command''s late answer' } else { 'the second command''s answer' }
            Rename-Item -LiteralPath $p.FullName ($p.Name + '.done'); [IO.File]::WriteAllText([IO.Path]::ChangeExtension($p.FullName, '.out'), $answer)
        }
    } }
""" + transport() + r"""
$script:phase = 'first'
try { Invoke-GamePy 'print(1)' 1 | Out-Null; $e = 'returned' } catch { $e = "$_" }
$script:phase = 'second'
$r = Invoke-GamePy 'print(2)' 5
"first: $e"
"second: $r"
"sent: $($script:names.Count)"
""")
    assert re.search(r'first: no reply to c\S+$', out, flags=re.M), out
    assert 'second: the second command\'s answer' in out and 'sent: 2' in out, out


def test_an_empty_or_white_space_reply_is_refused_unless_the_call_allows_it():
    """Item 2: ConvertTo-ReplyText took a reply of only white space and returned '' after trimming. It is refused like an empty one; a call
    that says printing nothing is its answer (-AllowEmpty) gets ''. Neither is a lost driver."""
    out = run_stubbed(TRANSPORT_STUBS + r"""
function Invoke-GamePy($Code, $T) { switch ($Code) { 'empty' { '' } 'null' { $null } 'space' { "  `r`n `t" } default { ' answer ' } } }
""" + wrapper() + r"""
foreach ($c in 'empty', 'null', 'space') {
    try { Invoke-GameReply $c | Out-Null; "$c taken" } catch { "$c refused: $_" }
    "$c allowed: [$(Invoke-GameReply $c -AllowEmpty)]"
}
"full: [$(Invoke-GameReply 'full')]"
"latched: [$(Get-DriverLostReason)]"
""")
    for c in ('empty', 'null', 'space'):
        assert f'{c} refused: empty reply from the game (it may have exited) to: {c}' in out and f'{c} allowed: []' in out, (c, out)
    assert 'full: [answer]' in out and 'latched: []' in out, out


def test_the_shared_wrapper_refuses_after_a_lost_reply_and_its_input_helpers_refuse_too():
    """B2: the rule lives in one place. After a reply that never came, Invoke-GameReply and Invoke-GameReplyFile send nothing, a
    silence-allowed wait refuses but never sets the condition itself, and every key press, mouse press or move, wheel turn and focus
    attempt of realgame.ps1 refuses; releases still go out."""
    out = run_stubbed(r"""$ErrorActionPreference = 'Stop'
. '__REALGAME__'
$script:sent = 0; $script:native = @()
function Invoke-GamePy($Code, $T) { $script:sent++; if ($Code -eq 'slow') { throw 'no reply to c-invented' }; 'ok' }
function Invoke-GamePyFile($Path, $Folder, $T) { $script:sent++; 'ok' }
function Invoke-NativeKey($Vk, $Scan, $Flags) { $script:native += "key $Scan $Flags" }
function Invoke-NativeMouse($Flags) { $script:native += "mouse $Flags" }
function Get-RunGameProcess { [pscustomobject]@{ MainWindowHandle = 1 } }
function Get-ForegroundWindowHandle { 1 }
function Save-HeldInputs { }
try { Invoke-GameReply 'slow' 5 -SilenceAllowed | Out-Null } catch { }
"silence latched: [$(Get-DriverLostReason)]"
Send-KeyDown W
try { Invoke-GameReply 'slow' | Out-Null } catch { }
$n = $script:sent
foreach ($call in { Invoke-GameReply 'fast' }, { Invoke-GameReplyFile 'p.py' }, { Invoke-GameReply 'fast' 8 -SilenceAllowed }) {
    try { & $call | Out-Null; 'sent' } catch { "refused: $_" } }
$keys = $script:native.Count
foreach ($call in { Send-KeyDown E }, { Send-Key F }, { Send-MouseEvent 0x2 }, { Send-Mouse 1 1 }, { Send-Wheel 1 }, { Focus-Game }, { Assert-GameForeground }) {
    try { & $call | Out-Null; 'input sent' } catch { "input refused: $_" } }
"inputs after: $($script:native.Count - $keys)"
"release: $(Release-HeldInputs)"
"sent after: $($script:sent - $n)"
""".replace('__REALGAME__', str(R / 'realgame.ps1')))
    assert 'silence latched: []' in out, out
    assert out.count('refused: not sent: the driver stopped answering (no reply to c-invented)') == 3 and '\nsent\n' not in out, out
    assert out.count('input refused: refusing to send input: the driver stopped answering') == 7 and 'input sent' not in out, out
    assert 'inputs after: 0' in out and 'release: released key W' in out and 'sent after: 0' in out, out


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
