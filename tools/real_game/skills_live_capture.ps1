# Skills live capture (first run 2026-11-06; fixed after it, see NATIVE_SKILL_BEHAVIOURS.md). One real-game session in Three Horns Divide (Ice_P) that drives
# scripts/skills_live.py through the openwillow_realgame command channel. Steps, in order (-Steps a,b,c runs only those); each sets
# its grades in memory, runs, then puts them back. A failed step keeps its error, and two failures in a row end the run:
#   discover                   the SkillDefinition paths that match each skill name (sl_find_skills)
#   killskill_timer            Life Tap at grade 5 (a kill skill); two unwounded, controlled adult bullymongs killed with the gun
#                              alone while UpdateKillSkills and KillSkillTimeRemaining are sampled every 0.05 s (sl_killskill_watch)
#   converge                   Converge at grade 1; one Phaselock cast (F key) on a lifted bullymong; TakeDamage recorded
#   minds_eye_crit             the crit bonus and its modifier stack as saved, at grade 1 and at grade 5 (sl_crit_bonus)
#   subsequence_sweet_release  Sub-Sequence 5 and Sweet Release 5; Maya's health lowered; the lifted bullymong killed during the lock;
#                              OnKilledTarget, InterruptPhaseLock, EndSkill counted per cast and Maya's health (sl_killedtarget_watch)
#   helios                     Helios at grade 5; one cast, same recording as converge
#   ruin                       Ruin at grade 1; one cast, same recording
# Takes the keyboard only (no frame capture). Everything written is game data under ignored local/realgame/skills_live/:
# one JSON per step, summary.json, and save_compare.json.
# Locks, saves and cleanup follow phaselock_integrity_capture.ps1: the hub lock and the run lock; saves backed up; saving
# blocked in game from the first frame; afterwards the save folder is compared with the backup (changed files restored).
param(
    # The machine-wide hub lock lives in the main checkout's local/ (shared by every worktree on this machine).
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'skills-live',
    # quick_startup's -Character slot: the level-8 Maya (Phaselock) in Sanctuary, as the Phaselock integrity capture uses.
    # The hub may pass the level-70 save's file name instead.
    [string]$Character = 'Save0008.sav',
    # Only these steps (comma-separated names, one argument); none named: all of them.
    [string[]]$Steps = @(),
    # Give up waiting for the hub lock after this long and report its contents; a stale lock is removed by its owner or the
    # maintainer, never by this script.
    [int]$HubWaitMinutes = 120
)
$ErrorActionPreference = 'Stop'
$Steps = @($Steps | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'skills_live'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o'); lane = $Lane; character = $Character }
$summary = [ordered]@{ steps = @() }

function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }
# The driver prints a Python traceback as its reply; that text goes into the error instead of a JSON parse message.
function PyJson([string]$Code, [int]$Timeout = 30) {
    $raw = Py $Code $Timeout
    try { $raw | ConvertFrom-Json -ErrorAction Stop } catch { throw "python: $raw" }
}
# One scripts/skills_live.py (or phaselock) function call, its return value printed as JSON.
function PyCall([string]$Expr, [int]$Timeout = 30) { PyJson "import json; print(json.dumps($Expr, default=str))" $Timeout }

# {'Helios': 5} for the Python side
function ToPyDict([hashtable]$Map) {
    $parts = foreach ($k in $Map.Keys) { "'$k': $([int]$Map[$k])" }
    '{' + ($parts -join ', ') + '}'
}

# Adult bullymongs (radius 120 or more) near Maya, alive, not yet killed: the names of the first $Count, Ice_P only.
function Get-Bullymongs([int]$Count) {
    $r = PyCall "sl_bullymongs($Count)"
    if (@($r.picks).Count -lt $Count) { throw "need $Count adult bullymongs, found $(@($r.picks).Count): $($r.rows | ConvertTo-Json -Compress -Depth 4)" }
    @($r.picks)   # one name comes back as a plain string: callers wrap the call in @() before indexing it
}

# Put Maya 650 uu from the target, looking at it, with the target isolated and immune player; aim at its origin.
function Set-Up([string]$Target) {
    $s = PyCall "{'isolate': isolate('$Target'), 'place': place('$Target', 650.0)}"
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

# A real kill: the target wounded to 1 health, then the held gun fired at it, up to three short bursts. The last resort is the
# one large TakeDamage, which the record names, because the first run's kills of that kind did not start the kill-skill timer.
function Invoke-GunKill([string]$Target) {
    $wound = PyCall "sl_wound('$Target')"
    $how = 'not dead'
    for ($try = 1; $try -le 3; $try++) {
        PyCall "aim('$Target')" | Out-Null
        PyCall 'sl_fire(True)' | Out-Null; Start-Sleep -Milliseconds 400; PyCall 'sl_fire(False)' | Out-Null
        Start-Sleep -Milliseconds 300
        if ((PyCall "sl_is_dead('$Target')") -eq $true) { $how = "gun, burst $try"; break }
    }
    if ($how -eq 'not dead') { PyCall "sl_kill('$Target')" | Out-Null; $how = 'TakeDamage fallback (the gun missed)' }
    @{ target = $Target; wound = $wound; how = $how }
}

# A kill with nothing done to the target first: Maya near it, not isolated, so it and the others keep their controllers, and the gun
# held on it until it dies or the time is up. No fallback: a target that does not die is a result (the step says so).
function Invoke-FreshKill([string]$Target, [double]$Seconds = 20) {
    $place = PyCall "place('$Target', 450.0)"
    Start-Sleep 1
    Focus-Game | Out-Null
    $before = PyCall "{'health': ai_pawn('$Target').GetHealth(), 'controlled': ai_pawn('$Target').Controller is not None}"
    $start = Get-Date; $dead = $false
    PyCall 'sl_fire(True)' | Out-Null
    try {
        while (((Get-Date) - $start).TotalSeconds -lt $Seconds) {
            PyCall "aim('$Target')" | Out-Null
            Start-Sleep -Milliseconds 400
            if ((PyCall "sl_is_dead('$Target')") -eq $true) { $dead = $true; break }
        }
    } finally { PyCall 'sl_fire(False)' | Out-Null }
    @{ target = $Target; place = $place; before = $before; dead = $dead; seconds = [math]::Round(((Get-Date) - $start).TotalSeconds, 2) }
}

# What a step came for, as named yes/no checks. A step whose body ran without an error can still have got nothing: no check, or a
# check that is not true, means it did not. Returns the names that failed ("no evidence" when there are none).
function Get-MissingEvidence($Evidence) {
    if ($null -eq $Evidence -or !@($Evidence.Keys).Count) { return @('no evidence') }
    @($Evidence.Keys | Where-Object { $Evidence[$_] -ne $true })
}

# Runs one step and writes its record. The grades are set first (a step whose grades cannot be set does not run), and always put
# back, and any watch the step left open is stopped. A failure stops the step at once with its text (a Python traceback included)
# in the step's JSON. Two steps failing in a row end the run, so a broken setup cannot use up the slot.
$script:FailedInARow = 0
$script:GradesBroken = $false   # a grade did not go back: later steps would read under a wrong grade
$script:RunningUnknown = $false   # the running skills cannot be read: "restored" could only speak for the tree
function Invoke-Step([string]$Name, [hashtable]$Grades, [scriptblock]$Body) {
    if ($Steps -and $Name -notin $Steps) { return }
    $rec = [ordered]@{ step = $Name; grades = $Grades; started = (Get-Date).ToString('o') }
    if ($script:GradesBroken) {
        $summary.steps += [pscustomobject]@{ step = $Name; ok = $false; error = 'not run: a grade from an earlier step did not restore' }
        return
    }
    if ($script:FailedInARow -ge 2) {
        $summary.steps += [pscustomobject]@{ step = $Name; ok = $false; error = 'not run: the two steps before it failed' }
        return
    }
    try {
        if ($Grades.Count) {
            $set = PyCall "sl_set_grades($(ToPyDict $Grades))"
            $rec.grades_set = $set
            $bad = @($set.rows | Where-Object { $_.status -ne 'set' })
            if ($set.status -eq 'not implemented' -or $bad.Count) { throw "grades not set: $($set | ConvertTo-Json -Compress -Depth 6)" }
        }
        # Could an earlier step's skill be in this reading? The record says so; the run goes on, and the session is not complete.
        $keys = ($Grades.Keys | ForEach-Object { "'$_'" }) -join ', '
        $rec.active_check = PyCall "sl_unexpected_active([$keys])"
        $rec.possibly_contaminated = $script:RunningUnknown -or $rec.active_check.readable -eq $false -or @($rec.active_check.unexpected | Where-Object { $_ }).Count -gt 0
        $rec.result = & $Body
        # The body's last hashtable: a stray value it wrote to the pipeline must not lose the evidence.
        $rec.evidence = (@($rec.result) | Where-Object { $_ -is [hashtable] } | Select-Object -Last 1).evidence
        $rec.missing = @(Get-MissingEvidence $rec.evidence)
        if ($rec.missing.Count) { "step $Name ran but did not get: $($rec.missing -join ', ')" }
        $rec.ok = $true
        $script:FailedInARow = 0
    } catch {
        $rec.ok = $false; $rec.error = "$_"
        $script:FailedInARow++
    } finally {
        try { $left = PyCall 'sl_stop_watches()'; if (@($left.PSObject.Properties).Count) { $rec.watches_left_open = $left } }
        catch { $rec.watch_cleanup_error = "$_" }
        try {
            $rec.grades_restored = PyCall 'sl_restore_grades()'
            # A running skill's grade can follow the tree a tick later: one more look before calling it stuck.
            if (@($rec.grades_restored.rows | Where-Object { $_ -and $_.status -eq 'not restored' }).Count) {
                Start-Sleep 1
                $rec.grades_restored_first_try = $rec.grades_restored
                $rec.grades_restored = PyCall 'sl_restore_grades()'
            }
            if (@($rec.grades_restored.rows | Where-Object { $_ -and $_.status -notin 'restored', 'tree only' }).Count) { $script:GradesBroken = $true }
            if (@($rec.grades_restored.rows | Where-Object { $_ -and $_.status -eq 'tree only' }).Count) {
                $script:RunningUnknown = $true
                "WARNING: step $Name restored the tree's grades, but the running skills could not be read: later readings may include this step's skills"
            }
        }
        catch { $rec.restore_error = "$_"; $script:GradesBroken = $true }
        $rec.finished = (Get-Date).ToString('o')
        $rec | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $Out "$Name.json")
        $summary.steps += [pscustomobject]@{ step = $Name; ok = $rec.ok; error = $rec.error; possibly_contaminated = [bool]$rec.possibly_contaminated; missing = @($rec.missing) }
    }
}

# One Phaselock cast (F key) on the lifted target with the damage watch running; the rows are the step's result.
function Invoke-DamageCast([string]$Name, [hashtable]$Grades) {
    Invoke-Step $Name $Grades {
        $t = @(Get-Bullymongs 1)[0]
        $setup = Set-Up $t
        $watch = PyCall "sl_damage_watch(7.0, '$t')"
        Send-Key F
        Start-Sleep 7
        $res = PyCall 'sl_watch_result("damage")'
        $ready = Wait-Ready
        $hits = @($res.rows | Where-Object { $_.victim -eq $t -and ($_.damage -as [double]) -gt 0 })
        @{ target = $t; setup = $setup; watch_start = $watch; result = $res; ready = $ready
           evidence = @{ target_took_damage = $hits.Count -gt 0 } }
    }
}

# One root into the backup, verified by hash. Any failure stops the run before the game starts; a root that did not verify is never
# restored from, because Restore-Tree deletes live files that a partial backup lacks.
function Copy-VerifiedRoot([string]$Source, [string]$Backup) {
    if (!(Test-Path -LiteralPath $Backup)) { Copy-Item -Recurse -LiteralPath $Source -Destination $Backup -ErrorAction Stop }
    $different = @(Compare-TreeHashes $Source $Backup)
    if ($different.Count) { throw "backup of $Source differs from the original in $($different.Count) files, first: $($different[0])" }
}

# The line for the summary. A root created during the run is left in place for the maintainer, and it does mean "not identical".
function Get-SavesVerdict([object[]]$Rows, [string]$Backup) {
    $bad = @($Rows | Where-Object { $_.status -like 'MISMATCH*' -or $_.status -like 'RESTORE FAILED*' -or $_.status -like 'BACKUP FAILED*' -or $_.status -like 'created during the run*' })
    if ($bad.Count) { "NOT IDENTICAL to $Backup ($($bad.Count) problems: $(($bad | ForEach-Object { $_.file }) -join ', '))" }
    else { "identical to $Backup after $(@($Rows).Count) restores" }
}

# Complete means every requested step passed, got what it came for (its evidence) and had no possible mix-up from an earlier step, and
# everything is back as it was. A step that ran without an error is not enough: that is how a run with no kills once reported complete.
function Get-SessionComplete($Log, [object[]]$StepRows, [string[]]$Requested, $Cleanup, [bool]$GameGone, [bool]$GradesBroken, [bool]$RunningUnknown) {
    if ($Log.Contains('error') -or !$GameGone -or $GradesBroken -or $RunningUnknown) { return $false }
    if ("$($Cleanup.saves)" -notlike 'identical*') { return $false }
    if ($Cleanup.driver -notin 'removed', 'not installed by this run; untouched') { return $false }
    $rows = @($StepRows | Where-Object { $_ })
    if (!$rows.Count) { return $false }
    if (@($rows | Where-Object { !$_.ok -or $_.possibly_contaminated }).Count) { return $false }
    if (@($rows | Where-Object { @($_.missing).Count -or $null -eq $_.missing }).Count) { return $false }
    foreach ($name in $Requested) { if ($name -notin $rows.step) { return $false } }
    $true
}

$extraRoots = [ordered]@{ sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$rootState = @{}   # backup state per root: verified, absent (no such folder at the backup) or failed
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
    # Verified before anything can change them; a failure ends the run here, before the driver and the game.
    $backupRoots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
    foreach ($d in $extraRoots.Keys) { $backupRoots[$d] = $extraRoots[$d] }
    foreach ($d in $backupRoots.Keys) { $rootState[$d] = 'failed' }
    foreach ($d in $backupRoots.Keys) {
        if (!(Test-Path -LiteralPath $backupRoots[$d])) { $rootState[$d] = 'absent'; continue }
        Copy-VerifiedRoot $backupRoots[$d] (Join-Path $bak $d)
        $rootState[$d] = 'verified'
    }
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
    foreach ($f in 'phaselock.py', 'phaselock_matched.py', 'skills_live.py') {
        $reply = Invoke-GamePyFile (Join-Path $PSScriptRoot "scripts/$f") skills_live
        if ("$reply" -match 'Traceback') { throw "loading $f failed: $reply" }
    }
    # Immunity for the whole run, once: it refuses damage to Maya; nothing else about her changes.
    $log.protect = Py 'print(len(protect_player(True)))'
    $log.baseline = PyCall 'sl_mark_baseline()'
    if ($log.baseline.readable -eq $false) {
        $script:RunningUnknown = $true
        'WARNING: the running skills cannot be read: "restored" speaks only for the tree, and every reading may include an earlier step''s skills'
    }

    Invoke-Step 'discover' @{} {
        $found = PyCall "{'Helios': sl_find_skills('Helios'), 'Ruin': sl_find_skills('Ruin'), 'Converge': sl_find_skills('Converge'), 'SubSequence': sl_find_skills('Sub-Sequence'), 'SweetRelease': sl_find_skills('Sweet Release'), 'MindsEye': sl_find_skills('Minds Eye'), 'LifeTap': sl_find_skills('Life Tap'), 'Phaselock': sl_find_skills('Phaselock')}"
        @{ found = $found; evidence = @{ skills_found = @($found.PSObject.Properties | Where-Object { @($_.Value).Count -gt 0 }).Count -gt 0 } }
    }

    # 1. Kill-skill timer, first, before anything isolates a pawn. Life Tap (a Harmony kill skill) at grade 5 makes the kill-skill
    # switch run; two kills with the gun alone on unwounded, controlled targets, then long enough to see it run out (refresh or add,
    # and the duration). The earlier runs' targets were wounded by a direct TakeDamage first, and the timer stayed at 0.
    Invoke-Step 'killskill_timer' @{ LifeTap = 5 } {
        $targets = Get-Bullymongs 2
        $watch = PyCall 'sl_killskill_watch(75.0)'
        Start-Sleep 1
        $k1 = Invoke-FreshKill $targets[0]
        $k2 = Invoke-FreshKill $targets[1]
        Start-Sleep 16
        $res = PyCall 'sl_watch_result("killskill")'
        $samples = @($res.rows | Where-Object { $null -ne $_.remaining })
        @{ targets = $targets; watch_start = $watch; kills = @($k1, $k2); result = $res
           evidence = @{ both_kills_happened = ($k1.dead -eq $true -and $k2.dead -eq $true); timer_sampled = $samples.Count -gt 0 } }
    }

    # 2. Converge: its blast leaves the target alive, so the next step can use it (isolate detaches every other pawn).
    Invoke-DamageCast 'converge' @{ Converge = 1 }

    # 5. Mind's Eye: the crit bonus and its stack as the save has it, at grade 1 and at grade 5. Grade 0 cannot be read this way:
    # an active skill keeps a grade of at least 1 (Skill.UpdateGrade). Restored at the end of the step.
    Invoke-Step 'minds_eye_crit' @{} {
        $asSaved = PyCall 'sl_crit_bonus()'
        $set1 = PyCall "sl_set_grades({'MindsEye': 1})"
        Start-Sleep 2
        $at1 = PyCall 'sl_crit_bonus()'
        $set5 = PyCall "sl_set_grades({'MindsEye': 5})"
        Start-Sleep 2
        $at5 = PyCall 'sl_crit_bonus()'
        $read = { param($r) ($r.values.CurrentInstantHitCriticalHitBonus -as [double]) -gt 0 }
        @{ as_saved = $asSaved; grade1 = $at1; grade5 = $at5; set1 = $set1; set5 = $set5
           evidence = @{ bonus_read_at_each_grade = ((& $read $asSaved) -and (& $read $at1) -and (& $read $at5)) } }
    }

    # 6. Sub-Sequence and Sweet Release: the lifted bullymong is killed 1.5 s after the F press, during the lock.
    Invoke-Step 'subsequence_sweet_release' @{ SubSequence = 5; SweetRelease = 5 } {
        $t = @(Get-Bullymongs 1)[0]
        $setup = Set-Up $t
        $health = PyCall 'sl_maya_health(0.5)'
        $watch = PyCall 'sl_killedtarget_watch(12.0)'
        Send-Key F
        Start-Sleep -Milliseconds 1500
        $kill = Invoke-GunKill $t
        Start-Sleep 9
        $res = PyCall 'sl_watch_result("killedtarget")'
        $ready = Wait-Ready
        $interrupts = @($res.rows | Where-Object { $_.func -eq 'InterruptPhaseLock' })
        # The immunity hook is on for the whole run and refuses TakeDamage on Maya, and the heal is read as healing damage: a heal
        # cannot be told from no heal until the hook lets it through (NATIVE_SKILL_BEHAVIOURS.md, "What a useful fifth run needs").
        $immune = $true
        @{ target = $t; setup = $setup; maya_health = $health; watch_start = $watch; kill = $kill; result = $res; ready = $ready
           evidence = @{ held_target_killed_by_the_gun = "$($kill.how)" -like 'gun*'; interrupt_seen = $interrupts.Count -gt 0
                         heal_could_be_observed = (!$immune -and ($health.after -as [double]) -lt ($health.max -as [double])) } }
    }

    # 4 and 5. Helios and Ruin last: Ruin's pulses kill the target, and one controlled target is all that is left by now. Helios
    # goes first, so its restore has to deactivate it: if that fails, the restore check stops the run before Ruin's reading.
    Invoke-DamageCast 'helios' @{ Helios = 5 }
    Invoke-DamageCast 'ruin' @{ Ruin = 1 }
    # Normal end: the game quits itself (the finally block still stops it if it has not).
    # The game may be gone before it answers: no reply, or an empty one, is the expected end here.
    try { Invoke-GamePy 'protect_player(False); get_pc().ConsoleCommand("quit", False)' 10 | Out-Null } catch { 'quit sent (no reply)' }
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
                    if ($rootState[$sub] -eq 'verified') { $rows += @(Restore-Tree $b $live $sub) }
                    elseif ($rootState[$sub] -eq 'absent') {
                        # Absent at the backup and created during the run (an SDK mod's own store): reported, not deleted.
                        if (Test-Path -LiteralPath $live) { $rows += [pscustomobject]@{ file = $sub; status = 'created during the run; left for the maintainer' } }
                    }
                    else { $rows += [pscustomobject]@{ file = $sub; status = 'BACKUP FAILED; not restored, and nothing was started' } }
                } catch { $rows += [pscustomobject]@{ file = $sub; status = "RESTORE FAILED: $_" } }
            }
            $cleanup.saves = Get-SavesVerdict $rows $bak
        } catch { $cleanup.saves = "RESTORE FAILED: $_ (backup kept at $bak)" }
    }
    try {
        $log.finished = (Get-Date).ToString('o'); $log.cleanup = $cleanup
        $log.complete = Get-SessionComplete $log $summary.steps $Steps $cleanup $gameGone $script:GradesBroken $script:RunningUnknown
        [ordered]@{ run = $log; steps = $summary.steps } | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'summary.json')
        if ($bak) { @{ backup = $bak; compared = (Get-Date).ToString('o'); changes = @($rows) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Out 'save_compare.json') }
    } catch { "log write failed: $_" }
    try { if ($runLock) { Exit-RunLock } } catch { "run lock release failed: $_" }
    try { if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force } } catch { "hub lock release failed: $_" }
    $cleanup.GetEnumerator() | ForEach-Object { "cleanup $($_.Key): $($_.Value)" }
    "session complete: $($log.complete)"
    $rows
}
# The exit code says the same: a failed step or an unclean cleanup is not a success.
if ($log.complete -eq $true) { exit 0 } else { exit 1 }
