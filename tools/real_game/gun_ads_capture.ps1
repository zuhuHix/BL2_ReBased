# Real ADS frames of named guns (Maya, Save0008, Sanctuary), the way lane E took its frames: one idle still and one still while the zoom
# is held, each with a live read, for the host's ADS rule (docs/verification/WEAPON_VISUALS.md section 23). Takes the screen, keyboard and
# mouse. Everything written is game data under ignored local/realgame/gun_ads/. Locks: the shared hub lock and the run lock; saves are
# backed up, saving is blocked in game from the first frame, and afterwards the save folder is compared with the backup.
# With -MeasureFov, also per gun, at hip and at full zoom: known view directions projected through the HUD canvas, and two frames a known turn apart.
# "Steam must be running" at launch is a licence problem, not Steam: see the end of Steam's logs/console_log.txt for AppError_5.
param(
    # The machine-wide hub lock lives in the main checkout's local/ (shared by every worktree on this machine).
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'gun-ads',
    [string]$Character = 'Save0008.sav',   # quick_startup's -Character slot: the level-8 Maya in Sanctuary
    # Recipes inside this checkout's local/. A file's name is the gun's id. A comma-separated string also works: -File passes a list as one.
    [string[]]$Recipes = @('local/items/slice/slice_shotgun.json', 'local/wpa/sample_matrix/m_weapontype_hyperion_pistol.json'),
    [switch]$MeasureFov,
    [int]$Level = 8,                        # Every gun is made at Maya's level, which a recipe's own stage can be above: only the parts decide the look.
    # Give up waiting for the hub lock after this long and report its contents; a stale lock is removed by its owner or the
    # maintainer, never by this script.
    [int]$HubWaitMinutes = 120
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
# Asked-for guns first: a run refused here touches no evidence.
$Recipes = @($Recipes | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$guns = [ordered]@{}
foreach ($recipe in $Recipes) {
    $id = [IO.Path]::GetFileNameWithoutExtension($recipe)
    if ($id -notmatch '^[A-Za-z0-9_]+$') { throw "gun id '$id' is not of the form [A-Za-z0-9_]+" }
    if (!(Test-Path -LiteralPath (Join-Path $script:Repo $recipe) -PathType Leaf)) { throw "no recipe $recipe in this checkout" }
    $guns[$id] = $recipe -replace '\\', '/'
}
if (!$guns.Count) { throw 'no guns asked for' }
$Out = Join-Path $script:RG 'gun_ads'
New-Item -ItemType Directory -Force $Out | Out-Null
# Stale evidence: a final write that fails must not leave an earlier run's completed behind, and an earlier run's frame must not pass for this one's.
# Kept under a new name, not deleted: after a run that needs recovery, its record names the backup and says what went wrong.
$keptAs = "$(Get-Date -Format 'yyyyMMdd-HHmmss')-$PID"
foreach ($stale in 'session', 'save_compare') { $file = Join-Path $Out "$stale.json"; if (Test-Path -LiteralPath $file) { Move-Item -LiteralPath $file (Join-Path $Out "$stale.$keptAs.json") } }
$oldFrames = @(Get-ChildItem -LiteralPath $Out -Filter *.png -File)
if ($oldFrames.Count) { $keptFrames = New-Item -ItemType Directory (Join-Path $Out "earlier-$keptAs"); $oldFrames | Move-Item -Destination $keptFrames.FullName }
# End of stale evidence.
$log = [ordered]@{ started = (Get-Date).ToString('o'); guns = [ordered]@{} }

function Py([string]$Code, [int]$Timeout = 30) { ConvertTo-ReplyText (Invoke-GamePy $Code $Timeout) $Code }   # an empty reply is refused (realgame.ps1)
# Py returns a traceback as text; here it cannot pass as an answer, and the error says what it was.
function PyJson([string]$Expr, [int]$Timeout = 30) {
    $reply = Py "import json; print(json.dumps($Expr, default=str))" $Timeout
    if ($reply -match 'Traceback') { throw "game command failed: $Expr`: $reply" }
    $reply | ConvertFrom-Json
}
# A string as a JSON string literal, which Python reads as the same str: arguments never become source code.
function Q([string]$Text) { ConvertTo-Json -InputObject $Text -Compress }

# A gun's frame: the game in front before it, and still in front after it, without a second try: one that needed bringing back may show another window.
function Save-GunFrame([string]$Name) {
    Assert-GameForeground
    $frame = Save-Shot $Name
    if ((Get-ForegroundWindowHandle) -ne (Get-RunGameProcess).MainWindowHandle) { throw "the game lost the foreground during $Name" }
    $frame
}

# After the zoom is let go: wait for the settled state, so the next gun starts from it. Returns the last read.
function Wait-Unzoomed {
    for ($i = 0; $i -lt 50; $i++) {
        $read = PyJson 'ads_read()'
        if ($read.zoom_state -ceq 'ZST_NotZoomed') { break }
        Start-Sleep -Milliseconds 100
    }
    $read
}

# Python errors come back as text; a line that must succeed cannot ignore them.
function PyOk([string]$Code, [int]$Timeout = 30) {
    $reply = Py $Code $Timeout
    if ($reply -match 'Traceback') { throw "game command failed: $reply" }
    $reply
}

# Lane E's zoom: the right button held, and read back live; $Body runs once it has settled, given that read. The press is always released, and this returns only once it is.
function Invoke-WhileZoomed([scriptblock]$Body) {
    Assert-GameForeground
    Send-MouseEvent 0x8
    try {
        for ($i = 0; $i -lt 30; $i++) {
            $settled = PyJson 'ads_read()'
            if ($settled.zoomed -and $settled.zoomed_rate -eq 0) { break }
            Start-Sleep -Milliseconds 100
        }
        Start-Sleep -Milliseconds 300
        & $Body $settled
    } finally { Send-MouseEvent 0x10 }
}

function Save-AimedFrame([string]$Gun) {
    Invoke-WhileZoomed {
        param($before)
        $sample = Py "print(wp_sample($(Q "${Gun}_ads")))"
        $frame = Save-GunFrame "gun_ads/${Gun}_ads.png"
        [pscustomobject]@{ before = $before; sample = $sample; frame = $frame; after = (PyJson 'ads_read()') }
    }
}

# One state's FOV: the canvas projection, then two frames $TurnDegrees apart for the turn test, then the view back. The hook is off again whatever happens.
$TurnDegrees = 4
function Measure-ViewFov([string]$Name) {
    $result = [ordered]@{ read = (PyJson 'ads_read()'); view_0 = (PyJson 'ads_view()') }
    $result.arm = PyJson 'ads_fov_arm()'
    try { Start-Sleep -Milliseconds 500 } finally { $result.canvas = PyJson 'ads_fov_read()' }
    $result.frame_0 = Save-GunFrame "gun_ads/fov/${Name}_0.png"
    PyJson "ads_turn($TurnDegrees)" | Out-Null
    try {
        Start-Sleep -Milliseconds 400
        $result.frame_turned = Save-GunFrame "gun_ads/fov/${Name}_$TurnDegrees.png"
        # Read back: the frame counts only if the controller really stood the turn away from the first. The pixels are the real check, offline.
        $result.turned = PyJson "ads_turned_by($($result.view_0.yaw_units), $TurnDegrees)"
    } finally { PyJson 'ads_turn(0)' | Out-Null }
    Start-Sleep -Milliseconds 400
    [pscustomobject]$result
}

$extraRoots = [ordered]@{ sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$snapshots = @{}
$roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
foreach ($d in $extraRoots.Keys) { $roots[$d] = $extraRoots[$d] }
$stagesRequested = @($guns.Keys) + $(if ($MeasureFov) { @($guns.Keys | ForEach-Object { "${_}_fov" }) } else { @() }) + 'gun_cleanup'
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
    $proc = Start-Game $gameArgs; $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'gun_ads/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    Start-Sleep 5; Assert-GameForeground; Send-Key Enter; Start-Sleep 45
    $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
    "map: $map"; if ($map -notmatch 'Sanctuary' -or $map -notmatch 'WillowPlayerPawn') { Save-Shot 'gun_ads/load_failed.png' | Out-Null; throw "not Maya in Sanctuary: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 1

    # The probe seeds its own four slice guns as it loads, as in lane E. They are not used here, and its cleanup removes them.
    $seed = Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/weapon_parity_probe.py') gun_ads 60
    if ($seed -match 'Traceback') { throw "probe seed failed: $seed" }
    Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/gun_ads_probe.py') gun_ads | Out-Null
    # The record says Maya at this level, so it must be so before any frame.
    $log.character = PyJson 'ads_character()'
    if (!$log.character.is_maya -or $log.character.level -ne $Level) { throw "not Maya at level $Level`: $($log.character | ConvertTo-Json -Compress)" }
    $log.wall = PyJson 'wp_face_wall()' 60
    Save-Shot 'gun_ads/wall.png' | Out-Null
    $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')

    foreach ($id in $guns.Keys) {
        $g = [ordered]@{ recipe = $guns[$id] }; $log.guns[$id] = $g
        try {
            $g.seed = PyJson "ads_seed($(Q $id), $(Q $guns[$id]), $Level)" 60
            $heldSlot = Py 'w=get_pc().Pawn.Weapon; print(int(w.QuickSelectSlot) if w else -1)'
            $slot = if ($heldSlot -ceq '1') { 2 } else { 1 }
            $g.equip = Py "print(wp_equip($(Q $id), $slot))"
            Send-Key "$slot"; Start-Sleep 3
            # The reply itself decides, and only its exact words pass (an earlier guard read a log line, and passed nothing).
            $g.held_as = Py "print(ads_held($(Q $id)))"
            if ($g.held_as -cne 'exact' -and $g.held_as -cne 'parts') { throw "held weapon is not this gun: $($g.held_as)" }
            Py 'print(wp_reface())' | Out-Null; Start-Sleep 2
            $g.idle = PyJson 'ads_read()'
            Py "print(wp_sample($(Q "${id}_idle")))" | Out-Null
            $g.idle_frame = Save-GunFrame "gun_ads/${id}_idle.png"
            $g.ads = Save-AimedFrame $id
            $g.released = Wait-Unzoomed
            Set-Stage $id (($g.idle.zoom_state -ceq 'ZST_NotZoomed') -and $g.ads.before.zoomed -and ($g.ads.before.zoomed_rate -eq 0) -and $g.ads.after.zoomed -and ($g.released.zoom_state -ceq 'ZST_NotZoomed')) 'the idle frame was zoomed, the ADS frame was not fully zoomed, or the zoom did not settle after release'
            if ($MeasureFov) {
                try {
                    $g.fov_hip = Measure-ViewFov "${id}_hip"
                    $g.fov_zoom = Invoke-WhileZoomed {
                        param($before)
                        $measured = Measure-ViewFov "${id}_zoom"
                        # Last, while held: a native called from Python once lost a session (TOOLING.md); the frames are on disk by now.
                        $foreground = PyJson 'ads_foreground()' 10
                        [pscustomobject]@{ before = $before; measured = $measured; foreground = $foreground; after = (PyJson 'ads_read()') }
                    }
                    $g.fov_released = Wait-Unzoomed
                    $states = ($g.fov_hip.read.zoom_state -ceq 'ZST_NotZoomed') -and $g.fov_zoom.before.zoomed -and $g.fov_zoom.after.zoomed -and ($g.fov_released.zoom_state -ceq 'ZST_NotZoomed')
                    # A measure must answer: the turn at both states, since a canvas alone would leave the pixels unchecked. status: completed cannot mean nothing was measured.
                    $turned = ($g.fov_hip.turned.moved -eq $true) -and ($g.fov_zoom.measured.turned.moved -eq $true)
                    Set-Stage "${id}_fov" ($states -and $turned) "states as measured: $states; view turned at hip and zoom: $turned (canvas fired $($g.fov_hip.canvas.fired), $($g.fov_zoom.measured.canvas.fired) times)"
                } catch { $g.fov_error = "$_"; Set-Stage "${id}_fov" $false "$_" }
            }
        } catch { $g.error = "$_"; Set-Stage $id $false "$_" }
        "gun $id`: held $($g.held_as), idle $($g.idle.zoom_state), ADS $($g.ads.after.zoom_state) $($g.error)"
        $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')
    }

    # Spawned guns go before the quit (TOOLING.md). A gun left in the game is a failed run, though the frames stand and the saves are restored anyway.
    try {
        # As lane E: a slot the hand is not on first, then the hand moves there, then the rest. wp_cleanup refuses a probe gun still held.
        $heldSlot = Py 'w=get_pc().Pawn.Weapon; print(int(w.QuickSelectSlot) if w else -1)'
        $other = if ($heldSlot -ceq '1') { 2 } else { 1 }
        PyOk "print(wp_restore_slot($other))" | Out-Null; Send-Key "$other"; Start-Sleep 3
        foreach ($s in 1..4) { if ($s -ne $other) { try { PyOk "print(wp_restore_slot($s))" | Out-Null } catch { "restore slot $s`: $_" } } }
        $log.gun_cleanup = PyJson 'wp_cleanup()' 30
        $left = @($log.gun_cleanup.remaining)
        Set-Stage 'gun_cleanup' ($null -ne $log.gun_cleanup.remaining -and $left.Count -eq 0) "spawned guns left: $($left -join ', ')"
    } catch { $log.gun_cleanup = "not finished: $_"; Set-Stage 'gun_cleanup' $false "$_" }
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
    $cleanup.inputs = Release-HeldInputs   # every key and button this run still holds is let go before anything else
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
