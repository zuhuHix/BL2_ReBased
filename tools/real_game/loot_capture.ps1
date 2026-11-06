# Lootable and pickup ground truth (vendors lane): start the game, load the Maya save into Sanctuary with saves blocked from
# the first frame, then for each lootable type take closed, opening and opened frames, then walk over an ammo and a cash
# pickup, and read what material each mesh and placed object uses. Everything written is game data under ignored
# local/realgame/loot_capture/. Takes the screen and keyboard.
# Safety as in phaselock_integrity_capture.ps1: SaveData, Config, the SDK's settings and Sanity Saver's store are backed up and
# verified first, and restored and compared afterwards. Nothing is saved. A game exit not confirmed restores nothing and keeps the
# locks; a restore that does not compare clean keeps the locks and the backup. Both print the one recovery command. Exit 0 only when
# every stage completed. Only the game PID this run started is stopped.
# Questions: is there floating text on loot on the ground? Does collecting a pickup show a message? Which way does each lid
# open? What do the storage locker and strong box render? Placed positions are found in the running game; the only other
# source is the ignored -Interactives file, read at run time for a type not found from the start.
param(
    # The machine-wide hub lock lives in the main checkout's local/ (shared by every worktree on this machine).
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'vendors',
    [string]$Character = 'Save0008.sav',
    [string]$Interactives = (Join-Path $PSScriptRoot '../../local/slice/interactives.json'),
    # Give up waiting for the hub lock after this long and report its contents; a stale lock is removed by its owner or the
    # maintainer, never by this script.
    [int]$HubWaitMinutes = 120
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Folder = 'loot_capture'
$Out = Join-Path $script:RG $Folder
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o'); types = [ordered]@{}; pickups = [ordered]@{} }
# The command channel prints a script's exception as its reply, so a failed step must be turned back into a stop here: a
# failed lc_stand must not be followed by a use key press.
function Py([string]$Code, [int]$Timeout = 30) {
    $reply = (Invoke-GamePy $Code $Timeout).Trim()
    if ($reply -match 'Traceback \(most recent call last\)') { throw "game script failed: $reply" }
    $reply
}
# Keys go to whichever window has focus, so nothing is sent unless it is the game's.
function Assert-Focus { if (-not (Focus-Game)) { throw 'the game window did not take focus; stopping before sending any key' } }
# The mesh a placed object shows, and the balance that places it in the ignored data.
$types = [ordered]@{
    'Skel_DahlChest_Ammo' = 'ObjectGrade_DahlAmmo'; 'Skel_Dumpster' = 'ObjectGrade_Dumpster'
    'Skel_StorageLocker' = 'ObjectGrade_StorageLocker'; 'Skel_StrongBox' = 'ObjectGrade_StrongBox'
    'Skel_CardboardBox' = 'ObjectGrade_Cardboard_Box'; 'Skel_LaundryMachine' = 'ObjectGrade_LaundryMachine'
}
$stagesRequested = @('objects', 'materials', 'ammo') + @($types.Keys) + @('ammo_pickup', 'cash_pickup')

function Test-Shot([string]$Name) { Test-Path -LiteralPath (Join-Path $Out $Name) }

function Capture-Type([string]$Mesh) {
    $row = [ordered]@{}
    $row.stand = Py "print(json.dumps(lc_stand('$Mesh')))"
    Start-Sleep 2
    Assert-Focus
    $row.closed = Save-Shot "$Folder/${Mesh}_closed.png"
    $row.burst = (Invoke-Burst "$Folder/${Mesh}_open" 3 100 { Send-Key E } 1.0).frames
    $row.opened = Save-Shot "$Folder/${Mesh}_opened.png"
    $row.after = Py "print(json.dumps(lc_after('$Mesh')))"
    $row.side = Py "print(json.dumps(lc_side('$Mesh')))"
    Start-Sleep 2
    $row.side_shot = Save-Shot "$Folder/${Mesh}_opened_side.png"
    $row
}

function Capture-Pickup([string]$Kind) {
    $row = [ordered]@{}
    $row.stand = Py "print(json.dumps(lc_stand_pickup('$Kind')))"
    if ($row.stand -match '"pickup": null') { $row.result = 'none on the ground'; return $row }
    Start-Sleep 2
    Assert-Focus
    $row.before = Save-Shot "$Folder/${Kind}_before.png"
    try {
        $row.walk = (Invoke-Burst "$Folder/${Kind}_collect_walk" 2 100 { Send-KeyDown W } 1.0).frames
    } finally { Send-KeyUp W }
    $row.after_burst = (Invoke-Burst "$Folder/${Kind}_collect_after" 2 100 $null 1.0).frames
    $row.state = Py 'print(json.dumps(lc_pickup_state()))'
    $row
}

$extraRoots = [ordered]@{ sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$snapshots = @{}
$roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
foreach ($d in $extraRoots.Keys) { $roots[$d] = $extraRoots[$d] }
$stagesDone = New-Object Collections.Generic.List[string]; $stageProblems = New-Object Collections.Generic.List[string]; $runError = ''
$log.status = 'running'; $log.requested_stages = $stagesRequested
function Set-Stage([string]$Stage, [bool]$Passed, [string]$Why) { if ($Passed) { $stagesDone.Add($Stage) } else { $stageProblems.Add("${Stage}: $Why") } }
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
    # A root is marked usable only after its copy compares identical; cleanup never restores from any other. The SDK's settings
    # and Sanity Saver's store are in the list because both change on every session.
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
    $proc = Start-Game @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', "-Character=$Character", $marker); $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) {
        if ($proc.HasExited) { break }
        try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 }
    }
    if (-not $ok) { $log.boot = 'driver did not answer'; Save-Shot "$Folder/boot.png" | Out-Null; throw 'driver did not answer (check Steam console_log for AppError_5)' }
    $log.block_saves = Py 'print(block_saves())'
    Start-Sleep 5; Assert-Focus; Send-Key Enter; Start-Sleep 45
    $map = ''
    for ($i = 0; $i -lt 6 -and $map -notmatch 'WillowPlayerPawn'; $i++) {   # the level load can hold the command channel
        try { $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)' 30 }
        catch { Start-Sleep 10 }
    }
    "map: $map"; $log.map = $map
    if ($map -notmatch 'WillowPlayerPawn') { Save-Shot "$Folder/load_failed.png" | Out-Null; throw "no player pawn: $map" }
    if ($map -notmatch '(?i)sanctuary') { Save-Shot "$Folder/wrong_map.png" | Out-Null; throw "not in Sanctuary: $map" }
    Assert-Focus; Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 10   # let the streamed levels and their objects spawn
    Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/loot_capture.py') $Folder | Out-Null
    $counts = Py "print(json.dumps(lc_discover('objects_start')))" 60 | ConvertFrom-Json
    "counts: $($counts | ConvertTo-Json -Compress)"; $log.counts_start = $counts

    # A type not in the world yet: go to the first place the ignored data lists for it, and look again.
    if (Test-Path -LiteralPath $Interactives) {
        $places = (Get-Content -Raw -LiteralPath $Interactives | ConvertFrom-Json).objects
        foreach ($mesh in $types.Keys) {
            if ($counts.$mesh -gt 0) { continue }
            $place = $places | Where-Object { "$($_.balance)".EndsWith(".$($types[$mesh])") } | Select-Object -First 1
            if (-not $place) { continue }
            $at = $place.location -join ', '
            Py "print(lc_goto($at))" | Out-Null; Start-Sleep 6
        }
        $counts = Py "print(json.dumps(lc_discover('objects_after_visits')))" 60 | ConvertFrom-Json
        $log.counts_after_visits = $counts
    }
    Set-Stage 'objects' $true ''

    $log.materials = Py "print(json.dumps(lc_materials('materials')))" 60
    "materials: $($log.materials)"
    Set-Stage 'materials' (Test-Shot 'materials.json') 'no materials.json written'
    $log.ammo = Py 'print(json.dumps(lc_lower_ammo()))'
    "ammo: $($log.ammo)"
    $ammo = $log.ammo | ConvertFrom-Json
    Set-Stage 'ammo' (@($ammo.after.PSObject.Properties).Count -gt 0) 'no ammo pool found to lower'

    # One failed type must not lose the others; a lost focus or a dead game still stops the run through its next call.
    foreach ($mesh in $types.Keys) {
        if (-not ($counts.$mesh -gt 0)) { $log.types[$mesh] = 'not in the world'; Set-Stage $mesh $false 'not in the world'; "$mesh`: not in the world"; continue }
        try {
            $row = Capture-Type $mesh; $log.types[$mesh] = $row; "$mesh`: done"
            Set-Stage $mesh (($row.burst -gt 0) -and (Test-Shot "${mesh}_closed.png") -and (Test-Shot "${mesh}_opened.png")) 'frames missing'
        } catch {
            if ($_.Exception.Message -match 'focus') { throw }
            $log.types[$mesh] = "failed: $($_.Exception.Message)"; Set-Stage $mesh $false $_.Exception.Message; "$mesh`: failed: $($_.Exception.Message)"
        }
    }
    foreach ($kind in 'ammo', 'cash') {
        try {
            $row = Capture-Pickup $kind; $log.pickups[$kind] = $row; "$kind`: $($row.state)$($row.result)"
            # Whether a message showed is judged on the frames. This says only that the pickup was walked over and taken.
            Set-Stage "${kind}_pickup" ($row.state -and (($row.state | ConvertFrom-Json).gone -eq $true)) ($(if ($row.result) { $row.result } else { 'not collected' }))
        } catch {
            if ($_.Exception.Message -match 'focus') { throw }
            $log.pickups[$kind] = "failed: $($_.Exception.Message)"; Set-Stage "${kind}_pickup" $false $_.Exception.Message; "$kind`: failed: $($_.Exception.Message)"
        }
    }
    $log.end_shot = Save-Shot "$Folder/end.png"
    # The game may close before it answers this one.
    try { Invoke-GamePy 'get_pc().ConsoleCommand("quit", False)' 15 | Out-Null } catch { "quit: no reply ($_); waiting for the exit" }
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
