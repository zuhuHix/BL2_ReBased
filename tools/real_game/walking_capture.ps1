# Walking ground truth from the running game (docs: COLLISION_WALKING_VERIFICATION.md, "Routes and stairs").
# Loads the last character (Continue) into Sanctuary with saves blocked, reads the pawn's movement settings and every
# volume's state, then walks each probe of tools/walking_routes.py's candidate list with the steering autopilot of
# tools/real_game/scripts/walking.py while holding the forward key. Takes the screen and keyboard.
# Everything written is game data under ignored local/realgame/walking/. Saves are compared with the backup afterwards.
param(
    [Parameter(Mandatory)][string]$Probes,          # JSON list from walking_routes.py candidates (plus any hand routes)
    [string]$HubLock = 'C:\Users\yorad\Documents\BL2_ReBased\local\hub-ue.lock',
    [string]$Lane = 'B',
    [string]$Character = 'Save0008.sav',            # quick_startup's -Character slot (a character standing in Sanctuary); '' = Continue
    [switch]$CallerHoldsHubLock,                    # the caller already took the shared UE/game lock
    [double]$Speed = 150                            # cm/s used only to size each probe's time limit
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$probeList = Get-Content $Probes -Raw | ConvertFrom-Json
$hub = $null; $bak = $null
if (-not $CallerHoldsHubLock) { $hub = New-Item -ItemType File -Path $HubLock -Value "$Lane $((Get-Date).ToString('o'))" -ErrorAction Stop }
try {
    Enter-RunLock 600
    $bak = Backup-Saves; "backup $bak"
    Install-Driver
    Get-ChildItem (Join-Path $script:RG 'cmd') -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $gameArgs = @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies')
    if ($Character) { $gameArgs += "-Character=$Character" }  # the quick_startup mod puts this slot on the menu's Continue
    Start-Game $gameArgs | Out-Null
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Invoke-GamePy 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { throw 'driver did not answer' }
    Invoke-GamePy 'print(block_saves())' | Out-Null
    Focus-Game | Out-Null
    if ($Character) { Start-Sleep 5; Focus-Game | Out-Null; Send-Key Enter; Start-Sleep 45 }
    else { Send-Key C; Start-Sleep 2; Send-Key Enter; Start-Sleep 3; Send-Key Enter; Start-Sleep 40 }
    $map = Invoke-GamePy 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name)' 20
    "map: $map"
    if ($map -notmatch 'Sanctuary') { throw "not in Sanctuary: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 1
    # Start from an empty trace folder so no probe can reuse a trace from an earlier capture.
    Get-ChildItem (Join-Path $script:RG 'walking') -Filter *.jsonl -ErrorAction SilentlyContinue | Remove-Item
    Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/walking.py') walking 60 | Out-Null
    "dump: " + (Invoke-GamePy "print(walk_dump('walk_info'))" 60)
    Save-Shot 'walking/loaded.png' | Out-Null
    foreach ($probe in $probeList) {
        $pts = @($probe.points)
        $a = $pts[0]; $b = $pts[1]
        $yaw = [math]::Atan2($b[1] - $a[1], $b[0] - $a[0]) * 180 / [math]::PI
        $length = 0; for ($k = 1; $k -lt $pts.Count; $k++) { $length += [math]::Sqrt([math]::Pow($pts[$k][0] - $pts[$k-1][0], 2) + [math]::Pow($pts[$k][1] - $pts[$k-1][1], 2)) }
        $limit = [math]::Round($length / $Speed + 6, 1)
        Invoke-GamePy ("walk_goto({0}, {1}, {2}, {3})" -f $a[0], $a[1], $a[2], $yaw) 20 | Out-Null
        Start-Sleep -Milliseconds 2500
        Focus-Game | Out-Null
        $json = ($pts | ForEach-Object { '[{0},{1},{2}]' -f $_[0], $_[1], $_[2] }) -join ','
        Send-KeyDown W
        try {
            Invoke-GamePy ("print(walk_route('{0}', [{1}], {2}))" -f $probe.name, $json, $limit) 20 | Out-Null
            $deadline = (Get-Date).AddSeconds($limit + 5)
            do { Start-Sleep -Milliseconds 500; $status = Invoke-GamePy 'print(walk_status())' 10 } while ($status -match "'running': True" -and (Get-Date) -lt $deadline)
        } finally { Send-KeyUp W }
        Invoke-GamePy 'walk_stop()' 10 | Out-Null
        "$($probe.name): $status"
        Save-Shot "walking/$($probe.name)_end.png" | Out-Null
    }
    # One standing jump on flat ground at the first probe's start, recorded every tick.
    $a = @($probeList[0].points)[0]
    Invoke-GamePy ("walk_goto({0}, {1}, {2}, 0)" -f $a[0], $a[1], $a[2]) 20 | Out-Null
    Start-Sleep -Milliseconds 2500; Focus-Game | Out-Null
    Invoke-GamePy "print(walk_record('jump', 3.0))" 20 | Out-Null
    Start-Sleep -Milliseconds 300; Send-Key Space 80; Start-Sleep 3.5
    "jump: " + (Invoke-GamePy 'print(walk_status())' 10)
    Invoke-GamePy 'walk_stop(); get_pc().ConsoleCommand("quit", False)' 10 | Out-Null
    for ($i = 0; $i -lt 20 -and (Get-GameProcess); $i++) { Start-Sleep 2 }
} finally {
    Start-Sleep 3
    if (Get-GameProcess) { Stop-Process -Id (Get-GameProcess).Id -Force }
    Remove-Driver
    if ($bak) {
        $src = Join-Path $env:USERPROFILE 'Documents/My Games/Borderlands 2/WillowGame/SaveData'
        $b = $bak + '\SaveData'
        Get-ChildItem $b -Recurse -File | ForEach-Object {
            $o = Join-Path $src $_.FullName.Substring($b.Length)
            if ((Get-FileHash $o).Hash -ne (Get-FileHash $_.FullName).Hash) { "restoring $o"; Copy-Item $_.FullName $o -Force }
        }
        'save compare done'
    }
    Exit-RunLock
    if ($hub) { Remove-Item $HubLock -Force -ErrorAction SilentlyContinue }
}
