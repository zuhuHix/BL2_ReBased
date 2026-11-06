# Which sound events Borderlands 2 posts for the slice's sounds: one real-game session in Three Horns Divide (Ice_P) that logs
# the game's sound calls (scripts/audio_events.py) in labelled windows around a Phaselock cast, shots at the sky, shots at a
# target and a picked-up item, with an idle window as the baseline.
# Logging only: the hooks record and change nothing. A function called from native code passes no hook, so a stage that ends
# incomplete means "not seen here". A result is an observation of one session and one gun: it stays UNVERIFIED until compared in play.
# Takes the screen and keyboard. Everything written is game data under ignored local/realgame/audio_events/.
# Locks: the shared hub lock and the run lock; saves are backed up, saving is blocked in game from the first frame, and afterwards
# all four roots are compared with the backup (changed files are restored from it). The tail is phaselock_integrity_capture.ps1's.
param(
    # The machine-wide hub lock lives in the main checkout's local/ (shared by every worktree on this machine).
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'audio-events',
    [string]$Character = 'Save0008.sav',   # quick_startup's -Character slot: the level-8 Maya (Phaselock) in Sanctuary
    [switch]$ProbeOnly,                     # stop after installing the hooks
    # Give up waiting for the hub lock after this long and report its contents; a stale lock is removed by its owner or the
    # maintainer, never by this script.
    [int]$HubWaitMinutes = 120
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'audio_events'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o') }

function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }

# One labelled window: $Act runs with the hooks counting. What it adds beyond the $Baselines windows lands in $log.window_<label>.
function Invoke-Window([string]$Label, [string[]]$Baselines, [scriptblock]$Act) {
    Py "ae_begin('$Label'); print('ok')" | Out-Null
    & $Act | Out-Null
    Start-Sleep -Milliseconds 800     # the last sound's calls
    $names = ($Baselines | ForEach-Object { "'$_'" }) -join ', '
    $result = Py "import json; print(json.dumps(ae_end([$names]), default=str))" | ConvertFrom-Json
    $log["window_$Label"] = $result
    "$Label`: rows $($result.rows), new events: $($result.new -join ' ')"
}

function Send-Shots([int]$Count) {
    Focus-Game | Out-Null; Start-Sleep -Milliseconds 300
    for ($i = 0; $i -lt $Count; $i++) { Send-Click; Start-Sleep -Milliseconds 450 }
}

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
$stagesRequested = if ($ProbeOnly) { @('hooks') } else { @('hooks', 'phaselock', 'fire_away', 'fire_hit', 'pickup') }
$stagesDone = New-Object Collections.Generic.List[string]; $stageProblems = New-Object Collections.Generic.List[string]; $runError = ''
$log.status = 'running'; $log.requested_stages = $stagesRequested
function Set-Stage([string]$Stage, [bool]$Passed, [string]$Why) { if ($Passed) { $stagesDone.Add($Stage) } else { $stageProblems.Add("${Stage}: $Why") } }
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $target = $null; $runLock = $false; $driverOwned = $false
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
    # The SDK's settings change every session and Sanity Saver has rewritten its store before, so both are compared too.
    # A root is marked usable only after its copy compares identical; cleanup never restores from any other.
    foreach ($d in 'SaveData', 'Config') { $snapshots[$d] = Assert-SnapshotMatches (Join-Path $script:WillowDir $d) (Join-Path $bak $d) }
    foreach ($d in $extraRoots.Keys) {
        if (Test-Path -LiteralPath $extraRoots[$d]) { $snapshots[$d] = New-VerifiedSnapshot $extraRoots[$d] (Join-Path $bak $d) } else { $snapshots[$d] = 'absent' }
    }
    Save-VerifiedBackupRecord $bak $roots $snapshots
    # -Exclusive re-checks that the folder is absent (it may have appeared while this run waited for the locks) and removes
    # its own partial install on failure, so the folder is this run's only once the call has returned.
    Install-Driver -BlockSavesAtStart -Exclusive
    $driverOwned = $true
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $gameArgs = @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', $marker)
    if ($Character) { $gameArgs += "-Character=$Character" }
    $proc = Start-Game $gameArgs; $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'audio_events/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Py 'print(block_saves())'
    Start-Sleep 5; Focus-Game | Out-Null; Send-Key Enter; Start-Sleep 45
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
        Focus-Game | Out-Null; Start-Sleep -Milliseconds 500
    }
    Set-Up

    # Baseline: what the world posts with nobody acting.
    Invoke-Window 'idle' @() { Start-Sleep 3 }

    # 1. Phaselock. A cast that did not lift the target is retried, each attempt in its own window.
    $lifted = $false; $castNew = @()
    for ($attempt = 1; $attempt -le 3 -and !$lifted; $attempt++) {
        $name = if ($attempt -eq 1) { 'phaselock' } else { "phaselock_try$attempt" }
        if ($attempt -gt 1) { $log["${name}_ready"] = Wait-Ready 40; Set-Up }
        Py "import json; print(json.dumps(pi_start('$name', '$target'), default=str))" | Out-Null
        Invoke-Window $name @('idle') { Focus-Game | Out-Null; Send-Key F; Start-Sleep 6 }
        $result = Py 'import json; print(json.dumps(pi_result(), default=str))' | ConvertFrom-Json
        Py 'pi_stop(); print("stopped")' | Out-Null
        $log["${name}_lifted"] = $result.target_lifted
        $lifted = $result.target_lifted -eq $true
        if ($lifted) { $castNew = @($log["window_$name"].new) }
    }
    Set-Stage 'phaselock' ($lifted -and $castNew.Count -gt 0) 'target not lifted, or no new sound event seen'
    Start-Sleep 6     # the lock ends and the target lands

    # 2. Shots at the sky: the gun's own sound, with nothing hit.
    Py "pc = get_pc(); pc.Rotation = unrealsdk.make_struct('Rotator', Pitch=10922, Yaw=pc.Rotation.Yaw, Roll=0); print('ok')" | Out-Null
    Invoke-Window 'fire_away' @('idle') { Send-Shots 3 }
    Set-Stage 'fire_away' (@($log.window_fire_away.new).Count -gt 0) 'no new sound event seen'

    # 3. Shots at the target: what the hit adds to the shots above. The health says whether a shot landed.
    Set-Up
    $before = [double](Py "print(ae_health('$target'))")
    Invoke-Window 'fire_hit' @('idle', 'fire_away') { Send-Shots 5 }
    $after = [double](Py "print(ae_health('$target'))")
    $log.fire_hit_health = @{ before = $before; after = $after }
    Set-Stage 'fire_hit' (($after -lt $before) -and @($log.window_fire_hit.new).Count -gt 0) "health $before -> $after, or no new sound event seen"

    # 4. Pickup: drop a backpack item, stand at it and press use.
    $log.thrown = Py 'import json; print(json.dumps(ae_throw(), default=str))' | ConvertFrom-Json
    Start-Sleep 2
    $pickup = Py 'print(ae_face_pickup())'
    $log.pickup_target = $pickup
    $gone = $false
    if ($pickup -ne 'None') {
        Start-Sleep 1
        Invoke-Window 'pickup' @('idle') { Focus-Game | Out-Null; Send-Key E; Start-Sleep 1; Send-Key E; Start-Sleep 1 }
        $gone = (Py "print(ae_pickup_gone('$pickup'))") -eq 'True'
    }
    $log.pickup_gone = $gone
    Set-Stage 'pickup' ($gone -and @($log.window_pickup.new).Count -gt 0) "pickup '$pickup' gone: $gone, or no new sound event seen"
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
    $gameGone = $true
    if ($proc) {
        try {
            for ($i = 0; $i -lt 10 -and !$proc.HasExited; $i++) { Start-Sleep 1 }
            $cleanup.game = if ($proc.HasExited) { 'exited' } else { Stop-OwnGame $proc $procStart $marker }
            if ($cleanup.game -eq 'stopped' -and !$proc.WaitForExit(15000)) { $cleanup.game = 'stopped, but still running after 15 s' }
            $proc.Refresh(); $gameGone = $proc.HasExited
        } catch { $cleanup.game = "error: $_"; $gameGone = $false }
    }
    $recovery = ''
    # One way to restore by hand: this helper re-checks the backup against its record and refuses a root that changed.
    $restoreCommand = "Restore-VerifiedBackup '$bak'"
    if (!$gameGone) {
        $cleanup.driver = "NOT REMOVED (game exit not confirmed): $script:DriverDir"
        $cleanup.saves = "NOT RESTORED (game exit not confirmed); backup kept at $bak"
        $recovery = "game exit not confirmed ($($cleanup.game))"
        @"
MANUAL RECOVERY: Borderlands 2 (pid $($proc.Id)) did not confirm its exit ($($cleanup.game)). Nothing was removed or restored.
  1. Close Borderlands 2 yourself.
  2. In a new PowerShell window: `$env:OPENWILLOW_BL2 = '$env:OPENWILLOW_BL2'; . '$PSScriptRoot\realgame.ps1'
     Remove-Item -Recurse -Force '$script:DriverDir'
     $restoreCommand
     Every row must read 'added, removed', 'missing, restored' or 'changed, restored'. A 'MISMATCH after restore', 'RESTORE FAILED' or 'BACKUP CHANGED' row means that root is not recovered; a changed backup is refused, so compare it with the live files by hand.
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
    $rows = @(); $holdLocks = $false
    if ($bak) {
        try {
            $restored = Restore-VerifiedRoots $bak $roots $snapshots
            $rows = @($restored.rows); $cleanup.saves = $restored.summary; $holdLocks = !$restored.clean
            if ($holdLocks) { $recovery = "restore did not compare clean ($($restored.summary))" }
        } catch { $cleanup.saves = "RESTORE FAILED: $_ (backup kept at $bak)"; $holdLocks = $true; $recovery = "restore failed ($_)" }
    }
    if (!$recovery -and $cleanup.driver -match '^(STILL PRESENT|error)') { $recovery = "driver not removed ($($cleanup.driver))" }
    try {
        $log.completed_stages = @($stagesDone); $log.stage_problems = @($stageProblems)
        $log.run_error = $runError
        $log.status = Get-RunStatus $stagesRequested @($stagesDone) $runError $recovery
        $log.finished = (Get-Date).ToString('o'); $log.cleanup = $cleanup
        $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')
        if ($bak) { @{ backup = $bak; compared = (Get-Date).ToString('o'); changes = @($rows) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Out 'save_compare.json') }
    } catch { "log write failed: $_" }
    # A failed restore keeps both locks, so no other run starts on files that still need recovery.
    if ($holdLocks) { $runLock = $false; $hub = $null }
    try { if ($runLock) { Exit-RunLock } } catch { "run lock release failed: $_" }
    try { if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force } } catch { "hub lock release failed: $_" }
    "status: $($log.status)"
    if ($runError) { "run error: $runError" }
    $cleanup.GetEnumerator() | ForEach-Object { "cleanup $($_.Key): $($_.Value)" }
    Format-RestoreRows $rows
    if ($holdLocks) {
        @"
MANUAL RECOVERY: the restore did not compare clean ($($cleanup.saves)). Both locks are held and the backup is kept at $bak.
  1. Make sure Borderlands 2 is closed. In a new PowerShell window: `$env:OPENWILLOW_BL2 = '$env:OPENWILLOW_BL2'; . '$PSScriptRoot\realgame.ps1'
     $restoreCommand
     Every row must read 'added, removed', 'missing, restored' or 'changed, restored'. A 'MISMATCH after restore', 'RESTORE FAILED' or 'BACKUP CHANGED' row means that root is not recovered; a changed backup is refused, so compare it with the live files by hand.
  2. Only when no row reports a mismatch, remove the locks this run left: '$HubLock' (if it names '$Lane') and '$script:Lock'.
"@
    }
    # Exit 0 only for a completed run with nothing left to recover. Here in the finally, so -ProbeOnly's return passes through it too.
    if ($log.status -ne 'completed') { exit 1 }
}
