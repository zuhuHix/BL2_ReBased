# Vending machine ground truth (vendors lane): start the game, load the Maya save into Sanctuary with saves blocked
# from the first frame, run scripts/vendors_probe.py vp_dump() once (read-only), quit. Everything written is game data
# under ignored local/realgame/vendors/. Locks as in hud_layout_capture.ps1; the saves and config are backed up first and
# restored byte-identical afterwards (Restore-Tree), and only the game PID this run started is stopped.
param(
    [string]$HubLock = 'C:\Users\yorad\Documents\BL2_ReBased\local\hub-ue.lock',
    [string]$Lane = 'vendors',
    [string]$Character = 'Save0008.sav',
    [string]$Name = 'sanctuary'
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'vendors'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o') }
function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }
$marker = '-owvendorsprobe'   # an argument the game ignores; Stop-OwnGame checks for it in the command line

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
    if (-not $ok) { $log.boot = 'driver did not answer'; Save-Shot 'vendors/boot.png' | Out-Null; throw 'driver did not answer (check Steam console_log for AppError_5)' }
    $log.block_saves = Py 'print(block_saves())'
    Start-Sleep 5; Focus-Game | Out-Null; Send-Key Enter; Start-Sleep 45
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
    $log.shot = Save-Shot 'vendors/shot.png'
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
