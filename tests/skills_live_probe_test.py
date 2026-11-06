"""Tests for the skills live capture without the game (invented objects, no game data).

The probe tools/real_game/scripts/skills_live.py runs on a stand-in for pyunrealsdk's `unrealsdk`, shaped by how the probe's first
run failed: GetSkillState needs its out struct and answers (found, state). The capture's step runner is taken out of
tools/real_game/skills_live_capture.ps1 and run with a stand-in for the game's reply. These check the helpers' logic only, not that
the game behaves as assumed. Run: python tests/skills_live_probe_test.py
"""
import json
import shutil
import subprocess
import sys
import tempfile
import types
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROBE = ROOT / 'tools' / 'real_game' / 'scripts' / 'skills_live.py'
CAPTURE = ROOT / 'tools' / 'real_game' / 'skills_live_capture.ps1'
REALGAME = ROOT / 'tools' / 'real_game' / 'realgame.ps1'


class Struct:
    def __init__(self, **fields):
        self.__dict__.update(fields)


class UObject:
    def _path_name(self):
        return getattr(self, 'path', 'Pkg.Object')


class Definition(UObject):
    def __init__(self, path):
        self.path, self.Name = path, path.split('.')[-1]


class Tree:
    def __init__(self, grades):
        self.grades = grades

    def GetSkillState(self, defn, out):  # the first run called it without `out`: a TypeError
        out.SkillGrade = self.grades.get(defn.path, 0)
        return defn.path in self.grades, out


class Field:
    def __init__(self, name, kind='IntProperty'):
        self.Name, self.Class = name, Struct(Name=kind)


class ControllerClass:
    SuperField = None

    def _fields(self):
        return [Field('CurrentInstantHitCriticalHitBonus'), Field('CurrentInstantHitCriticalHitBonusBaseValue'),
                Field('InstantHitCriticalHitBonusModifierStack', 'ArrayProperty'), Field('StartFire', 'Function'), Field('Other')]


class RunningSkill:
    """A running instance: its grade is separate from the tree's, as in the game (Skill.UpdateGrade keeps it at 1 or more)."""
    def __init__(self, definition, grade, state=1):
        self.Definition, self.Grade, self.SkillState = definition, grade, state


class Manager:
    def __init__(self, unreadable=False, deactivates=True):
        self.running, self.unreadable, self.deactivates = [], unreadable, deactivates

    @property
    def ActiveSkills(self):
        if self.unreadable:
            raise AttributeError('no such list')
        return self.running

    def DeactivateSkill(self, _instigator, defn):
        if self.deactivates:
            for skill in self.running:
                if skill.Definition == defn:
                    skill.SkillState = 0


class Controller(UObject):
    Class = ControllerClass()

    def __init__(self, tree, apply_grades=True):
        self.PlayerSkillTree, self.apply_grades = tree, apply_grades
        self.manager = Manager()
        self.calls = []
        self.CurrentInstantHitCriticalHitBonus = 4.5
        self.CurrentInstantHitCriticalHitBonusBaseValue = 2.0
        self.InstantHitCriticalHitBonusModifierStack = [Struct(Type='MT_Scale', Value=0.15)]
        self.Pawn = Struct(Weapon=Struct(Name='Gun_1'))
        self.WorldInfo = Struct(TimeSeconds=12.5)

    def GetSkillManager(self):
        return self.manager

    def ServerSetSkillSaveGameData(self, defn, grade):
        self.calls.append((defn.path, grade))
        if self.apply_grades:
            self.PlayerSkillTree.grades[defn.path] = grade

    def StartFire(self, mode):
        self.calls.append(('StartFire', mode))

    def StopFire(self, mode):
        self.calls.append(('StopFire', mode))


class Pawn(UObject):
    def __init__(self, name, health=100.0, radius=130, cls='CharClass_PrimalBeast', controller=True, deleted=False):
        self.Name, self.health, self.bDeleteMe = name, health, deleted
        self.AIClass = Struct(_path_name=lambda: 'Pkg.' + cls)
        self.CylinderComponent = Struct(CollisionRadius=radius)
        self.Controller = object() if controller else None
        self.Location = Struct(X=0.0, Y=0.0, Z=0.0)

    def GetHealth(self):
        return self.health

    def TakeDamage(self, amount, *_rest):
        self.health = max(0.0, self.health - amount)


class Corpse(Pawn):
    @property
    def AIClass(self):
        raise AttributeError('the corpse cannot be read')

    @AIClass.setter
    def AIClass(self, _value):
        pass


def load_probe(pc, pawns, definitions=()):
    sdk = types.ModuleType('unrealsdk')
    sdk.make_struct = lambda name, **fields: Struct(**fields)
    sdk.find_all = lambda name, exact=True: list(definitions)
    sdk.find_class = lambda name: object()
    sdk.find_object = lambda cls, path: next((d for d in definitions if d.path == path), None)
    hooks = types.ModuleType('unrealsdk.hooks')
    hooks.Type = Struct(PRE=0, POST_UNCONDITIONAL=1)
    hooks.add_hook = hooks.remove_hook = lambda *a: None
    unreal = types.ModuleType('unrealsdk.unreal')
    unreal.UObject = UObject
    sys.modules.update({'unrealsdk': sdk, 'unrealsdk.hooks': hooks, 'unrealsdk.unreal': unreal})

    def ai_pawn(name):
        if name not in pawns:
            raise LookupError(name)
        return pawns[name]

    namespace = {'get_pc': lambda: pc, 'ai_pawn': ai_pawn, 'lift_skill': lambda: None,
                 'list_ai': lambda: [(i, n, 'WillowAIPawn', bool(p.bDeleteMe)) for i, (n, p) in enumerate(pawns.items())]}
    exec(compile(PROBE.read_text(encoding='utf-8'), str(PROBE), 'exec'), namespace)
    return namespace


def test_grades_read_back_and_restore():
    helios = Definition('Pkg.Cataclysm.Helios')
    tree = Tree({helios.path: 2})
    pc = Controller(tree)
    probe = load_probe(pc, {}, [helios])
    assert probe['_read_grade'](tree, helios) == {'grade': 2}
    row = probe['sl_set_grades']({'Helios': 5})['rows'][0]
    assert row['status'] == 'set' and row['before'] == {'grade': 2} and row['after'] == {'grade': 5}
    assert pc.calls == [(helios.path, 5)], 'the grade goes through the save-load path'
    restored = probe['sl_restore_grades']()['rows'][0]
    assert restored['status'] == 'restored' and tree.grades[helios.path] == 2
    assert probe['SL']['originals'] == {}


def test_grade_not_confirmed_stops_the_step():
    helios = Definition('Pkg.Cataclysm.Helios')
    tree = Tree({helios.path: 2})
    probe = load_probe(Controller(tree, apply_grades=False), {}, [helios])
    row = probe['sl_set_grades']({'Helios': 5})['rows'][0]
    assert row['status'] == 'not confirmed' and row['after'] == {'grade': 2}, row


def test_unreadable_original_is_refused():
    helios = Definition('Pkg.Cataclysm.Helios')

    class BrokenTree(Tree):
        def GetSkillState(self, defn, out):
            raise TypeError('no state')

    pc = Controller(BrokenTree({helios.path: 2}))
    probe = load_probe(pc, {}, [helios])
    row = probe['sl_set_grades']({'Helios': 5})['rows'][0]
    assert row['status'] == 'original not readable', row
    assert pc.calls == [], 'the grade was not touched'
    assert probe['SL']['originals'] == {}


def test_restore_is_read_back():
    helios = Definition('Pkg.Cataclysm.Helios')
    tree = Tree({helios.path: 2})
    pc = Controller(tree)
    probe = load_probe(pc, {}, [helios])
    probe['sl_set_grades']({'Helios': 5})
    pc.apply_grades = False  # the restoring call does nothing
    row = probe['sl_restore_grades']()['rows'][0]
    assert row['status'] == 'not restored' and row['after'] == {'grade': 5}, row
    assert helios.path in probe['SL']['originals'], 'kept for another try'


def _restore_with(tree_grade, running_grade, **manager):
    """Helios original `tree_grade`, set to 5 with a running instance, then restored; the running grade is left at running_grade."""
    helios = Definition('Pkg.Cataclysm.Helios')
    tree = Tree({helios.path: tree_grade})
    pc = Controller(tree)
    pc.manager = Manager(**manager)
    probe = load_probe(pc, {}, [helios])
    probe['sl_set_grades']({'Helios': 5})
    pc.manager.running = [RunningSkill(helios, running_grade)]
    return probe['sl_restore_grades']()['rows'][0], tree, helios, pc, probe


def test_tree_back_at_zero_but_skill_still_running():
    row, tree, helios, _pc, probe = _restore_with(0, 1, deactivates=False)
    assert tree.grades[helios.path] == 0, 'the tree alone reads as restored'
    assert row['status'] == 'not restored' and row['running'] == [1], row
    assert helios.path in probe['SL']['originals']


def test_running_skill_is_deactivated_at_zero():
    row, tree, helios, pc, _probe = _restore_with(0, 1)
    assert row['status'] == 'restored' and row['running'] == [], row
    assert pc.manager.running[0].SkillState == 0


def test_running_grade_must_match_above_zero():
    assert _restore_with(3, 5)[0]['status'] == 'not restored'
    assert _restore_with(3, 3)[0]['status'] == 'restored'


def test_unreadable_running_state_is_not_restored_as_if_covered():
    row, *_ = _restore_with(0, 1, unreadable=True)
    assert row['status'] == 'tree only' and row['running'] is None, row


def test_unexpected_active_reports_another_steps_skill():
    helios, ruin = Definition('Pkg.Cataclysm.Helios'), Definition('Pkg.Cataclysm.Ruin')
    eye = Definition('Pkg.Harmony.MindsEye')
    tree = Tree({helios.path: 0, ruin.path: 0, eye.path: 3})
    pc = Controller(tree)
    pc.manager.running = [RunningSkill(eye, 3), RunningSkill(Definition('Pkg.NotInTheTree'), 1)]
    probe = load_probe(pc, {}, [helios, ruin, eye])
    assert probe['sl_mark_baseline']() == {'readable': True, 'skills': {eye.path: 3}}
    assert probe['sl_unexpected_active'](['Ruin']) == {'readable': True, 'unexpected': []}
    pc.manager.running.append(RunningSkill(helios, 1))
    pc.manager.running.append(RunningSkill(ruin, 1))
    assert probe['sl_unexpected_active'](['Ruin'])['unexpected'] == [helios.path], 'this step\'s own skill is expected'
    pc.manager.unreadable = True
    assert probe['sl_unexpected_active'](['Ruin'])['readable'] is False


def test_bullymongs_leave_out_a_corpse():
    pawns = {'P1': Pawn('P1'), 'P2': Corpse('P2'), 'P3': Pawn('P3', health=1.0), 'P4': Pawn('P4', radius=50),
             'P5': Pawn('P5', controller=False), 'P6': Pawn('P6', deleted=True), 'P7': Pawn('P7')}
    out = load_probe(Controller(Tree({})), pawns)['sl_bullymongs'](2)
    assert out['picks'] == ['P1', 'P7'], out
    assert [s['name'] for s in out['skipped']] == ['P2']


def test_wound_fire_and_dead():
    pawns = {'P1': Pawn('P1', health=250.0)}
    pc = Controller(Tree({}))
    probe = load_probe(pc, pawns)
    assert probe['sl_is_dead']('P1') is False
    assert probe['sl_wound']('P1') == {'victim': 'P1', 'before': 250.0, 'after': 1.0}
    probe['sl_fire'](True)
    probe['sl_fire'](False)
    assert pc.calls == [('StartFire', 0), ('StopFire', 0)]
    pawns['P1'].health = 0.0
    assert probe['sl_is_dead']('P1') is True
    assert probe['sl_is_dead']('gone') is True, 'a pawn that no longer exists is dead'


def test_crit_bonus_reads_the_stack():
    out = load_probe(Controller(Tree({})), {})['sl_crit_bonus']()
    assert out['values']['CurrentInstantHitCriticalHitBonus'] == 4.5
    assert out['values']['InstantHitCriticalHitBonusModifierStack'] == [{'type': 'MT_Scale', 'value': 0.15}]
    assert 'StartFire' not in out['values'] and 'Other' not in out['values']
    assert out['weapon'] == 'Gun_1'


def test_maya_health_set_and_sampled():
    pc = Controller(Tree({}))
    pc.Pawn = Pawn('Maya', health=200.0)
    pc.Pawn.GetMaxHealth = lambda: 400.0
    pc.Pawn.SetHealth = lambda value: setattr(pc.Pawn, 'health', value)
    probe = load_probe(pc, {})
    out = probe['sl_maya_health'](0.25)
    assert (out['before'], out['max'], out['after']) == (200.0, 400.0, 100.0), out
    watch = {'t0': 0.0, 'last': -1.0, 'rows': []}
    probe['_sample_maya_health'](watch, 1.0)
    probe['_sample_maya_health'](watch, 1.1)
    pc.Pawn.health = 115.0
    probe['_sample_maya_health'](watch, 1.2)
    assert [r['health'] for r in watch['rows']] == [100.0, 115.0], watch['rows']


def test_target_health_rows_only_on_change():
    pawns = {'P1': Pawn('P1')}
    probe = load_probe(Controller(Tree({})), pawns)
    watch = {'t0': 0.0, 'last': -1.0, 'rows': []}
    sample = probe['_sample_target_health']('P1')
    sample(watch, 1.0)
    sample(watch, 1.1)
    pawns['P1'].health = 60.0
    sample(watch, 1.11)  # inside 0.05 s of the last sample: not read yet
    sample(watch, 1.2)
    assert [r['health'] for r in watch['rows']] == [100.0, 60.0], watch['rows']


# The capture's own functions, taken out of the script by the PowerShell parser, with the game's reply replaced: every step's first
# Python call answers with a traceback.
RUNNER = r'''
$ErrorActionPreference = 'Stop'
$errors = $null; $tokens = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile('%CAPTURE%', [ref]$tokens, [ref]$errors)
foreach ($f in $ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true)) {
    if ($f.Name -in 'PyJson', 'PyCall', 'Invoke-Step', 'Copy-VerifiedRoot', 'Get-SavesVerdict', 'Get-SessionComplete') { . ([scriptblock]::Create($f.Extent.Text.Replace('function ', 'function global:'))) }
}
function global:Py([string]$Code, [int]$Timeout = 30) {
    if ($Code -match 'broken') { return "Traceback (most recent call last):`n  LookupError: WillowAIPawn_9" }
    if ($global:BadRestore -and $Code -match 'sl_restore_grades') { return '{"rows":[{"path":"P","status":"not restored"}]}' }
    if ($global:TreeOnly -and $Code -match 'sl_restore_grades') { return '{"rows":[{"path":"P","status":"tree only"}]}' }
    if ($global:Mixed -and $Code -match 'sl_unexpected_active') { return '{"readable":true,"unexpected":["Pkg.Helios"]}' }
    return '{}'
}
$Out = '%OUT%'
$script:FailedInARow = 0
$summary = [ordered]@{ steps = @() }
$script:ran = @()
Invoke-Step 'good' @{} { $script:ran += 'good'; PyCall 'fine()' }
Invoke-Step 'bad1' @{} { $script:ran += 'bad1'; PyCall 'broken()'; $script:ran += 'bad1_went_on' }
Invoke-Step 'recovers' @{} { $script:ran += 'recovers'; PyCall 'fine()' }
Invoke-Step 'bad2' @{} { $script:ran += 'bad2'; PyCall 'broken()' }
Invoke-Step 'bad3' @{} { $script:ran += 'bad3'; PyCall 'broken()' }
Invoke-Step 'skipped' @{} { $script:ran += 'skipped' }
$script:FailedInARow = 0
$Steps = @('picked')
Invoke-Step 'not_picked' @{} { $script:ran += 'not_picked' }
Invoke-Step 'picked' @{} { $script:ran += 'picked' }
$Steps = @()
$global:BadRestore = $true
Invoke-Step 'grade_stuck' @{} { $script:ran += 'grade_stuck' }
$global:BadRestore = $false
Invoke-Step 'after_stuck' @{} { $script:ran += 'after_stuck' }
$script:GradesBroken = $false
$global:Mixed = $true
Invoke-Step 'mixed' @{} { $script:ran += 'mixed' }
$global:Mixed = $false
$global:TreeOnly = $true
Invoke-Step 'tree_only' @{} { $script:ran += 'tree_only' }
$global:TreeOnly = $false
Invoke-Step 'after_tree_only' @{} { $script:ran += 'after_tree_only' }
$unknownAfter = $script:RunningUnknown
$script:RunningUnknown = $false
# Complete needs every requested step passed and a clean cleanup.
$cleanOk = @{ saves = 'identical to B after 2 restores'; driver = 'removed' }
$passed = @([pscustomobject]@{ step = 'a'; ok = $true; possibly_contaminated = $false })
$noError = [ordered]@{ started = 'x' }
$complete = [ordered]@{
    good = Get-SessionComplete $noError $passed @() $cleanOk $true $false $false
    step_failed = Get-SessionComplete $noError @([pscustomobject]@{ step = 'a'; ok = $false; possibly_contaminated = $false }) @() $cleanOk $true $false $false
    contaminated = Get-SessionComplete $noError @([pscustomobject]@{ step = 'a'; ok = $true; possibly_contaminated = $true }) @() $cleanOk $true $false $false
    no_steps = Get-SessionComplete $noError @() @() $cleanOk $true $false $false
    requested_missing = Get-SessionComplete $noError $passed @('a', 'typo') $cleanOk $true $false $false
    driver_left = Get-SessionComplete $noError $passed @() @{ saves = 'identical to B'; driver = 'STILL PRESENT: D' } $true $false $false
    saves_differ = Get-SessionComplete $noError $passed @() @{ saves = 'NOT IDENTICAL to B'; driver = 'removed' } $true $false $false
    game_running = Get-SessionComplete $noError $passed @() $cleanOk $false $false $false
    run_error = Get-SessionComplete ([ordered]@{ error = 'x' }) $passed @() $cleanOk $true $false $false
    grades_broken = Get-SessionComplete $noError $passed @() $cleanOk $true $true $false
    running_unknown = Get-SessionComplete $noError $passed @() $cleanOk $true $false $true
}
# The backup guard, on real folders with the real hash compare.
$realgame = [System.Management.Automation.Language.Parser]::ParseFile('%REALGAME%', [ref]$tokens, [ref]$errors)
foreach ($f in $realgame.FindAll({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true)) {
    if ($f.Name -eq 'Compare-TreeHashes') { . ([scriptblock]::Create($f.Extent.Text.Replace('function ', 'function global:'))) }
}
$src = Join-Path $Out 'src'; New-Item -ItemType Directory -Force (Join-Path $src 'a') | Out-Null; Set-Content (Join-Path $src 'a\f.txt') 'one'
Copy-VerifiedRoot $src (Join-Path $Out 'good')
$partial = Join-Path $Out 'partial'; New-Item -ItemType Directory -Force $partial | Out-Null
$guard = 'no error'; try { Copy-VerifiedRoot $src $partial } catch { $guard = "$_" }
$created = Get-SavesVerdict @([pscustomobject]@{ file = 'SanitySaver'; status = 'created during the run; left for the maintainer' }) 'BAK'
$clean = Get-SavesVerdict @([pscustomobject]@{ file = 'x'; status = 'changed, restored' }) 'BAK'
$failed = Get-SavesVerdict @([pscustomobject]@{ file = 'Config'; status = 'BACKUP FAILED; not restored, and nothing was started' }) 'BAK'
[ordered]@{ ran = $script:ran; steps = $summary.steps; guard = $guard; created = $created; clean = $clean; failed = $failed; complete = $complete; unknown_after = $unknownAfter } | ConvertTo-Json -Depth 5 -Compress
'''


def test_step_runner_keeps_the_traceback_and_stops():
    shell = shutil.which('powershell') or shutil.which('pwsh')
    if not shell:
        print('(no PowerShell: the step runner check was not run)')
        return
    with tempfile.TemporaryDirectory() as out:
        script = Path(out) / 'runner.ps1'
        script.write_text(RUNNER.replace('%CAPTURE%', str(CAPTURE)).replace('%REALGAME%', str(REALGAME)).replace('%OUT%', out), encoding='utf-8-sig')
        done = subprocess.run([shell, '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)],
                              capture_output=True, text=True, timeout=120)
        assert done.returncode == 0, done.stderr
        result = json.loads(done.stdout.strip().splitlines()[-1])
        record = json.loads((Path(out) / 'bad1.json').read_text(encoding='utf-8-sig'))
    assert result['ran'] == ['good', 'bad1', 'recovers', 'bad2', 'bad3', 'picked', 'grade_stuck', 'mixed', 'tree_only', 'after_tree_only'], result['ran']
    assert result['unknown_after'] is True, 'tree only leaves the session marked'
    assert result['complete'].pop('good') is True
    assert not any(result['complete'].values()), result['complete']
    assert 'differs from the original' in result['guard'], result['guard']
    assert result['created'].startswith('NOT IDENTICAL') and 'SanitySaver' in result['created'], result['created']
    assert result['failed'].startswith('NOT IDENTICAL') and 'Config' in result['failed'], result['failed']
    assert result['clean'].startswith('identical'), result['clean']
    assert 'not_picked' not in {s['step'] for s in result['steps']}, '-Steps leaves the others out'
    by_step = {s['step']: s for s in result['steps']}
    assert record['ok'] is False and 'python: Traceback' in record['error'] and 'LookupError: WillowAIPawn_9' in record['error'], record
    assert by_step['good']['ok'] is True and by_step['recovers']['ok'] is True, 'a success resets the count'
    assert by_step['skipped']['error'] == 'not run: the two steps before it failed'
    assert by_step['after_stuck']['error'] == 'not run: a grade from an earlier step did not restore'
    assert by_step['mixed']['ok'] is True and by_step['mixed']['possibly_contaminated'] is True
    assert by_step['after_tree_only']['possibly_contaminated'] is True, 'after a tree-only restore every reading is in doubt'
    assert by_step['good']['possibly_contaminated'] is False


def test_capture_tolerates_the_quit_reply():
    text = CAPTURE.read_text(encoding='utf-8')
    assert 'if ($log.complete -eq $true) { exit 0 } else { exit 1 }' in text, 'exit 0 only when complete'
    assert """try { Invoke-GamePy 'protect_player(False); get_pc().ConsoleCommand("quit", False)' 10 | Out-Null }""" in text
    assert """    Py 'protect_player(False); get_pc().ConsoleCommand("quit", False)'""" not in text, 'the bare call has no reply to trim'


def test_one_target_is_a_whole_name():
    """The rerun's LookupError "W": a function's one-item array unrolls to a string, and [0] then takes its first letter."""
    text = CAPTURE.read_text(encoding='utf-8')
    assert '(Get-Bullymongs 1)[0]' in text and '$t = (Get-Bullymongs' not in text
    shell = shutil.which('powershell') or shutil.which('pwsh')
    if not shell:
        return
    code = ("function Get-Bullymongs([int]$Count) { $r = [pscustomobject]@{ picks = @('WillowAIPawn_9') }; @($r.picks) }; "
            "@(Get-Bullymongs 1)[0]")
    done = subprocess.run([shell, '-NoProfile', '-Command', code], capture_output=True, text=True, timeout=60)
    assert done.stdout.strip() == 'WillowAIPawn_9', done.stdout


if __name__ == '__main__':
    tests = [v for k, v in sorted(globals().items()) if k.startswith('test_')]
    for test in tests:
        test()
    print(f'{len(tests)} skills live probe tests passed')
