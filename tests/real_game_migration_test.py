"""The seven older capture scripts stop only their own game and send input only through the guarded helpers. No game and no real input.

Run: python tests/real_game_migration_test.py
"""
from pathlib import Path
import os
import re
import subprocess
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


def test_the_doc_bars_exactly_the_seven():
    doc = (ROOT / 'docs/TOOLING.md').read_text(encoding='utf-8')
    line = next(l for l in doc.splitlines() if l.startswith('- **Not to be run:**'))
    barred = re.findall(r'`([^`]+)`', line.split('. ', 1)[0])
    assert len(barred) == 7 and set(barred) == set(SEVEN), barred
    # The first five really have no restore, which is the reason given for them; when one gains it, its bar should lift.
    for name in SEVEN[:5]:
        assert not re.search('restore-', text(name), flags=re.I), name


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


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
