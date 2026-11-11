# Which sound events Borderlands 2 posts for the slice's sounds: one real-game session in Three Horns Divide (Ice_P) that logs
# the game's sound calls (scripts/audio_events.py) in labelled windows around a Phaselock cast, shots at the sky, shots at a
# target and a picked-up item, with an idle window as the baseline.
# Logging only: the hooks record and change nothing. A function called from native code passes no hook, so a stage that ends
# incomplete means "not seen here". A result is an observation of one session and one gun: it stays UNVERIFIED until compared in play.
# Takes the screen and keyboard. Everything written is game data under ignored local/realgame/audio_events/.
# Locks: the shared hub lock and the run lock; saves are backed up, saving is blocked in game from the first frame, and afterwards
# all four roots are compared with the backup (changed files are restored from it). The tail is phaselock_integrity_capture.ps1's, but for
# which hooks it stops (a test holds that). Nothing changes a file before the driver install and game start: a run with neither skips the restore.
param(
    # The machine-wide hub lock lives in the main checkout's local/ (shared by every worktree on this machine).
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'audio-events',
    [string]$Character = 'Save0008.sav',   # quick_startup's -Character slot: the level-8 Maya (Phaselock) in Sanctuary
    # Slice sessions (2026-11-11): every SDK mod's setting turned off for this run (Set-SdkModsOff; the restore puts the settings back from the backup).
    [switch]$SdkModsOff,
    # With -SdkModsOff: the exact setting names it may turn off; any other enabled set is refused before anything is written.
    [string[]]$SdkModsExpected = @(),
    # Slice sessions: a slice_watch spec (tools/real_game/scripts/slice_watch.py). The read-only recorder runs beside the stages, and its dump is
    # checked (tools/real_game/slice_watch_check.py) before the game closes. Empty: no watch.
    [string]$WatchSpec = '',
    [switch]$ProbeOnly,                     # stop after installing the hooks
    # Give up waiting for the hub lock after this long and report its contents; a stale lock is removed by its owner or the
    # maintainer, never by this script.
    [int]$HubWaitMinutes = 120
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
. (Join-Path $PSScriptRoot 'slice_watch.ps1')
# Slice sessions: a bad spec, or too little free disk for the backups and the recording, stops the run here, before anything is changed.
if ($WatchSpec) { Assert-WatchSpec $WatchSpec }
if ($WatchSpec -or $SdkModsOff) { $null = Assert-DiskReserve }
$Out = Join-Path $script:RG 'audio_events'
New-Item -ItemType Directory -Force $Out | Out-Null
# Stale evidence: a final write that fails must not leave an earlier run's completed behind. Kept under a new name, not deleted: after a run that
# needs recovery, its record is what names the backup and says what went wrong, and a rejected retry must not lose that.
$keptAs = "$(Get-Date -Format 'yyyyMMdd-HHmmss')-$PID"
foreach ($stale in 'session', 'save_compare') { $file = Join-Path $Out "$stale.json"; if (Test-Path -LiteralPath $file) { Move-Item -LiteralPath $file (Join-Path $Out "$stale.$keptAs.json") } }
# End of stale evidence.
# Stale event files: a window a later run does not reach (a retry it did not need) would otherwise keep an earlier run's file. They are that run's only
# record of its calls, so they move aside rather than go.
$staleEvents = @(Get-ChildItem -LiteralPath $Out -Filter 'events_*.jsonl' -File -ErrorAction SilentlyContinue)
if ($staleEvents.Count) {
    $earlier = Join-Path $Out "earlier-$keptAs"
    New-Item -ItemType Directory -Force $earlier | Out-Null
    $staleEvents | Move-Item -Destination $earlier
}
# End of stale event files.
$log = [ordered]@{ started = (Get-Date).ToString('o') }

function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }

# One labelled window: $Act runs with the hooks counting. What it adds beyond the $Baselines windows lands in $log.window_<label>, with `tied`:
# the new events that followed at least $Need of the action's moments within $Within seconds. Stages accept only `tied`, so an ambient
# sound that merely falls in the window does not count. That is still correlation, not proof the event belongs to the action.
function Invoke-Window([string]$Label, [string[]]$Baselines, [scriptblock]$Act, [string]$Marker = '', [double]$Within = 0.5, [int]$Need = 1) {
    $script:Presses = New-Object Collections.Generic.List[double]
    Py "ae_begin('$Label'); print('ok')" | Out-Null
    & $Act | Out-Null
    Start-Sleep -Milliseconds 800     # the last sound's calls
    $names = ($Baselines | ForEach-Object { "'$_'" }) -join ', '
    $times = ($script:Presses | ForEach-Object { $_.ToString('R', [Globalization.CultureInfo]::InvariantCulture) }) -join ', '
    $result = Py "import json; print(json.dumps(ae_end([$names], [$times], '$Marker', $Within, $Need), default=str))" | ConvertFrom-Json
    $log["window_$Label"] = $result
    "$Label`: rows $($result.rows), errors $($result.errors), dropped $($result.dropped), new events: $($result.new -join ' '); tied to the action ($($result.anchored_by), $($result.anchors) moments): $($result.tied -join ' ')"
}

# Why a window cannot count: a hook that failed, or rows past the cap, lost calls, so "no event" would mean nothing. Empty when none were lost.
function Get-HookErrors($Window) {
    if (!$Window) { return 'no window; ' }
    $lost = ''
    if ($Window.errors -gt 0) { $lost += "hooks failed $($Window.errors) times, first: $($Window.first_error); " }
    if ($Window.dropped -gt 0) { $lost += "$($Window.dropped) calls dropped past the row limit; " }
    $lost
}

function Send-Shots([int]$Count) {
    Assert-GameForeground; Start-Sleep -Milliseconds 300
    for ($i = 0; $i -lt $Count; $i++) { $script:Presses.Add((Get-QpcSeconds)); Send-Click; Start-Sleep -Milliseconds 450 }
}
function Send-ActionKey([string]$Key) { $script:Presses.Add((Get-QpcSeconds)); Send-Key $Key }

# Wait until the lift skill is idle and the cooldown pool is empty (or $Timeout s).
function Wait-Ready([double]$Timeout = 40) {
    $deadline = (Get-Date).AddSeconds($Timeout)
    while ((Get-Date) -lt $deadline) {
        $r = Py 'import json; s = lift_skill(); p = get_pc().SkillCooldownPool.Data; print(json.dumps({"state": str(s.CurrentState), "pool": p.CurrentValue}))' | ConvertFrom-Json
        if ($r.state -eq '0' -and $r.pool -le 0.001) { return $r }
        Start-Sleep 1
    }
    throw 'cooldown did not end'
}

$extraRoots = [ordered]@{ sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$snapshots = @{}
$roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
foreach ($d in $extraRoots.Keys) { $roots[$d] = $extraRoots[$d] }
$stagesRequested = if ($ProbeOnly) { @('hooks') } else { @('hooks', 'baseline', 'phaselock', 'fire_away', 'fire_hit', 'pickup') }
if ($WatchSpec -and !$ProbeOnly) { $stagesRequested = @($stagesRequested) + @('watch') }
$stagesDone = New-Object Collections.Generic.List[string]; $stageProblems = New-Object Collections.Generic.List[string]; $runError = ''
$log.status = 'running'; $log.requested_stages = $stagesRequested
function Set-Stage([string]$Stage, [bool]$Passed, [string]$Why) { if ($Passed) { $stagesDone.Add($Stage) } else { $stageProblems.Add("${Stage}: $Why") } }
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $target = $null; $runLock = $false; $driverOwned = $false; $driverAttempted = $false; $launchAttempted = $false
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
    # The SDK's settings change every session and Sanity Saver has rewritten its store before, so both are compared too.
    # A root is marked usable only after its copy compares identical; cleanup never restores from any other.
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
    # After the flag: from here the unchanged finalizer restores every root from the verified backup, so settings turned off here are put back
    # even when the launch never happens or the write stops half way.
    if ($SdkModsOff) { Invoke-SdkModsOffStep $log $SdkModsExpected }
    $proc = Start-Game $gameArgs; $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'audio_events/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    Start-Sleep 5; Assert-GameForeground; Send-Key Enter; Start-Sleep 45
    $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
    "map: $map"; if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'audio_events/load_failed.png' | Out-Null; throw "no player pawn: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 1
    Py 'get_pc().ConsoleCommand("open Ice_P", False); print("sent")' | Out-Null
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep 3
        try { $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn is not None)' 8 } catch { continue }
        if ($map -match 'Ice_P True') { break }
    }
    "map: $map"; if ($map -notmatch 'Ice_P True') { throw "travel to Ice_P failed: $map" }
    Start-Sleep 8
    foreach ($f in 'phaselock.py', 'phaselock_matched.py', 'phaselock_integrity.py', 'audio_events.py') { Invoke-GamePyFile (Join-Path $PSScriptRoot "scripts/$f") audio_events | Out-Null }
    if ($WatchSpec -and !$ProbeOnly) { $log.watch_start = Start-SliceWatch $WatchSpec 'audio_events' 'audio_events'; $log.watch_arrival = Save-SliceWatchPawns 'arrival' }
    $log.hooks = Py 'import json; print(json.dumps(ae_install()))' | ConvertFrom-Json
    "hooked $(@($log.hooks.hooked).Count), missing: $($log.hooks.missing -join ' ')"
    Set-Stage 'hooks' (@($log.hooks.hooked).Count -gt 0) 'no sound function could be hooked'
    $log.weapon = Py 'import json; print(json.dumps(ae_weapon()))' | ConvertFrom-Json
    if ($ProbeOnly) { return }

    # The same target rule as the Phaselock capture: the exact Adult, the one that capture has lifted.
    for ($i = 0; $i -lt 24 -and !$target; $i++) {
        $pick = Py @'
import json
rows = []
for d, n, c, dead in list_ai():
    p = ai_pawn(n)
    cls = p.AIClass._path_name() if p.AIClass else ""
    radius = p.CylinderComponent.CollisionRadius if p.CylinderComponent else 0
    rows.append({"name": n, "dist": d, "class": cls.split(".")[-1], "radius": radius, "dead": dead, "ctrl": p.Controller is not None})
pick = pi_pick_target(rows)
print(json.dumps({"pick": pick["name"] if pick else None}))
'@
        $target = ($pick | ConvertFrom-Json).pick
        if (!$target) { Start-Sleep 5 }
    }
    if (!$target) { throw "no live CharClass_PrimalBeastAdult after 2 minutes of polling: $pick" }
    $log.target = $target; "target $target"
    function Set-Up {
        Py "protect_player(True); isolate('$target'); place('$target', 650.0); print('ok')" | Out-Null
        Start-Sleep 3
        Py "isolate('$target'); aim('$target'); print('ok')" | Out-Null
        Assert-GameForeground; Start-Sleep -Milliseconds 500
    }
    Set-Up

    # Baseline: what the world posts with nobody acting.
    Invoke-Window 'idle' @() { Start-Sleep 3 }
    # Failed hooks here would empty the baseline, and every later event would look new.
    $idleErrors = Get-HookErrors $log.window_idle
    Set-Stage 'baseline' (!$idleErrors) $idleErrors

    # 1. Phaselock. A cast that did not lock the target is retried, each attempt in its own window. Whether it locked comes from the skill's
    # own LockTarget call: no recorder, so no pawn is held across commands.
    $lifted = $false; $castTied = @()
    for ($attempt = 1; $attempt -le 3 -and !$lifted; $attempt++) {
        $name = if ($attempt -eq 1) { 'phaselock' } else { "phaselock_try$attempt" }
        if ($attempt -gt 1) { $log["${name}_ready"] = Wait-Ready 40; Set-Up }
        Invoke-Window $name @('idle') { Assert-GameForeground; Send-ActionKey F; Start-Sleep 6 } 'PhaseLockTarget' 1.0 1
        $lifted = $log["window_$name"].calls.LockTarget -gt 0
        $log["${name}_locked"] = $lifted
        if ($lifted) { $castTied = @($log["window_$name"].tied) }
    }
    $castErrors = Get-HookErrors $log["window_$name"]
    Set-Stage 'phaselock' (!$castErrors -and $lifted -and $castTied.Count -gt 0) "${castErrors}target not locked, or no new sound event followed the cast"
    Start-Sleep 6     # the lock ends and the target lands

    # 2. Shots at the sky: the gun's own sound, with nothing hit.
    Py "pc = get_pc(); pc.Rotation = unrealsdk.make_struct('Rotator', Pitch=10922, Yaw=pc.Rotation.Yaw, Roll=0); print('ok')" | Out-Null
    Invoke-Window 'fire_away' @('idle') { Send-Shots 3 } 'PlayFiringSound' 0.4 2
    $awayErrors = Get-HookErrors $log.window_fire_away
    Set-Stage 'fire_away' (!$awayErrors -and @($log.window_fire_away.tied).Count -gt 0) "${awayErrors}no new sound event followed at least 2 shots"

    # 3. Shots at the target: what the hit adds to the shots above. The health says whether a shot landed.
    Set-Up
    $before = Py "import json; print(json.dumps(ae_target('$target')))" | ConvertFrom-Json
    Invoke-Window 'fire_hit' @('idle', 'fire_away') { Send-Shots 5 } 'PlayFiringSound' 0.5 2
    $after = Py "import json; print(json.dumps(ae_target('$target')))" | ConvertFrom-Json
    $log.fire_hit_target = @{ before = $before; after = $after }
    # A target gone after the shots counts as landed, and $log says it was gone rather than at zero.
    $landed = ($before.gone -eq $false) -and (($after.gone -eq $true) -or ($after.health -lt $before.health))
    $hitErrors = Get-HookErrors $log.window_fire_hit
    Set-Stage 'fire_hit' (!$hitErrors -and $landed -and @($log.window_fire_hit.tied).Count -gt 0) "${hitErrors}no shot shown to land (health $($before.health) -> $($after.health), gone $($after.gone)), or no new sound event followed at least 2 shots"

    # 4. Pickup: drop a backpack item, stand at it and press use.
    $log.thrown = Py 'import json; print(json.dumps(ae_throw(), default=str))' | ConvertFrom-Json
    Start-Sleep 2
    $pickup = Py 'print(ae_face_pickup())'
    $log.pickup_target = $pickup
    $gone = $false
    if ($pickup -ne 'None') {
        Start-Sleep 1
        Invoke-Window 'pickup' @('idle') { Assert-GameForeground; Send-ActionKey E; Start-Sleep 1; Send-ActionKey E; Start-Sleep 1 } 'HandlePickup' 1.0 1
        $gone = (Py "print(ae_pickup_gone('$pickup'))") -eq 'True'
    }
    $log.pickup_gone = $gone
    $pickupErrors = Get-HookErrors $log.window_pickup
    Set-Stage 'pickup' (!$pickupErrors -and $gone -and @($log.window_pickup.tied).Count -gt 0) "${pickupErrors}pickup '$pickup' of '$($log.thrown.thrown)' gone: $gone, or no new sound event followed the press"
    # A tied event that other windows also saw is weaker evidence: kept beside the results rather than decided here.
    $log.event_overlap = Py 'import json; print(json.dumps(ae_overlap()))' | ConvertFrom-Json
    # The watch's usability check, while the game still runs: a failed check is this stage's failure, and the run still quits and restores in order.
    if ($WatchSpec -and !$ProbeOnly) {
        try { $log.watch = Complete-SliceWatch $WatchSpec $Out; $log.watch.check; Set-Stage 'watch' $log.watch.usable 'the watch dump is not usable (see watch.check)' }
        catch { Set-Stage 'watch' $false "$_" }
    }
    # The game may close before it answers this one.
    try { Invoke-GamePy 'ae_remove(); protect_player(False); get_pc().ConsoleCommand("quit", False)' 15 | Out-Null } catch { "quit: no reply ($_); waiting for the exit" }
    for ($i = 0; $i -lt 20 -and (Get-GameProcess); $i++) { Start-Sleep 2 }
} catch {
    $runError = "$_"
    throw
} finally {
    # Each step is isolated so a failure in one (a log write) cannot skip the restore. Order: stop our game, and only once it
    # has exited remove our driver and restore and verify the saves and config; then write the logs and release the locks.
    # If the game's exit is not confirmed, a live game could overwrite restored files: nothing is removed or restored, the
    # backup is kept and manual-recovery steps are printed.
    $cleanup = [ordered]@{}
    $cleanup.inputs = Release-HeldInputs   # every key and button this run still holds is let go before anything else
    # A throw can leave this run's logging hooks and the player's protection on: stop them while the game can still answer.
    if ($proc -and !$proc.HasExited) {
        # Py returns a traceback as text, so only the exact answer counts.
        try { $reply = Py 'ae_remove(); protect_player(False); print("stopped")' 5; $cleanup.hooks = if ($reply -ceq 'stopped') { 'stopped' } else { "not stopped: $reply" } }
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
    $log.completed_stages = @($stagesDone); $log.stage_problems = @($stageProblems)
    $log.run_error = $runError
    $log.status = Get-RunStatus $stagesRequested @($stagesDone) $runError $recovery
    if ($stageProblems.Count -and $log.status -ne 'completed') { $log.status = "$($log.status) ($($stageProblems -join '; '))" }
    if ($evidenceError -and !$recovery) { $log.status = "failed: evidence not written ($evidenceError)" }
    $log.finished = (Get-Date).ToString('o'); $log.cleanup = $cleanup
    try { $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json') }
    catch {
        "log write failed: $_"
        if (!$recovery) { $log.status = "failed: evidence not written ($_)" }
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
    # Exit 0 only for a completed run with nothing left to recover. Here in the finally, so -ProbeOnly's return passes through it too.
    if ($log.status -ne 'completed') { exit 1 }
}
