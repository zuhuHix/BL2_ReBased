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
# one JSON per step, save_compare.json, and summary.json last, with the final status.
# Locks, saves and cleanup follow phaselock_integrity_capture.ps1: the hub lock and the run lock; saves backed up and verified; saving
# blocked in game from the first frame, or the run stops; input only while the game is the foreground window; afterwards every root is
# restored and compared, unless a game may still be running. Anything left for a person keeps both locks and prints the recovery steps.
# Exit 0 only for status 'completed': every requested step passed and got its evidence, and nothing is left to recover.
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
# Stale evidence: a final write that fails must not leave an earlier run's result behind. Kept under a new name, not deleted: after a run that
# needs recovery, its record is what names the backup and says what went wrong, and a rejected retry must not lose that.
$allSteps = @('discover', 'killskill_timer', 'converge', 'minds_eye_crit', 'subsequence_sweet_release', 'helios', 'ruin')
$keptAs = "$(Get-Date -Format 'yyyyMMdd-HHmmss')-$PID"
foreach ($stale in @('summary', 'save_compare') + $allSteps) { $file = Join-Path $Out "$stale.json"; if (Test-Path -LiteralPath $file) { Move-Item -LiteralPath $file (Join-Path $Out "$stale.$keptAs.json") } }
# End of stale evidence.
$stepsRequested = if ($Steps) { @($Steps) } else { $allSteps }
$runError = ''
$log = [ordered]@{ started = (Get-Date).ToString('o'); lane = $Lane; character = $Character; status = 'running'; requested_steps = $stepsRequested }
$summary = [ordered]@{ steps = @() }

function Py([string]$Code, [int]$Timeout = 30) { Invoke-GameReply $Code $Timeout }   # the shared wrapper (realgame.ps1): refuses once the driver is lost, latches a lost reply, refuses an empty one
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
    Assert-GameForeground; Start-Sleep -Milliseconds 500
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
    PyCall "sl_damage_watch(6.0, '$Target')" | Out-Null
    for ($try = 1; $try -le 3; $try++) {
        PyCall "aim('$Target')" | Out-Null
        PyCall 'sl_fire(True)' | Out-Null; Start-Sleep -Milliseconds 400; PyCall 'sl_fire(False)' | Out-Null
        Start-Sleep -Milliseconds 300
        if ((PyCall "sl_is_dead('$Target')") -eq $true) { $how = "gun, burst $try"; break }
    }
    if ($how -eq 'not dead') { PyCall "sl_kill('$Target')" | Out-Null; $how = 'TakeDamage fallback (the gun missed)' }
    $hits = (PyCall 'sl_watch_result("damage")').rows
    @{ target = $Target; wound = $wound; how = $how; dead = ((PyCall "sl_is_dead('$Target')") -eq $true); hits = $hits; controller = (Py 'print(get_pc().Name)') }
}

# A kill with nothing done to the target first: Maya near it, not isolated, so it and the others keep their controllers, and the gun
# held on it until it dies or the time is up. No fallback: a target that does not die is a result (the step says so).
function Invoke-FreshKill([string]$Target, [double]$Seconds = 20) {
    $place = PyCall "place('$Target', 450.0)"
    Start-Sleep 1
    Assert-GameForeground
    $before = PyCall "{'health': ai_pawn('$Target').GetHealth(), 'controlled': ai_pawn('$Target').Controller is not None}"
    $start = Get-Date; $dead = $false
    PyCall "sl_damage_watch($($Seconds + 2), '$Target')" | Out-Null
    PyCall 'sl_fire(True)' | Out-Null
    try {
        while (((Get-Date) - $start).TotalSeconds -lt $Seconds) {
            PyCall "aim('$Target')" | Out-Null
            Start-Sleep -Milliseconds 400
            if ((PyCall "sl_is_dead('$Target')") -eq $true) { $dead = $true; break }
        }
    } finally { PyCall 'sl_fire(False)' | Out-Null }
    $hits = (PyCall 'sl_watch_result("damage")').rows
    @{ target = $Target; place = $place; before = $before; dead = $dead; seconds = [math]::Round(((Get-Date) - $start).TotalSeconds, 2)
       hits = $hits; controller = (Py 'print(get_pc().Name)') }
}

# What a step came for, as named yes/no checks. A step whose body ran without an error can still have got nothing: no check, or a
# check that is not true, means it did not. Returns the names that failed ("no evidence" when there are none).
function Get-MissingEvidence($Evidence) {
    if ($null -eq $Evidence -or !@($Evidence.Keys).Count) { return @('no evidence') }
    @($Evidence.Keys | Where-Object { $Evidence[$_] -ne $true })
}

function Test-Number($Value) { $Value -is [ValueType] -and $Value -isnot [bool] }

# The hits a target's TakeDamage hook recorded: one row per hit (the hook also fires for each parent class's TakeDamage, Actor's has no
# numbers), with a number above 0.
function Get-TargetHits([object[]]$Rows, [string]$Target) {
    @($Rows | Where-Object { $_ -and $_.victim -eq $Target -and $_.via -and $_.via -ne 'Actor' -and (Test-Number $_.damage) -and $_.damage -gt 0 })
}

# The target died and its last hit came from Maya's controller with one of the game's damage sources. The probe's own TakeDamage
# (sl_wound, sl_kill) uses the plain DamageType class, so a fallback or a wound does not count as her kill.
function Test-KilledByMaya($Kill) {
    if ($Kill.dead -ne $true) { return $false }
    $hits = @(Get-TargetHits $Kill.hits $Kill.target)
    if (!$hits.Count) { return $false }
    $last = $hits[-1]
    ($last.instigator -eq $Kill.controller) -and ("$($last.damage_type)" -like 'WillowDmgSource_*')
}

# This step's cast started, a skill hit reached the target, and its health really fell at or after that hit (a hook on TakeDamage
# sees the damage asked for, not what was applied).
function Get-CastEvidence([object[]]$Rows, [string]$Target, $KeyBefore, $KeyAfter) {
    $hits = @(Get-TargetHits $Rows $Target | Where-Object { $_.damage_type -eq 'WillowDmgSource_Skill' })
    $fell = $false
    if ($hits.Count) {
        $at = $hits[0].t
        $health = @($Rows | Where-Object { $_ -and $_.event -eq 'target_health' -and (Test-Number $_.health) })
        $before = @($health | Where-Object { $_.t -le $at })
        if ($before.Count) { $fell = @($health | Where-Object { $_.t -ge $at -and $_.health -lt $before[-1].health }).Count -gt 0 }
    }
    @{ cast_started = ($null -ne $KeyAfter -and $KeyAfter -ne $KeyBefore); skill_hit_on_target = $hits.Count -gt 0; health_fell_after_the_hit = $fell }
}

# Every timer sample a number, every kill Maya's, and the timer seen running and then running out.
function Get-TimerEvidence([object[]]$Rows, [object[]]$Kills) {
    $samples = @($Rows | Where-Object { $_ -and $_.PSObject.Properties.Name -contains 'remaining' } | ForEach-Object { $_.remaining })
    $numeric = $samples.Count -gt 0 -and !@($samples | Where-Object { !(Test-Number $_) }).Count
    $lastRunning = -1; $expired = $false
    if ($numeric) {
        for ($i = 0; $i -lt $samples.Count; $i++) { if ($samples[$i] -gt 0) { $lastRunning = $i } }
        $expired = $lastRunning -ge 0 -and $lastRunning -lt ($samples.Count - 1)
    }
    @{ timer_samples_numeric = $numeric; timer_ran = $lastRunning -ge 0; timer_ran_out = $expired
       kills_by_maya = (@($Kills).Count -gt 0 -and !@(@($Kills) | Where-Object { !(Test-KilledByMaya $_) }).Count) }
}

# Both grades set and read back, and the bonus moving with the grade (the saved grade, when it lies between, in between too).
function Get-CritEvidence($Saved, $At1, $At5, $Set1, $Set5) {
    $wasSet = { param($Set, $Grade) $Set.status -eq 'done' -and @($Set.rows).Count -gt 0 -and !@(@($Set.rows) | Where-Object { $_.status -ne 'set' -or $_.after.grade -ne $Grade }).Count }
    $b0 = $Saved.values.CurrentInstantHitCriticalHitBonus; $b1 = $At1.values.CurrentInstantHitCriticalHitBonus; $b5 = $At5.values.CurrentInstantHitCriticalHitBonus
    $moves = (Test-Number $b0) -and (Test-Number $b1) -and (Test-Number $b5) -and $b5 -gt $b1
    $savedGrade = @($Set1.rows)[0].before.grade
    if ($moves -and (Test-Number $savedGrade) -and $savedGrade -gt 1 -and $savedGrade -lt 5) { $moves = $b1 -lt $b0 -and $b0 -lt $b5 }
    @{ grade_1_set_and_read_back = (& $wasSet $Set1 1); grade_5_set_and_read_back = (& $wasSet $Set5 5); bonus_moves_with_the_grade = $moves }
}

# Each name looked up found something; a missing one is named.
function Get-DiscoverEvidence($Found, [string[]]$Names) {
    $evidence = @{}
    foreach ($name in $Names) { $evidence["found_$name"] = @($Found.$name | Where-Object { $_ }).Count -gt 0 }
    $evidence
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
        $keyBefore = PyCall 'sl_cast_key()'
        Send-Key F
        Start-Sleep 7
        $res = PyCall 'sl_watch_result("damage")'
        $ready = Wait-Ready
        $keyAfter = PyCall 'sl_cast_key()'
        @{ target = $t; setup = $setup; watch_start = $watch; result = $res; ready = $ready; cast = @($keyBefore, $keyAfter)
           evidence = Get-CastEvidence $res.rows $t $keyBefore $keyAfter }
    }
}

# The steps that count as done for the run's status: passed, with their evidence, and no possible mix-up.
function Get-StepsDone([object[]]$StepRows) {
    @(@($StepRows | Where-Object { $_ }) | Where-Object { $_.ok -and !$_.possibly_contaminated -and $null -ne $_.missing -and !@($_.missing).Count } | ForEach-Object { $_.step })
}

# Complete means every requested step passed, got what it came for (its evidence) and had no possible mix-up from an earlier step, and
# everything is back as it was. A step that ran without an error is not enough: that is how a run with no kills once reported complete.
function Get-SessionComplete($Log, [object[]]$StepRows, [string[]]$Requested, $Cleanup, [bool]$GameGone, [bool]$GradesBroken, [bool]$RunningUnknown) {
    if ($Log.run_error -or !$GameGone -or $GradesBroken -or $RunningUnknown) { return $false }
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
$snapshots = @{}
$roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
foreach ($d in $extraRoots.Keys) { $roots[$d] = $extraRoots[$d] }
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $runLock = $false; $driverOwned = $false; $driverAttempted = $false; $launchAttempted = $false
# A token on the game's command line that identifies the process this run started (Stop-OwnGame checks it with the
# image path and the start time before stopping a PID). Unknown switches are ignored by the game.
$marker = "-owrun=$Lane-$PID"
try {
    Assert-NoPendingRecords   # an earlier run's pending record stops this one before it takes a lock or changes anything (pending.ps1)
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
    # A root is marked usable only after its copy compares identical; cleanup never restores from any other. A failure ends the run here,
    # before the driver and the game.
    foreach ($d in 'SaveData', 'Config') { $snapshots[$d] = Assert-SnapshotMatches (Join-Path $script:WillowDir $d) (Join-Path $bak $d) }
    foreach ($d in $extraRoots.Keys) {
        if (Test-Path -LiteralPath $extraRoots[$d]) { $snapshots[$d] = New-VerifiedSnapshot $extraRoots[$d] (Join-Path $bak $d) } else { $snapshots[$d] = 'absent' }
    }
    Save-VerifiedBackupRecord $bak $roots $snapshots
    # -Exclusive re-checks that the folder is absent (it may have appeared while this run waited for the locks) and removes
    # its own partial install on failure, so the folder is this run's only once the call has returned.
    $driverAttempted = $true   # before the call: a failed install can leave the folder behind
    Install-Driver -BlockSavesAtStart -Exclusive
    $driverOwned = $true
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $gameArgs = @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', $marker)
    if ($Character) { $gameArgs += "-Character=$Character" }
    $foreign = Get-ForeignGamesSeen $null
    if ($foreign) { throw "Borderlands2 (pid $foreign) started during the setup; not launching a second one" }
    $launchAttempted = $true
    $proc = Start-Game $gameArgs; $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Invoke-GameReply 'print(get_pc())' 8 -SilenceAllowed | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'skills_live/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    Start-Sleep 5; Assert-GameForeground; Send-Key Enter; Start-Sleep 45
    $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
    "map: $map"; if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'skills_live/load_failed.png' | Out-Null; throw "no player pawn: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 1
    # Travel to Three Horns Divide (Ice_P) by the console, as the Phaselock integrity capture does.
    Py 'get_pc().ConsoleCommand("open Ice_P", False); print("sent")' | Out-Null
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep 3
        try { $map = Invoke-GameReply 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn is not None)' 8 -SilenceAllowed } catch { continue }
        if ($map -match 'Ice_P True') { break }
    }
    "map: $map"; if ($map -notmatch 'Ice_P True') { throw "travel to Ice_P failed: $map" }
    Start-Sleep 8
    foreach ($f in 'phaselock.py', 'phaselock_matched.py', 'skills_live.py') {
        $reply = Invoke-GameReplyFile (Join-Path $PSScriptRoot "scripts/$f") skills_live -AllowEmpty
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
        @{ found = $found; evidence = Get-DiscoverEvidence $found @('Helios', 'Ruin', 'Converge', 'SubSequence', 'SweetRelease', 'MindsEye', 'LifeTap', 'Phaselock') }
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
        @{ targets = $targets; watch_start = $watch; kills = @($k1, $k2); result = $res
           evidence = Get-TimerEvidence $res.rows @($k1, $k2) }
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
        @{ as_saved = $asSaved; grade1 = $at1; grade5 = $at5; set1 = $set1; set5 = $set5
           evidence = Get-CritEvidence $asSaved $at1 $at5 $set1 $set5 }
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
           evidence = @{ held_target_killed_by_maya = (Test-KilledByMaya $kill); interrupt_seen = $interrupts.Count -gt 0
                         heal_could_be_observed = (!$immune -and ($health.after -as [double]) -lt ($health.max -as [double])) } }
    }

    # 4 and 5. Helios and Ruin last: Ruin's pulses kill the target, and one controlled target is all that is left by now. Helios
    # goes first, so its restore has to deactivate it: if that fails, the restore check stops the run before Ruin's reading.
    Invoke-DamageCast 'helios' @{ Helios = 5 }
    Invoke-DamageCast 'ruin' @{ Ruin = 1 }
    # Normal end: the game quits itself (the finally block still stops it if it has not).
    # The game may be gone before it answers: no reply, or an empty one, is the expected end here.
    try { Invoke-GameReply 'protect_player(False); get_pc().ConsoleCommand("quit", False)' 10 -AllowEmpty | Out-Null } catch { "quit: no reply ($_); waiting for the exit" }
    for ($i = 0; $i -lt 20 -and (Get-GameProcess); $i++) { Start-Sleep 2 }
} catch {
    $runError = "$_"
    throw
} finally {
    # Each step is isolated so a failure in one (a log write) cannot skip the restore. Order: stop the hooks and put back the grades in game,
    # stop our game, and only once it has exited remove our driver and restore and verify the saves and config; then write the logs and
    # release the locks. If a game may still be running, it could overwrite restored files: nothing is removed or restored, the backup is
    # kept and manual-recovery steps are printed.
    $cleanup = [ordered]@{}
    $cleanup.inputs = Release-HeldInputs   # every key and button this run still holds is let go before anything else
    # A throw between a watch's start and its stop leaves hooks reading pawns into shutdown: stop them while the game can still answer.
    if ($proc -and !$proc.HasExited) {
        # Py returns a traceback as text, so only the exact answer counts.
        try { $reply = Py 'print(sl_stop_all())' 15; $cleanup.hooks = if ($reply -ceq 'stopped') { 'stopped' } else { "not stopped: $reply" } }
        catch { $cleanup.hooks = "not stopped: $_" }
    }
    $gameGone = $true
    if ($proc) {
        try {
            for ($i = 0; $i -lt 10 -and !$proc.HasExited; $i++) { Start-Sleep 1 }
            $cleanup.game = if ($proc.HasExited) { 'exited' } else { Stop-OwnGame $proc $procStart $marker }
            if ($cleanup.game -eq 'stopped' -and !$proc.WaitForExit(15000)) { $cleanup.game = 'stopped, but still running after 15 s' }
            $proc.Refresh(); $gameGone = $proc.HasExited
        } catch { $cleanup.game = "error: $_"; $gameGone = $false }
    }
    $restored = $null; $restoreError = ''
    # Those flags are set before their calls, so a failure inside still counts. An install can only need its folder cleaned up; only a launched
    # game can have changed the saves, so only that calls for a restore, which could otherwise erase another writer's changes.
    $launched = $launchAttempted -or $proc
    $touched = $driverAttempted -or $launched
    # Reading only: a game this run did not start is never stopped, and nothing is removed or restored under or after it.
    $untracked = if ($touched) { Get-ForeignGamesSeen $proc } else { '' }
    $safe = $gameGone -and !$untracked
    if (!$safe) {
        $cleanup.driver = "NOT REMOVED (a game may still run): $script:DriverDir"
        $cleanup.saves = "NOT RESTORED (a game may still run); backup kept at $bak"
    }
    try {
        if ($safe -and $driverOwned) {
            Remove-Driver
            $cleanup.driver = if (Test-Path -LiteralPath $script:DriverDir) { "STILL PRESENT: $script:DriverDir" } else { 'removed' }
        } elseif ($safe -and $driverAttempted) {
            # The install failed. Its own rollback is best effort, and the folder may not be this run's, so it is reported, not removed.
            $cleanup.driver = if (Test-Path -LiteralPath $script:DriverDir) { "STILL PRESENT after a failed install: $script:DriverDir" } else { 'install failed; nothing left' }
        } elseif (!$cleanup.Contains('driver')) { $cleanup.driver = 'not installed by this run; untouched' }
    } catch { $cleanup.driver = "error: $_" }
    $rows = @()
    if ($bak -and $safe -and !$launched) { $cleanup.saves = 'not touched: this run launched no game' }
    elseif ($bak -and $safe) {
        # Once more, immediately before the restore: a game launched during the cleanup could overwrite it.
        $untracked = Get-ForeignGamesSeen $proc
        if ($untracked) { $safe = $false; $cleanup.saves = "NOT RESTORED (a game may still run); backup kept at $bak" }
    }
    if ($bak -and $launched -and $safe) {
        try {
            $restored = Restore-VerifiedRoots $bak $roots $snapshots
            $rows = @($restored.rows); $cleanup.saves = $restored.summary
        } catch { $cleanup.saves = "RESTORE FAILED: $_ (backup kept at $bak)"; $restoreError = "$_" }
    }
    $recovery = Get-RecoveryReason $gameGone $cleanup.game $restored $restoreError $cleanup.driver $untracked
    # This run's pending record (pending.ps1) goes only once every root is the backup again by tree hash, with no game and no driver left.
    $cleanup.pending = Complete-PendingRecord $safe
    if ($cleanup.pending -notlike 'removed*' -and $cleanup.pending -ne 'none written' -and !$recovery) { $recovery = "pending record $($cleanup.pending)" }
    $holdLocks = [bool]$recovery
    # The evidence first and summary.json last, so the status it holds accounts for the rest. Missing evidence cannot read as completed; the
    # restore has already run, and recovery keeps its status.
    $evidenceError = ''
    try {
        if ($restored) { @{ backup = $bak; compared = (Get-Date).ToString('o'); changes = @($rows) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Out 'save_compare.json') }
    } catch { $evidenceError = "$_" }
    $log.completed_steps = @(Get-StepsDone $summary.steps)
    $log.run_error = $runError
    $log.status = Get-RunStatus $stepsRequested @($log.completed_steps) $runError $recovery
    # Every step's evidence is in; the session's own rule also asks for its grades and its cleanup.
    if ($log.status -eq 'completed' -and !(Get-SessionComplete $log $summary.steps $stepsRequested $cleanup $gameGone $script:GradesBroken $script:RunningUnknown)) {
        $log.status = "incomplete: grades or cleanup not clean (grades_broken=$([bool]$script:GradesBroken), running_unknown=$([bool]$script:RunningUnknown), saves=$($cleanup.saves), driver=$($cleanup.driver))"
    }
    if ($evidenceError -and !$recovery) { $log.status = "failed: evidence not written ($evidenceError)" }
    $log.complete = $log.status -eq 'completed'
    $log.finished = (Get-Date).ToString('o'); $log.cleanup = $cleanup
    try { [ordered]@{ run = $log; steps = $summary.steps } | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'summary.json') }
    catch {
        "log write failed: $_"
        if (!$recovery) { $log.status = "failed: evidence not written ($_)"; $log.complete = $false }
    }
    # Anything left for a person keeps both locks, so no other run starts on it.
    if ($holdLocks) { $runLock = $false; $hub = $null }
    try { if ($runLock) { Exit-RunLock } } catch { "run lock release failed: $_" }
    try { if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force } } catch { "hub lock release failed: $_" }
    "status: $($log.status)"
    if ($runError) { "run error: $runError" }
    $cleanup.GetEnumerator() | ForEach-Object { "cleanup $($_.Key): $($_.Value)" }
    Format-RestoreRows $rows
    if ($holdLocks) { Format-ManualRecovery $recovery $bak $HubLock $Lane }
    # Exit 0 only for a completed run with nothing left to recover. Here in the finally, so an error's rethrow passes through it too.
    if ($log.status -ne 'completed') { exit 1 }
}
