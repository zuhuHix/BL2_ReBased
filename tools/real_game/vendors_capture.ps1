# Vending machine ground truth (vendors lane): start the game, load the Maya save into Sanctuary with saves blocked
# from the first frame, run scripts/vendors_probe.py vp_dump() once (read-only), quit. Everything written is game data
# under ignored local/realgame/vendors/.
# Safety as in phaselock_integrity_capture.ps1: SaveData, Config, the SDK's settings and Sanity Saver's store are backed up and
# verified first, and restored and compared afterwards. A game exit not confirmed restores nothing and keeps the locks; a restore that
# does not compare clean keeps the locks and the backup. Both print the one recovery command. Exit 0 only when the dump completed.
# Only the game PID this run started is stopped.
param(
    # The machine-wide hub lock lives in the main checkout's local/ (shared by every worktree on this machine).
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'vendors',
    [string]$Character = 'Save0008.sav',
    [string]$Name = 'sanctuary',
    # Give up waiting for the hub lock after this long and report its contents; a stale lock is removed by its owner or the
    # maintainer, never by this script.
    [int]$HubWaitMinutes = 120
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'vendors'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o') }
function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }
$stagesRequested = @('dump')

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
    if (-not $ok) { $log.boot = 'driver did not answer'; Save-Shot 'vendors/boot.png' | Out-Null; throw 'driver did not answer (check Steam console_log for AppError_5)' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    Start-Sleep 5; Assert-GameForeground; Send-Key Enter; Start-Sleep 45
    $map = ''
    for ($i = 0; $i -lt 6 -and $map -notmatch 'WillowPlayerPawn'; $i++) {   # the level load can hold the command channel
        try { $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)' 30 }
        catch { Start-Sleep 10 }
    }
    "map: $map"; $log.map = $map
    if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'vendors/load_failed.png' | Out-Null; throw "no player pawn: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 10   # let the streamed levels and their vending machines spawn
    Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/vendors_probe.py') vendors | Out-Null
    $log.dump = Py "d = vp_dump('$Name'); print(len(d['machines']), [len(m['stock']) if isinstance(m['stock'], list) else m['stock'] for m in d['machines']])" 120
    "dump: $($log.dump)"
    # The command channel prints a script's exception as its reply: only a count of machines is a dump.
    Set-Stage 'dump' ($log.dump -match '^[1-9]\d* \[') "no machines in the dump: $($log.dump)"
    $log.shot = Save-Shot 'vendors/shot.png'
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
    $restored = $null; $restoreError = ''
    if (!$gameGone) {
        $cleanup.driver = "NOT REMOVED (game exit not confirmed): $script:DriverDir"
        $cleanup.saves = "NOT RESTORED (game exit not confirmed); backup kept at $bak"
    }
    try {
        if ($driverOwned -and $gameGone) {
            Remove-Driver
            $cleanup.driver = if (Test-Path -LiteralPath $script:DriverDir) { "STILL PRESENT: $script:DriverDir" } else { 'removed' }
        } elseif (!$cleanup.Contains('driver')) { $cleanup.driver = 'not installed by this run; untouched' }
    } catch { $cleanup.driver = "error: $_" }
    $rows = @()
    # Before the driver is installed or the game started, nothing can have changed, so a partial backup is nothing to recover from.
    if ($bak -and $gameGone -and !$driverOwned -and !$proc) { $cleanup.saves = 'not touched: the game never started' }
    elseif ($bak -and $gameGone) {
        try {
            $restored = Restore-VerifiedRoots $bak $roots $snapshots
            $rows = @($restored.rows); $cleanup.saves = $restored.summary
        } catch { $cleanup.saves = "RESTORE FAILED: $_ (backup kept at $bak)"; $restoreError = "$_" }
    }
    $recovery = Get-RecoveryReason $gameGone $cleanup.game $restored $restoreError $cleanup.driver
    $holdLocks = [bool]$recovery
    try {
        $log.completed_stages = @($stagesDone); $log.stage_problems = @($stageProblems)
        $log.run_error = $runError
        $log.status = Get-RunStatus $stagesRequested @($stagesDone) $runError $recovery
        $log.finished = (Get-Date).ToString('o'); $log.cleanup = $cleanup
        $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')
        if ($bak -and $gameGone) { @{ backup = $bak; compared = (Get-Date).ToString('o'); changes = @($rows) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Out 'save_compare.json') }
    } catch {
        # The evidence is missing, so this run cannot read as completed. The restore has already run, and recovery keeps its status.
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
