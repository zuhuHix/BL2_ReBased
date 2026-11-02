#Requires -Version 7.0
# Weapon part assets: live reads and matched captures from the running game (a Maya save in Sanctuary, 1280x720 windowed).
#   -Mode Census   runs scripts/weapon_visual_census.py once (every gestalt table, weapon part, weapon type and skin MIC).
#   -Mode Capture  runs scripts/weapon_part_capture.py: spawns the guns of -RecipeList (local recipe JSONs) in memory, reads each
#                  gun's live gestalt fragments and triangle ranges, and takes a first-person and an Inspect frame per gun.
# Everything written is game data under ignored local/realgame/wpa/. Locks: the shared hub lock (waits while another lane
# holds it) and the run lock; saves are backed up, saving is blocked in game, and afterwards the save folder is compared with
# the backup (changed files are restored). Takes the screen and keyboard while it runs.
param(
    [ValidateSet('Census', 'Capture')][string]$Mode = 'Census',
    [string]$Character = 'Save0008.sav',
    # Capture mode: a text file listing local recipe JSON paths, one per line (a file, because pwsh -File splits arrays).
    [string]$RecipeList = '',
    [string]$HubLock = 'C:\Users\yorad\Documents\BL2_ReBased\local\hub-ue.lock',
    [string]$Lane = 'weaponparts',
    # Inspect frames too (needs the gun in an equipped slot; on 2026-10-23 the SDK-held gun stayed unslotted, so off by default).
    [switch]$Inspect
)
$ErrorActionPreference = 'Stop'
$Recipes = if ($RecipeList) { @(Get-Content $RecipeList | ForEach-Object { $_.Trim() } | Where-Object { $_ }) } else { @() }
if ($Mode -eq 'Capture' -and -not $Recipes.Count) { throw '-Mode Capture needs -RecipeList' }
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'wpa'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o'); mode = $Mode; character = $Character }
function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }

$hub = $null; $bak = $null; $proc = $null; $runLock = $false
try {
    while (!$hub) {
        try { $hub = New-Item -ItemType File -Path $HubLock -Value "$Lane $((Get-Date).ToString('o'))" -ErrorAction Stop }
        catch { "hub lock held by '$(Get-Content $HubLock -Raw -ErrorAction SilentlyContinue)'; retry in 60 s"; Start-Sleep 60 }
    }
    # Another lane may drive the game without the hub lock: wait for its game to exit (never touch it), up to 90 min.
    for ($i = 0; (Get-GameProcess) -and $i -lt 540; $i++) { if ($i % 30 -eq 0) { 'Borderlands2 is running (not ours); waiting' }; Start-Sleep 10 }
    if (Get-GameProcess) { throw 'Borderlands2 still running after 90 min; not touching it' }
    Enter-RunLock 600; $runLock = $true
    $bak = Backup-Saves; $log.backup = $bak; "backup $bak"
    Install-Driver
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $proc = Start-Game @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', "-Character=$Character")
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'wpa/boot.png' | Out-Null; throw 'driver did not answer' }
    $log.block_saves = Py 'print(block_saves())'
    if ($Mode -eq 'Census') {
        # The definitions are loaded with Startup at the title screen; no need to enter the world.
        Start-Sleep 10
        $log.census = Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/weapon_visual_census.py') wpa 300
        "census: $($log.census)"
    } else {
        Start-Sleep 5; Focus-Game | Out-Null; Send-Key Enter; Start-Sleep 45
        $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
        "map: $map"; $log.map = $map
        if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'wpa/load_failed.png' | Out-Null; throw "no player pawn: $map" }
        Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 3
        Focus-Game | Out-Null; Start-Sleep 2
        $rect = Get-GameRect
        if ($rect.Width -ne 1280 -or $rect.Height -ne 720) { throw "client area is $($rect.Width)x$($rect.Height), not 1280x720" }
        $list = ($Recipes | ForEach-Object { "r'" + ((Resolve-Path $_).Path -replace '\\', '/') + "'" }) -join ', '
        Invoke-GamePy ("RG_OUT = r'$(($Out) -replace '\\', '/')'`nWPC_RECIPES = [$list]`n" + [IO.File]::ReadAllText((Join-Path $PSScriptRoot 'scripts/weapon_part_capture.py'))) 120 | Out-Null
        # A one-time "Unlocked!" notice appears the first time the inventory opens and swallows keys: dismiss it first.
        Send-Key I; Start-Sleep 3
        Save-Shot 'wpa/notice_before.png' | Out-Null
        # The notices are a queue (DLC unlocks, Premiere Club): closing one shows the next.
        $log.dialogs = @()
        for ($n = 0; $n -lt 15; $n++) {
            $d = Py 'print(wpc_close_dialogs())' 30 | ConvertFrom-Json -AsHashtable
            $log.dialogs += , $d.closed
            if (-not $d.closed.Count) { break }
            Start-Sleep 2
        }
        "dialogs closed: $(($log.dialogs | ForEach-Object { $_ }) -join ', ')"
        Save-Shot 'wpa/notice_after.png' | Out-Null
        Send-Key Esc; Start-Sleep 2
        Save-Shot 'wpa/after_notice.png' | Out-Null
        if ($Inspect) { $log.cleared = Py 'print(wpc_clear_backpack())' 60; "backpack cleared in memory: $($log.cleared)" }
        $log.guns = @()
        foreach ($recipe in $Recipes) {
            $id = [IO.Path]::GetFileNameWithoutExtension($recipe)
            $r = Py "print(wpc_spawn('$id'))" 60 | ConvertFrom-Json -AsHashtable
            if (-not $r.ok) { "$id : $($r.error)"; $log.guns += @{ id = $id; error = $r.error }; continue }
            $slot = [string]$r.slot
            $log.hold = Py "print(wpc_hold('$id'))" 30
            Focus-Game | Out-Null
            for ($w = 0; $w -lt 30; $w++) {  # put-away + draw take a few seconds: wait until this gun is in hand
                Start-Sleep -Milliseconds 500
                if ((Py "print(wpc_holding('$id'))" 15 | ConvertFrom-Json).held) { break }
                if ($w % 4 -eq 3) { Py "print(wpc_hold('$id'))" 30 | Out-Null }  # the first request after a spawn may not take
            }
            Start-Sleep 2  # let the draw animation settle before the frame
            # The earlier spawned gun is out of hand now: remove it (chain or backpack), so the backpack holds only this gun.
            $log.prune = Py "print(wpc_prune('$id'))" 30
            "$id prune: $($log.prune)"
            $c = Py "print(wpc_check('$id'))" 60 | ConvertFrom-Json -AsHashtable
            if (-not $c.ok) { Py "print(wpc_hold('$id'))" 30 | Out-Null; Start-Sleep 3; $c = Py "print(wpc_check('$id'))" 60 | ConvertFrom-Json -AsHashtable }
            "$id held=$($c.ok) slot=$($c.slot) triangles=$($c.triangles)"
            Save-Shot "wpa/fp_$id.png" | Out-Null
            if ($Inspect) {
                # Inspect: F on the held gun's equipped cell, then F closes it again.
                # Open, close and reopen: on the first open the backpack list can still show the gun removed just before.
                Send-Key I; Start-Sleep 2; Send-Key Esc; Start-Sleep 1; Send-Key I; Start-Sleep 2
                if ($env:WPA_DEBUG_KEYS) { Save-Shot "wpa/dbg_${id}_1open.png" | Out-Null }
                # The inventory opens on equipped slot 1 (seen 2026-10-23); Right goes to the backpack, whose only weapon is this gun.
                Send-Key Right; Start-Sleep 1; Send-Key Left; Start-Sleep 1; Send-Key Right; Start-Sleep 1
                if ($env:WPA_DEBUG_KEYS) { Save-Shot "wpa/dbg_${id}_2left.png" | Out-Null }
                Send-Key F; Start-Sleep 2
                Save-Shot "wpa/insp_$id.png" | Out-Null
                Send-Key F; Start-Sleep 1; Send-Key Esc; Start-Sleep 2
            }
            $log.guns += @{ id = $id; live = $r; check = $c }
        }
        Send-Key 1; Start-Sleep 2  # hold slot 1 so cleanup never removes the gun in hand
        $log.cleanup = Py 'print(wpc_cleanup())' 60
        "cleanup: $($log.cleanup)"
    }
    try { Invoke-GamePy 'get_pc().ConsoleCommand("quit", False)' 10 | Out-Null } catch { "quit sent (no reply)" }
    for ($i = 0; $i -lt 30 -and (Get-GameProcess); $i++) { Start-Sleep 2 }
    $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
    if ($p -and $p.ProcessName -eq 'Borderlands2') { Stop-Process -Id $proc.Id -Force; Start-Sleep 3 }
    $proc = $null
} finally {
    $log.finished = (Get-Date).ToString('o')
    $log | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $Out "session_$($Mode.ToLower()).json")
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
        @{ backup = $bak; compared = (Get-Date).ToString('o'); changes = @($rows) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Out "save_compare_$($Mode.ToLower()).json")
        "save compare: $(@($rows).Count) differences"; $rows
    }
}
