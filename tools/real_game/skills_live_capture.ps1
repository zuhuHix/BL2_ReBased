# Skills live capture (prepared 2026-11-04, not yet run). One real-game session in Three Horns Divide (Ice_P) that drives
# scripts/skills_live.py through the openwillow_realgame command channel. Steps, in order; each sets its grades in memory,
# runs, then puts them back. A failed step is recorded and the run moves on:
#   discover                   the SkillDefinition paths that match each skill name (sl_find_skills)
#   killskill_timer            Life Tap at grade 5 (a kill skill); two adult bullymongs killed about 2 s apart while
#                              UpdateKillSkills and KillSkillTimeRemaining are sampled every 0.05 s (sl_killskill_watch)
#   helios                     Helios at grade 5; one Phaselock cast (F key) on a lifted bullymong; TakeDamage recorded
#   ruin                       Ruin at grade 1; one cast, same recording
#   converge                   Converge at grade 1; one cast, same recording
#   minds_eye_crit             Mind's Eye at grade 0, then 5: CurrentInstantHitCriticalHitBonus read at each
#   subsequence_sweet_release  Sub-Sequence 5 and Sweet Release 5; the lifted bullymong is killed during the lock;
#                              OnKilledTarget, InterruptPhaseLock and EndSkill counted per cast (sl_killedtarget_watch)
# Takes the keyboard only (no frame capture). Everything written is game data under ignored local/realgame/skills_live/:
# one JSON per step, summary.json, and save_compare.json.
# Locks, saves and cleanup follow phaselock_integrity_capture.ps1: the hub lock and the run lock; saves backed up; saving
# blocked in game from the first frame; afterwards the save folder is compared with the backup (changed files restored).
# Results that depend on an unconfirmed API are recorded as 'not implemented' (see the TODO(live) marks in scripts/skills_live.py).
param(
    # The machine-wide hub lock lives in the main checkout's local/ (shared by every worktree on this machine).
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'skills-live',
    # quick_startup's -Character slot: the level-8 Maya (Phaselock) in Sanctuary, as the Phaselock integrity capture uses.
    # The hub may pass the level-70 save's file name instead.
    [string]$Character = 'Save0008.sav',
    # Give up waiting for the hub lock after this long and report its contents; a stale lock is removed by its owner or the
    # maintainer, never by this script.
    [int]$HubWaitMinutes = 120
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'skills_live'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o'); lane = $Lane; character = $Character }
$summary = [ordered]@{ steps = @() }

function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }
function PyJson([string]$Code, [int]$Timeout = 30) { Py $Code $Timeout | ConvertFrom-Json }
# One scripts/skills_live.py (or phaselock) function call, its return value printed as JSON.
function PyCall([string]$Expr, [int]$Timeout = 30) { PyJson "import json; print(json.dumps($Expr, default=str))" $Timeout }

# {'Helios': 5} for the Python side
function ToPyDict([hashtable]$Map) {
    $parts = foreach ($k in $Map.Keys) { "'$k': $([int]$Map[$k])" }
    '{' + ($parts -join ', ') + '}'
}

# Adult bullymongs (radius 120 or more) near Maya, alive, not yet killed: the names of the first $Count, Ice_P only.
function Get-Bullymongs([int]$Count) {
    $r = PyJson @"
import json
rows = []
for d, n, c, dead in list_ai():
    p = ai_pawn(n)
    cls = p.AIClass._path_name() if p.AIClass else ""
    radius = p.CylinderComponent.CollisionRadius if p.CylinderComponent else 0
    rows.append({"name": n, "dist": d, "class": cls.split(".")[-1], "radius": radius, "dead": dead, "ctrl": p.Controller is not None, "hp": p.GetHealth()})
ok = [r for r in rows if not r["dead"] and r["ctrl"] and r["hp"] > 0 and "PrimalBeast" in r["class"] and r["radius"] >= 120]
ok.sort(key=lambda r: (not r["class"].endswith("CharClass_PrimalBeast"), r["dist"]))
print(json.dumps({"picks": [r["name"] for r in ok[:$Count]], "rows": rows[:30]}))
"@
    if (@($r.picks).Count -lt $Count) { throw "need $Count adult bullymongs, found $(@($r.picks).Count): $($r.rows | ConvertTo-Json -Compress -Depth 4)" }
    @($r.picks)
}

# Put Maya 650 uu from the target, looking at it, with the target isolated and immune player; aim at its origin.
function Set-Up([string]$Target) {
    $s = PyCall "{'protect': len(protect_player(True)), 'isolate': isolate('$Target'), 'place': place('$Target', 650.0)}"
    Start-Sleep 3
    $a = PyCall "{'isolate': isolate('$Target'), 'pitch': aim('$Target'), 'measure': measure('$Target'), 'camera': camera(), 'bounds': bounds('$Target')}"
    Focus-Game | Out-Null; Start-Sleep -Milliseconds 500
    @{ setup = $s; aim = $a }
}

# Wait until the lift skill is idle and the cooldown pool is empty (or $Timeout s).
function Wait-Ready([double]$Timeout = 60) {
    $deadline = (Get-Date).AddSeconds($Timeout)
    while ((Get-Date) -lt $deadline) {
        $r = Py 'import json; s = lift_skill(); p = get_pc().SkillCooldownPool.Data; print(json.dumps({"state": str(s.CurrentState), "pool": p.CurrentValue}))' | ConvertFrom-Json
        if ($r.state -eq '0' -and $r.pool -le 0.001) { return $r }
        Start-Sleep 1
    }
    throw 'cooldown did not end'
}

# Runs one step and writes its record. The grades are set first (a step whose grades cannot be set does not run), and always put
# back, and any watch the step left open is stopped. A failure is recorded in the step's JSON and the run moves on.
function Invoke-Step([string]$Name, [hashtable]$Grades, [scriptblock]$Body) {
    $rec = [ordered]@{ step = $Name; grades = $Grades; started = (Get-Date).ToString('o') }
    try {
        if ($Grades.Count) {
            $set = PyCall "sl_set_grades($(ToPyDict $Grades))"
            $rec.grades_set = $set
            $bad = @($set.rows | Where-Object { $_.status -ne 'set' })
            if ($set.status -eq 'not implemented' -or $bad.Count) { throw "grades not set: $($set | ConvertTo-Json -Compress -Depth 6)" }
        }
        $rec.result = & $Body
        $rec.ok = $true
    } catch {
        $rec.ok = $false; $rec.error = "$_"
    } finally {
        try { $left = PyCall 'sl_stop_watches()'; if (@($left.PSObject.Properties).Count) { $rec.watches_left_open = $left } }
        catch { $rec.watch_cleanup_error = "$_" }
        try { if ($Grades.Count) { $rec.grades_restored = PyCall 'sl_restore_grades()' } }
        catch { $rec.restore_error = "$_" }
        $rec.finished = (Get-Date).ToString('o')
        $rec | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $Out "$Name.json")
        $summary.steps += [pscustomobject]@{ step = $Name; ok = $rec.ok; error = $rec.error }
    }
}

# One Phaselock cast (F key) on the lifted target with the damage watch running; the rows are the step's result.
function Invoke-DamageCast([string]$Name, [hashtable]$Grades) {
    Invoke-Step $Name $Grades {
        $t = (Get-Bullymongs 1)[0]
        $setup = Set-Up $t
        $watch = PyCall 'sl_damage_watch(7.0)'
        Send-Key F
        Start-Sleep 7
        $res = PyCall 'sl_watch_result("damage")'
        $ready = Wait-Ready
        @{ target = $t; setup = $setup; watch_start = $watch; result = $res; ready = $ready }
    }
}

$extraRoots = [ordered]@{ sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $runLock = $false; $driverOwned = $false
# A token on the game's command line that identifies the process this run started (Stop-OwnGame checks it with the
# image path and the start time before stopping a PID). Unknown switches are ignored by the game.
$marker = "-owrun=$Lane-$PID"
try {
    # Preflight before any lock or backup: bl-sdk present, our driver folder not already there.
    if (!(Test-Path -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods'))) { throw "bl-sdk is not installed (no sdk_mods in $env:OPENWILLOW_BL2); not installing it" }
    if (Test-Path -LiteralPath $script:DriverDir) { throw "$script:DriverDir already exists; this run did not create it and leaves it alone" }
    $hubDeadline = (Get-Date).AddMinutes($HubWaitMinutes)
    while (!$hub) {
        try { $hub = New-Item -ItemType File -Path $HubLock -Value "$Lane $((Get-Date).ToString('o'))" -ErrorAction Stop }
        catch {
            $held = Get-Content $HubLock -Raw -ErrorAction SilentlyContinue
            if ((Get-Date) -gt $hubDeadline) { throw "hub lock $HubLock still held after $HubWaitMinutes min by '$held'; if it is stale, its owner or the maintainer removes it" }
            "hub lock held by '$held'; retry in 60 s"; Start-Sleep 60
        }
    }
    if (Get-GameProcess) { throw 'Borderlands2 is running although the hub lock was free; not touching it' }
    Enter-RunLock 1800; $runLock = $true
    $bak = Backup-Saves; $log.backup = $bak; "backup $bak"
    # The SDK's settings and Sanity Saver's private store are compared too (an earlier session saw Sanity Saver rewrite one).
    foreach ($d in $extraRoots.Keys) { if (Test-Path $extraRoots[$d]) { Copy-Item -Recurse $extraRoots[$d] (Join-Path $bak $d) } }
    # -Exclusive re-checks that the folder is absent and removes its own partial install on failure.
    Install-Driver -BlockSavesAtStart -Exclusive
    $driverOwned = $true
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $gameArgs = @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', $marker)
    if ($Character) { $gameArgs += "-Character=$Character" }
    $proc = Start-Game $gameArgs; $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'skills_live/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Py 'print(block_saves())'
    Start-Sleep 5; Focus-Game | Out-Null; Send-Key Enter; Start-Sleep 45
    $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
    "map: $map"; if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'skills_live/load_failed.png' | Out-Null; throw "no player pawn: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 1
    # Travel to Three Horns Divide (Ice_P) by the console, as the Phaselock integrity capture does.
    Py 'get_pc().ConsoleCommand("open Ice_P", False); print("sent")' | Out-Null
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep 3
        try { $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn is not None)' 8 } catch { continue }
        if ($map -match 'Ice_P True') { break }
    }
    "map: $map"; if ($map -notmatch 'Ice_P True') { throw "travel to Ice_P failed: $map" }
    Start-Sleep 8
    foreach ($f in 'phaselock.py', 'phaselock_matched.py', 'skills_live.py') { Invoke-GamePyFile (Join-Path $PSScriptRoot "scripts/$f") skills_live | Out-Null }
    # Immunity: protect_player(True) (in Set-Up) refuses damage to Maya; nothing else about her changes.

    Invoke-Step 'discover' @{} {
        PyCall "{'Helios': sl_find_skills('Helios'), 'Ruin': sl_find_skills('Ruin'), 'Converge': sl_find_skills('Converge'), 'SubSequence': sl_find_skills('Sub-Sequence'), 'SweetRelease': sl_find_skills('Sweet Release'), 'MindsEye': sl_find_skills('Minds Eye'), 'LifeTap': sl_find_skills('Life Tap'), 'Phaselock': sl_find_skills('Phaselock')}"
    }

    # 1. Kill-skill timer. Life Tap (a Harmony kill skill) at grade 5 makes the kill-skill switch run; two kills, about 2 s apart.
    Invoke-Step 'killskill_timer' @{ LifeTap = 5 } {
        $targets = Get-Bullymongs 2
        $watch = PyCall 'sl_killskill_watch(9.0)'
        Start-Sleep 1
        $k1 = PyCall "sl_kill('$($targets[0])')"
        Start-Sleep 2
        $k2 = PyCall "sl_kill('$($targets[1])')"
        Start-Sleep 7
        $res = PyCall 'sl_watch_result("killskill")'
        @{ targets = $targets; watch_start = $watch; kills = @($k1, $k2); result = $res }
    }

    # 2 to 4. One Phaselock cast per skill, each with its own grade set.
    Invoke-DamageCast 'helios' @{ Helios = 5 }
    Invoke-DamageCast 'ruin' @{ Ruin = 1 }
    Invoke-DamageCast 'converge' @{ Converge = 1 }

    # 5. Mind's Eye: the crit bonus at grade 0 (set by Invoke-Step), then at grade 5 (set here; restored at the end of the step).
    Invoke-Step 'minds_eye_crit' @{ MindsEye = 0 } {
        Start-Sleep 1
        $at0 = PyCall 'sl_crit_bonus()'
        $set5 = PyCall "sl_set_grades({'MindsEye': 5})"
        Start-Sleep 1
        $at5 = PyCall 'sl_crit_bonus()'
        @{ grade0 = $at0; grade5 = $at5; set5 = $set5 }
    }

    # 6. Sub-Sequence and Sweet Release: the lifted bullymong is killed 1.5 s after the F press, during the lock.
    Invoke-Step 'subsequence_sweet_release' @{ SubSequence = 5; SweetRelease = 5 } {
        $t = (Get-Bullymongs 1)[0]
        $setup = Set-Up $t
        $watch = PyCall 'sl_killedtarget_watch(12.0)'
        Send-Key F
        Start-Sleep -Milliseconds 1500
        $kill = PyCall "sl_kill('$t')"
        Start-Sleep 9
        $res = PyCall 'sl_watch_result("killedtarget")'
        $ready = Wait-Ready
        @{ target = $t; setup = $setup; watch_start = $watch; kill = $kill; result = $res; ready = $ready }
    }
    # Normal end: the game quits itself (the finally block still stops it if it has not).
    Py 'protect_player(False); get_pc().ConsoleCommand("quit", False)' | Out-Null
    for ($i = 0; $i -lt 20 -and (Get-GameProcess); $i++) { Start-Sleep 2 }
} catch {
    $log.error = "$_"
    "error: $_"
} finally {
    # Each step is isolated so a failure in one (a log write) cannot skip the restore. Order: put back the grades and remove the
    # hooks in game, stop our game, and only once it has exited remove our driver and restore and verify the saves and config;
    # then write the logs and release the locks. If the game's exit is not confirmed, a live game could overwrite restored
    # files: nothing is removed or restored, the backup is kept and manual-recovery steps are printed.
    $cleanup = [ordered]@{}
    $gameGone = $true
    if ($proc) {
        try { if (Get-GameProcess) { $cleanup.shutdown = PyCall 'sl_shutdown()' 15 } } catch { $cleanup.shutdown = "error: $_" }
    }
    if ($proc) {
        try {
            for ($i = 0; $i -lt 10 -and !$proc.HasExited; $i++) { Start-Sleep 1 }
            $cleanup.game = if ($proc.HasExited) { 'exited' } else { Stop-OwnGame $proc $procStart $marker }
            if ($cleanup.game -eq 'stopped' -and !$proc.WaitForExit(15000)) { $cleanup.game = 'stopped, but still running after 15 s' }
            $proc.Refresh(); $gameGone = $proc.HasExited
        } catch { $cleanup.game = "error: $_"; $gameGone = $false }
    }
    if (!$gameGone) {
        $cleanup.driver = "NOT REMOVED (game exit not confirmed): $script:DriverDir"
        $cleanup.saves = "NOT RESTORED (game exit not confirmed); backup kept at $bak"
        @"
MANUAL RECOVERY: Borderlands 2 (pid $($proc.Id)) did not confirm its exit ($($cleanup.game)). Nothing was removed or restored.
  1. Close Borderlands 2 yourself.
  2. In a new PowerShell window: `$env:OPENWILLOW_BL2 = '$env:OPENWILLOW_BL2'; . '$PSScriptRoot\realgame.ps1'
     Remove-Item -Recurse -Force '$script:DriverDir'
     foreach (`$sub in 'SaveData', 'Config', 'sdk_settings', 'SanitySaver') { `$b = Join-Path '$bak' `$sub; if (Test-Path `$b) { Restore-Tree `$b `$(@{ SaveData = (Join-Path `$script:WillowDir 'SaveData'); Config = (Join-Path `$script:WillowDir 'Config'); sdk_settings = '$($extraRoots.sdk_settings)'; SanitySaver = '$($extraRoots.SanitySaver)' }[`$sub]) `$sub } }
     Every row must read 'added, removed', 'missing, restored' or 'changed, restored'; a 'MISMATCH after restore' row means it failed.
  3. Then remove the locks this run left: '$HubLock' (if it names '$Lane') and '$script:Lock'.
"@
        $bak = $null; $driverOwned = $false; $runLock = $false; $hub = $null   # skip the steps below; locks stay held on purpose
    }
    try {
        if ($driverOwned) {
            Remove-Driver
            $cleanup.driver = if (Test-Path -LiteralPath $script:DriverDir) { "STILL PRESENT: $script:DriverDir" } else { 'removed' }
        } elseif (!$cleanup.Contains('driver')) { $cleanup.driver = 'not installed by this run; untouched' }
    } catch { $cleanup.driver = "error: $_" }
    $rows = @()
    if ($bak) {
        try {
            $roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
            foreach ($d in $extraRoots.Keys) { $roots[$d] = $extraRoots[$d] }
            foreach ($sub in $roots.Keys) {
                $b = Join-Path $bak $sub; $live = $roots[$sub]
                try {
                    if (Test-Path -LiteralPath $b) { $rows += @(Restore-Tree $b $live $sub) }
                    elseif (Test-Path -LiteralPath $live) {
                        # Absent at the backup and created during the run (an SDK mod's own store): reported, not deleted.
                        $rows += [pscustomobject]@{ file = $sub; status = 'created during the run; left for the maintainer' }
                    }
                } catch { $rows += [pscustomobject]@{ file = $sub; status = "RESTORE FAILED: $_" } }
            }
            $bad = @($rows | Where-Object { $_.status -like 'MISMATCH*' -or $_.status -like 'RESTORE FAILED*' })
            $cleanup.saves = if ($bad.Count) { "NOT IDENTICAL to $bak ($($bad.Count) problems)" } else { "identical to $bak after $($rows.Count) restores" }
        } catch { $cleanup.saves = "RESTORE FAILED: $_ (backup kept at $bak)" }
    }
    try {
        $log.finished = (Get-Date).ToString('o'); $log.cleanup = $cleanup
        [ordered]@{ run = $log; steps = $summary.steps } | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'summary.json')
        if ($bak) { @{ backup = $bak; compared = (Get-Date).ToString('o'); changes = @($rows) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Out 'save_compare.json') }
    } catch { "log write failed: $_" }
    try { if ($runLock) { Exit-RunLock } } catch { "run lock release failed: $_" }
    try { if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force } } catch { "hub lock release failed: $_" }
    $cleanup.GetEnumerator() | ForEach-Object { "cleanup $($_.Key): $($_.Value)" }
    $rows
}
