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

$hub = $null; $bak = $null; $proc = $null; $runLock = $false
try {
    while (!$hub) {
        try { $hub = New-Item -ItemType File -Path $HubLock -Value "$Lane $((Get-Date).ToString('o'))" -ErrorAction Stop }
        catch { "hub lock held by '$(Get-Content $HubLock -Raw -ErrorAction SilentlyContinue)'; retry in 60 s"; Start-Sleep 60 }
    }
    if (Get-GameProcess) { throw 'Borderlands2 is running although the hub lock was free; not touching it' }
    Enter-RunLock 600; $runLock = $true
    $bak = Backup-Saves; $log.backup = $bak; "backup $bak"
    Install-Driver
    foreach ($res in $Resolutions) {
        $w, $h = $res -split 'x'; $run = [ordered]@{}; $log.runs[$res] = $run
        Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
        $proc = Start-Game @('-windowed', "-ResX=$w", "-ResY=$h", '-nostartupmovies', "-Character=$Character")
        $ok = $false
        for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
        if (-not $ok) { Save-Shot "hud_layout/boot_$res.png" | Out-Null; throw "driver did not answer at $res" }
        $run.block_saves = Py 'print(block_saves())'
        Start-Sleep 5; Focus-Game | Out-Null; Send-Key Enter; Start-Sleep 45
        $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
        "map at ${res}: $map"; $run.map = $map
        if ($map -notmatch 'WillowPlayerPawn') { Save-Shot "hud_layout/load_failed_$res.png" | Out-Null; throw "no player pawn: $map" }
        Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 3
        Focus-Game | Out-Null; Start-Sleep 2
        $rect = Get-GameRect; $run.client = @{ x = $rect.X; y = $rect.Y; width = $rect.Width; height = $rect.Height }
        Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/hud_layout.py') hud_layout | Out-Null
        $run.dump = Py "import json; print(json.dumps(hl_dump('layout_$res', $clipsPy), default=str))" 60
        $run.shot = Save-Shot "hud_layout/shot_$res.png"
        "dump ${res}: $($run.dump); client $($rect.Width)x$($rect.Height)"
        try { Invoke-GamePy 'get_pc().ConsoleCommand("quit", False)' 10 | Out-Null } catch { "quit sent (no reply)" }
        for ($i = 0; $i -lt 30 -and (Get-GameProcess); $i++) { Start-Sleep 2 }
        $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
        if ($p -and $p.ProcessName -eq 'Borderlands2') { Stop-Process -Id $proc.Id -Force; Start-Sleep 3 }
        $proc = $null
    }
} finally {
    $log.finished = (Get-Date).ToString('o')
    $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')
    if ($proc) { Start-Sleep 2; $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue; if ($p -and $p.ProcessName -eq 'Borderlands2') { Stop-Process -Id $proc.Id -Force } }
    try { Remove-Driver } catch { "Remove-Driver failed: $_" }
    if ($runLock) { Exit-RunLock }
    if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force }
    if ($bak) {
        $src = Join-Path $env:USERPROFILE 'Documents/My Games/Borderlands 2/WillowGame'
        $rows = foreach ($sub in 'SaveData', 'Config') {
            $b = Join-Path $bak $sub; $s = Join-Path $src $sub
            Get-ChildItem $s -Recurse -File | ForEach-Object {
                $rel = $_.FullName.Substring($s.Length + 1); $old = Join-Path $b $rel
                if (!(Test-Path $old)) { [pscustomobject]@{ file = "$sub\$rel"; status = 'added (left in place)' } }
                elseif ((Get-FileHash $old).Hash -ne (Get-FileHash $_.FullName).Hash) {
                    Copy-Item $old $_.FullName -Force; [pscustomobject]@{ file = "$sub\$rel"; status = 'changed, restored' }
                }
            }
            Get-ChildItem $b -Recurse -File | Where-Object { !(Test-Path (Join-Path $s $_.FullName.Substring($b.Length + 1))) } |
                ForEach-Object { [pscustomobject]@{ file = "$sub\$($_.FullName.Substring($b.Length + 1))"; status = 'missing' } }
        }
        @{ backup = $bak; compared = (Get-Date).ToString('o'); changes = @($rows) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Out 'save_compare.json')
        "save compare: $(@($rows).Count) differences"; $rows
    }
}
