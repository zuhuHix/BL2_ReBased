#Requires -Version 7.0
# HUD parity ground truth (a Maya save in Sanctuary, 1280x720 windowed): reads the player's HUD movie, equipped weapon,
# ammo and vitals in the running game (scripts/hud_parity.py). Cycles the quick-select keys 1-4 and records each weapon
# (weapon_<k>.png), returns to the starting weapon, samples the crosshair while moving and while firing, fires three
# shots at nothing and reads the same values again (after_fire.png). Takes the screen and keyboard.
# Everything written is game data under ignored local/realgame/hud_parity/ (values.json, session.json, save_compare.json,
# PNG screenshots). Locks: the shared hub lock (waits while another lane holds it) and the run lock; saves are backed up,
# saving is blocked in game, and afterwards the save folder is compared with the backup (changed files are restored).
param(
    [string]$HubLock = 'C:\Users\yorad\Documents\BL2_ReBased\local\hub-ue.lock',
    [string]$Lane = 'hudparity',
    # quick_startup's -Character slot: the user's own Maya save file name, loaded into Sanctuary.
    [Parameter(Mandatory=$true)][string]$Character
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'hud_parity'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o') }
function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }
function Snap([string]$Label) { Py "print(hp_out(hp_snapshot('$Label')))" 90 | ConvertFrom-Json -AsHashtable }
function Quick([string]$Label) { Py "print(hp_out(hp_quick('$Label')))" 20 | ConvertFrom-Json -AsHashtable }

# One row per snapshot: the weapon's type and the crosshair/text values the HUD shows for it.
function Row([string]$Name, $s) {
    $w = $s.weapon; $h = $s.hud
    [pscustomobject]@{
        snapshot = $Name
        weapon = if ($w) { $w.path } else { $null }
        weapon_class = if ($w) { $w.class } else { $null }
        weapon_type = if ($w -and $w.definition) { $w.definition['WeaponTypeDefinition'] } else { $null }
        ammo = if ($w) { $w.fields } else { $null }
        crosshair_outer = $h['p1.crosshairs.outer._currentframe']
        crosshair_spread = $h['p1.crosshairs.outer.weapon._currentframe']
        crosshair_up = $h['p1.crosshairs.outer.weapon.up._currentframe']
        crosshair_visible = $h['p1.crosshairs._visible']
        frames = [ordered]@{ health = $h['p1.health._currentframe']; shield = $h['p1.shield._currentframe']
            bullets = $h['p1.bullets._currentframe']; grenades = $h['p1.grenades._currentframe']
            xpbar = $h['p1.xpbar._currentframe']; bullet_icon = $h['p1.bullets.bullet_icon._currentframe']
            shield_icon = $h['p1.shield.icon._currentframe']; health_icon = $h['p1.health.health_icon._currentframe'] }
        texts = [ordered]@{ bullets = $h['p1.bullets.bullet_c.text']; grenades = $h['p1.grenades.grenadesDisp.text']
            health = $h['p1.health.health_disp.text']; shield = $h['p1.shield.shield_disp.text']
            level = $h['p1.levelClassMod.level.text']; comm = $h['p1.levelClassMod.comm.text'] }
        pools = @($s.pools | ForEach-Object { [pscustomobject]@{ definition = $_.definition; numbers = $_.numbers; definition_numbers = $_.definition_numbers } })
    }
}

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
    Install-Driver
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $proc = Start-Game @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', "-Character=$Character", $marker); $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'hud_parity/boot.png' | Out-Null; throw 'driver did not answer' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    Start-Sleep 5; Assert-GameForeground; Send-Key Enter; Start-Sleep 45
    $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
    "map: $map"; $log.map = $map
    if ($map -notmatch 'Sanctuary' -or $map -notmatch 'WillowPlayerPawn') { Save-Shot 'hud_parity/load_failed.png' | Out-Null; throw "not Sanctuary with a player pawn: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 3
    Assert-GameForeground; Start-Sleep 2
    $rect = Get-GameRect; $log.client = @{ x = $rect.X; y = $rect.Y; width = $rect.Width; height = $rect.Height }
    if ($rect.Width -ne 1280 -or $rect.Height -ne 720) { throw "client area is $($rect.Width)x$($rect.Height), not 1280x720" }
    Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/hud_parity.py') hud_parity | Out-Null
    Start-Sleep 3

    # Current weapon first, then quick-select keys 1-4 (1.5 s each), then back to the current weapon.
    $snaps = [ordered]@{}
    Save-Shot 'hud_parity/start.png' | Out-Null
    $snaps.start = Snap 'start'
    foreach ($k in 1..4) {
        Send-Key ([string]$k); Start-Sleep -Milliseconds 1500
        $snaps["slot$k"] = Snap "slot$k"
        Save-Shot "hud_parity/weapon_$k.png" | Out-Null
    }
    $startPath = if ($snaps.start.weapon) { $snaps.start.weapon.path } else { $null }
    $startSlot = $null
    if ($startPath) { foreach ($k in 1..4) { if ($snaps["slot$k"].weapon -and $snaps["slot$k"].weapon.path -eq $startPath) { $startSlot = $k } } }
    $log.start_slot = $startSlot
    if ($startSlot) { Send-Key ([string]$startSlot); Start-Sleep -Milliseconds 1500 }
    else { "starting weapon is not in quick slots 1-4; firing with the weapon in hand" }

    # Crosshair while moving (hold W for about 0.6 s), then at rest, then while firing three shots at nothing.
    Assert-GameForeground
    $movement = @()
    Send-KeyDown 'W'
    try { for ($i = 0; $i -lt 6; $i++) { $movement += Quick "move$i"; Start-Sleep -Milliseconds 100 } } finally { Send-KeyUp 'W' }
    Start-Sleep -Seconds 1
    $idle = @(Quick 'idle')
    $snaps.pre_fire = Snap 'pre_fire'
    $firing = @()
    for ($shot = 1; $shot -le 3; $shot++) {
        Send-Click
        for ($i = 0; $i -lt 4; $i++) { $firing += Quick "fire$shot.$i"; Start-Sleep -Milliseconds 100 }
        Start-Sleep -Milliseconds 400
    }
    Start-Sleep -Seconds 1
    $snaps.after_fire = Snap 'after_fire'
    Save-Shot 'hud_parity/after_fire.png' | Out-Null

    $summary = @()
    foreach ($name in 'start', 'slot1', 'slot2', 'slot3', 'slot4', 'pre_fire', 'after_fire') { $summary += Row $name $snaps[$name] }
    $values = [ordered]@{ started = $log.started; map = $map; client = $log.client; start_slot = $startSlot
        summary = $summary; snapshots = $snaps; movement = $movement; idle = $idle; firing = $firing }
    $values | ConvertTo-Json -Depth 30 | Set-Content (Join-Path $Out 'values.json')
    "wrote values.json: $(@($summary).Count) summary rows, $(@($movement).Count + @($firing).Count) quick samples"
    $summary | Select-Object snapshot, weapon_type, crosshair_outer, crosshair_spread, crosshair_up, @{n = 'bullets'; e = { $_.texts.bullets } }, @{n = 'grenades'; e = { $_.texts.grenades } }, @{n = 'health'; e = { $_.texts.health } }, @{n = 'shield'; e = { $_.texts.shield } }, @{n = 'level'; e = { $_.texts.level } } | Format-Table -AutoSize | Out-String -Width 220

    try { Invoke-GamePy 'get_pc().ConsoleCommand("quit", False)' 10 | Out-Null } catch { "quit sent (no reply)" }
    for ($i = 0; $i -lt 30 -and (Get-GameProcess); $i++) { Start-Sleep 2 }
    $stopped = Stop-OwnGame $proc $procStart $marker; "game: $stopped"
    if ($stopped -like 'refused*') { throw "the game was not stopped: $stopped" }
    if ($stopped -eq 'stopped') { Start-Sleep 3 }
    $proc = $null
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
