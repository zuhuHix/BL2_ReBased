# Phaselock integrity ground truth (Maya): one real-game session in Three Horns Divide (Ice_P) that records, every rendered
# frame, the lifted bullymong's animation sequences, collision flags, the bubble emitters' instance parameters, the
# hand-orb custom event and the cooldown pool (scripts/phaselock_integrity.py), around:
#   1. a hit cast at 650 uu, with 20 fps full-size frames and named PNG frames at the host capture times;
#   2. a miss cast (view turned away) for the fizzle hand event;
#   3. a hit cast where Maya is put into Fight For Your Life 2.0 s after the cast (the lock-vs-down rule), last.
# Takes the screen and keyboard. Everything written is game data under ignored local/realgame/phaselock_integrity/.
# Locks: the shared hub lock and the run lock; saves are backed up, saving is blocked in game from the first frame, and
# afterwards the save folder is compared with the backup (changed files are restored from it).
param(
    # The machine-wide hub lock lives in the main checkout's local/ (shared by every worktree on this machine).
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'phaselock-integrity',
    [string]$Character = 'Save0008.sav',   # quick_startup's -Character slot: the level-8 Maya (Phaselock) in Sanctuary
    # Slice sessions (2026-11-11): every SDK mod's setting turned off for this run (Set-SdkModsOff; the restore puts the settings back from the backup).
    [switch]$SdkModsOff,
    # With -SdkModsOff: the exact setting names it may turn off; any other enabled set is refused before anything is written.
    [string[]]$SdkModsExpected = @(),
    # Slice sessions: a slice_watch spec (tools/real_game/scripts/slice_watch.py). The read-only recorder runs beside the stages, and its dump is
    # checked (tools/real_game/slice_watch_check.py) before the game closes. Empty: no watch.
    [string]$WatchSpec = '',
    [switch]$ProbeOnly,                     # stop after pi_discover / pi_flags
    [switch]$SkipDown,
    # Instead of the hit, miss and down casts, two separate reads: kill the lifted target (the skill, and the HUD emblem through
    # scripts/action_skill_hud.py), and the real shield's capacity against its level. A miss in one does not blank the other.
    [switch]$KillAndShield,
    # Only the shield read of -KillAndShield: no target is needed, and none is picked.
    [switch]$ShieldOnly,
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
$Out = Join-Path $script:RG 'phaselock_integrity'
New-Item -ItemType Directory -Force $Out | Out-Null
# Stale evidence: a final write that fails must not leave an earlier run's completed behind. Kept under a new name, not deleted: after a run that
# needs recovery, its record is what names the backup and says what went wrong, and a rejected retry must not lose that.
$keptAs = "$(Get-Date -Format 'yyyyMMdd-HHmmss')-$PID"
foreach ($stale in 'session', 'save_compare') { $file = Join-Path $Out "$stale.json"; if (Test-Path -LiteralPath $file) { Move-Item -LiteralPath $file (Join-Path $Out "$stale.$keptAs.json") } }
# End of stale evidence.
$log = [ordered]@{ started = (Get-Date).ToString('o') }

function Py([string]$Code, [int]$Timeout = 30) { ConvertTo-ReplyText (Invoke-GamePy $Code $Timeout) $Code }   # an empty reply is refused (realgame.ps1)

# Frames every $IntervalMs from $Before s before the F press to $After s after it, full size, JPEG in memory; $Shots maps a
# name to seconds after the press (the first frame at or after it is also written as PNG). $Action runs once at $ActionAt s.
function Invoke-CastCapture([string]$Name, [double]$Before, [double]$After, [hashtable]$Shots, [int]$IntervalMs = 50,
                            [double]$ActionAt = -1, [scriptblock]$Action = $null) {
    $rect = Get-GameRect; $dir = Join-Path $Out $Name; New-Item -ItemType Directory -Force $dir | Out-Null
    $jpeg = [Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object { $_.MimeType -eq 'image/jpeg' }
    $ep = New-Object Drawing.Imaging.EncoderParameters 1
    $ep.Param[0] = New-Object Drawing.Imaging.EncoderParameter ([Drawing.Imaging.Encoder]::Quality, [long]90)
    $frames = New-Object Collections.Generic.List[object]; $pending = @{} + $Shots; $saved = @{}
    $t0 = Get-QpcSeconds; $press = $null; $acted = $null
    while ($true) {
        $ts = Get-QpcSeconds
        if ($null -eq $press -and $ts - $t0 -ge $Before) { Send-KeyDown F; try { $press = Get-QpcSeconds; Start-Sleep -Milliseconds 60 } finally { Send-KeyUp F }; continue }
        if ($press -and $ts - $press -gt $After) { break }
        if ($press -and $Action -and $null -eq $acted -and $ts - $press -ge $ActionAt) { $acted = & $Action; continue }
        $bmp = [Drawing.Bitmap]::new($rect.Width, $rect.Height)
        $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($rect.Location, [Drawing.Point]::Empty, $rect.Size); $g.Dispose()
        if ($press) {
            foreach ($k in @($pending.Keys)) {
                if ($ts - $press -ge $pending[$k]) {
                    $png = Join-Path $dir "$k.png"
                    $bmp.Save($png, [Drawing.Imaging.ImageFormat]::Png); $saved[$k] = [math]::Round($ts - $press, 3); $pending.Remove($k)
                }
            }
        }
        $ms = New-Object IO.MemoryStream; $bmp.Save($ms, $jpeg, $ep); $bmp.Dispose()
        $frames.Add([pscustomobject]@{ t = $ts; data = $ms.ToArray() }); $ms.Dispose()
        $next = $ts + $IntervalMs / 1000.0
        while ((Get-QpcSeconds) -lt $next) { Start-Sleep -Milliseconds 2 }
    }
    $index = for ($i = 0; $i -lt $frames.Count; $i++) {
        $file = 'f{0:D3}.jpg' -f $i; [IO.File]::WriteAllBytes((Join-Path $dir $file), $frames[$i].data)
        [pscustomobject]@{ i = $i; file = $file; t = $frames[$i].t; rel = [math]::Round($frames[$i].t - $press, 4) }
    }
    @{ press_qpc = $press; frames = @($index); shots = $saved; action = $acted } | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $dir 'frames.json')
    [pscustomobject]@{ press = $press; frames = $frames.Count; shots = $saved; action = $acted }
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
$stagesRequested = if ($ProbeOnly) { @('discover') } elseif ($ShieldOnly) { @('discover', 'shield_levels') } elseif ($KillAndShield) { @('discover', 'kill', 'shield_levels') } elseif ($SkipDown) { @('hit', 'miss') } else { @('hit', 'miss', 'down') }
if ($WatchSpec -and !$ProbeOnly) { $stagesRequested = @($stagesRequested) + @('watch') }
$stagesDone = New-Object Collections.Generic.List[string]; $stageProblems = New-Object Collections.Generic.List[string]; $runError = ''
$log.status = 'running'; $log.requested_stages = $stagesRequested
function Set-Stage([string]$Stage, [bool]$Passed, [string]$Why) { if ($Passed) { $stagesDone.Add($Stage) } else { $stageProblems.Add("${Stage}: $Why") } }
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $target = $null; $runLock = $false; $driverOwned = $false; $driverAttempted = $false; $launchAttempted = $false
# A token on the game's command line that identifies the process this run started (Stop-OwnGame checks it with the
# image path and the start time before stopping a PID). Unknown switches are ignored by the game.
$marker = "-owrun=$Lane-$PID"
$hostShots = @{ 'hit-0.25' = 0.25; 'hit-0.50' = 0.5; 'hit-0.80' = 0.8; 'hit-1.50' = 1.5; 'hit-3.00' = 3.0; 'hit-4.50' = 4.5; 'hit-5.30' = 5.3; 'hit-6.20' = 6.2 }
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
    if (-not $ok) { Save-Shot 'phaselock_integrity/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    Start-Sleep 5; Assert-GameForeground; Send-Key Enter; $enterAt = Get-Date
    # The save loads: reads only, bounded (Wait-PlayerPawn, realgame.ps1), with the menu's online-session wait named when it runs out.
    $log.load_wait = Wait-PlayerPawn $enterAt $proc $MenuWaitSeconds $LoadWaitSeconds
    $map = $log.load_wait.map
    "map: $map ($($log.load_wait.after_enter_s) s after the Enter)"
    if (!$log.load_wait.ok) { Save-Shot 'phaselock_integrity/load_failed.png' | Out-Null; throw "no player pawn: $($log.load_wait.why)" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 1
    Py 'get_pc().ConsoleCommand("open Ice_P", False); print("sent")' | Out-Null
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep 3
        try { $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn is not None)' 8 } catch { continue }
        if ($map -match 'Ice_P True') { break }
    }
    "map: $map"; if ($map -notmatch 'Ice_P True') { throw "travel to Ice_P failed: $map" }
    Start-Sleep 8
    foreach ($f in 'phaselock.py', 'phaselock_matched.py', 'phaselock_integrity.py', 'action_skill_hud.py') { Invoke-GamePyFile (Join-Path $PSScriptRoot "scripts/$f") phaselock_integrity | Out-Null }
    if ($WatchSpec -and !$ProbeOnly) { $log.watch_start = Start-SliceWatch $WatchSpec 'phaselock_integrity' 'phaselock_integrity'; $log.watch_arrival = Save-SliceWatchPawns 'arrival' }

    # The exact Adult or nothing: a variant has its own archetype, attachments and material, so its reads would answer another question.
    for ($i = 0; $i -lt 24 -and !$target -and !$ShieldOnly; $i++) {
        $pick = Py @'
import json
rows = []
for d, n, c, dead in list_ai():
    p = ai_pawn(n)
    cls = p.AIClass._path_name() if p.AIClass else ""
    radius = p.CylinderComponent.CollisionRadius if p.CylinderComponent else 0
    rows.append({"name": n, "dist": d, "class": cls.split(".")[-1], "radius": radius, "dead": dead, "ctrl": p.Controller is not None})
pick = pi_pick_target(rows)
print(json.dumps({"pick": pick["name"] if pick else None, "pick_class": pick["class"] if pick else None, "rows": rows[:30]}))
'@
        $res = $pick | ConvertFrom-Json; $log.candidates = $res.rows; $target = $res.pick
        if (!$target) { Start-Sleep 5 }
    }
    if (!$ShieldOnly) {
    if (!$target) { throw "no live CharClass_PrimalBeastAdult after 2 minutes of polling: $pick" }
    $liveClass = Py "print(ai_pawn('$target').AIClass._path_name().split('.')[-1])"
    if ($liveClass -ne 'CharClass_PrimalBeastAdult') { throw "target $target is $liveClass, not the Adult" }
    $log.target = $target; $log.target_class = $liveClass; "target $target ($liveClass)"
    }
    $log.discover = Py 'import json; print(json.dumps(pi_discover(), default=str))' | ConvertFrom-Json
    # pi_flags() (CanPhaseLockTarget called from Python for every pawn) got no reply on 2026-11-02 and the session was lost; the
    # cast's own CanPhaseLockTarget result is marked by mark_lock() instead.
    $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')
    Set-Stage 'discover' ($null -ne $log.discover) 'no discovery reply'
    if ($ProbeOnly) { return }

    function Set-Up([string]$Label) {
        $s = Py "import json; print(json.dumps({'protect': len(protect_player(True)), 'isolate': isolate('$target'), 'place': place('$target', 650.0)}))" | ConvertFrom-Json
        Start-Sleep 3
        $a = Py "import json; isolate('$target'); print(json.dumps({'pitch': aim('$target'), 'measure': measure('$target'), 'camera': camera(), 'bounds': bounds('$target')}))" | ConvertFrom-Json
        Py "mark_lock('$target'); print(json.dumps(pi_start('$Label', '$target'), default=str))" | Out-Null
        Assert-GameForeground; Start-Sleep -Milliseconds 500
        @{ setup = $s; aim = $a }
    }

    # A cast that did not lift the target (2026-11-06: the first was blocked, cause unknown) is retried, each attempt under its own
    # label so nothing is overwritten. Returns the attempt's result; the last one is returned when none lifted it.
    function Invoke-LiftCast([string]$Label, [double]$Before, [double]$After, [hashtable]$Shots, [int]$IntervalMs, [double]$ActionAt, [scriptblock]$Action, [int]$Attempts = 3) {
        $result = $null
        for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
            $name = if ($attempt -eq 1) { $Label } else { "${Label}_try$attempt" }
            $log["${Label}_attempt"] = $name
            $log["${name}_setup"] = Set-Up $name
            $log[$name] = Invoke-CastCapture "${name}_burst" $Before $After $Shots $IntervalMs $ActionAt $Action
            $result = Py 'import json; print(json.dumps(pi_result(), default=str))' | ConvertFrom-Json
            $log["${name}_result"] = $result
            Write-Host "$name press at $($log[$name].press); frames $($log[$name].frames); lifted: $($result.lifted -join ','); target lifted: $($result.target_lifted)"
            Py 'pi_stop(); unmark_lock(); print("stopped")' | Out-Null
            if ($result.target_lifted -eq $true) { return $result }
            if ($attempt -lt $Attempts) { $log["${name}_ready"] = Wait-Ready 40 }
        }
        Write-Host "$Label`: the target was not lifted in $Attempts attempts"
        $result
    }

    if ($KillAndShield -or $ShieldOnly) {
        # KILL STAGE. Kill the lifted target 2.0 s after the cast, during the hold. The burst's frames and kill.jsonl show the skill, kill_hud.jsonl
        # the emblem. The stage's result is set only after the hooks are stopped and the emblem samples are checked.
        if (!$ShieldOnly) {
        $killPassed = $false; $killWhy = 'did not finish'
        try {
            $killShots = @{ 'kill-2.10' = 2.1; 'kill-2.50' = 2.5; 'kill-3.50' = 3.5; 'kill-5.00' = 5.0; 'kill-6.00' = 6.0 }
            $ash = Py "import json; print(json.dumps(ash_start('kill_hud'), default=str))" | ConvertFrom-Json
            if (!$ash.movie) { throw "the HUD sampler did not start: $($ash | ConvertTo-Json -Compress)" }
            $kill = { Py "import json; print(json.dumps(pi_kill('$target'), default=str))" 15 }
            $killResult = Invoke-LiftCast 'kill' 1.0 8.0 $killShots 100 2.0 $kill
            $killCapture = $log[$log.kill_attempt]
            $killAction = if ($killCapture.action) { $killCapture.action | ConvertFrom-Json } else { $null }
            "kill: $($killCapture.action); skill calls: $($killResult.skill_calls -join ',')"
            $killPassed = ($killResult.target_lifted -eq $true) -and ($killAction.killed -eq $true) -and (Test-CaptureComplete $killCapture $killShots)
            $killWhy = 'target not lifted, not killed, or frames or shots missing'
        } catch { $killPassed = $false; $killWhy = "$_" }
        finally {
            try { Py 'pi_stop(); unmark_lock(); ash_stop(); print("stopped")' | Out-Null }
            catch { $killPassed = $false; $killWhy = "$killWhy; hooks not stopped: $_" }
        }
        if ($killPassed) {
            try {
                $log.kill_hud = Py "import json; print(json.dumps(pi_hud_evidence($($killAction.at), $($killAction.since))))" | ConvertFrom-Json
                if ($log.kill_hud.usable -ne $true) { $killPassed = $false; $killWhy = "no usable HUD samples around the kill ($($log.kill_hud.before) before, $($log.kill_hud.after) after)" }
            } catch { $killPassed = $false; $killWhy = "HUD samples not checked: $_" }
        }
        Set-Stage 'kill' $killPassed $killWhy
        $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')
        }
        # END KILL STAGE

        # SHIELD STAGE. The real shield's capacity at several levels: the equipped shield's balance, respawned at each level and equipped. Every
        # row stays in the log. The stage needs each level but her own shield's measured, and her own shield back; a clamped level is a finding,
        # not a retry. Her own level is refused: the key could not tell the two shields apart.
        $shieldLevels = @(1, 5, 8, 15, 30, 50)
        $shieldRows = @(); $shieldWhy = ''; $restored = $null
        try {
            $log.shields_start = Py 'import json; print(json.dumps(pi_shields(True), default=str))' | ConvertFrom-Json
            # With no equipped shield found, nothing is spawned or changed, so there is nothing to restore either.
            if (!$log.shields_start.equipped) { $shieldLevels = @(); $shieldWhy = 'no equipped shield found: nothing was changed' }
            # The game did not equip a shield above the player's level (live, 2026-11-07, reason not read), so those levels cannot be measured.
            $shieldLevels = @($shieldLevels | Where-Object { $_ -le $log.shields_start.player_level })
            foreach ($level in $shieldLevels) {
                $row = [ordered]@{ level = $level }
                $restoreState = 'unknown'
                try {
                    $row.tried = Py "import json; print(json.dumps(pi_shield_try($level), default=str))" | ConvertFrom-Json
                    if ($row.tried -and !$row.tried.skipped) {
                        $until = (Get-Date).AddSeconds(5)
                        do {
                            $row.read = Py 'import json; print(json.dumps(pi_shield_read(), default=str))' | ConvertFrom-Json
                            if ($row.read.counts -eq $true) { break }
                            Start-Sleep -Milliseconds 250
                        } while ((Get-Date) -lt $until)
                    }
                } catch { $row.error = "$_" }
                finally {
                    try {
                        $restored = Py 'import json; print(json.dumps(pi_shield_restore(), default=str))' | ConvertFrom-Json
                        $until = (Get-Date).AddSeconds(5)
                        while ($restored.ok -ne $true -and (Get-Date) -lt $until) {
                            Start-Sleep -Milliseconds 250
                            $restored = Py 'import json; print(json.dumps(pi_shield_restore(), default=str))' | ConvertFrom-Json
                        }
                        $row.restore = $restored; $restoreState = if ($restored.ok -eq $true) { 'ok' } else { 'not restored' }
                    } catch {
                        # A failed command or a traceback says nothing about the shield: ask what is there before saying.
                        $row.restore_error = "$_"
                        try {
                            $row.restore_check = Py 'import json; print(json.dumps(pi_shields_check(), default=str))' | ConvertFrom-Json
                            $restoreState = if ($row.restore_check.ok -eq $true) { 'ok' } else { 'not restored' }
                        } catch { $row.restore_check_error = "$_" }
                    }
                }
                $shieldRows += [pscustomobject]$row
                if ($restoreState -eq 'not restored') { $shieldWhy = "SHIELD NOT RESTORED after level ${level}: confirmed; the live character keeps the wrong shield or backpack until the game exits"; break }
                if ($restoreState -eq 'unknown') { $shieldWhy = "SHIELD STATE UNKNOWN after level ${level}: neither the restore nor its check answered, so treat the live character as changed until the game exits"; break }
            }
            $log.shields_end = Py 'import json; print(json.dumps(pi_shields(), default=str))' | ConvertFrom-Json
        } catch { $shieldWhy = "$shieldWhy $_".Trim() }
        $log.shield_levels = $shieldRows
        $ownLevel = $log.shields_start.equipped.level
        $required = @($shieldLevels | Where-Object { $_ -ne $ownLevel })
        $measured = @($shieldRows | Where-Object { $_.read.counts -eq $true -and $_.level -in $required })
        "shields: $($log.shields_start.equipped.name) level $ownLevel capacity $($log.shields_start.max_strength); $($measured.Count) of $($required.Count) required levels measured"
        if (!$shieldWhy -and !$required.Count) { $shieldWhy = 'no level to measure' }
        if (!$shieldWhy -and $measured.Count -ne $required.Count) { $shieldWhy = "$($measured.Count) of $($required.Count) required levels measured" }
        Set-Stage 'shield_levels' (!$shieldWhy) $shieldWhy
        $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')
        # END SHIELD STAGE
    } else {
    # 1. Hit.
    $hitResult = Invoke-LiftCast 'hit' 1.0 8.0 $hostShots 50 -1 $null
    Set-Stage 'hit' (($hitResult.target_lifted -eq $true) -and (Test-CaptureComplete $log[$log.hit_attempt] $hostShots)) 'target not lifted, or frames or shots missing'
    $log.hit_ready = Wait-Ready 40

    # 2. Miss: look away and up at the sky (yaw + 180, pitch 60 degrees), since looking away level lifted a second bullymong on
    # 2026-11-06. Whether it really was a miss is in miss_result ("lifted" empty).
    Py "import json; isolate('$target'); pc = get_pc(); pc.Rotation = unrealsdk.make_struct('Rotator', Pitch=10922, Yaw=(pc.Rotation.Yaw + 32768) & 0xFFFF, Roll=0); print(json.dumps(pi_start('miss', '$target'), default=str))" | Out-Null
    Assert-GameForeground; Start-Sleep -Milliseconds 500
    $missShots = @{ 'miss-0.20' = 0.2; 'miss-0.40' = 0.4; 'miss-0.70' = 0.7 }
    $log.miss = Invoke-CastCapture 'miss_burst' 0.5 3.0 $missShots
    $log.miss_result = Py 'import json; print(json.dumps(pi_result(), default=str))' | ConvertFrom-Json
    "miss: lifted '$($log.miss_result.lifted -join ',')' (empty is a real miss)"
    Set-Stage 'miss' (($log.miss_result.cast_observed -eq $true) -and (@($log.miss_result.lifted).Count -eq 0) -and (Test-CaptureComplete $log.miss $missShots)) 'no cast observed, a pawn was lifted, or frames or shots missing'
    Py 'pi_stop(); print("stopped")' | Out-Null
    $log.miss_ready = Wait-Ready 40
    $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')

    # 3. Down during the lock (last: Maya then bleeds out). Waits out the target's diminishing returns (25 s) is not
    # needed: the rule under test is the end of the lock, not its length.
    if (!$SkipDown) {
        $injure = { Py "import json; protect_player(False); print(json.dumps(pi_injure('$target'), default=str))" 15 }
        $downShots = @{ 'down-1.90' = 1.9; 'down-2.50' = 2.5; 'down-3.50' = 3.5 }
        $downResult = Invoke-LiftCast 'down' 1.0 6.0 $downShots 100 2.0 $injure
        "down: $($log[$log.down_attempt].action)"
        Set-Stage 'down' (($downResult.target_lifted -eq $true) -and ($downResult.injure.injured -eq $true) -and (Test-CaptureComplete $log[$log.down_attempt] $downShots)) 'target not lifted, Maya not down, or frames or shots missing'
    }
    }
    # The watch's usability check, while the game still runs: a failed check is this stage's failure, and the run still quits and restores in order.
    if ($WatchSpec -and !$ProbeOnly) {
        try { $log.watch = Complete-SliceWatch $WatchSpec $Out; $log.watch.check; Set-Stage 'watch' $log.watch.usable 'the watch dump is not usable (see watch.check)' }
        catch { Set-Stage 'watch' $false "$_" }
    }
    # The game may close before it answers this one (2026-11-06: the empty reply stopped the script after all the casts).
    # Nothing is sent to a driver that stopped answering: the finalizer stops this run's own game by its PID, and the restore never needs the driver.
    if (Get-DriverLostReason) { "quit not sent: the driver stopped answering ($(Get-DriverLostReason))" }
    else {
        try { Invoke-GamePy 'protect_player(False); get_pc().ConsoleCommand("quit", False)' 15 | Out-Null } catch { "quit: no reply ($_); waiting for the exit" }
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
    # A throw between pi_start and pi_stop leaves the per-frame hook reading a held pawn into shutdown: stop it while the game can still answer.
    if ($proc -and !$proc.HasExited) {
        # Py returns a traceback as text, so only the exact answer counts. Nothing is sent to a driver that stopped answering.
        if (Get-DriverLostReason) { $cleanup.hooks = "not sent: the driver stopped answering ($(Get-DriverLostReason))" }
        else {
            # Each stop only if its probe was loaded; a run that failed before the probes has nothing to stop, which is a result, not an error.
            try { $reply = Py "fs = [globals().get(n) for n in ('pi_stop', 'unmark_lock')]; [f() for f in fs if f]; print('stopped' if any(fs) else 'nothing to stop')" 5; $cleanup.hooks = if ($reply -ceq 'stopped') { 'stopped' } elseif ($reply -ceq 'nothing to stop') { 'nothing to stop: never loaded' } else { "not stopped: $reply" } }
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
    # Exit 0 only for a completed run with nothing left to recover. Here in the finally, so -ProbeOnly's return passes through it too.
    if ($log.status -ne 'completed') { exit 1 }
}
