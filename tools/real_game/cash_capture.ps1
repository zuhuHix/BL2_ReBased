# Two reads for the range lane in one game session, each its own stage with its own result (session.json stage_results):
# - readings, what a money pickup is worth: start the game, load the Maya save with saves blocked from the first frame, travel to Three Horns
#   Divide (Ice_P), kill enemies with real shots (left at one hit point first) and walk onto the money they drop, reading the cash before and
#   after (scripts/cash_probe.py). Stops at -WantItems single-item readings, -MaxKills kills or -MaxMinutes. Written to cash.json first.
# - den_read, how a den engages a player who stands still: first-blow spread, chase speed, where swings start (scripts/den_probe.py). The map
#   is loaded again for each of -DenTrials trials. Runs after the cash read, so nothing it does can lose that read. -SkipDen leaves it out.
# Takes the screen and keyboard, about 25 minutes at most. Everything written is game data under ignored local/realgame/cash/. Heavy: run it
# only after the hub has answered GRANTED, with Steam logged in as the save's owner (the licence account).
#
# Data safety, on every exit (tests/cash_capture_test.py runs this script with the game stubbed):
# - SaveData, Config, the SDK's settings and SanitySaver are backed up. A root is usable only after its copy compares identical; its hashes
#   are stored and re-checked before any restore, and cleanup restores from no other root.
# - The game must be confirmed gone before anything is removed or restored; otherwise nothing is touched and the locks stay held.
# - A restore that fails or still differs, or a backup that changed, keeps both locks and the backup, and prints the one recovery command.
# - A root created during the run is left in place, named, and the summary says NOT IDENTICAL.
# - session.json has a real status: completed, incomplete, failed or "recovery needed". The exit code is 0 only for completed.
# Only the game PID this run started is stopped (Stop-OwnGame checks its path, start time and marker).
param(
    # The machine-wide hub lock lives in the main checkout's local/ (shared by every worktree on this machine).
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'range-cash',
    [string]$Character = 'Save0008.sav',
    [ValidateRange(1, 1000)][int]$WantItems = 8,
    [int]$MaxKills = 40,
    [int]$MaxMinutes = 12,
    [int]$DenTrials = 3,
    [int]$DenSeconds = 25,
    [switch]$SkipDen,
    # Slice sessions (2026-11-11): every SDK mod's setting turned off for this run (Set-SdkModsOff; the restore puts the settings back from the backup).
    [switch]$SdkModsOff,
    # Slice sessions: a slice_watch spec (tools/real_game/scripts/slice_watch.py). The read-only recorder runs beside the stages, and its dump is
    # checked (tools/real_game/slice_watch_check.py) before the game closes. Empty: no watch.
    [string]$WatchSpec = '',
    # Slice sessions: after the den read, one more den trial with the immunity hook removed, so the pack can down Maya (no input; she stands
    # still). It waits up to -DownSeconds for the down, then -RespawnSeconds for her to stand again. Needs the den read (not with -SkipDen).
    [switch]$DenDown,
    [int]$DownSeconds = 150,
    [int]$RespawnSeconds = 90,
    # What to dot-source for the real-game functions. Only tests/cash_capture_test.py passes another file.
    [string]$Helpers = (Join-Path $PSScriptRoot 'realgame.ps1')
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. $Helpers
. (Join-Path $PSScriptRoot 'slice_watch.ps1')
$Out = Join-Path $script:RG 'cash'
New-Item -ItemType Directory -Force $Out | Out-Null
# Stale evidence: a final write that fails must not leave an earlier run's completed behind, and a dump that fails must not be checked against an earlier
# run's file. Kept under a new name, not deleted: after a run that needs recovery, its record is what names the backup and says what went wrong, and a
# rejected retry must not lose that.
$keptAs = "$(Get-Date -Format 'yyyyMMdd-HHmmss')-$PID"
foreach ($stale in 'session', 'save_compare', 'cash', 'den') { $file = Join-Path $Out "$stale.json"; if (Test-Path -LiteralPath $file) { Move-Item -LiteralPath $file (Join-Path $Out "$stale.$keptAs.json") } }
# End of stale evidence.
$log = [ordered]@{ started = (Get-Date).ToString('o'); status = 'running' }
# The driver writes a game-side error into the reply as text, so it is never taken for an answer.
function Py([string]$Code, [int]$Timeout = 30) {
    $reply = (Invoke-GamePy $Code $Timeout).Trim()
    if ($reply -match 'Traceback \(most recent call last\)') { throw "game python error: $reply" }
    $reply
}
function Use-Probe([string]$Name) {
    $reply = "$(Invoke-GamePyFile (Join-Path $PSScriptRoot "scripts/$Name") cash)"
    if ($reply -match 'Traceback \(most recent call last\)') { throw "$Name did not load: $reply" }
}

$extraRoots = [ordered]@{ sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
foreach ($d in $extraRoots.Keys) { $roots[$d] = $extraRoots[$d] }
$snapshots = @{}
$stagesRequested = @('travel', 'readings'); if (!$SkipDen) { $stagesRequested += 'den_read' }
if ($DenDown -and $SkipDen) { throw '-DenDown needs the den read; it cannot be combined with -SkipDen' }
if ($DenDown) { $stagesRequested += 'den_down' }
if ($WatchSpec) { $stagesRequested += 'watch' }
$stageResults = [ordered]@{}; foreach ($stage in $stagesRequested) { $stageResults[$stage] = 'not run' }
$log.requested_stages = $stagesRequested
$stagesDone = New-Object Collections.Generic.List[string]; $stageProblems = New-Object Collections.Generic.List[string]; $runError = ''
function Set-Stage([string]$Stage, [bool]$Passed, [string]$Why) {
    $stageResults[$Stage] = if ($Passed) { 'passed' } else { "failed: $Why" }
    if ($Passed) { $stagesDone.Add($Stage) } else { $stageProblems.Add("${Stage}: $Why") }
}
# What a dump step reported must match the file it wrote: a stage does not pass on a reply alone.
function Read-DumpFile([string]$Name) {
    $path = Join-Path $Out $Name
    if (!(Test-Path -LiteralPath $path)) { throw "$Name was not written" }
    Get-Content -LiteralPath $path -Raw -Encoding UTF8 | ConvertFrom-Json
}
function Test-CashDump([string]$Reply, [int]$Kills, [int]$Single) {
    $said = $Reply | ConvertFrom-Json
    $file = Read-DumpFile 'cash.json'
    $rows = @($file.real)
    $inFile = @($rows | Where-Object { $null -ne $_.value_per_item }).Count
    if ($said.single_item_rows -ne $Single -or $inFile -ne $Single) { throw "cash.json holds $inFile single-item rows, the probe said $($said.single_item_rows), the run counted $Single" }
    if ($said.collected -ne $rows.Count) { throw "cash.json holds $($rows.Count) collects, the probe said $($said.collected)" }
    if ($said.kills -ne $Kills -or $file.kills -ne $Kills) { throw "cash.json says $($file.kills) kills, the probe said $($said.kills), the run counted $Kills" }
}
function Enter-IceMap {
    Py 'get_pc().ConsoleCommand("open Ice_P", False); print("sent")' | Out-Null
    $reached = ''
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep 3
        try { $reached = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn is not None)' 8 } catch { continue }
        if ($reached -match 'Ice_P True') { break }
    }
    $reached
}
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $runLock = $false; $driverOwned = $false; $driverAttempted = $false; $launchAttempted = $false; $denLoaded = $false
$marker = "-owrun=$Lane-$PID"   # an argument the game ignores; Stop-OwnGame looks for it on the command line
try {
    Assert-NoPendingRecords   # an earlier run's pending record stops this one before it takes a lock or changes anything (pending.ps1)
    # Preflight before any lock or backup: bl-sdk present, our driver folder not already there.
    if (!(Test-Path -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods'))) { throw "bl-sdk is not installed (no sdk_mods in $env:OPENWILLOW_BL2); not installing it" }
    if (Test-Path -LiteralPath $script:DriverDir) { throw "$script:DriverDir already exists; this run did not create it and leaves it alone" }
    # No waiting here: the hub grants the slot first, so a held lock means ask the hub.
    try { $hub = New-Item -ItemType File -Path $HubLock -Value "$Lane $((Get-Date).ToString('o'))" -ErrorAction Stop }
    catch { throw "hub lock $HubLock held by '$(Get-Content $HubLock -Raw -ErrorAction SilentlyContinue)'; not waiting (ask the hub)" }
    if (Get-GameProcess) { throw 'Borderlands2 is running although the hub lock was free; not touching it' }
    Enter-RunLock 600; $runLock = $true
    $bak = Backup-Saves; $log.backup = $bak; "backup $bak"
    foreach ($d in 'SaveData', 'Config') { $snapshots[$d] = Assert-SnapshotMatches (Join-Path $script:WillowDir $d) (Join-Path $bak $d) }
    foreach ($d in $extraRoots.Keys) {
        if (Test-Path -LiteralPath $extraRoots[$d]) { $snapshots[$d] = New-VerifiedSnapshot $extraRoots[$d] (Join-Path $bak $d) } else { $snapshots[$d] = 'absent' }
    }
    # The record outlives this process: the printed recovery command restores from it, and refuses a root that changed since.
    Save-VerifiedBackupRecord $bak $roots $snapshots
    if ($SdkModsOff) { Invoke-SdkModsOffStep $log }
    # -Exclusive re-checks the folder is absent and removes its own partial install on failure, so it is ours only once this returns.
    $driverAttempted = $true   # before the call: a failed install can leave the folder behind
    Install-Driver -BlockSavesAtStart -Exclusive
    $driverOwned = $true
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $foreign = Get-ForeignGamesSeen $null
    if ($foreign) { throw "a game this run did not start is running (pid $foreign); not launching" }
    $launchAttempted = $true
    $proc = Start-Game @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', "-Character=$Character", $marker); $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) {
        if ($proc.HasExited) { break }
        try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 }
    }
    if (-not $ok) { Save-Shot 'cash/boot.png' | Out-Null; throw 'driver did not answer (check Steam console_log for AppError_5)' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    Start-Sleep 5; Assert-GameForeground; Send-Key Enter; Start-Sleep 45
    $map = ''
    for ($i = 0; $i -lt 6 -and $map -notmatch 'WillowPlayerPawn'; $i++) {   # the level load can hold the command channel
        try { $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)' 30 }
        catch { Start-Sleep 10 }
    }
    if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'cash/load_failed.png' | Out-Null; throw "no player pawn: $map" }
    $map = Enter-IceMap
    "map: $map"; $log.map = $map
    Set-Stage 'travel' ($map -match 'Ice_P True') "travel to Ice_P failed: $map"
    if ($map -notmatch 'Ice_P True') { throw "travel to Ice_P failed: $map" }
    Start-Sleep 8; Assert-GameForeground; Send-Key Enter; Start-Sleep 1
    Use-Probe 'combat_probe.py'
    Use-Probe 'cash_probe.py'
    # The immunity is what keeps Maya alive and the measured cash clean: without it, no enemy is touched.
    $log.start = Py 'import json; print(json.dumps(cp_start(), default=str))'
    $started = $log.start | ConvertFrom-Json
    if (!$started -or $started.immune -ne $true) { throw "the immunity hook is not installed: $($log.start)" }
    $log.player_level = Py 'print(cp_level())'; $log.cash_start = Py 'print(cp_cash())'
    if ($WatchSpec) { $log.watch_start = Start-SliceWatch $WatchSpec 'cash' 'cash'; $log.watch_arrival = Save-SliceWatchPawns 'arrival' }
    "player level $($log.player_level), cash $($log.cash_start)"

    $deadline = (Get-Date).AddMinutes($MaxMinutes)
    $kills = 0; $single = 0; $swept = $false; $noTarget = 0; $noReading = 0
    while ($kills -lt $MaxKills -and $single -lt $WantItems -and (Get-Date) -lt $deadline) {
        # Money on the ground first: walk onto it.
        $name = Py 'print(cp_collect_start())'
        if ($name -ne 'None') {
            Send-Key W 200; Start-Sleep 2
            $row = Py 'import json; print(json.dumps(cp_collect_end(), default=str))'
            $log.last_collect = $row
            "collect: $row"
            # A pickup the walk cannot collect would be tried for ever: stop after a few tries in a row without a reading.
            if ($row -match '"value_per_item": \d') { $single++; $noReading = 0 }
            elseif (++$noReading -ge 6) { $log.stopped = "6 collects in a row without a single-item reading"; break }
            continue
        }
        $target = Py 'print(cp_target())'
        if ($target -eq 'None') {
            if (++$noTarget -gt 5) { $log.stopped = 'no enemy within reach'; break }
            Start-Sleep 5; continue
        }
        $noTarget = 0
        Py "print(cp_ready_kill('$target'))" | Out-Null
        Start-Sleep 1; Assert-GameForeground
        for ($shot = 0; $shot -lt 3; $shot++) { Send-Click; Start-Sleep -Milliseconds 700 }
        $kills++
        Start-Sleep 3   # the loot lands
        # The first money seen: the item's own value call at other levels, and its definition. Before any walk onto it.
        if (-not $swept -and (Py 'print(len(cp_money()))') -ne '0') {
            $log.sweep = Py 'import json; print(json.dumps(cp_sweep(), default=str))'
            $log.definition = Py 'import json; print(json.dumps(cp_definition(), default=str))'
            $swept = $true
            "sweep: $($log.sweep)"
        }
    }
    $log.kills = $kills; $log.single_item_readings = $single
    $log.dump = Py 'import json; print(json.dumps(cp_dump(), default=str))' 60
    "dump: $($log.dump)"
    $dumpProblem = ''
    try { Test-CashDump $log.dump $kills $single } catch { $dumpProblem = "$_" }
    # Readings taken with Maya down, or with the immunity gone, are not this stage's readings.
    if (!$dumpProblem -and (Py 'print(cp_up() and cp_immune())') -ne 'True') { $dumpProblem = 'Maya is down or the immunity is gone after the readings' }
    Set-Stage 'readings' (($single -ge $WantItems) -and !$dumpProblem) $(if ($dumpProblem) { $dumpProblem } else { "$single of $WantItems single-item readings in $kills kills" })
    # The den read has its own result: whatever fails in here is that stage's failure, and the session still quits and restores in order.
    if (!$SkipDen) {
        try {
            Use-Probe 'den_probe.py'
            $denLoaded = $true
            $log.den_hooks = Py 'print(dp_start())'
            $counted = 0; $tried = 0
            for ($trial = 0; $trial -lt $DenTrials; $trial++) {
                $reached = Enter-IceMap   # a fresh population: the cash read killed what was near
                if ($reached -notmatch 'Ice_P True') { throw "travel for trial $trial failed: $reached" }
                Start-Sleep 8; Assert-GameForeground; Send-Key Enter; Start-Sleep 1
                if ((Py 'print(cp_immune())') -ne 'True') { throw "the immunity hook is gone before trial $trial" }
                $first = Py 'print(dp_begin())'
                if ($first -eq 'None') { continue }
                $tried++
                Start-Sleep $DenSeconds
                $row = Py 'import json; print(json.dumps(dp_end(), default=str))' 60
                "den trial ${trial}: $row"
                if ($row -match '"counts": true') { $counted++ }
            }
            $log.den_trials_tried = $tried; $log.den_trials_counted = $counted
            $log.den_dump = Py 'import json; print(json.dumps(dp_dump(), default=str))' 60
            $denFile = Read-DumpFile 'den.json'
            if (@($denFile.trials).Count -ne $tried) { throw "den.json holds $(@($denFile.trials).Count) trials, the run tried $tried" }
            # What the stage must have seen, as the probe worked it out and as the file holds it, not only that trials ran.
            $denSaid = $log.den_dump | ConvertFrom-Json
            $unanswered = @($denSaid.missing) + @($denFile.missing) | Where-Object { $_ } | Select-Object -Unique
            Py 'dp_stop(); print("stopped")' | Out-Null
            $why = if ($counted -lt 1) { "$counted of $tried trials saw a melee call and a member move, with Maya whole and no hook error" } else { "$counted of $tried trials counted, unanswered: $($unanswered -join '; ')" }
            Set-Stage 'den_read' (($counted -ge 1) -and !$unanswered) $why
        } catch {
            Set-Stage 'den_read' $false "$_"
            "den read: $_"
        }
    }
    # The den down: the same placement as a den trial, then the immunity hook (this run's own, cp_start) is removed and Maya stands still. No key
    # is pressed. The game's own pack downs her, she bleeds out and respawns; the watch records the hits, the down and the respawn. Saves stay blocked.
    if ($DenDown) {
        try {
            if (!$denLoaded) { Use-Probe 'den_probe.py'; $denLoaded = $true }
            $reached = Enter-IceMap
            if ($reached -notmatch 'Ice_P True') { throw "travel for the den down failed: $reached" }
            Start-Sleep 8; Send-Key Enter; Start-Sleep 1   # Send-Key asserts the game's focus itself
            $first = Py 'print(dp_begin())'
            if ($first -eq 'None') { throw 'no enemy within reach for the den down' }
            if ($WatchSpec) { Save-SliceWatchSnap 'before_down'; $log.watch_before_down = Save-SliceWatchPawns 'before_down' }
            $log.den_down_immune_after_off = Py 'print(cp_immune_off())'
            if ($log.den_down_immune_after_off -ne 'False') { throw "the immunity hook is still installed: $($log.den_down_immune_after_off)" }
            $downAt = $null; $t0 = Get-Date
            while (((Get-Date) - $t0).TotalSeconds -lt $DownSeconds) {
                if ((Py 'print(cp_up())') -ne 'True') { $downAt = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1); break }
                Start-Sleep 1
            }
            $log.den_down_after_s = $downAt
            if ($WatchSpec -and $null -ne $downAt) { Save-SliceWatchSnap 'down' }
            $upAt = $null
            if ($null -ne $downAt) {
                $t1 = Get-Date
                while (((Get-Date) - $t1).TotalSeconds -lt $RespawnSeconds) {
                    Start-Sleep 2
                    if ((Py 'print(cp_up())') -eq 'True') { $upAt = [math]::Round(((Get-Date) - $t1).TotalSeconds, 1); break }
                }
                # The respawn dialog and the fee come a few seconds after she stands: leave them time before the last snapshot.
                Start-Sleep 15
                if ($WatchSpec) { Save-SliceWatchSnap 'after_respawn' }
            }
            $log.den_down_up_after_s = $upAt
            Py 'import json; print(json.dumps(dp_end(), default=str))' 60 | Out-Null
            Set-Stage 'den_down' ($null -ne $downAt -and $null -ne $upAt) "down after $downAt s (limit $DownSeconds), standing again after $upAt s (limit $RespawnSeconds)"
        } catch {
            Set-Stage 'den_down' $false "$_"
            "den down: $_"
        }
    }
    # The watch's usability check, while the game still runs.
    if ($WatchSpec) {
        try { $log.watch = Complete-SliceWatch $WatchSpec $Out; $log.watch.check; Set-Stage 'watch' $log.watch.usable 'the watch dump is not usable (see watch.check)' }
        catch { Set-Stage 'watch' $false "$_" }
    }
    $log.shot = Save-Shot 'cash/shot.png'
    try { Invoke-GamePy 'get_pc().ConsoleCommand("quit", False)' 10 | Out-Null } catch { "quit: no reply ($_); waiting for the exit" }
    for ($i = 0; $i -lt 30 -and -not $proc.HasExited; $i++) { Start-Sleep 2 }
} catch {
    $runError = "$_"
    "run error: $runError"
} finally {
    # Each step is isolated so a failure in one (a log write) cannot skip the restore. Order: stop our game, and only once it has
    # exited remove our driver and restore and verify the roots; then write the logs and release the locks.
    $cleanup = [ordered]@{}
    $cleanup.inputs = Release-HeldInputs   # every key and button this run still holds is let go before anything else
    # A throw in the den read leaves its per-frame hook reading pawns into shutdown: stop it while the game can still answer.
    if ($denLoaded -and $proc -and !$proc.HasExited) {
        # Py returns a traceback as text, so only the exact answer counts.
        try { $reply = Py 'dp_stop(); print("stopped")' 5; $cleanup.hooks = if ($reply -ceq 'stopped') { 'stopped' } else { "not stopped: $reply" } }
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
    # The evidence first and session.json last, so the status it holds accounts for the rest. Missing evidence cannot read as completed; the
    # restore has already run, and recovery keeps its status.
    $evidenceError = ''
    try {
        if ($restored) { @{ backup = $bak; compared = (Get-Date).ToString('o'); changes = @($rows) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Out 'save_compare.json') }
    } catch { $evidenceError = "$_" }
    $log.completed_stages = @($stagesDone); $log.stage_problems = @($stageProblems); $log.stage_results = $stageResults
    # The run's own result, and the status: recovery of the maintainer's files outranks the result.
    $log.run_status = Get-RunStatus $stagesRequested @($stagesDone) $runError
    $log.status = Get-RunStatus $stagesRequested @($stagesDone) $runError $recovery
    if ($evidenceError -and !$recovery) { $log.status = "failed: evidence not written ($evidenceError)" }
    $log.locks_held = $holdLocks
    $log.finished = (Get-Date).ToString('o'); $log.cleanup = $cleanup
    try { $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json') }
    catch {
        "log write failed: $_"
        if (!$recovery) { $log.status = "failed: evidence not written ($_)" }
    }
    # Held locks stop another run starting on files that still need recovery.
    if ($holdLocks) { $runLock = $false; $hub = $null }
    try { if ($runLock) { Exit-RunLock } } catch { "run lock release failed: $_" }
    try { if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force } } catch { "hub lock release failed: $_" }
    "status: $($log.status)"
    if ($runError) { "run error: $runError" }
    $cleanup.GetEnumerator() | ForEach-Object { "cleanup $($_.Key): $($_.Value)" }
    Format-RestoreRows $rows   # as text, so the rows and the steps below reach the screen before `exit`
    if ($holdLocks) { Format-ManualRecovery $recovery $bak $HubLock $Lane }
}
exit $(if ($log.status -eq 'completed') { 0 } else { 1 })
