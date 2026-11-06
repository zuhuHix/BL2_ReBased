# Marcus's mission screen and the respawn dialog in the real game (mission UI lane): one session in Sanctuary with the level-8 Maya. It answers what the
# host's page must do: which keys the screen takes, what closes it, whether the world runs behind it, what the reward page shows, and what the
# respawn dialog does. Rows from scripts/mission_screen_probe.py and named frames, under ignored local/realgame/mission_screen/.
# Takes the screen and keyboard. Locks: the shared hub lock and the run lock; saves are backed up, saving is blocked in game from the first frame, and
# afterwards all four roots are compared with the backup (changed files are restored from it).
param(
    # The machine-wide hub lock lives in the main checkout's local/ (shared by every worktree on this machine).
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'mission-screen',
    [string]$Character = 'Save0008.sav',   # quick_startup's -Character slot: the level-8 Maya in Sanctuary
    [switch]$SkipRespawn,
    # Give up waiting for the hub lock after this long and report its contents; a stale lock is removed by its owner or the
    # maintainer, never by this script.
    [int]$HubWaitMinutes = 120
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'mission_screen'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o') }

function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }
function PyJson([string]$Expr, [int]$Timeout = 30) { Py "import json; print(json.dumps($Expr, default=str))" $Timeout | ConvertFrom-Json }
function Shot([string]$Name) { Save-Shot "mission_screen/$Name.png" | Out-Null }
# One read and one frame after a key: what the key changed, and how it looks.
function Read-After([string]$Label, [double]$Wait = 0.6) {
    Start-Sleep -Milliseconds ([int]($Wait * 1000))
    $row = PyJson "ms_read('$Label')"; Shot $Label; $log["read_$Label"] = $row; $row
}
# Press a key on an open screen (every send refuses without game focus); W is held for a second, Mouse moves the view. Esc goes only to a screen that is open: otherwise it opens the pause menu.
function Test-Key([string]$Stage, [string]$Key) {
    Assert-GameForeground
    switch ($Key) {
        'W' { Send-KeyDown W; Start-Sleep -Milliseconds 1000; Send-KeyUp W }
        'Mouse' { 1..10 | ForEach-Object { Send-Mouse 30 0; Start-Sleep -Milliseconds 30 } }
        default { Send-Key $Key }
    }
    Read-After "${Stage}_after_$Key"
}
# Stand in front of Marcus and press E until his screen opens.
function Open-Screen([string]$Label) {
    $row = $null
    for ($try = 1; $try -le 4; $try++) {
        $log["stand_$Label"] = PyJson 'fm_stand_in_front()'
        Start-Sleep -Milliseconds 800; Assert-GameForeground; Send-Key E
        $row = Read-After $Label 1.0
        if ($row.open) { return $row }
        Start-Sleep 2
    }
    $row
}

$extraRoots = [ordered]@{ sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$snapshots = @{}
$roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
foreach ($d in $extraRoots.Keys) { $roots[$d] = $extraRoots[$d] }
$stagesRequested = @('start', 'offer', 'offer_esc', 'accept', 'ready', 'turnin', 'reward', 'reward_taken', 'closed') + $(if ($SkipRespawn) { @() } else { @('respawn') })
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
    # The SDK's settings and Sanity Saver's private store are compared too (an earlier session saw Sanity Saver rewrite one).
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
    if (-not $ok) { Save-Shot 'mission_screen/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    Start-Sleep 5; Assert-GameForeground; Send-Key Enter; Start-Sleep 45
    $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
    "map: $map"; if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'mission_screen/load_failed.png' | Out-Null; throw "no player pawn: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 1
    foreach ($f in 'fire_mission_probe.py', 'mission_screen_probe.py') { Invoke-GamePyFile (Join-Path $PSScriptRoot "scripts/$f") mission_screen | Out-Null }
    $log.hooks = PyJson 'ms_start()'
    $log.before = Read-After 'before'
    Set-Stage 'start' ($log.before.status -eq 0) "the Fire mission is not at status 0 in this save (it is $($log.before.status))"

    # 1. The offer: does E open a page first, and which keys does the page take? E again is the first test; Enter comes last, since it accepts.
    $row = Open-Screen 'offer'
    $log.settings = PyJson 'ms_settings()'
    Set-Stage 'offer' ($row.open -eq $true) 'E at Marcus did not open the screen'
    if (!$row.open) { throw 'the screen did not open at the offer' }
    foreach ($key in 'E', 'W', 'Mouse', 'F', 'Space') { Test-Key 'offer' $key | Out-Null }
    Assert-GameForeground; Send-Key Esc; $esc = Read-After 'offer_after_Esc'
    Set-Stage 'offer_esc' ($esc.open -eq $false) 'Esc did not close the offer'
    if ($esc.open) { throw 'the offer stayed open after Esc' }
    $row = Open-Screen 'offer_again'
    if (!$row.open) { throw 'the screen did not open again at the offer' }
    Assert-GameForeground; Send-Key Enter
    $accept = Read-After 'accept_0.1' 0.1; Read-After 'accept_1.0' 0.9 | Out-Null
    Set-Stage 'accept' ($accept.status -eq 1) "Enter did not accept: status $($accept.status)"

    # 2. To the turn-in by the shortcut (ms_advance). Marcus walks to the range after the accept, so the wait lets him arrive.
    $ready = $null
    for ($i = 0; $i -lt 30 -and !$ready.ready; $i++) { $ready = PyJson 'ms_advance()'; Start-Sleep 1 }
    if (!$ready.ready) { $ready = PyJson 'ms_force_ready()'; $log.forced_ready = $true }
    $log.ready = $ready
    Set-Stage 'ready' ($ready.ready -eq $true) "status $($ready.status)"
    if (!$ready.ready) { throw 'the mission did not reach ready to turn in' }
    Start-Sleep 15

    # 3. The turn-in list, then the reward page, with the same key tests. Enter on the page is last: it takes the reward.
    $row = Open-Screen 'turnin'
    Set-Stage 'turnin' ($row.open -eq $true) 'E at Marcus did not open the turn-in screen'
    if (!$row.open) { throw 'the screen did not open at the turn-in' }
    foreach ($key in 'E', 'W', 'Mouse') { Test-Key 'turnin' $key | Out-Null }
    Py "print(ms_mark('turnin_enter'))" | Out-Null
    Assert-GameForeground; Send-Key Enter
    foreach ($at in '0.5', '1.0', '1.5') { Read-After "reward_$at" 0.4 | Out-Null }
    Start-Sleep 2; $page = Read-After 'reward_page'
    $log.reward_shown = PyJson 'ms_reward_shown()'
    Set-Stage 'reward' ($log.reward_shown.ok -eq $true) "no reward page after the Enter: $($log.reward_shown | ConvertTo-Json -Compress)"
    if ($page.status -ne 4) { throw 'the turn-in did not complete' }
    # Each press is marked, so the verdict can say which one took the reward, if a key test did.
    Py "print(ms_mark('reward_shown'))" | Out-Null
    $last = $page
    foreach ($key in 'E', 'W', 'Mouse', 'F', 'Esc') {
        if ($last.open) { Py "print(ms_mark('reward_press_$key'))" | Out-Null; $last = Test-Key 'reward' $key }
    }
    $log.reward_closed_by_key_tests = !$last.open
    $after = $last
    if ($after.open) {
        Py "print(ms_mark('reward_press_Enter'))" | Out-Null
        Assert-GameForeground; Send-Key Enter
        foreach ($at in '0.5', '1.0') { $after = Read-After "reward_taken_$at" 0.4 }
    }
    $log.reward_taken = PyJson 'ms_reward_taken()'
    Set-Stage 'reward_taken' ($log.reward_taken.ok -eq $true) "the reward was not taken once by a marked press: $($log.reward_taken | ConvertTo-Json -Compress)"
    # What is left: a list with the next mission, or nothing. Esc only on a screen that is still open.
    foreach ($key in 'E', 'Esc') { if ($after.open) { $after = Test-Key 'after' $key } }
    Set-Stage 'closed' ($after.open -eq $false) 'the screen is still open after Esc'
    $closed = Read-After 'closed_world' 1.0
    Py "print(ms_dump('screen'))" | Out-Null
    $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')

    # 4. The respawn: down, bleed-out, the station, the dialog. A frame about each second; Enter once the dialog has stood for about three.
    if (!$SkipRespawn) {
        # Downing Maya with the screen open is the crash risk: stop here unless the screen is confirmed closed.
        if ($closed.open -ne $false -or $closed.movie_playing -eq $true -or 'closed' -notin $stagesDone) { throw "the screen is not confirmed closed (open $($closed.open), movie playing $($closed.movie_playing)); not going on to the respawn" }
        $down = PyJson 'ms_down()'
        if ($down.refused) { throw "ms_down refused: $($down.refused)" }
        $log.cash_before = $down.read.cash
        $deadline = (Get-Date).AddSeconds(60); $n = 0; $dialogAt = $null; $row = $null
        while ((Get-Date) -lt $deadline) {
            $row = Read-After ('respawn_{0:D2}' -f $n) 0.8; $n++
            if (!$dialogAt -and (PyJson 'ms_respawn()').dialog) { $dialogAt = $n }
            if ($dialogAt -and $n -eq $dialogAt + 4) { Assert-GameForeground; Send-Key Enter }
            if ($dialogAt -and $n -gt $dialogAt + 8) { break }
        }
        $log.respawn_frames = $n; $log.dialog_frame = $dialogAt; $log.cash_after = $row.cash
        $log.respawn = PyJson 'ms_respawn()'
        Py "print(ms_dump('screen'))" | Out-Null
        Set-Stage 'respawn' ($log.respawn.ok -eq $true) "no down, respawn, dialog and matching charge in order: $($log.respawn | ConvertTo-Json -Compress)"
    }
    Py 'print(ms_stop())' | Out-Null
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
    # A throw between pi_start and pi_stop leaves the per-frame hook reading a held pawn into shutdown: stop it while the game can still answer.
    if ($proc -and !$proc.HasExited) {
        try { Py 'pi_stop(); unmark_lock(); print("stopped")' 5 | Out-Null; $cleanup.hooks = 'stopped' } catch { $cleanup.hooks = "not stopped: $_" }
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
    # Exit 0 only for a completed run with nothing left to recover. Here in the finally, so an early return passes through it too.
    if ($log.status -ne 'completed') { exit 1 }
}
