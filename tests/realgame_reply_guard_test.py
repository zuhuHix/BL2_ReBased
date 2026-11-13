"""A static guard on the real-game tools (no game, nothing run): every capture script reaches the in-game driver only through the shared
reply wrapper of tools/real_game/realgame.ps1 (Invoke-GameReply, Invoke-GameReplyFile), where the driver-lost rule lives: once a reply never
came, nothing more is sent, and a transport failure sets the condition. Before the session gate's review of 2026-11-13 each capture had its
own wrapper, and one of them (phaselock_integrity_capture's Py) ignored the condition (B2).

The rule, per capture script (every tools/real_game/*.ps1 but realgame.ps1, which holds the transport and the wrapper, and pending.ps1,
which sends nothing to the driver):
- no call of the raw transport, Invoke-GamePy or Invoke-GamePyFile, not even for a command whose answer it drops: a probe load or a quit
  that times out must set the condition too;
- no reply wrapper of its own: no call of ConvertTo-ReplyText, no definition of the wrapper's pieces, and no write to the condition.
A capture may keep a short helper (Py, PyJson) for readability; with no raw transport call it can reach the game only through the shared
wrapper, so the refusal and the latching cannot be left out. A script that cannot follow the rule yet goes into EXCEPTIONS with its reason
(as OW_OUTSIDE_CTEST does in tests/CMakeLists.txt); an entry that no longer breaks the rule fails the test, so the list cannot go stale.

Run: python tests/realgame_reply_guard_test.py
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT / 'tools' / 'real_game'
NOT_CAPTURES = {'realgame.ps1', 'pending.ps1'}
# script file name -> why it may break the rule for now. Empty since every capture was converted (2026-11-13).
EXCEPTIONS = {}

RAW = re.compile(r'\bInvoke-GamePy(File)?\b')
OWN_WRAPPER = re.compile(r'\bConvertTo-ReplyText\b'
                         r'|^\s*function\s+(Invoke-GameReply|Invoke-GameReplyFile|ConvertTo-ReplyText|Set-SliceWatchDriverLost|Get-DriverLostReason'
                         r'|Assert-DriverAnswers|Assert-InputAllowed)\b'
                         r'|\$(script:)?SliceWatchDriverLost\s*=(?!=)', re.IGNORECASE)


def code_lines(text):
    """(number, line) of each line that is not a whole-line comment. An inline comment is scanned too: a name there is written out of the
    code's way rather than taken on trust."""
    for number, line in enumerate(text.replace('\r\n', '\n').split('\n'), 1):
        if not line.lstrip().startswith('#'):
            yield number, line


def breaches(text):
    found = []
    for number, line in code_lines(text):
        if RAW.search(line):
            found.append((number, 'raw transport', line.strip()[:120]))
        if OWN_WRAPPER.search(line):
            found.append((number, 'own reply wrapper', line.strip()[:120]))
    return found


def captures():
    return sorted(p for p in TOOLS.glob('*.ps1') if p.name not in NOT_CAPTURES)


class ReplyGuardTest(unittest.TestCase):
    def test_every_capture_reaches_the_driver_only_through_the_shared_wrapper(self):
        bad = {}
        for script in captures():
            if script.name in EXCEPTIONS:
                continue
            found = breaches(script.read_text(encoding='utf-8'))
            if found:
                bad[script.name] = found
        self.assertEqual(bad, {}, 'a capture script talks to the driver past the shared wrapper (Invoke-GameReply, realgame.ps1)')

    def test_each_exception_still_needs_its_entry_and_gives_a_reason(self):
        names = {p.name for p in captures()}
        for name, reason in EXCEPTIONS.items():
            self.assertIn(name, names, f'{name}: no such capture script')
            self.assertTrue(reason.strip(), f'{name}: an exception needs its reason')
            self.assertTrue(breaches((TOOLS / name).read_text(encoding='utf-8')), f'{name} follows the rule now: remove its exception')

    def test_the_scan_reaches_the_captures_and_the_wrapper_is_where_it_says(self):
        self.assertGreaterEqual(len(captures()), 19)   # 18 captures and slice_watch.ps1 on 2026-11-13; a new one is scanned as it comes
        for name in ('cash_capture.ps1', 'phaselock_integrity_capture.ps1', 'audio_events_capture.ps1', 'mission_screen_capture.ps1',
                     'slice_watch.ps1', 'walking_capture.ps1'):
            self.assertIn(TOOLS / name, captures())
            self.assertIn('Invoke-GameReply', (TOOLS / name).read_text(encoding='utf-8'), name)
        shared = (TOOLS / 'realgame.ps1').read_text(encoding='utf-8')
        for name in ('Invoke-GameReply', 'Invoke-GameReplyFile', 'Set-SliceWatchDriverLost', 'Assert-DriverAnswers', 'Assert-InputAllowed'):
            self.assertRegex(shared, rf'(?m)^function {name}\b', name)

    def test_the_scan_tells_a_breach_from_a_clean_line(self):
        # Synthetic lines, so the scan is shown to work even once no real script breaks the rule.
        for line in ("function Py([string]$Code) { ConvertTo-ReplyText (Invoke-GamePy $Code 30) $Code }",
                     "Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/x.py') x | Out-Null",
                     "try { Invoke-GamePy 'quit' 10 | Out-Null } catch { }",
                     "catch { $script:SliceWatchDriverLost = \"$_\"; throw }",
                     "function Set-SliceWatchDriverLost($Err) { }",
                     "$reply = ConvertTo-ReplyText $raw $Code"):
            self.assertTrue(breaches(line), line)
        for line in ("function Py([string]$Code, [int]$Timeout = 30) { Invoke-GameReply $Code $Timeout }",
                     "Invoke-GameReplyFile (Join-Path $PSScriptRoot 'scripts/x.py') x -AllowEmpty | Out-Null",
                     "if ($script:SliceWatchDriverLost) { return }",
                     "if ($script:SliceWatchDriverLost -eq '') { }",
                     "    # Invoke-GamePy is the raw transport (a whole-line comment)"):
            self.assertFalse(breaches(line), line)


if __name__ == '__main__':
    unittest.main()
