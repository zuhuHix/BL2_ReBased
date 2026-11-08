# HUD layout ground truth: start the game windowed at each resolution, load the Maya save into Sanctuary and dump the
# HUD movie's placement once (scripts/hud_layout.py) plus one full-window Save-Shot. Takes the screen and keyboard.
# Everything written is game data under ignored local/realgame/hud_layout/. Locks, save backup and save compare as in
# action_skill_hud_capture.ps1; the resolution is given on the command line only (the ini is compared and restored).
param(
    [string]$HubLock = 'C:\Users\yorad\Documents\BL2_ReBased\local\hub-ue.lock',
    [string]$Lane = 'laneC',
    [string]$Character = 'Save0008.sav',
    [string[]]$Resolutions = @('1280x720', '1920x1080')
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'hud_layout'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o'); runs = [ordered]@{} }
function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }

# p1's direct children: the first p1 section of the HUD tree listing (local/ui/hud_tree.txt), plus a few grandchildren.
$tree = Join-Path $script:Repo 'local/ui/hud_tree.txt'
$names = @(); $inP1 = $false
foreach ($line in Get-Content $tree) {
    if ($line -match '^  p1 ') { if ($inP1) { break }; $inP1 = $true; continue }
    if ($inP1 -and $line -match '^    (\w+) ') { $names += $Matches[1] }
}
$clips = @($names | ForEach-Object { "p1.$_" }) + @('p1.character.cooldown', 'p1.activeAbility.bar', 'p1.health.health_disp',
    'p1.shield.shield_disp', 'p1.levelClassMod.level', 'p1.bullets.bullet_c')
$clipsPy = '[' + (($clips | ForEach-Object { "'$_'" }) -join ',') + ']'

$marker = "-owrun=$Lane-$PID"   # with the image path and start time, what Stop-OwnGame checks before stopping a PID
$procStart = $null
$hub = $null; $bak = $null; $proc = $null; $runLock = $false
try {
    Assert-NoPendingRecords   # an earlier run's pending record stops this one before it takes a lock or changes anything (pending.ps1)
    while (!$hub) {
        try { $hub = New-Item -ItemType File -Path $HubLock -Value "$Lane $((Get-Date).ToString('o'))" -ErrorAction Stop }
        catch { "hub lock held by '$(Get-Content $HubLock -Raw -ErrorAction SilentlyContinue)'; retry in 60 s"; Start-Sleep 60 }
    }
    if (Get-GameProcess) { throw 'Borderlands2 is running although the hub lock was free; not touching it' }
    Enter-RunLock 600; $runLock = $true
    $bak = Backup-Saves; $log.backup = $bak; "backup $bak"
    Install-Driver -Exclusive
    foreach ($res in $Resolutions) {
        $w, $h = $res -split 'x'; $run = [ordered]@{}; $log.runs[$res] = $run
        Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
        $proc = Start-Game @('-windowed', "-ResX=$w", "-ResY=$h", '-nostartupmovies', "-Character=$Character", $marker); $procStart = $proc.StartTime
        $ok = $false
        for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
        if (-not $ok) { Save-Shot "hud_layout/boot_$res.png" | Out-Null; throw "driver did not answer at $res" }
        $run.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
        Start-Sleep 5; Assert-GameForeground; Send-Key Enter; Start-Sleep 45
        $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
        "map at ${res}: $map"; $run.map = $map
        if ($map -notmatch 'WillowPlayerPawn') { Save-Shot "hud_layout/load_failed_$res.png" | Out-Null; throw "no player pawn: $map" }
        Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 3
        Assert-GameForeground; Start-Sleep 2
        $rect = Get-GameRect; $run.client = @{ x = $rect.X; y = $rect.Y; width = $rect.Width; height = $rect.Height }
        Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/hud_layout.py') hud_layout | Out-Null
        $run.dump = Py "import json; print(json.dumps(hl_dump('layout_$res', $clipsPy), default=str))" 60
        $run.shot = Save-Shot "hud_layout/shot_$res.png"
        "dump ${res}: $($run.dump); client $($rect.Width)x$($rect.Height)"
        try { Invoke-GamePy 'get_pc().ConsoleCommand("quit", False)' 10 | Out-Null } catch { "quit sent (no reply)" }
        for ($i = 0; $i -lt 30 -and (Get-GameProcess); $i++) { Start-Sleep 2 }
        $stopped = Stop-OwnGame $proc $procStart $marker; "game: $stopped"
        if ($stopped -like 'refused*') { throw "the game was not stopped: $stopped" }
        if ($stopped -eq 'stopped') { Start-Sleep 3 }
        $proc = $null
    }
} finally {
    "cleanup inputs: $(Release-HeldInputs)"   # every key and button this run still holds is let go before anything else
    $log.finished = (Get-Date).ToString('o')
    $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')
    if ($proc) { Start-Sleep 2; "cleanup game: $(Stop-OwnGame $proc $procStart $marker)"; Start-Sleep 3 }
    try { Remove-Driver } catch { "Remove-Driver failed: $_" }
    # Restore and verify while both locks are held (Complete-BackupRun): this run's pending record goes only when every root compares clean,
    # and anything left for a person keeps both locks.
    $keepLocks = $false
    if ($bak) { $done = Complete-BackupRun $bak (Join-Path $Out 'save_compare.json'); $done.lines; $keepLocks = !$done.clean }
    if ($runLock -and !$keepLocks) { Exit-RunLock }
    if (!$keepLocks -and $hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force }
}
