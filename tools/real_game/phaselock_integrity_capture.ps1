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
    [switch]$ProbeOnly,                     # stop after pi_discover / pi_flags
    [switch]$SkipDown,
    # Give up waiting for the hub lock after this long and report its contents; a stale lock is removed by its owner or the
    # maintainer, never by this script.
    [int]$HubWaitMinutes = 120
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'phaselock_integrity'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o') }

function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }

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
        if ($null -eq $press -and $ts - $t0 -ge $Before) { Send-KeyDown F; $press = Get-QpcSeconds; Start-Sleep -Milliseconds 60; Send-KeyUp F; continue }
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
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $target = $null; $runLock = $false; $driverOwned = $false
# A token on the game's command line that identifies the process this run started (Stop-OwnGame checks it with the
# image path and the start time before stopping a PID). Unknown switches are ignored by the game.
$marker = "-owrun=$Lane-$PID"
$hostShots = @{ 'hit-0.25' = 0.25; 'hit-0.50' = 0.5; 'hit-0.80' = 0.8; 'hit-1.50' = 1.5; 'hit-3.00' = 3.0; 'hit-4.50' = 4.5; 'hit-5.30' = 5.3; 'hit-6.20' = 6.2 }
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
    foreach ($d in $extraRoots.Keys) { if (Test-Path $extraRoots[$d]) { Copy-Item -Recurse $extraRoots[$d] (Join-Path $bak $d) } }
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
    if (-not $ok) { Save-Shot 'phaselock_integrity/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Py 'print(block_saves())'
    Start-Sleep 5; Focus-Game | Out-Null; Send-Key Enter; Start-Sleep 45
    $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
    "map: $map"; if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'phaselock_integrity/load_failed.png' | Out-Null; throw "no player pawn: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 1
    Py 'get_pc().ConsoleCommand("open Ice_P", False); print("sent")' | Out-Null
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep 3
        try { $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn is not None)' 8 } catch { continue }
        if ($map -match 'Ice_P True') { break }
    }
    "map: $map"; if ($map -notmatch 'Ice_P True') { throw "travel to Ice_P failed: $map" }
    Start-Sleep 8
    foreach ($f in 'phaselock.py', 'phaselock_matched.py', 'phaselock_integrity.py') { Invoke-GamePyFile (Join-Path $PSScriptRoot "scripts/$f") phaselock_integrity | Out-Null }

    for ($i = 0; $i -lt 8 -and !$target; $i++) {
        $pick = Py @'
import json
rows = []
for d, n, c, dead in list_ai():
    p = ai_pawn(n)
    cls = p.AIClass._path_name() if p.AIClass else ""
    radius = p.CylinderComponent.CollisionRadius if p.CylinderComponent else 0
    rows.append({"name": n, "dist": d, "class": cls.split(".")[-1], "radius": radius, "dead": dead, "ctrl": p.Controller is not None})
ok = [r for r in rows if not r["dead"] and r["ctrl"] and "PrimalBeast" in r["class"] and r["radius"] >= 120]
ok.sort(key=lambda r: (not r["class"].endswith("CharClass_PrimalBeast"), r["dist"]))
print(json.dumps({"pick": ok[0]["name"] if ok else None, "rows": rows[:30]}))
'@
        $res = $pick | ConvertFrom-Json; $log.candidates = $res.rows; $target = $res.pick
        if (!$target) { Start-Sleep 5 }
    }
    if (!$target) { throw "no adult bullymong: $pick" }
    $log.target = $target; "target $target"
    $log.discover = Py 'import json; print(json.dumps(pi_discover(), default=str))' | ConvertFrom-Json
    # pi_flags() (CanPhaseLockTarget called from Python for every pawn) got no reply on 2026-11-02 and the session was lost; the
    # cast's own CanPhaseLockTarget result is marked by mark_lock() instead.
    $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')
    if ($ProbeOnly) { return }

    function Set-Up([string]$Label) {
        $s = Py "import json; print(json.dumps({'protect': len(protect_player(True)), 'isolate': isolate('$target'), 'place': place('$target', 650.0)}))" | ConvertFrom-Json
        Start-Sleep 3
        $a = Py "import json; isolate('$target'); print(json.dumps({'pitch': aim('$target'), 'measure': measure('$target'), 'camera': camera(), 'bounds': bounds('$target')}))" | ConvertFrom-Json
        Py "mark_lock('$target'); print(json.dumps(pi_start('$Label', '$target'), default=str))" | Out-Null
        Focus-Game | Out-Null; Start-Sleep -Milliseconds 500
        @{ setup = $s; aim = $a }
    }

    # 1. Hit.
    $log.hit_setup = Set-Up 'hit'
    $log.hit = Invoke-CastCapture 'hit_burst' 1.0 8.0 $hostShots
    "hit press at $($log.hit.press); frames $($log.hit.frames)"
    Py 'pi_stop(); unmark_lock(); print("stopped")' | Out-Null
    $log.hit_ready = Wait-Ready 40

    # 2. Miss: look away from every pawn (yaw + 180, level).
    Py "import json; pc = get_pc(); pc.Rotation = unrealsdk.make_struct('Rotator', Pitch=0, Yaw=(pc.Rotation.Yaw + 32768) & 0xFFFF, Roll=0); print(json.dumps(pi_start('miss', '$target'), default=str))" | Out-Null
    Focus-Game | Out-Null; Start-Sleep -Milliseconds 500
    $log.miss = Invoke-CastCapture 'miss_burst' 0.5 3.0 @{ 'miss-0.20' = 0.2; 'miss-0.40' = 0.4; 'miss-0.70' = 0.7 }
    Py 'pi_stop(); print("stopped")' | Out-Null
    $log.miss_ready = Wait-Ready 40
    $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')

    # 3. Down during the lock (last: Maya then bleeds out). Waits out the target's diminishing returns (25 s) is not
    # needed: the rule under test is the end of the lock, not its length.
    if (!$SkipDown) {
        $log.down_setup = Set-Up 'down'
        $injure = { Py "import json; protect_player(False); print(json.dumps(pi_injure('$target'), default=str))" 15 }
        $log.down = Invoke-CastCapture 'down_burst' 1.0 6.0 @{ 'down-1.90' = 1.9; 'down-2.50' = 2.5; 'down-3.50' = 3.5 } 100 2.0 $injure
        "down: $($log.down.action)"
        Py 'pi_stop(); unmark_lock(); print("stopped")' | Out-Null
    }
    Py 'protect_player(False); get_pc().ConsoleCommand("quit", False)' | Out-Null
    for ($i = 0; $i -lt 20 -and (Get-GameProcess); $i++) { Start-Sleep 2 }
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
    if (!$gameGone) {
        $cleanup.driver = "NOT REMOVED (game exit not confirmed): $script:DriverDir"
        $cleanup.saves = "NOT RESTORED (game exit not confirmed); backup kept at $bak"
        @"
MANUAL RECOVERY: Borderlands 2 (pid $($proc.Id)) did not confirm its exit ($($cleanup.game)). Nothing was removed or restored.
  1. Close Borderlands 2 yourself.
  2. In a new PowerShell window: `$env:OPENWILLOW_BL2 = '$env:OPENWILLOW_BL2'; . '$PSScriptRoot\realgame.ps1'
     Remove-Item -Recurse -Force '$script:DriverDir'
     foreach (`$sub in 'SaveData', 'Config', 'sdk_settings', 'SanitySaver') { `$b = Join-Path '$bak' `$sub; if (Test-Path `$b) { Restore-Tree `$b `$(@{ SaveData = (Join-Path `$script:WillowDir 'SaveData'); Config = (Join-Path `$script:WillowDir 'Config'); sdk_settings = '$($extraRoots.sdk_settings)'; SanitySaver = '$($extraRoots.SanitySaver)' }[`$sub]) `$sub } }
     Every row must read 'added, removed', 'missing, restored' or 'changed, restored'; a 'MISMATCH after restore' row means it failed.
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
    $rows = @()
    if ($bak) {
        try {
            $roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
            foreach ($d in $extraRoots.Keys) { $roots[$d] = $extraRoots[$d] }
            foreach ($sub in $roots.Keys) {
                $b = Join-Path $bak $sub; $live = $roots[$sub]
                try {
                    if (Test-Path -LiteralPath $b) { $rows += @(Restore-Tree $b $live $sub) }
                    elseif (Test-Path -LiteralPath $live) {
                        # Absent at the backup and created during the run (an SDK mod's own store): reported, not deleted.
                        $rows += [pscustomobject]@{ file = $sub; status = 'created during the run; left for the maintainer' }
                    }
                } catch { $rows += [pscustomobject]@{ file = $sub; status = "RESTORE FAILED: $_" } }
            }
            $bad = @($rows | Where-Object { $_.status -like 'MISMATCH*' -or $_.status -like 'RESTORE FAILED*' })
            $cleanup.saves = if ($bad.Count) { "NOT IDENTICAL to $bak ($($bad.Count) problems)" } else { "identical to $bak after $($rows.Count) restores" }
        } catch { $cleanup.saves = "RESTORE FAILED: $_ (backup kept at $bak)" }
    }
    try {
        $log.finished = (Get-Date).ToString('o'); $log.cleanup = $cleanup
        $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')
        if ($bak) { @{ backup = $bak; compared = (Get-Date).ToString('o'); changes = @($rows) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Out 'save_compare.json') }
    } catch { "log write failed: $_" }
    try { if ($runLock) { Exit-RunLock } } catch { "run lock release failed: $_" }
    try { if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force } } catch { "hub lock release failed: $_" }
    $cleanup.GetEnumerator() | ForEach-Object { "cleanup $($_.Key): $($_.Value)" }
    $rows
}
