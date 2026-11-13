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
    # At most 150: with a watch each kill asks for one labelled 'drop' snapshot, and the watch keeps 200 labelled reads, so this leaves room
    # for start, arrival, before_down, down, after_respawn, end and the smoke item's own labels (review of #251).
    [ValidateRange(1, 150)][int]$MaxKills = 40,
    [int]$MaxMinutes = 12,
    [int]$DenTrials = 3,
    [int]$DenSeconds = 25,
    [switch]$SkipDen,
    # Slice sessions (2026-11-11): every SDK mod's setting turned off for this run (Set-SdkModsOff; the restore puts the settings back from the backup).
    [switch]$SdkModsOff,
    # With -SdkModsOff: the exact setting names it may turn off; any other enabled set is refused before anything is written.
    [string[]]$SdkModsExpected = @(),
    # Slice sessions: a slice_watch spec (tools/real_game/scripts/slice_watch.py). The read-only recorder runs beside the stages, and its dump is
    # checked (tools/real_game/slice_watch_check.py) before the game closes. Empty: no watch.
    [string]$WatchSpec = '',
    # Slice sessions: after the den read, one more den trial with the immunity hook removed, so the pack can down Maya (no input; she stands
    # still). It waits up to -DownSeconds for the down, then -RespawnSeconds for her to stand again. Needs the den read (not with -SkipDen).
    [switch]$DenDown,
    # The S0 smoke item only (with -WatchSpec): in each den trial, one labelled snapshot asked for right after the travel, before the map settles
    # (the watch must record it as not taken), then one wait for the settle. Reads only.
    [switch]$WatchSmoke,
    [int]$DownSeconds = 240,
    [int]$RespawnSeconds = 90,
    # The den down's re-placement (S0 of 2026-11-12: each melee hit knocked Maya away, and the pack let go before a down; session 2 analysis,
    # A6: her shield refilled between engagements, so a health-plus-shield rule re-placed her too late or not at all). While she is up, her
    # health has been read since the last placement and has not fallen for -ReplaceAfterSeconds, whatever the shield does, dp_replace puts
    # her at the pack again: the strongest kind in reach first (an adult), by AI class (in one game call that places her only while she is
    # still up), at most -MaxReplacements times inside -DownSeconds. None after a down; once the driver is lost, nothing more is sent.
    [int]$ReplaceAfterSeconds = 20,
    [ValidateRange(0, 4)][int]$MaxReplacements = 4,
    # The save's load after the menu's Enter (Wait-PlayerPawn in realgame.ps1): how long the game may stay on the menu creating its online
    # session, then how long the load may take once the menu is left. Session 2 of 2026-11-12 stopped twice at the first; tests pass small values.
    [int]$MenuWaitSeconds = 240,
    [int]$LoadWaitSeconds = 240,
    # What to dot-source for the real-game functions. Only tests/cash_capture_test.py passes another file.
    [string]$Helpers = (Join-Path $PSScriptRoot 'realgame.ps1')
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. $Helpers
. (Join-Path $PSScriptRoot 'slice_watch.ps1')
# Slice sessions: a bad spec, or too little free disk for the backups and the recording, stops the run here, before anything is changed.
if ($WatchSpec) { Assert-WatchSpec $WatchSpec }
if ($WatchSpec -or $SdkModsOff) { $null = Assert-DiskReserve }
$Out = Join-Path $script:RG 'cash'
New-Item -ItemType Directory -Force $Out | Out-Null
# Stale evidence: a final write that fails must not leave an earlier run's completed behind, and a dump that fails must not be checked against an earlier
# run's file. Kept under a new name, not deleted: after a run that needs recovery, its record is what names the backup and says what went wrong, and a
# rejected retry must not lose that.
$keptAs = "$(Get-Date -Format 'yyyyMMdd-HHmmss')-$PID"
foreach ($stale in 'session', 'save_compare', 'cash', 'den') { $file = Join-Path $Out "$stale.json"; if (Test-Path -LiteralPath $file) { Move-Item -LiteralPath $file (Join-Path $Out "$stale.$keptAs.json") } }
# End of stale evidence.
$log = [ordered]@{ started = (Get-Date).ToString('o'); status = 'running' }
# The driver writes a game-side error into the reply as text, so it is never taken for an answer. The one driver-lost condition (Get-DriverLost)
# holds here as in the helpers (review of #251/#255): once it is set, Py sends nothing and throws, and a reply that never comes sets it. The
# cleanup and the restore never go through Py's refusal: the finalizer's own stop calls are already skipped when the driver is lost.
function Py([string]$Code, [int]$Timeout = 30) {
    if (Get-DriverLost) { throw "not sent: the driver stopped answering ($(Get-DriverLost))" }
    try { $reply = (Invoke-GamePy $Code $Timeout).Trim() }
    catch { if ("$_" -match 'no reply' -and !(Get-DriverLost)) { $script:driverLost = "$_"; $script:SliceWatchDriverLost = "$_" }; throw }
    if ($reply -match 'Traceback \(most recent call last\)') { throw "game python error: $reply" }
    $reply
}
# The waits that expect no reply for a while (the boot, a map load holding the command channel): they refuse once the driver is lost, but
# their own timeouts are part of the wait and set nothing.
function PyWait([string]$Code, [int]$Timeout = 30) {
    if (Get-DriverLost) { throw "not sent: the driver stopped answering ($(Get-DriverLost))" }
    $reply = (Invoke-GamePy $Code $Timeout).Trim()
    if ($reply -match 'Traceback \(most recent call last\)') { throw "game python error: $reply" }
    $reply
}
function Use-Probe([string]$Name) {
    if (Get-DriverLost) { throw "not sent: the driver stopped answering ($(Get-DriverLost))" }
    try { $reply = "$(Invoke-GamePyFile (Join-Path $PSScriptRoot "scripts/$Name") cash)" }
    catch { if ("$_" -match 'no reply' -and !(Get-DriverLost)) { $script:driverLost = "$_"; $script:SliceWatchDriverLost = "$_" }; throw }
    if ($reply -match 'Traceback \(most recent call last\)') { throw "$Name did not load: $reply" }
}

$extraRoots = [ordered]@{ sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
foreach ($d in $extraRoots.Keys) { $roots[$d] = $extraRoots[$d] }
$snapshots = @{}
$stagesRequested = @('travel', 'readings'); if (!$SkipDen) { $stagesRequested += 'den_read' }
if ($DenDown -and $SkipDen) { throw '-DenDown needs the den read; it cannot be combined with -SkipDen' }
if ($DenDown) { $stagesRequested += 'den_down' }
if ($WatchSmoke -and !$WatchSpec) { throw '-WatchSmoke needs -WatchSpec' }
if ($WatchSmoke) { $stagesRequested += 'smoke_hold' }
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
# A number the game printed, or $null.
function ConvertTo-Clock([string]$Text) {
    $d = 0.0
    if ([double]::TryParse("$Text".Trim(), [Globalization.NumberStyles]::Float, [Globalization.CultureInfo]::InvariantCulture, [ref]$d)) { $d } else { $null }
}
# Travel to Ice_P. The map clock is read before the travel is sent; a reply counts as the arrival only once the clock has restarted below
# that value (the new map), so the old map's state is never taken for the arrival (the same map is loaded again for every den trial).
function Enter-IceMap {
    $before = $null
    try { $before = ConvertTo-Clock (PyWait 'print(get_pc().WorldInfo.TimeSeconds)  # ow_clock_before' 8) } catch { }
    Py 'get_pc().ConsoleCommand("open Ice_P", False); print("sent")' | Out-Null
    $reached = ''
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep 3
        if (Get-DriverLost) { break }
        try { $reply = PyWait 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn is not None, pc.WorldInfo.TimeSeconds)' 8 } catch { continue }
        $parts = @("$reply".Trim() -split '\s+')
        $clock = if ($parts.Count -ge 3) { ConvertTo-Clock $parts[2] } else { $null }
        if ($null -ne $before -and ($null -eq $clock -or $clock -ge $before)) { continue }   # still the old map
        $reached = $reply
        if ($reached -match 'Ice_P True') { break }
    }
    $reached
}
# The stage log: each stage's entry and end, stamped before its first risky command, in session.json, on the console and as a flushed mark
# in the watch's recording, so a crash is attributed to a stage by time, not by line order.
# The stamps reach the disk at once: each is appended to this run's own stages-<run>.log in the output folder before the risky command it
# precedes (session.json is written only in the cleanup, and how fast the redirected console reaches the item's log is not something this
# script can prove).
$stagesFile = Join-Path $Out "stages-$keptAs.log"
function Write-StageMark([string]$Stage, [string]$Event) {
    $mark = [ordered]@{ stage = $Stage; event = $Event; utc = (Get-Date).ToUniversalTime().ToString('o') }
    $log.stage_marks = @(@($log.stage_marks) + @($mark) | Where-Object { $_ })
    try { Add-Content -LiteralPath $stagesFile -Value "$($mark.utc) $Stage $Event" -Encoding UTF8 } catch { Write-Warning "stage stamp not written: $_" }
    "stage $Stage $Event $($mark.utc)"
    if ($WatchSpec) { Save-SliceWatchMark "$Stage.$Event" }
}
# The one driver-lost condition: a reply that never came in any stage, here or in the watch's helpers (slice_watch.ps1).
function Get-DriverLost { if ($script:driverLost) { $script:driverLost } else { $script:SliceWatchDriverLost } }
# The game this run started is still there. Once it is gone, no later stage is attempted.
function Test-RunGameAlive { $null -ne $proc -and !$proc.HasExited }
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $runLock = $false; $driverOwned = $false; $driverAttempted = $false; $launchAttempted = $false; $denLoaded = $false
$driverLost = ''; $log.stages_file = $stagesFile
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
    # -Exclusive re-checks the folder is absent and removes its own partial install on failure, so it is ours only once this returns.
    $driverAttempted = $true   # before the call: a failed install can leave the folder behind
    Install-Driver -BlockSavesAtStart -Exclusive
    $driverOwned = $true
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $foreign = Get-ForeignGamesSeen $null
    if ($foreign) { throw "a game this run did not start is running (pid $foreign); not launching" }
    $launchAttempted = $true
    # After the flag: from here the unchanged finalizer restores every root from the verified backup, so settings turned off here are put back
    # even when the launch never happens or the write stops half way.
    if ($SdkModsOff) { Invoke-SdkModsOffStep $log $SdkModsExpected }
    $proc = Start-Game @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', "-Character=$Character", $marker); $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) {
        if ($proc.HasExited) { break }
        try { PyWait 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 }
    }
    if (-not $ok) { Save-Shot 'cash/boot.png' | Out-Null; throw 'driver did not answer (check Steam console_log for AppError_5)' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    Assert-DriverAnswers; Start-Sleep 5; Assert-GameForeground; Send-Key Enter; $enterAt = Get-Date
    # The save loads: reads only, bounded (Wait-PlayerPawn, realgame.ps1), with the menu's online-session wait named when it runs out.
    $log.load_wait = Wait-PlayerPawn $enterAt $proc $MenuWaitSeconds $LoadWaitSeconds
    "load: $(if ($log.load_wait.ok) { 'player pawn' } else { 'failed' }) $($log.load_wait.after_enter_s) s after the Enter"
    if (!$log.load_wait.ok) { Save-Shot 'cash/load_failed.png' | Out-Null; throw "no player pawn: $($log.load_wait.why)" }
    $map = Enter-IceMap
    "map: $map"; $log.map = $map
    Set-Stage 'travel' ($map -match 'Ice_P True') "travel to Ice_P failed: $map"
    if ($map -notmatch 'Ice_P True') { throw "travel to Ice_P failed: $map" }
    Assert-DriverAnswers; Start-Sleep 8; Assert-GameForeground; Send-Key Enter; Start-Sleep 1
    Use-Probe 'combat_probe.py'
    Use-Probe 'cash_probe.py'
    # The immunity is what keeps Maya alive and the measured cash clean: without it, no enemy is touched.
    $log.start = Py 'import json; print(json.dumps(cp_start(), default=str))'
    $started = $log.start | ConvertFrom-Json
    if (!$started -or $started.immune -ne $true) { throw "the immunity hook is not installed: $($log.start)" }
    # What cb_mods_off switched off in memory: a mod's name, or "name: error" when it failed. Printed for the runner; any error stops the run before
    # the first kill, since a live loot mod would change what is read.
    $log.loot_mods_off = @($started.mods_off)
    "loot mods off in memory: $(@($log.loot_mods_off) -join '; ')"
    $lootErrors = @($log.loot_mods_off | Where-Object { "$_" -match ': ' })
    if ($lootErrors.Count) { throw "a loot mod was not switched off in memory: $($lootErrors -join '; ')" }
    $log.player_level = Py 'print(cp_level())'; $log.cash_start = Py 'print(cp_cash())'
    if ($WatchSpec) { Write-StageMark 'watch' 'start'; $log.watch_start = Start-SliceWatch $WatchSpec 'cash' 'cash'; $log.watch_arrival = Save-SliceWatchPawns 'arrival' }
    "player level $($log.player_level), cash $($log.cash_start)"
    Write-StageMark 'readings' 'enter'
    if ($WatchSmoke) { $log.groups_stage1 = Install-SliceWatchGroups 'stage1' }

    # The readings record their own failure. Only a lost driver lets the run go on, so that the later stages report themselves as skipped
    # (each refuses at once); any other failure (the game lost the foreground, a game-side traceback, a script error) is thrown on and ends
    # the run through the outer handler and its cleanup, as it did at e7b5fa3: nothing unexpected is followed by a den placement.
    try {
        $deadline = (Get-Date).AddMinutes($MaxMinutes)
        $dropsUnsettled = 0; $kills = 0; $deaths = 0; $single = 0; $swept = $false; $noTarget = 0; $noReading = 0; $failedPickups = 0
        while ($kills -lt $MaxKills -and $single -lt $WantItems -and (Get-Date) -lt $deadline) {
            # Money on the ground first: walk onto it.
            $name = Py 'print(cp_collect_start())'
            if ($name -ne 'None') {
                Assert-DriverAnswers; Send-Key W 200; Start-Sleep 2
                $row = Py 'import json; print(json.dumps(cp_collect_end(), default=str))'
                $log.last_collect = $row
                "collect: $row"
                # A pickup that gave no reading is skipped from then on (cp_collect_end), so each failure is another pickup: stop after six in a
                # row, all distinct (session 2 analysis, A2: the same pickup was retried until the loop gave up).
                if ($row -match '"value_per_item": \d') { $single++; $noReading = 0 }
                else {
                    $failedPickups++
                    if (++$noReading -ge 6) { $log.stopped = "6 distinct pickups in a row without a single-item reading (each skipped after its one try)"; break }
                }
                continue
            }
            $target = Py 'print(cp_target())'
            if ($target -eq 'None') {
                if (++$noTarget -gt 5) { $log.stopped = 'no enemy within reach'; break }
                Start-Sleep 5; continue
            }
            $noTarget = 0
            Py "print(cp_ready_kill('$target'))" | Out-Null
            Start-Sleep 1; Assert-DriverAnswers; Assert-GameForeground
            for ($shot = 0; $shot -lt 3; $shot++) { Send-Click; Start-Sleep -Milliseconds 700 }
            $kills++   # a kill attempt; whether it killed is asked next, by the enemy's fields only
            Start-Sleep 3   # the loot lands
            # One labelled snapshot per drop, after the wait (session 2 analysis, A3): its pickups part has each item's LifeSpan, draw scale and
            # balance, so a drop's lifetime can be read across the recording. Reads only. Taken only when one read says the map has settled
            # (no wait): asked earlier it would be recorded as not taken and fail the watch's check (review of #251); such a drop is counted.
            if ($WatchSpec) { if (Test-SliceWatchSettled) { Save-SliceWatchSnap 'drop' } else { $dropsUnsettled++ } }
            if ((Py "print(cp_died('$target'))") -eq 'True') { $deaths++ }
            # The first money seen: the item's own value call at other levels, and its definition. Before any walk onto it.
            if (-not $swept -and (Py 'print(len(cp_money()))') -ne '0') {
                $log.sweep = Py 'import json; print(json.dumps(cp_sweep(), default=str))'
                $log.definition = Py 'import json; print(json.dumps(cp_definition(), default=str))'
                $swept = $true
                "sweep: $($log.sweep)"
            }
        }
        $log.kills = $kills; $log.kill_attempts = $kills; $log.deaths = $deaths; $log.failed_pickups = $failedPickups; $log.drop_snapshots_unsettled = $dropsUnsettled; $log.single_item_readings = $single
        $log.dump = Py 'import json; print(json.dumps(cp_dump(), default=str))' 60
        "dump: $($log.dump)"
        $dumpProblem = ''
        try { Test-CashDump $log.dump $kills $single } catch { $dumpProblem = "$_" }
        # Readings taken with Maya down, or with the immunity gone, are not this stage's readings.
        if (!$dumpProblem -and (Py 'print(cp_up() and cp_immune())') -ne 'True') { $dumpProblem = 'Maya is down or the immunity is gone after the readings' }
        Set-Stage 'readings' (($single -ge $WantItems) -and !$dumpProblem) $(if ($dumpProblem) { $dumpProblem } else { "$single of $WantItems single-item readings in $kills kills" })
    } catch {
        Set-Stage 'readings' $false "$_"
        "readings: $_"
        if (!(Get-DriverLost)) { Write-StageMark 'readings' 'done'; throw }
    }
    Write-StageMark 'readings' 'done'
    # Set before stage 2, so a stage skipped at once still leaves a smoke result and stage 3 still runs its own checks and stamps.
    $smokeHeld = $false; $smokeWhy = 'the smoke hold did not run'
    # The den read has its own result: whatever fails in here is that stage's failure, and the session still quits and restores in order.
    if (!$SkipDen) {
        Write-StageMark 'den_read' 'enter'
        try {
            if (!(Test-RunGameAlive)) { throw 'skipped: the game has exited' }
            if (Get-DriverLost) { throw "skipped: the driver stopped answering in an earlier stage ($(Get-DriverLost))" }
            if ($WatchSmoke) { $log.groups_stage2 = Install-SliceWatchGroups 'stage2' }
            Use-Probe 'den_probe.py'
            $denLoaded = $true
            $log.den_hooks = Py 'print(dp_start())'
            $counted = 0; $tried = 0
            for ($trial = 0; $trial -lt $DenTrials; $trial++) {
                $reached = Enter-IceMap   # a fresh population: the cash read killed what was near
                if ($reached -notmatch 'Ice_P True') { throw "travel for trial $trial failed: $reached" }
                if ($WatchSmoke -and $WatchSpec) { $log["smoke_unsettled_$trial"] = Save-SliceWatchSnap 'smoke_unsettled'; $log["smoke_unsettled_$trial"] }
                # The snapshot latches a lost driver without throwing: nothing is pressed after it (review of #251, A5).
                Assert-DriverAnswers; Start-Sleep 8; Assert-GameForeground; Send-Key Enter; Start-Sleep 1
                if ($WatchSmoke -and $WatchSpec) {
                    $log["smoke_settled_$trial"] = Wait-SliceWatchSettled 40
                    if (!$log["smoke_settled_$trial"]) { $smokeWhy = "trial ${trial}: the map did not settle within 40 s" }
                }
                if ((Py 'print(cp_immune())') -ne 'True') { throw "the immunity hook is gone before trial $trial" }
                $first = Py 'print(dp_begin())'
                if ($first -eq 'None') { if ($WatchSmoke) { $smokeWhy = "trial ${trial}: no enemy within reach, so the hold did not run" }; continue }
                $tried++
                Start-Sleep $DenSeconds
                if ($WatchSmoke -and $log["smoke_settled_$trial"]) { $smokeHeld = $true; $smokeWhy = '' }
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
            if ("$_" -match 'no reply' -and !(Get-DriverLost)) { $driverLost = "$_"; $script:SliceWatchDriverLost = "$_" }   # the first reason is kept
        }
        # S0's settled five-minute hold: the map settled and the hold ran with an enemy in reach, or the stage fails.
        if ($WatchSmoke) { Set-Stage 'smoke_hold' $smokeHeld $smokeWhy }
        Write-StageMark 'den_read' 'done'
    }
    # The den down: the same placement as a den trial, then the immunity hook (this run's own, cp_start) is removed and Maya stands still. No key
    # is pressed. The game's own pack downs her, she bleeds out and respawns; the watch records the hits, the down and the respawn. Saves stay blocked.
    if ($DenDown) {
        Write-StageMark 'den_down' 'enter'
        try {
            if (!(Test-RunGameAlive)) { throw 'skipped: the game has exited' }
            if (Get-DriverLost) { throw "skipped: the driver stopped answering in an earlier stage ($(Get-DriverLost))" }
            if (!$denLoaded) { Use-Probe 'den_probe.py'; $denLoaded = $true }
            $reached = Enter-IceMap
            if ($reached -notmatch 'Ice_P True') { throw "travel for the den down failed: $reached" }
            Assert-DriverAnswers; Start-Sleep 8; Send-Key Enter; Start-Sleep 1   # Send-Key asserts the game's focus itself
            # The watch scans nothing until the new map has settled; its before_down snapshot must be taken before the immunity goes.
            if ($WatchSpec) {
                $log.den_down_settled = Wait-SliceWatchSettled 40
                if (!$log.den_down_settled) { throw 'the map did not settle within 40 s, so the watch could not take before_down: the den down is not attempted' }
            }
            if ($WatchSmoke) { $log.groups_stage3 = Install-SliceWatchGroups 'stage3' }
            $first = Py 'print(dp_begin())'
            if ($first -eq 'None') { throw 'no enemy within reach for the den down' }
            if ($WatchSpec) { Save-SliceWatchSnap 'before_down'; $log.watch_before_down = Save-SliceWatchPawns 'before_down' }
            $off = Py 'import json; print(json.dumps(cp_immune_off()))' | ConvertFrom-Json
            $log.den_down_immune_remaining = @($off.remaining)
            if ($null -eq $off.remaining -or @($off.remaining).Count) { throw "the immunity hook is still installed on: $(@($off.remaining) -join ', ')" }
            # Only an exact state counts: 'up', or a named down condition. Any other reply (a traceback, an empty answer) is a failed poll, counted;
            # five in a row fail the stage. The condition that first fired is kept, so a down seen only at bleed-out is not read as the time to the down.
            $script:pollErrors = 0; $script:inARow = 0
            # Each poll waits only for the time left in its window, less 1 s, and none starts with under 6 s left, so every wait is 5 s or more
            # (review of #245). A reply that never came within its wait, shortened or not, is the driver lost: the one condition is latched and
            # the stage ends, as for every other command; 5 s is far above a normal reply, so a live but slow game is not taken for a lost one
            # by a wait cut to a second. A reply that comes back after the window is not taken, and no state is updated from it.
            $poll = {
                param([datetime]$Until)
                $left = ($Until - (Get-Date)).TotalSeconds
                if ($left -lt 6) { return $null }
                $wait = [int][math]::Min(30, [math]::Floor($left - 1))
                try {
                    $text = Py 'import json; print(json.dumps(cp_down_state()))' $wait
                    if ((Get-Date) -gt $Until) { return $null }   # too late: not taken
                    $r = $text | ConvertFrom-Json
                    if ($r.state -in 'up', 'no_pawn', 'no_health', 'injured_flag') { $script:inARow = 0; return $r }
                    throw "unexpected state '$($r.state)'"
                } catch {
                    if ("$_" -match 'no reply') {
                        if (!(Get-DriverLost)) { $script:driverLost = "$_"; $script:SliceWatchDriverLost = "$_" }
                        throw
                    }
                    $script:pollErrors++; $script:inARow++
                    if ($script:inARow -ge 5) { throw "five den-down polls in a row failed: $_" }
                    return $null
                }
            }
            $downAt = $null; $t0 = Get-Date; $downUntil = $t0.AddSeconds($DownSeconds)
            $lastHit = $t0; $lastHealth = $null; $log.den_down_replacements = 0; $log.den_down_replace_results = @()
            while (((Get-Date) - $t0).TotalSeconds -lt $DownSeconds -and !(Get-DriverLost)) {
                $r = & $poll $downUntil
                if ($r -and $r.state -ne 'up') { $downAt = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1); $log.den_down_reason = $r.state; $log.den_down_injured_field = $r.injured_field; break }
                # A fall of her health since the last poll means a hit reached her body. The shield is not counted: it refills between
                # engagements and absorbs the first hits, so a fall of health plus shield said little about whether the pack still fought her.
                if ($r -and $null -ne $r.health) {
                    if ($null -ne $lastHealth -and [double]$r.health -lt [double]$lastHealth) { $lastHit = Get-Date }
                    $lastHealth = $r.health
                }
                # Up, her health read since the last placement (an unreadable health never reads as unhit), unhit for -ReplaceAfterSeconds,
                # under the cap, the driver answering: placed again. The stamp is on disk first; a stamp whose mark finds the driver lost ends it.
                if ($r -and $r.state -eq 'up' -and $null -ne $lastHealth -and !(Get-DriverLost) -and $log.den_down_replacements -lt $MaxReplacements -and
                    ((Get-Date) - $lastHit).TotalSeconds -ge $ReplaceAfterSeconds -and (Get-Date) -lt $downUntil) {
                    $n = $log.den_down_replacements + 1
                    Write-StageMark 'den_down' "replace$n"
                    if (Get-DriverLost) { break }
                    if ((Get-Date) -ge $downUntil) { break }   # the window ended during the stamp: no placement after it
                    $log.den_down_replacements = $n
                    # One call in the game: it reads the down state first and places her only while she is up (den_probe.dp_replace). It gets
                    # the window's end (wall-clock seconds since 1970, the clock both sides read on this machine) and moves nothing once that
                    # has passed ('late'); its reply wait is clamped to the time left, like the polls' (review of #255).
                    $left = ($downUntil - (Get-Date)).TotalSeconds
                    if ($left -lt 2) { break }
                    $untilEpoch = (([DateTimeOffset]$downUntil).ToUnixTimeMilliseconds() / 1000.0).ToString('0.000', [Globalization.CultureInfo]::InvariantCulture)
                    $again = Py "import json; print(json.dumps(dp_replace(until=$untilEpoch)))" ([int][math]::Min(30, [math]::Floor($left - 1)))
                    $log.den_down_replace_results = @(@($log.den_down_replace_results) + @("$([math]::Round(((Get-Date) - $t0).TotalSeconds, 1)) s: $again"))
                    $lastHit = Get-Date; $lastHealth = $null
                }
                $pause = [math]::Min(1000, ($downUntil - (Get-Date)).TotalMilliseconds)   # the loop's pause never runs past the window
                if ($pause -gt 0) { Start-Sleep -Milliseconds ([int]$pause) }
            }
            $log.den_down_after_s = $downAt
            if (Get-DriverLost) { throw "the driver stopped answering during the den down ($(Get-DriverLost))" }
            # The snapshot is labelled by what was seen: 'down' only when the injured flag fired (the moment of the down); a down first seen as
            # no health (bleed-out) or no pawn is labelled so, and taken (or recorded not taken) as such, never as 'down'.
            if ($WatchSpec -and $null -ne $downAt) {
                $log.den_down_label = switch ($log.den_down_reason) { 'injured_flag' { 'down' } 'no_health' { 'down_seen_no_health' } default { 'down_seen_no_pawn' } }
                Save-SliceWatchSnap $log.den_down_label
            }
            $upAt = $null
            if ($null -ne $downAt -and !(Get-DriverLost)) {
                $t1 = Get-Date
                while (((Get-Date) - $t1).TotalSeconds -lt $RespawnSeconds -and !(Get-DriverLost)) {
                    $pause = [math]::Min(2000, ($t1.AddSeconds($RespawnSeconds) - (Get-Date)).TotalMilliseconds)   # never past the window
                    if ($pause -gt 0) { Start-Sleep -Milliseconds ([int]$pause) }
                    $r = & $poll $t1.AddSeconds($RespawnSeconds)
                    if ($r -and $r.state -eq 'up') { $upAt = [math]::Round(((Get-Date) - $t1).TotalSeconds, 1); break }
                }
                # The respawn dialog and the fee come a few seconds after she stands: leave them time before the last snapshot.
                Start-Sleep 15
                if ($WatchSpec) { Save-SliceWatchSnap 'after_respawn' }
            }
            $log.den_down_poll_errors = $script:pollErrors
            # What 'up' after the down measures: the health back above 0 with the injured flag clear, which the game does in the holding cell
            # at the respawn (session 2 of 2026-11-12), before she is placed at the station and shown: not the moment she stands. The watch's
            # rows give the teleport and the unhide. den_down_up_after_s is kept, with the same value, for the readers of earlier runs.
            $log.den_down_health_back_s = $upAt
            $log.den_down_up_after_s = $upAt
            # dp_begin opened a den trial for the placement; close it. Its rows are not this stage's evidence: any other error is only logged.
            if (Get-DriverLost) { $log.den_down_trial_close = "not sent: the driver stopped answering ($(Get-DriverLost))" }
            else {
                # Its rows are not evidence, but a reply that never came is the driver lost: latched, so the stage fails and nothing more is sent.
                try { Py 'import json; print(json.dumps(dp_end(), default=str))' 60 | Out-Null }
                catch { $log.den_down_trial_close = "$_"; if ("$_" -match 'no reply' -and !(Get-DriverLost)) { $driverLost = "$_"; $script:SliceWatchDriverLost = "$_" } }
            }
            # A driver lost anywhere in the stage fails it, even after the down and the respawn were seen (the after_respawn snapshot then is not).
            $why = if (Get-DriverLost) { "the driver stopped answering during the den down ($(Get-DriverLost))" }
                   elseif ($null -eq $downAt) { "no down in $DownSeconds s after $($log.den_down_replacements) re-placements" }
                   elseif ($null -eq $upAt) { "down after $downAt s (limit $DownSeconds), health not back within $RespawnSeconds s" }
                   else { "down after $downAt s (limit $DownSeconds), health back (holding cell, not standing) after $upAt s (limit $RespawnSeconds)" }
            Set-Stage 'den_down' ($null -ne $downAt -and $null -ne $upAt -and !(Get-DriverLost)) $why
        } catch {
            Set-Stage 'den_down' $false "$_"
            "den down: $_"
            if ("$_" -match 'no reply' -and !(Get-DriverLost)) { $driverLost = "$_"; $script:SliceWatchDriverLost = "$_" }   # the first reason is kept
        }
        Write-StageMark 'den_down' 'done'
    }
    # The watch's usability check, while the game still runs (not attempted once the game is gone).
    if ($WatchSpec) {
        try { if (!(Test-RunGameAlive)) { throw 'skipped: the game has exited' }; if (Get-DriverLost) { throw "skipped: the driver stopped answering ($(Get-DriverLost))" }; $log.watch = Complete-SliceWatch $WatchSpec $Out; $log.watch.check; Set-Stage 'watch' $log.watch.usable 'the watch dump is not usable (see watch.check)' }
        catch { Set-Stage 'watch' $false "$_" }
    }
    $log.shot = Save-Shot 'cash/shot.png'
    # Nothing is sent to a driver that stopped answering: the finalizer stops this run's own game by its PID, and the restore never needs the driver.
    # A quit that gets no reply latches the condition, so the finalizer sends the driver nothing more (review of #251, A6).
    if (Get-DriverLost) { "quit not sent: the driver stopped answering ($(Get-DriverLost))" }
    else {
        try { Invoke-GamePy 'get_pc().ConsoleCommand("quit", False)' 10 | Out-Null } catch { Set-SliceWatchDriverLost $_; "quit: no reply ($_); waiting for the exit" }
        for ($i = 0; $i -lt 30 -and -not $proc.HasExited; $i++) { Start-Sleep 2 }
    }
} catch {
    $runError = "$_"
    "run error: $runError"
} finally {
    # Each step is isolated so a failure in one (a log write) cannot skip the restore. Order: stop our game, and only once it has
    # exited remove our driver and restore and verify the roots; then write the logs and release the locks.
    $cleanup = [ordered]@{}
    $cleanup.inputs = Release-HeldInputs   # every key and button this run still holds is let go before anything else
    # A throw in the den read leaves its per-frame hook reading pawns into shutdown: stop it while the game can still answer.
    if ($denLoaded -and $proc -and !$proc.HasExited -and (Get-DriverLost)) { $cleanup.hooks = "not sent: the driver stopped answering ($(Get-DriverLost))" }
    elseif ($denLoaded -and $proc -and !$proc.HasExited) {
        # Py returns a traceback as text, so only the exact answer counts.
        try { $reply = Py 'dp_stop(); print("stopped")' 5; $cleanup.hooks = if ($reply -ceq 'stopped') { 'stopped' } else { "not stopped: $reply" } }
        catch { $cleanup.hooks = "not stopped: $_" }
    }
    # The watch's hooks too, best effort, the same shape: only while this run's game lives, 5 s, only the exact reply counts.
    if ($WatchSpec -and $proc -and !$proc.HasExited -and (Get-DriverLost)) { $cleanup.watch = "not sent: the driver stopped answering ($(Get-DriverLost))" }
    elseif ($WatchSpec -and $proc -and !$proc.HasExited) {
        # A run that failed before the watch was loaded has nothing to stop: that is a result, not an error.
        try { $reply = Py "print(sw_stop() if 'sw_stop' in globals() else 'nothing to stop')" 5; $cleanup.watch = if ($reply -ceq 'stopped') { 'stopped' } elseif ($reply -ceq 'nothing to stop') { 'nothing to stop: never loaded' } else { "not stopped: $reply" } }
        catch { $cleanup.watch = "not stopped: $_" }
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
