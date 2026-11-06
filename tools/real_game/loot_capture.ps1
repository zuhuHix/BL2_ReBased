# Lootable and pickup ground truth (vendors lane): start the game, load the Maya save into Sanctuary with saves blocked from
# the first frame, then for each lootable type take closed, opening and opened frames, then walk over an ammo and a cash
# pickup, and read what material each mesh and placed object uses. Everything written is game data under ignored
# local/realgame/loot_capture/. Locks as in vendors_capture.ps1; the saves and config are backed up first and restored
# byte-identical afterwards (Restore-Tree), nothing is saved, and only the game PID this run started is stopped.
# Questions: is there floating text on loot on the ground? Does collecting a pickup show a message? Which way does each lid
# open? What do the storage locker and strong box render? Placed positions are found in the running game; the only other
# source is the ignored -Interactives file, read at run time for a type not found from the start.
param(
    [string]$HubLock = 'C:\Users\yorad\Documents\BL2_ReBased\local\hub-ue.lock',
    [string]$Lane = 'vendors',
    [string]$Character = 'Save0008.sav',
    [string]$Interactives = (Join-Path $PSScriptRoot '../../local/slice/interactives.json')
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
$marker = '-owlootcapture'   # an argument the game ignores; Stop-OwnGame checks for it in the command line
# The mesh a placed object shows, and the balance that places it in the ignored data.
$types = [ordered]@{
    'Skel_DahlChest_Ammo' = 'ObjectGrade_DahlAmmo'; 'Skel_Dumpster' = 'ObjectGrade_Dumpster'
    'Skel_StorageLocker' = 'ObjectGrade_StorageLocker'; 'Skel_StrongBox' = 'ObjectGrade_StrongBox'
    'Skel_CardboardBox' = 'ObjectGrade_Cardboard_Box'; 'Skel_LaundryMachine' = 'ObjectGrade_LaundryMachine'
}

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

$hub = $null; $bak = $null; $proc = $null; $started = $null; $runLock = $false; $driver = $false
try {
    try { $hub = New-Item -ItemType File -Path $HubLock -Value "$Lane $((Get-Date).ToString('o'))" -ErrorAction Stop }
    catch { throw "hub lock held by '$(Get-Content $HubLock -Raw -ErrorAction SilentlyContinue)'; not waiting (ask the hub)" }
    if (Get-GameProcess) { throw 'Borderlands2 is running although the hub lock was free; not touching it' }
    Enter-RunLock 600; $runLock = $true
    $bak = Backup-Saves; $log.backup = $bak; "backup $bak"
    Install-Driver -BlockSavesAtStart -Exclusive; $driver = $true
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $started = Get-Date
    $proc = Start-Game @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', "-Character=$Character", $marker)
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

    $log.materials = Py "print(json.dumps(lc_materials('materials')))" 60
    "materials: $($log.materials)"
    $log.ammo = Py 'print(json.dumps(lc_lower_ammo()))'
    "ammo: $($log.ammo)"

    # One failed type must not lose the others; a lost focus or a dead game still stops the run through its next call.
    foreach ($mesh in $types.Keys) {
        if (-not ($counts.$mesh -gt 0)) { $log.types[$mesh] = 'not in the world'; "$mesh`: not in the world"; continue }
        try { $log.types[$mesh] = Capture-Type $mesh; "$mesh`: done" }
        catch {
            if ($_.Exception.Message -match 'focus') { throw }
            $log.types[$mesh] = "failed: $($_.Exception.Message)"; "$mesh`: failed: $($_.Exception.Message)"
        }
    }
    foreach ($kind in 'ammo', 'cash') {
        try { $log.pickups[$kind] = Capture-Pickup $kind; "$kind`: $($log.pickups[$kind].state)" }
        catch {
            if ($_.Exception.Message -match 'focus') { throw }
            $log.pickups[$kind] = "failed: $($_.Exception.Message)"; "$kind`: failed: $($_.Exception.Message)"
        }
    }
    $log.end_shot = Save-Shot "$Folder/end.png"
    try { Invoke-GamePy 'get_pc().ConsoleCommand("quit", False)' 10 | Out-Null } catch { 'quit sent (no reply)' }
    for ($i = 0; $i -lt 30 -and -not $proc.HasExited; $i++) { Start-Sleep 2 }
} finally {
    if ($proc -and -not $proc.HasExited) { $log.stop = Stop-OwnGame $proc $started $marker; "stop: $($log.stop)"; Start-Sleep 3 }
    if ($driver) { try { Remove-Driver } catch { "Remove-Driver failed: $_" } }
    if ($bak) {
        $rows = @(Restore-Tree (Join-Path $bak 'SaveData') (Join-Path $script:WillowDir 'SaveData') 'SaveData') +
                @(Restore-Tree (Join-Path $bak 'Config') (Join-Path $script:WillowDir 'Config') 'Config')
        $log.restore = $rows
        "restore: $(@($rows).Count) differences"; $rows
    }
    if ($runLock) { Exit-RunLock }
    if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force }
    $log.finished = (Get-Date).ToString('o')
    $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')
}
