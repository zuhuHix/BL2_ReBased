"""Tests for the skills live capture without the game (invented objects, no game data).

The probe tools/real_game/scripts/skills_live.py runs on a stand-in for pyunrealsdk's `unrealsdk`, shaped by how the probe's first
run failed: GetSkillState needs its out struct and answers (found, state). The capture's step runner is taken out of
tools/real_game/skills_live_capture.ps1 and run with a stand-in for the game's reply. These check the helpers' logic only, not that
the game behaves as assumed. Run: python tests/skills_live_probe_test.py
"""
import json
import os
import shutil
import subprocess
import realgame_test_env  # noqa: F401  (a fresh pending-record folder for every PowerShell run)
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
    if ($f.Name -in 'PyJson', 'PyCall', 'Invoke-Step', 'Get-StepsDone', 'Get-SessionComplete', 'Get-MissingEvidence', 'Test-Number', 'Get-TargetHits', 'Test-KilledByMaya', 'Get-CastEvidence', 'Get-TimerEvidence', 'Get-CritEvidence', 'Get-DiscoverEvidence') { . ([scriptblock]::Create($f.Extent.Text.Replace('function ', 'function global:'))) }
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
$passed = @([pscustomobject]@{ step = 'a'; ok = $true; possibly_contaminated = $false; missing = @() })
# The evidence gates, on invented rows. Each entry is what Get-MissingEvidence names.
function global:Rows([string]$Json) { @($Json | ConvertFrom-Json) }
function global:Missing($Evidence) { ,@(Get-MissingEvidence $Evidence | Sort-Object) }
$hit = '{"t":1.3,"via":"WillowAIPawn","victim":"T","damage":25.0,"damage_type":"WillowDmgSource_Skill","instigator":"PC"}'
$actor = '{"t":1.3,"via":"Actor","victim":"T","damage":"<AttributeError>","damage_type":"WillowDmgSource_Skill"}'
$full = '{"t":0.0,"event":"target_health","health":700.0}'
$less = '{"t":1.34,"event":"target_health","health":675.0}'
$gates = [ordered]@{}
$gates.cast_good = Missing (Get-CastEvidence (Rows "[$full,$actor,$hit,$less]") 'T' 'a' 'b')
$gates.cast_rejected_hit = Missing (Get-CastEvidence (Rows "[$full,$hit]") 'T' 'a' 'b')
$gates.cast_health_unchanged = Missing (Get-CastEvidence (Rows "[$full,$hit,$($less.Replace('675.0','700.0'))]") 'T' 'a' 'b')
$gates.cast_other_victim = Missing (Get-CastEvidence (Rows "[$full,$($hit.Replace('"T"','"U"')),$less]") 'T' 'a' 'b')
$gates.cast_not_a_skill = Missing (Get-CastEvidence (Rows "[$full,$($hit.Replace('_Skill','_Bullet')),$less]") 'T' 'a' 'b')
$gates.cast_fell_before_the_hit = Missing (Get-CastEvidence (Rows "[$full,$($less.Replace('1.34','0.5')),$($hit)]") 'T' 'a' 'b')
$gates.cast_no_new_cast = Missing (Get-CastEvidence (Rows "[$full,$hit,$less]") 'T' 'a' 'a')
$gun = Rows '[{"t":1.0,"via":"WillowAIPawn","victim":"T","damage":30.0,"damage_type":"WillowDmgSource_Bullet","instigator":"PC"}]'
$kill = @{ target = 'T'; dead = $true; hits = $gun; controller = 'PC' }
$timer = '[{"t":0,"remaining":0.0},{"t":1,"remaining":6.5},{"t":2,"event":"UpdateKillSkills"},{"t":8,"remaining":0.0}]'
$gates.timer_good = Missing (Get-TimerEvidence (Rows $timer) @($kill, $kill))
$gates.timer_all_zero = Missing (Get-TimerEvidence (Rows '[{"t":0,"remaining":0.0},{"t":1,"remaining":0.0}]') @($kill))
$gates.timer_marker = Missing (Get-TimerEvidence (Rows '[{"t":0,"remaining":"<AttributeError>"},{"t":1,"remaining":6.5},{"t":2,"remaining":0.0}]') @($kill))
$gates.timer_never_ran_out = Missing (Get-TimerEvidence (Rows '[{"t":0,"remaining":6.5},{"t":1,"remaining":5.5}]') @($kill))
$gates.timer_one_not_dead = Missing (Get-TimerEvidence (Rows $timer) @($kill, @{ target = 'T'; dead = $false; hits = $gun; controller = 'PC' }))
$gates.timer_other_killer = Missing (Get-TimerEvidence (Rows $timer) @(@{ target = 'T'; dead = $true; hits = $gun; controller = 'OtherPC' }))
$gates.timer_scripted_kill = Missing (Get-TimerEvidence (Rows $timer) @(@{ target = 'T'; dead = $true; controller = 'PC'; hits = (Rows '[{"t":1.0,"via":"WillowAIPawn","victim":"T","damage":1000000,"damage_type":"DamageType","instigator":"PC"}]') }))
$gates.timer_dead_no_hit = Missing (Get-TimerEvidence (Rows $timer) @(@{ target = 'T'; dead = $true; hits = @(); controller = 'PC' }))
$read = { param($v) [pscustomobject]@{ values = [pscustomobject]@{ CurrentInstantHitCriticalHitBonus = $v } } }
$set = { param($g, $before, $status) [pscustomobject]@{ status = 'done'; rows = @([pscustomobject]@{ status = $status; before = [pscustomobject]@{ grade = $before }; after = [pscustomobject]@{ grade = $g } }) } }
$gates.crit_good = Missing (Get-CritEvidence (& $read 2.3) (& $read 2.1) (& $read 2.5) (& $set 1 3 'set') (& $set 5 1 'set'))
$gates.crit_set_failed = Missing (Get-CritEvidence (& $read 2.3) (& $read 2.1) (& $read 2.5) (& $set 1 3 'error') (& $set 5 1 'set'))
$gates.crit_readback_differs = Missing (Get-CritEvidence (& $read 2.3) (& $read 2.1) (& $read 2.5) (& $set 1 3 'set') (& $set 4 1 'set'))
$gates.crit_flat_bonus = Missing (Get-CritEvidence (& $read 2.3) (& $read 2.3) (& $read 2.3) (& $set 1 3 'set') (& $set 5 1 'set'))
$gates.crit_flat_saved_at_an_end = Missing (Get-CritEvidence (& $read 2.3) (& $read 2.3) (& $read 2.3) (& $set 1 1 'set') (& $set 5 1 'set'))
$gates.crit_saved_not_between = Missing (Get-CritEvidence (& $read 9.0) (& $read 2.1) (& $read 2.5) (& $set 1 3 'set') (& $set 5 1 'set'))
$gates.crit_marker = Missing (Get-CritEvidence (& $read 2.3) (& $read '<AttributeError>') (& $read 2.5) (& $set 1 3 'set') (& $set 5 1 'set'))
$names = @('A', 'B', 'C')
$gates.discover_good = Missing (Get-DiscoverEvidence ('{"A":["x"],"B":["y"],"C":["z"]}' | ConvertFrom-Json) $names)
$gates.discover_one_empty = Missing (Get-DiscoverEvidence ('{"A":["x"],"B":[],"C":["z"]}' | ConvertFrom-Json) $names)
$gates.discover_one_absent = Missing (Get-DiscoverEvidence ('{"A":["x"],"B":["y"]}' | ConvertFrom-Json) $names)
# A step that ran and got nothing, through the real runner: the fourth run's shape (no kill, no error).
$script:FailedInARow = 0
Invoke-Step 'no_kills' @{} { @{ evidence = @{ both_kills_happened = $false; timer_sampled = $true } } }
Invoke-Step 'got_it' @{} { @{ evidence = @{ both_kills_happened = $true; timer_sampled = $true } } }
Invoke-Step 'no_claim' @{} { @{ something = 1 } }
$noKills = @($summary.steps | Where-Object { $_.step -eq 'no_kills' })
$gotIt = @($summary.steps | Where-Object { $_.step -eq 'got_it' })
$noClaim = @($summary.steps | Where-Object { $_.step -eq 'no_claim' })
$noError = [ordered]@{ started = 'x' }
$done_rows = @([pscustomobject]@{ step = 'good'; ok = $true; possibly_contaminated = $false; missing = @() },
               [pscustomobject]@{ step = 'failed'; ok = $false; possibly_contaminated = $false; missing = @() },
               [pscustomobject]@{ step = 'mixed'; ok = $true; possibly_contaminated = $true; missing = @() },
               [pscustomobject]@{ step = 'no_read'; ok = $true; possibly_contaminated = $false; missing = @('x') },
               [pscustomobject]@{ step = 'no_field'; ok = $true; possibly_contaminated = $false })
$completeWithEvidence = Get-SessionComplete $noError $gotIt @() $cleanOk $true $false $false
$complete = [ordered]@{
    good = Get-SessionComplete $noError $passed @() $cleanOk $true $false $false
    ran_but_no_kills = Get-SessionComplete $noError $noKills @() $cleanOk $true $false $false
    no_evidence = Get-SessionComplete $noError $noClaim @() $cleanOk $true $false $false
    missing_field_absent = Get-SessionComplete $noError @([pscustomobject]@{ step = 'a'; ok = $true; possibly_contaminated = $false }) @() $cleanOk $true $false $false
    step_failed = Get-SessionComplete $noError @([pscustomobject]@{ step = 'a'; ok = $false; possibly_contaminated = $false }) @() $cleanOk $true $false $false
    contaminated = Get-SessionComplete $noError @([pscustomobject]@{ step = 'a'; ok = $true; possibly_contaminated = $true }) @() $cleanOk $true $false $false
    no_steps = Get-SessionComplete $noError @() @() $cleanOk $true $false $false
    requested_missing = Get-SessionComplete $noError $passed @('a', 'typo') $cleanOk $true $false $false
    driver_left = Get-SessionComplete $noError $passed @() @{ saves = 'identical to B'; driver = 'STILL PRESENT: D' } $true $false $false
    saves_differ = Get-SessionComplete $noError $passed @() @{ saves = 'NOT IDENTICAL to B'; driver = 'removed' } $true $false $false
    game_running = Get-SessionComplete $noError $passed @() $cleanOk $false $false $false
    run_error = Get-SessionComplete ([ordered]@{ run_error = 'x' }) $passed @() $cleanOk $true $false $false
    grades_broken = Get-SessionComplete $noError $passed @() $cleanOk $true $true $false
    running_unknown = Get-SessionComplete $noError $passed @() $cleanOk $true $false $true
}
[ordered]@{ ran = $script:ran; steps = $summary.steps; complete = $complete; steps_done = @(Get-StepsDone $done_rows); complete_with_evidence = $completeWithEvidence; gates = $gates; unknown_after = $unknownAfter; no_kills = $noKills[0] } | ConvertTo-Json -Depth 5 -Compress
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
    wanted = {
        'cast_good': [], 'cast_rejected_hit': ['health_fell_after_the_hit'], 'cast_health_unchanged': ['health_fell_after_the_hit'], 'cast_fell_before_the_hit': ['health_fell_after_the_hit'],
        'cast_other_victim': ['health_fell_after_the_hit', 'skill_hit_on_target'],
        'cast_not_a_skill': ['health_fell_after_the_hit', 'skill_hit_on_target'], 'cast_no_new_cast': ['cast_started'],
        'timer_good': [], 'timer_all_zero': ['timer_ran', 'timer_ran_out'], 'timer_never_ran_out': ['timer_ran_out'],
        'timer_marker': ['timer_ran', 'timer_ran_out', 'timer_samples_numeric'], 'timer_one_not_dead': ['kills_by_maya'],
        'timer_other_killer': ['kills_by_maya'], 'timer_scripted_kill': ['kills_by_maya'], 'timer_dead_no_hit': ['kills_by_maya'],
        'crit_good': [], 'crit_set_failed': ['grade_1_set_and_read_back'], 'crit_readback_differs': ['grade_5_set_and_read_back'],
        'crit_flat_bonus': ['bonus_moves_with_the_grade'], 'crit_flat_saved_at_an_end': ['bonus_moves_with_the_grade'], 'crit_saved_not_between': ['bonus_moves_with_the_grade'],
        'crit_marker': ['bonus_moves_with_the_grade'],
        'discover_good': [], 'discover_one_empty': ['found_B'], 'discover_one_absent': ['found_C'],
    }
    got = {name: ([] if value is None else [value] if isinstance(value, str) else value) for name, value in result['gates'].items()}
    assert got == wanted, {k: (got.get(k), wanted.get(k)) for k in wanted if got.get(k) != wanted.get(k)}
    assert result['complete_with_evidence'] is True, 'a step with all its evidence can complete'
    assert result['complete']['ran_but_no_kills'] is False, 'the fourth run: every step ran, no kill happened, and it said complete'
    assert result['no_kills']['ok'] is True and result['no_kills']['missing'] == ['both_kills_happened'], result['no_kills']
    assert not any(result['complete'].values()), result['complete']
    assert result['steps_done'] == ['good'] or result['steps_done'] == 'good', result['steps_done']
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
    assert """try { Invoke-GamePy 'protect_player(False); get_pc().ConsoleCommand("quit", False)' 10 | Out-Null }""" in text
    assert """    Py 'protect_player(False); get_pc().ConsoleCommand("quit", False)'""" not in text, 'the bare call has no reply to trim'


def test_stop_all_answers_stopped_only_with_the_grades_back():
    helios = Definition('Pkg.Cataclysm.Helios')
    tree = Tree({helios.path: 2})
    pc = Controller(tree)
    probe = load_probe(pc, {}, [helios])
    probe['sl_set_grades']({'Helios': 5})
    assert probe['sl_stop_all']() == 'stopped' and tree.grades[helios.path] == 2
    assert probe['sl_stop_all']() == 'stopped', 'nothing left to restore'
    probe['sl_set_grades']({'Helios': 5})
    pc.apply_grades = False
    answer = probe['sl_stop_all']()
    assert answer.startswith('grades not restored') and 'not restored' in answer, answer


# The capture's own tail, from its catch to its end, after a body that fails or finishes, with the real helpers from realgame.ps1.
# Modelled on tests/realgame_snapshot_test.py, with this capture's steps and its evidence rule.
TAIL = r"""
$ErrorActionPreference = 'Stop'
. '__REALGAME__'
$errors = $null; $tokens = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile('__CAPTURE__', [ref]$tokens, [ref]$errors)
foreach ($f in $ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true)) {
    if ($f.Name -in 'Get-StepsDone', 'Get-SessionComplete') { . ([scriptblock]::Create($f.Extent.Text)) }
}
function Restore-VerifiedRoots {
    if (__CLEAN__) { [pscustomobject]@{ rows = @(); summary = 'identical to bak after 0 restores'; clean = $true } }
    else { [pscustomobject]@{ rows = @([pscustomobject]@{ file = 'SaveData\x'; status = 'MISMATCH after restore' });
                              summary = 'NOT IDENTICAL to bak (1 problems)'; clean = $false } }
}
$roots = [ordered]@{ SaveData = '__TMP__\live' }
$Out = '__OUT__'; $bak = '__TMP__'; $HubLock = '__TMP__\hub.lock'; $Lane = 'test'
Set-Content -LiteralPath $HubLock 'test now'
$log = [ordered]@{}; $snapshots = @{ SaveData = @{} }; $extraRoots = [ordered]@{}; $stepsRequested = @('a'); $runError = ''
$summary = [ordered]@{ steps = @() }; $script:GradesBroken = $false; $script:RunningUnknown = $false
function Pass([string]$Name = 'a', $Missing = @(), $Mixed = $false) { $summary.steps += [pscustomobject]@{ step = $Name; ok = $true; possibly_contaminated = $Mixed; missing = @($Missing) } }
__SETUP__
$hub = 'held'; $proc = __PROC__; $procStart = $null; $runLock = $false; $driverOwned = __OWNED__; $driverAttempted = __ATTEMPTED__; $launchAttempted = __LAUNCHED__; $marker = 'm'
function Get-GameProcesses { }
__STUBS__
try {
    __BODY__
__CAPTURE_TAIL__
"""
ALIVE = '[pscustomobject]@{ HasExited = $false; Id = 1 }'
EXITED = '([pscustomobject]@{ HasExited = $true; Id = 1 } | Add-Member -PassThru ScriptMethod Refresh { })'


def run_tail(body, clean=True, proc='$null', setup='', after=None, owned='$true', attempted=None, launched='$true', stubs='function Remove-Driver { }', out='__TMP__'):
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    tail = text[text.index('\n} catch {\n    $runError') + 1:]
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'driver.ps1'
        filled = (TAIL.replace('__OUT__', out).replace('__REALGAME__', str(REALGAME)).replace('__CAPTURE__', str(CAPTURE)).replace('__SETUP__', setup)
                  .replace('__LAUNCHED__', launched).replace('__OWNED__', owned).replace('__ATTEMPTED__', attempted or owned).replace('__PROC__', proc)
                  .replace('__STUBS__', stubs).replace('__BODY__', body).replace('__CAPTURE_TAIL__', tail).replace('__CLEAN__', '$true' if clean else '$false')
                  .replace('__TMP__', tmp))
        script.write_text(filled, encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True,
                              text=True, env=dict(os.environ, OPENWILLOW_BL2=tmp), timeout=180)
        done.hub_lock_kept = (Path(tmp) / 'hub.lock').exists()
        if after:
            after(tmp, done)
        return done


def powershell_here():
    if shutil.which('powershell'):
        return True
    print('(no Windows PowerShell: the capture tail checks were not run)')
    return False


def test_exit_0_only_for_a_completed_run_with_its_evidence_and_nothing_to_recover():
    if not powershell_here():
        return
    # (body, restore clean, game process, expected exit, expected status)
    for body, clean, proc, code, status in (('Pass', True, '$null', 0, 'completed'),
                                            ('Pass', True, EXITED, 0, 'completed'),
                                            ('Pass', False, '$null', 1, 'recovery needed: restore'),
                                            ("'no step'", True, '$null', 1, 'incomplete: a'),
                                            ("Pass 'other'", True, '$null', 1, 'incomplete: a'),
                                            ("Pass 'a' @('timer_ran')", True, '$null', 1, 'incomplete: a'),   # the fourth run: it ran, and read nothing
                                            ("Pass 'a' @() $true", True, '$null', 1, 'incomplete: a'),
                                            ('Pass; $script:GradesBroken = $true', True, '$null', 1, 'incomplete: grades or cleanup'),
                                            ('Pass; $script:RunningUnknown = $true', True, '$null', 1, 'incomplete: grades or cleanup'),
                                            ("Pass; throw 'x'", True, '$null', 1, 'failed: x'),
                                            ('Pass', True, ALIVE, 1, 'recovery needed: game exit')):
        done = run_tail(body, clean, proc)
        assert done.returncode == code and f'status: {status}' in done.stdout, (body, clean, proc, done.returncode, done.stdout, done.stderr)
        assert done.hub_lock_kept is status.startswith('recovery'), (body, clean, proc, done.stdout)
    # With no launch and no cleanup, all steps passing still says what is wrong rather than completed: the session rule needs a compared restore.
    done = run_tail('Pass', launched='$false')
    assert done.returncode == 1 and 'status: incomplete: grades or cleanup' in done.stdout, done.stdout


def test_a_failed_run_with_a_bad_restore_still_prints_the_rows_and_the_recovery():
    if not powershell_here():
        return
    for body in ("throw 'cooldown did not end'", 'Pass'):
        done = run_tail(body, clean=False)
        assert done.returncode == 1 and done.hub_lock_kept is True, done.stdout + done.stderr
        order = [done.stdout.find(text) for text in ('status: recovery needed: restore did not compare clean', 'MISMATCH after restore', 'MANUAL RECOVERY', 'Restore-VerifiedBackup')]
        assert -1 not in order and order == sorted(order), done.stdout


def test_a_leftover_driver_holds_the_locks_and_says_what_to_do():
    if not powershell_here():
        return
    make = "New-Item -ItemType Directory -Force (Join-Path $script:DriverDir 'x') | Out-Null"
    done = run_tail('Pass', setup=make)
    assert done.returncode == 1 and 'status: recovery needed: driver not removed' in done.stdout and done.hub_lock_kept is True, done.stdout
    assert 'MANUAL RECOVERY' in done.stdout and 'Remove-Item -Recurse -Force' in done.stdout, done.stdout
    done = run_tail('Pass', stubs="function Remove-Driver { throw 'in use' }")
    assert done.returncode == 1 and 'driver not removed (error: in use)' in done.stdout and done.hub_lock_kept is True, done.stdout


def test_a_game_this_run_did_not_start_stops_the_restore_and_holds_the_locks():
    if not powershell_here():
        return
    no_restore = "function Remove-Driver { Write-Host 'driver removed' }\nfunction Restore-VerifiedRoots { throw 'must not run' }\n"
    done = run_tail('Pass', stubs=no_restore + 'function Get-GameProcesses { [pscustomobject]@{ Id = 77 } }')
    assert done.returncode == 1 and 'status: recovery needed: a game this run did not start is running (pid 77)' in done.stdout, done.stdout
    assert done.hub_lock_kept is True and 'MANUAL RECOVERY' in done.stdout and 'NOT RESTORED' in done.stdout and 'driver removed' not in done.stdout, done.stdout
    # One that starts during the cleanup, after the first look.
    late = '$script:checks = 0\nfunction Get-GameProcesses { $script:checks++; if ($script:checks -gt 1) { [pscustomobject]@{ Id = 88 } } }'
    done = run_tail('Pass', stubs=no_restore + late)
    assert done.returncode == 1 and '(pid 88)' in done.stdout and done.hub_lock_kept is True and 'NOT RESTORED' in done.stdout, done.stdout


def test_a_run_that_launched_nothing_restores_nothing():
    if not powershell_here():
        return
    no_restore = "function Restore-VerifiedRoots { throw 'must not run' }"
    # The backup failed partway, before the driver or the game: nothing to recover, the locks released, and still not completed.
    done = run_tail("throw 'copy failed'", owned='$false', launched='$false', stubs=no_restore)
    assert done.returncode == 1 and 'status: failed: copy failed' in done.stdout and 'cleanup saves: not touched' in done.stdout, done.stdout
    assert done.hub_lock_kept is False and 'MANUAL RECOVERY' not in done.stdout, done.stdout
    # A failed install that left the folder: reported, never removed, and the locks kept.
    make = 'New-Item -ItemType Directory -Force $script:DriverDir | Out-Null'
    done = run_tail("throw 'install failed'", setup=make, owned='$false', attempted='$true', launched='$false', stubs="function Remove-Driver { throw 'must not run' }\n" + no_restore)
    assert done.returncode == 1 and 'status: recovery needed: driver not removed (STILL PRESENT after a failed install' in done.stdout and done.hub_lock_kept is True, done.stdout


def test_a_failed_run_stops_the_hooks_while_the_game_is_alive():
    if not powershell_here():
        return
    answer = "function Remove-Driver { }\nfunction Py([string]$Code, [int]$Timeout = 30) { Write-Host ('py: ' + $Code); __ANSWER__ }"
    done = run_tail("throw 'boom'", proc=ALIVE, stubs=answer.replace('__ANSWER__', "'stopped'"))
    assert 'py: print(sl_stop_all())' in done.stdout and 'cleanup hooks: stopped' in done.stdout, done.stdout
    done = run_tail("throw 'boom'", proc=ALIVE, stubs=answer.replace('__ANSWER__', "throw 'no reply'"))
    assert 'cleanup hooks: not stopped' in done.stdout and 'cleanup game:' in done.stdout, done.stdout    # the rest of the cleanup still runs
    done = run_tail("throw 'boom'", proc=ALIVE, stubs=answer.replace('__ANSWER__', "'Traceback: stopped'"))
    assert 'cleanup hooks: not stopped: Traceback: stopped' in done.stdout, done.stdout
    done = run_tail('Pass', proc=EXITED, stubs=answer.replace('__ANSWER__', "throw 'asked'"))
    assert 'py:' not in done.stdout and 'cleanup hooks' not in done.stdout, done.stdout    # an exited game is not asked


def test_the_summary_is_written_last_and_holds_the_final_status():
    if not powershell_here():
        return
    seen = {}

    def read(tmp, _done):
        seen['run'] = json.loads((Path(tmp) / 'summary.json').read_text(encoding='utf-8-sig'))['run']
    # save_compare.json cannot be written (a folder has its name): the summary, written after it, says so.
    done = run_tail('Pass', setup="New-Item -ItemType Directory -Force '__TMP__\\save_compare.json' | Out-Null", after=read)
    assert done.returncode == 1 and 'status: failed: evidence not written' in done.stdout, done.stdout
    assert seen['run']['status'].startswith('failed: evidence not written') and seen['run']['complete'] is False, seen
    # summary.json itself cannot be written: nothing says completed, and the exit is not 0.
    done = run_tail('Pass', setup="New-Item -ItemType Directory -Force '__TMP__\\summary.json' | Out-Null")
    assert done.returncode == 1 and 'status: failed: evidence not written' in done.stdout, done.stdout
    # The ordinary end: its record agrees with its exit.
    done = run_tail('Pass', after=read)
    assert done.returncode == 0 and seen['run']['status'] == 'completed' and seen['run']['complete'] is True and seen['run']['completed_steps'] in ('a', ['a']), seen


def test_earlier_evidence_is_gone_before_the_run():
    if not powershell_here():
        return
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    start = text.index('# Stale evidence:')
    block = text[start:text.index('# End of stale evidence.')]
    assert start < text.index('Enter-RunLock') and start < text.index('Backup-Saves')
    names = ('summary', 'save_compare', 'discover', 'ruin')
    with tempfile.TemporaryDirectory() as tmp:
        for name in names:
            (Path(tmp) / f'{name}.json').write_text('{"status": "completed"}', encoding='utf-8')
        script = Path(tmp) / 'stale.ps1'
        script.write_text(f"$ErrorActionPreference = 'Stop'\n$Out = '{tmp}'\n{block}", encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True, timeout=120)
        assert done.returncode == 0, done.stdout + done.stderr
        for name in names:
            assert not (Path(tmp) / f'{name}.json').exists(), name
            kept = list(Path(tmp).glob(f'{name}.*.json'))
            assert len(kept) == 1 and 'completed' in kept[0].read_text(encoding='utf-8'), kept


def test_the_capture_is_wired_to_the_safety_helpers():
    text = CAPTURE.read_text(encoding='utf-8')
    for call in ('$snapshots[$d] = Assert-SnapshotMatches', '$snapshots[$d] = New-VerifiedSnapshot', 'Restore-VerifiedRoots $bak $roots $snapshots', 'Save-VerifiedBackupRecord $bak $roots $snapshots',
                 'Format-ManualRecovery $recovery $bak $HubLock $Lane', '$recovery = Get-RecoveryReason', '$holdLocks = [bool]$recovery',
                 "Assert-SavesBlocked (Py 'print(block_saves())')", '$foreign = Get-ForeignGamesSeen $null',
                 'if ($holdLocks) { $runLock = $false; $hub = $null }', 'Format-RestoreRows $rows', "if ($log.status -ne 'completed') { exit 1 }"):
        assert call in text, call
    assert 'Focus-Game' not in text, 'Assert-GameForeground checks what Focus-Game only reported'
    order = [text.index(mark) for mark in ('Save-VerifiedBackupRecord $bak', '$driverAttempted = $true', '$driverOwned = $true', '$foreign = Get-ForeignGamesSeen', '$launchAttempted = $true', '$proc = Start-Game', 'Assert-SavesBlocked (Py', 'Send-Key Enter', "Invoke-Step 'discover'")]
    assert order == sorted(order), 'backup, driver, foreign check, launch, saves blocked, then input'


def test_every_step_the_capture_runs_is_a_step_it_names():
    """The list decides what "completed" needs and which earlier files are moved: a step missing from it would not count."""
    import re
    text = CAPTURE.read_text(encoding='utf-8')
    body = text[text.index('$launchAttempted = $true'):]
    ran = re.findall(r"(?:Invoke-Step|Invoke-DamageCast) '(\w+)'", body)
    named = re.search(r"\$allSteps = @\(([^)]*)\)", text).group(1)
    assert ran == re.findall(r"'(\w+)'", named), (ran, named)


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
