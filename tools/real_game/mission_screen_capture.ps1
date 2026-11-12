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
    # Slice sessions (2026-11-11): every SDK mod's setting turned off for this run (Set-SdkModsOff; the restore puts the settings back from the backup).
    [switch]$SdkModsOff,
    # With -SdkModsOff: the exact setting names it may turn off; any other enabled set is refused before anything is written.
    [string[]]$SdkModsExpected = @(),
    # Slice sessions: a slice_watch spec (tools/real_game/scripts/slice_watch.py). The read-only recorder runs beside the stages, and its dump is
    # checked (tools/real_game/slice_watch_check.py) before the game closes. Empty: no watch.
    [string]$WatchSpec = '',
    # The save's load after the menu's Enter (Wait-PlayerPawn in realgame.ps1): how long the game may stay on the menu creating its online
    # session, then how long the load may take once the menu is left. Session 2 of 2026-11-12 stopped twice at the first; tests pass small values.
    [int]$MenuWaitSeconds = 240,
    [int]$LoadWaitSeconds = 240,
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
$Out = Join-Path $script:RG 'mission_screen'
New-Item -ItemType Directory -Force $Out | Out-Null
# Stale evidence: a final write that fails must not leave an earlier run's completed behind. Kept under a new name, not deleted: after a run that
# needs recovery, its record is what names the backup and says what went wrong, and a rejected retry must not lose that.
$keptAs = "$(Get-Date -Format 'yyyyMMdd-HHmmss')-$PID"
foreach ($stale in 'session', 'save_compare') { $file = Join-Path $Out "$stale.json"; if (Test-Path -LiteralPath $file) { Move-Item -LiteralPath $file (Join-Path $Out "$stale.$keptAs.json") } }
# End of stale evidence.
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
# Returns the one read row, whatever the sends print. The mark proves the key reached the screen's own handler: a key that never arrived would read as "this key does nothing".
function Test-Key([string]$Stage, [string]$Key) {
    $mark = "key_${Stage}_$Key"
    Py "print(ms_mark('$mark'))" | Out-Null
    Assert-GameForeground
    $null = switch ($Key) {
        'W' { Send-Key W 1000 }   # released in Send-Key's finally
        'Mouse' { 1..10 | ForEach-Object { Send-Mouse 30 0; Start-Sleep -Milliseconds 30 } }
        default { Send-Key $Key }
    }
    $row = Read-After "${Stage}_after_$Key"
    if ($Key -ne 'Mouse') { $log["delivered_${Stage}_$Key"] = PyJson "ms_key_seen('$mark')" }
    $row
}
# The key tests on a screen that must stay open for the next step; returns the last read. A key that changes the mission's status ends them (2026-11-07: E accepted the offer, and there
# was nothing left to reopen): that is logged, and the caller carries on from the new status. A key that only closes the screen is logged too, and the screen is opened again.
function Test-Keys([string]$Stage, [string[]]$Keys, [int]$Status) {
    $row = $null
    foreach ($key in $Keys) {
        $row = Test-Key $Stage $key
        if ($row.status -ne $Status) { $log["${Stage}_status_changed_by_$key"] = $row.status; return $row }
        if (!$row.open) {
            $log["${Stage}_closed_by_$key"] = $true
            $row = Open-Screen "${Stage}_reopen_after_$key"
            if (!$row.open) { throw "the screen did not open again after $key closed it ($Stage)" }
        }
    }
    $row
}
# Stand in front of Marcus and press E until his screen opens.
function Open-Screen([string]$Label) {
    $row = $null
    for ($try = 1; $try -le 4; $try++) {
        $log["stand_$Label"] = PyJson 'fm_stand_in_front()'
        Start-Sleep -Milliseconds 800; Assert-GameForeground
        # The camera's trace across the open (offer and turn-in only): sampled every tick for 2 s from here.
        if ($Label -in 'offer', 'turnin') { Py 'print(ms_cam_trace(2.0))' | Out-Null }
        $null = Send-Key E
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
$stagesRequested = @('hooks', 'start', 'offer', 'offer_keys', 'accept', 'ready', 'turnin', 'reward', 'reward_keys', 'reward_taken', 'closed', 'camera') + $(if ($SkipRespawn) { @() } else { @('respawn') }) + @('hook_errors') + $(if ($WatchSpec) { @('watch') } else { @() })
$stagesDone = New-Object Collections.Generic.List[string]; $stageProblems = New-Object Collections.Generic.List[string]; $runError = ''
$log.status = 'running'; $log.requested_stages = $stagesRequested
function Set-Stage([string]$Stage, [bool]$Passed, [string]$Why) { if ($Passed) { $stagesDone.Add($Stage) } else { $stageProblems.Add("${Stage}: $Why") } }
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $runLock = $false; $driverOwned = $false; $driverAttempted = $false; $launchAttempted = $false
# A token on the game's command line that identifies the process this run started (Stop-OwnGame checks it with the
# image path and the start time before stopping a PID). Unknown switches are ignored by the game.
$marker = "-owrun=$Lane-$PID"
try {
    Assert-NoPendingRecords   # an earlier run's pending record stops this one before it takes a lock or changes anything (pending.ps1)
    # Preflight before any lock or backup: bl-sdk present, our driver folder not already there.
    if (!(Test-Path -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods'))) { throw "bl-sdk is not installed (no sdk_mods in $env:OPENWILLOW_BL2); not installing it" }
    if (Test-Path -LiteralPath $script:DriverDir) { throw "$script:DriverDir already exists; this run did not create it and leaves it alone" }
    # What was loaded: mods can change what a run sees (2026-11-07: Better UI Controls turned E into Enter on this screen). Reading only; a run without this record is not one to trust.
    $modsRoot = Join-Path $env:OPENWILLOW_BL2 'sdk_mods'
    $log.sdk_mods = @(Get-ChildItem -LiteralPath $modsRoot -Force | ForEach-Object { $_.Name } | Sort-Object)
    $log.sdk_mods_settings = [ordered]@{}
    foreach ($file in Get-ChildItem -LiteralPath (Join-Path $modsRoot 'settings') -Filter *.json) {
        try { $log.sdk_mods_settings[$file.BaseName] = (Get-Content -Raw -LiteralPath $file.FullName | ConvertFrom-Json) } catch { $log.sdk_mods_settings[$file.BaseName] = "unreadable: $_" }
    }
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
    if (-not $ok) { Save-Shot 'mission_screen/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    Start-Sleep 5; Assert-GameForeground; Send-Key Enter; $enterAt = Get-Date
    # The save loads: reads only, bounded (Wait-PlayerPawn, realgame.ps1), with the menu's online-session wait named when it runs out.
    $log.load_wait = Wait-PlayerPawn $enterAt $proc $MenuWaitSeconds $LoadWaitSeconds
    $map = $log.load_wait.map
    "map: $map ($($log.load_wait.after_enter_s) s after the Enter)"
    if (!$log.load_wait.ok) { Save-Shot 'mission_screen/load_failed.png' | Out-Null; throw "no player pawn: $($log.load_wait.why)" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 1
    foreach ($f in 'fire_mission_probe.py', 'mission_screen_probe.py') { Invoke-GamePyFile (Join-Path $PSScriptRoot "scripts/$f") mission_screen | Out-Null }
    $log.hooks = PyJson 'ms_start()'
    if ($WatchSpec) { $log.watch_start = Start-SliceWatch $WatchSpec 'mission_screen' 'mission_screen' }
    Set-Stage 'hooks' (@($log.hooks.missing).Count -eq 0) "functions not found, so their rows can never come: $(@($log.hooks.missing) -join '; ')"
    $log.before = Read-After 'before'
    if ($map -notmatch 'Sanctuary') { throw "not in Sanctuary, so Marcus is not here: $map" }
    Set-Stage 'start' ($log.before.status -eq 0) "the Fire mission is not at status 0 in this save (it is $($log.before.status))"
    if ($log.before.status -ne 0) { throw "the Fire mission is at status $($log.before.status) in this save: there is no offer to read" }

    # 1. The offer: does E open a page first, and which keys does the page take? E again is the first test; Enter comes last, since it accepts.
    $row = Open-Screen 'offer'
    $log.settings = PyJson 'ms_settings()'
    Set-Stage 'offer' ($row.open -eq $true -and $row.status -eq 0) "E at Marcus did not open the screen: open $($row.open), status $($row.status)"
    if (!$row.open) { throw 'the screen did not open at the offer' }
    # Better UI Controls is on (2026-11-07), and this reads the game's own paths around it. On this screen it turns the use key's release into Enter and the movement keys into arrows,
    # so neither E nor W, A, S, D is pressed here: those would be its behaviour. Enter, F, Space, Esc and the mouse are not in its rules for this screen. E only opens the screen, from the world.
    # Space is left out: the second run showed it accepts, through the movie and not through HandleInputKey or the mod. A key that still accepts is a finding: the run goes on from there, since
    # the page, the reward and the respawn are what it is for.
    $row = Test-Keys 'offer' @('Mouse', 'F') 0
    Set-Stage 'offer_keys' ($log.delivered_offer_F.delivered -eq $true) "F never reached the screen's handler, so nothing is known about what it does: $($log.delivered_offer_F | ConvertTo-Json -Compress)"
    # Enter straight away: Esc is not tried at the offer, since a surprise there could end the run before the reward page and the respawn it is for (two runs were lost at this page).
    $accept = $row
    # The camera's trace across the close that the accept makes, sampled every tick for 2 s.
    if ($row.status -eq 0) { Py 'print(ms_cam_trace(2.0))' | Out-Null }
    if ($row.status -eq 0) { $accept = Test-Key 'offer' 'Enter' }
    Read-After 'accept_after' 1.0 | Out-Null
    Set-Stage 'accept' ($accept.status -eq 1 -and $log.delivered_offer_Enter.delivered -eq $true) "Enter did not accept: status $($accept.status)"
    if ($accept.status -ne 1) { throw 'the offer was not accepted' }

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
    Set-Stage 'turnin' ($row.open -eq $true -and $row.status -eq 3) "E at Marcus did not open the turn-in screen: open $($row.open), status $($row.status)"
    if (!$row.open) { throw 'the screen did not open at the turn-in' }
    # Enter, as at the offer: E here would be the mod's. The mark is before it.
    Py "print(ms_mark('turnin_enter'))" | Out-Null
    $row = Test-Keys 'turnin' @('Mouse') 3
    if ($row.status -eq 3) { $turned = Test-Key 'turnin' 'Enter' }
    foreach ($at in '0.5', '1.0', '1.5') { Read-After "reward_$at" 0.4 | Out-Null }
    Start-Sleep 2; $page = Read-After 'reward_page'
    $log.reward_shown = PyJson 'ms_reward_shown()'
    Set-Stage 'reward' ($log.reward_shown.ok -eq $true) "no reward page after the Enter: $($log.reward_shown | ConvertTo-Json -Compress)"
    if ($page.status -ne 4) { throw 'the turn-in did not complete' }
    # Each press is marked, so the verdict can say which one took the reward, if a key test did.
    Py "print(ms_mark('reward_shown'))" | Out-Null
    $last = $page
    # The mod takes the reward on the use key's press, so E is not pressed here. F is left out too: on this page the script makes it inspect the selected item, and there is none.
    foreach ($key in 'Mouse', 'Esc') {
        if ($last.open) { Py "print(ms_mark('reward_press_$key'))" | Out-Null; $last = Test-Key 'reward' $key }
    }
    $log.reward_closed_by_key_tests = !$last.open
    Set-Stage 'reward_keys' ($log.delivered_reward_Esc.delivered -eq $true) "Esc never reached the reward page's handler, so nothing is known about what it does: closed by a key test $($log.reward_closed_by_key_tests); $($log.delivered_reward_Esc | ConvertTo-Json -Compress)"
    $after = $last
    $log.reward_after_keys = PyJson 'ms_reward_taken()'
    if ($after.open -and $log.reward_after_keys.count -eq 0) {
        Py "print(ms_mark('reward_press_Enter'))" | Out-Null
        Py 'print(ms_cam_trace(2.0))' | Out-Null   # the camera across the close
        $after = Test-Key 'reward' 'Enter'
        foreach ($at in '0.5', '1.0') { $after = Read-After "reward_taken_$at" 0.4 }
    }
    $log.reward_taken = PyJson 'ms_reward_taken()'
    Set-Stage 'reward_taken' ($log.reward_taken.ok -eq $true) "the reward was not taken once by a marked press: $($log.reward_taken | ConvertTo-Json -Compress)"
    # What is left: a list with the next mission, or nothing. Esc only, on a screen that is still open.
    if ($after.open) { $after = Test-Key 'after' 'Esc' }
    Set-Stage 'closed' ($after.open -eq $false) 'the screen is still open after Esc'
    $closed = Read-After 'closed_world' 1.0
    # A camera trace that read nothing fails its stage (session 1 analysis, A1: every camera row of both runs was an error, and nothing noticed).
    $log.camera_trace = PyJson 'ms_cam_counts()'
    Set-Stage 'camera' ($log.camera_trace.good -gt 0) "the camera trace has $($log.camera_trace.rows) rows and none read the camera: $($log.camera_trace.first_error)"
    Py "print(ms_dump('screen'))" | Out-Null
    $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')

    # 4. The respawn: down, bleed-out, the station, the message. A frame about each second. The script shows it with AddTrainingText for 7 s, not a dialog box, so no key is pressed:
    # the frames say whether it goes by itself. Ten more frames after the call.
    if (!$SkipRespawn) {
        # Downing Maya with the screen open is the crash risk: stop here unless the screen is confirmed closed.
        if ($closed.open -ne $false -or $closed.movie_playing -eq $true -or 'closed' -notin $stagesDone) { throw "the screen is not confirmed closed (open $($closed.open), movie playing $($closed.movie_playing)); not going on to the respawn" }
        if ($WatchSpec) { Save-SliceWatchSnap 'before_down' }
        $down = PyJson 'ms_down()'
        if ($down.refused) { throw "ms_down refused: $($down.refused)" }
        $log.cash_before = $down.read.cash
        $deadline = (Get-Date).AddSeconds(60); $n = 0; $dialogAt = $null; $row = $null
        while ((Get-Date) -lt $deadline) {
            $row = Read-After ('respawn_{0:D2}' -f $n) 0.8; $n++
            if (!$dialogAt -and (PyJson 'ms_respawn()').dialog) { $dialogAt = $n }
            if ($dialogAt -and $n -gt $dialogAt + 10) { break }
        }
        $log.respawn_frames = $n; $log.dialog_frame = $dialogAt; $log.cash_after = $row.cash
        $log.respawn = PyJson 'ms_respawn()'
        if ($WatchSpec) { Save-SliceWatchSnap 'after_respawn' }
        Py "print(ms_dump('screen'))" | Out-Null
        Set-Stage 'respawn' ($log.respawn.ok -eq $true) "no down, respawn, dialog and matching charge in order: $($log.respawn | ConvertTo-Json -Compress)"
    }
    $log.hook_errors = PyJson 'ms_errors()'
    Set-Stage 'hook_errors' ($log.hook_errors.hook_errors -eq 0) "errors in the hooks, so rows may be missing: $($log.hook_errors | ConvertTo-Json -Compress)"
    Py 'print(ms_stop())' | Out-Null
    # The watch's usability check, while the game still runs: a failed check is this stage's failure, and the run still quits and restores in order.
    if ($WatchSpec) {
        try { $log.watch = Complete-SliceWatch $WatchSpec $Out; $log.watch.check; Set-Stage 'watch' $log.watch.usable 'the watch dump is not usable (see watch.check)' }
        catch { Set-Stage 'watch' $false "$_" }
    }
    # The game may close before it answers this one.
    # Nothing is sent to a driver that stopped answering: the finalizer stops this run's own game by its PID, and the restore never needs the driver.
    if (Get-DriverLostReason) { "quit not sent: the driver stopped answering ($(Get-DriverLostReason))" }
    else {
        try { Invoke-GamePy 'get_pc().ConsoleCommand("quit", False)' 15 | Out-Null } catch { "quit: no reply ($_); waiting for the exit" }
        for ($i = 0; $i -lt 20 -and $proc -and !$proc.HasExited; $i++) { Start-Sleep 2 }   # this run's own game: another instance does not hold the wait
    }
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
    # A throw between ms_start and ms_stop leaves the hooks and the per-frame down job running into shutdown: stop them while the game can still answer.
    if ($proc -and !$proc.HasExited) {
        # Py returns a traceback as text, so only the exact answer counts. Nothing is sent to a driver that stopped answering.
        if (Get-DriverLostReason) { $cleanup.hooks = "not sent: the driver stopped answering ($(Get-DriverLostReason))" }
        else {
            # Each stop only if its probe was loaded; a run that failed before the probes has nothing to stop, which is a result, not an error.
            try { $reply = Py "fs = [globals().get(n) for n in ('ms_stop',)]; [f() for f in fs if f]; print('stopped' if any(fs) else 'nothing to stop')" 5; $cleanup.hooks = if ($reply -ceq 'stopped') { 'stopped' } elseif ($reply -ceq 'nothing to stop') { 'nothing to stop: never loaded' } else { "not stopped: $reply" } }
            catch { $cleanup.hooks = "not stopped: $_" }
        }
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
    # Exit 0 only for a completed run with nothing left to recover. Here in the finally, so an early return passes through it too.
    if ($log.status -ne 'completed') { exit 1 }
}
