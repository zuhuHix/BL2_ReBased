"""The suite and capture scripts take their -Engine default from OPENWILLOW_UE when it is set.

AI-assisted (Claude Opus 5.5), 2026-11-12. Synthetic: no editor and no script is run. PowerShell parses each script
(System.Management.Automation.Language.Parser) and evaluates only the default expression of its -Engine parameter,
once with OPENWILLOW_UE pointing at an invented folder and once with it unset. Parsing launches nothing, unlike running
a script up to its "UnrealEditor.exe not found" stop, which would first dot-source helpers and, in some scripts, take the
run lock or start a page server. Windows PowerShell or pwsh needed; the static checks run without it.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
POWERSHELL = shutil.which('pwsh') or shutil.which('powershell')
DEFAULT = r'C:\Program Files\Epic Games\UE_5.8'
INVENTED = r'X:\nowhere\Invented UE'
# The ten scripts that used a fixed path or a mandatory -Engine until 2026-11-12, and the gate script whose expression they copy.
SCRIPTS = ['test_quest', 'test_mover', 'test_audio', 'test_action_skill_hud', 'test_hud_minimap', 'run_hud_shots', 'test_ambient',
           'run_quest', 'capture_firing_frames', 'test_ue_viewer', 'test_slice_gate']

EVAL = r'''
$out = [ordered]@{}
foreach ($path in $args) {
    $tokens = $null; $errors = $null
    $ast = [System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors)
    if ($errors.Count) { $out[$path] = "PARSE ERROR: $($errors[0].Message)"; continue }
    $p = @($ast.ParamBlock.Parameters | Where-Object { $_.Name.VariablePath.UserPath -eq 'Engine' })
    if (!$p.Count) { $out[$path] = 'NO ENGINE PARAMETER'; continue }
    if ($null -eq $p[0].DefaultValue) { $out[$path] = 'NO DEFAULT'; continue }
    $out[$path] = [string](& ([scriptblock]::Create($p[0].DefaultValue.Extent.Text)))
}
$out | ConvertTo-Json -Compress
'''


def paths():
    return [str(ROOT / 'tools' / f'{name}.ps1') for name in SCRIPTS]


class StaticTest(unittest.TestCase):
    def test_no_mandatory_engine_and_one_expression(self):
        gate = (ROOT / 'tools' / 'test_slice_gate.ps1').read_text(encoding='utf-8-sig').splitlines()[1].strip()
        self.assertIn('$env:OPENWILLOW_UE', gate)
        for path in paths():
            with self.subTest(script=Path(path).name):
                text = Path(path).read_text(encoding='utf-8-sig')
                self.assertNotRegex(text, r'Mandatory[^\n]*\]\s*\[string\]\$Engine\b')
                self.assertIn(gate, [line.strip() for line in text.splitlines()])


@unittest.skipUnless(POWERSHELL, 'PowerShell not installed')
class DefaultTest(unittest.TestCase):
    def defaults(self, ue):
        env = dict(os.environ)
        env.pop('OPENWILLOW_UE', None)
        if ue is not None:
            env['OPENWILLOW_UE'] = ue
        done = subprocess.run([POWERSHELL, '-NoProfile', '-NonInteractive', '-Command', '& {' + EVAL + '}', *paths()],
                              capture_output=True, text=True, env=env, timeout=120)
        self.assertEqual(done.returncode, 0, done.stderr)
        return json.loads(done.stdout)

    def test_default_follows_the_variable(self):
        got = self.defaults(INVENTED)
        self.assertEqual(len(got), len(SCRIPTS))
        for path, value in got.items():
            with self.subTest(script=Path(path).name):
                self.assertEqual(value, INVENTED)

    def test_default_without_the_variable(self):
        for path, value in self.defaults(None).items():
            with self.subTest(script=Path(path).name):
                self.assertEqual(value, DEFAULT)


if __name__ == '__main__':
    unittest.main()
