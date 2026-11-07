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
    assert 'held: 8,16' in done.stdout and 'refused: 0' in done.stdout, done.stdout + done.stderr


def test_each_script_parses():
    names = ', '.join(f"'{R / (n + '.ps1')}'" for n in SEVEN)
    script = f"""foreach ($f in {names}) {{ $e = $null; [void][Management.Automation.Language.Parser]::ParseFile($f, [ref]$null, [ref]$e); "$f $(@($e).Count)" }}"""
    done = subprocess.run(['powershell', '-NoProfile', '-Command', script], capture_output=True, text=True, timeout=120, env=dict(os.environ))
    lines = [l for l in done.stdout.splitlines() if l.strip()]
    assert len(lines) == 7 and all(l.endswith(' 0') for l in lines), done.stdout + done.stderr


def test_the_doc_bars_exactly_the_five_without_a_restore():
    doc = (ROOT / 'docs/TOOLING.md').read_text(encoding='utf-8')
    line = next(l for l in doc.splitlines() if l.startswith('- **Not to be run:**'))
    barred = line[:line.index('They report')]
    for name in SEVEN[:5]:
        assert f'`{name}`' in barred, name
    for name in SEVEN[5:]:
        assert name not in barred and f'`{name}`' in line, name
    # Each of the five really has no restore, which is the reason given; when one gains it, its bar should lift.
    for name in SEVEN[:5]:
        assert 'Restore-' not in text(name), name


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
