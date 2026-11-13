#Requires -Version 7.0
# HUD integrity ground truth (a Maya save in Sanctuary, 1280x720 windowed): which HUD movie clips the game drives for the
# slice loop (scripts/hud_integrity.py, a per-frame watcher of the movie's clip frames, visibility and texts). Steps: at
# rest; in front of an NPC (the interaction prompt); the game's own down state started by WillowPlayerPawn.PlayInjured
# (no damage: Sanctuary has no enemies), left to bleed out, die and respawn, with the respawn fee; a second down state
# ended by the game's kill-while-down path (second wind). Screenshots at fixed moments. Takes the screen and keyboard.
# Everything written is game data under ignored local/realgame/hud_integrity/. Locks, save backup, blocked saves and the
# save comparison as in hud_parity_capture.ps1.
param(
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'hudintegrity',
    # quick_startup's -Character slot: the user's own Maya save file name, loaded into Sanctuary.
    [Parameter(Mandatory=$true)][string]$Character,
    # The NPC to stand in front of (a part of its AI class definition path).
    [string]$Npc = 'Marcus'
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'hud_integrity'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o'); steps = @() }
$census = @()
function Py([string]$Code, [int]$Timeout = 30) { ConvertTo-ReplyText (Invoke-GamePy $Code $Timeout) $Code }   # an empty reply is refused (realgame.ps1)
# One labelled read of the player and every watched HUD value, with a screenshot.
function Census([string]$Label) {
    $c = Py "print(hi_out({'pawn': hi_pawn(), 'hud': hi_census()}))" 30 | ConvertFrom-Json -AsHashtable
    $c.label = $Label; $c.qpc = Get-QpcSeconds; $script:census += , $c
    Save-Shot "hud_integrity/$Label.png" | Out-Null
    "$Label : state=$($c.pawn.state) health=$($c.pawn.Health) dying1=$($c.hud['p1.dying1._currentframe']) bar=$($c.hud['p1.dying1.bar._currentframe']) secondwind=$($c.hud['p1.secondwind._currentframe'])"
}
function Step([string]$Name, $Value) { $script:log.steps += , [ordered]@{ step = $Name; qpc = Get-QpcSeconds; value = $Value }; "$Name -> $Value" }
function WaitUntil([double]$Qpc) { $w = $Qpc - (Get-QpcSeconds); if ($w -gt 0) { Start-Sleep -Milliseconds ([int]($w * 1000)) } }

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
    Install-Driver -BlockSavesAtStart -Exclusive
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $proc = Start-Game @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', "-Character=$Character", $marker); $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'hud_integrity/boot.png' | Out-Null; throw 'driver did not answer' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    Start-Sleep 5; Assert-GameForeground; Send-Key Enter; Start-Sleep 45
    $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
    "map: $map"; $log.map = $map
    if ($map -notmatch 'Sanctuary' -or $map -notmatch 'WillowPlayerPawn') { Save-Shot 'hud_integrity/load_failed.png' | Out-Null; throw "not Sanctuary with a player pawn: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 3
    Assert-GameForeground; Start-Sleep 2
    $rect = Get-GameRect; $log.client = @{ x = $rect.X; y = $rect.Y; width = $rect.Width; height = $rect.Height }
    if ($rect.Width -ne 1280 -or $rect.Height -ne 720) { throw "client area is $($rect.Width)x$($rect.Height), not 1280x720" }
    Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/hud_integrity.py') hud_integrity | Out-Null
    Step 'watcher' (Py 'print(hi_start())')
    Start-Sleep 3
    Census 'rest'

    Step 'face' (Py "print(hi_out(hi_face('$Npc')))")
    Start-Sleep 2
    Census 'npc_prompt'
    Step 'face_away' (Py "pc=get_pc(); r=pc.Rotation; pc.SetRotation(unrealsdk.make_struct('Rotator', Pitch=0, Yaw=r.Yaw+32768, Roll=0)); print('ok')")
    Start-Sleep 2

    # Down state 1: left to bleed out, die and respawn, then the fee (live: dead state 12 s after the targeted state,
    # death camera 5.76 s, the fee about 11.8 s after the respawn).
    Step 'injure1' (Py 'print(hi_out(hi_injure()))')
    $t1 = Get-QpcSeconds
    foreach ($at in 0.5, 1.5, 3, 6, 9, 12, 14, 16, 19, 22, 26, 30, 36, 42) { WaitUntil ($t1 + $at); Census ('down1_{0:00.0}' -f $at) }
    Start-Sleep 5
    # Down state 2: ended 5 s in by the game's kill-while-down path (second wind).
    Step 'injure2' (Py 'print(hi_out(hi_injure()))')
    $t2 = Get-QpcSeconds
    foreach ($at in 1, 3, 5) { WaitUntil ($t2 + $at); Census ('down2_{0:0.0}' -f $at) }
    Step 'second_wind' (Py 'print(hi_out(hi_second_wind()))')
    $t3 = Get-QpcSeconds
    foreach ($at in 0.3, 0.8, 1.5, 2.5, 4, 6) { WaitUntil ($t3 + $at); Census ('wind_{0:0.0}' -f $at) }
    Start-Sleep 3
    Census 'after'
    Step 'stop' (Py 'print(hi_stop())')
    $rowsFile = (Join-Path $Out 'rows.json') -replace '\', '/'
    Step 'dump' (Py "print(hi_dump(r'$rowsFile'))" 60)
    @{ census = $census; steps = $log.steps } | ConvertTo-Json -Depth 12 | Set-Content (Join-Path $Out 'census.json')

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
