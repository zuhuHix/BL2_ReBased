#Requires -Version 7.0
# Generator confirmation in the running game (1280x720 windowed, one save loaded):
#  1. spawns the 40 sample guns of local/realgame/gen_confirm/sample.json (tools/real_game/gen_confirm.py plan) with
#     exactly their generated parts, names, manufacturer and stage (scripts/gen_confirm_live.py), and writes live.json;
#  2. reads each gun's evaluated attributes (InstantHitDamage, FireInterval, ...) from the weapon once it is readied in
#     a quick slot, with no card UI (stats.json); -WalkCards also walks the inventory screen for the printed cards;
#  3. rolls two common balances (a pistol and an SMG) at stage 30, 400 times each, records every roll's parts and
#     names and destroys it (frequency_rolls.json).
# Everything written is game data under ignored local/realgame/gen_confirm/. Locks: waits (up to -WaitMinutes) while
# another Borderlands2.exe runs or the hub lock is held, then takes the hub lock and the run lock. Saves are backed up
# and saving is blocked in game. Quits only the game this script started (by PID, after checking its command line).
param(
    # Optional save slot (the quick_startup mod's -Character value). Without it the title screen's first option
    # (Continue, the most recent save) is used; the loaded character is recorded in session.json.
    [string]$Character = '',
    [string]$HubLock = 'C:\Users\yorad\Documents\BL2_ReBased\local\hub-ue.lock',
    [string]$Lane = 'genconfirm',
    [int]$WaitMinutes = 30,
    [int]$FrequencyRolls = 400,
    [int]$FrequencyBatch = 50,
    [int]$MaxCardSteps = 150,
    # The card-UI walk (round 1) never opened the card; it stays off unless asked for.
    [switch]$WalkCards,
    # The frequency rolls (round 1, 800 rolls) are not repeated when this is set.
    [switch]$SkipFrequency
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'gen_confirm'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o') }
$log.notes = @()
function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }

$sample = Get-Content (Join-Path $Out 'sample.json') -Raw | ConvertFrom-Json
# Frequency subjects: the first common pistol and the first common SMG of the sample (the balance and its type).
$freqSubjects = @(
    $sample | Where-Object { $_.type_short -eq 'Pistol' -and $_.rarity -eq 'Common' } | Select-Object -First 1
    $sample | Where-Object { $_.type_short -eq 'SMG' -and $_.rarity -eq 'Common' } | Select-Object -First 1
)
$log.frequency_subjects = @($freqSubjects | ForEach-Object { [pscustomobject]@{ balance = $_.balance; type = $_.type_short } })

$hub = $null; $bak = $null; $proc = $null; $runLock = $false; $installed = $false
try {
    # 1. Wait for the game to be free (never touch one that is running) and for the hub lock.
    $deadline = (Get-Date).AddMinutes($WaitMinutes)
    while (($null -ne (Get-GameProcess)) -or (Test-Path $HubLock) -or (Get-Process -Name 'UnrealEditor*' -ErrorAction SilentlyContinue)) {
        if ((Get-Date) -gt $deadline) { throw "blocked after $WaitMinutes min: game running=$([bool](Get-GameProcess)) hub lock='$(Get-Content $HubLock -Raw -ErrorAction SilentlyContinue)'" }
        "waiting: game running=$([bool](Get-GameProcess)), hub lock=$(Test-Path $HubLock); next check in 60 s"; Start-Sleep 60
    }
    while (!$hub) {
        try { $hub = New-Item -ItemType File -Path $HubLock -Value "$Lane $((Get-Date).ToString('o'))" -ErrorAction Stop }
        catch { "hub lock taken; retry in 60 s"; Start-Sleep 60 }
    }
    Enter-RunLock 600; $runLock = $true
    # A fresh run: the frequency file appends to its groups, so a previous run's rolls are removed first. Only after
    # the locks are held, so a run that is blocked never deletes another run's output.
    Remove-Item (Join-Path $Out 'frequency_rolls.json') -ErrorAction SilentlyContinue

    # 2. Back up saves, install the command channel, start the game with the save.
    $bak = Backup-Saves; $log.backup = $bak; "backup $bak"
    Install-Driver; $installed = $true
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $gameArgs = @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies')
    if ($Character) { $gameArgs += "-Character=$Character" }
    $proc = Start-Game $gameArgs
    $log.game_pid = $proc.Id
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'gen_confirm/boot.png' | Out-Null; throw 'driver did not answer' }
    $log.block_saves = Py 'print(block_saves())'
    # The title menu opens with Continue highlighted; a dialog may be up first (its OK takes the first Enter). Press
    # Enter and poll for a player pawn, up to three Enter presses.
    Start-Sleep 5; Focus-Game | Out-Null
    $pawn = ''
    for ($try = 0; $try -lt 3 -and $pawn -notmatch 'WillowPlayerPawn'; $try++) {
        Send-Key Enter
        for ($i = 0; $i -lt 20 -and $pawn -notmatch 'WillowPlayerPawn'; $i++) {
            Start-Sleep 3
            $pawn = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
        }
    }
    "map: $pawn"; $log.map = $pawn
    if ($pawn -notmatch 'WillowPlayerPawn') { Save-Shot 'gen_confirm/load_failed.png' | Out-Null; throw "no player pawn: $pawn" }
    Start-Sleep 3
    Focus-Game | Out-Null; Start-Sleep 2
    $rect = Get-GameRect; $log.client = @{ width = $rect.Width; height = $rect.Height }
    if ($rect.Width -ne 1280 -or $rect.Height -ne 720) { throw "client area is $($rect.Width)x$($rect.Height), not 1280x720" }

    # 3. Helpers into the shared namespace, then spawn the sample with exact parts.
    Invoke-GamePyFile (Join-Path $script:Repo 'tools/real_game/scripts/weapon_cards.py') gen_confirm | Out-Null
    Invoke-GamePyFile (Join-Path $script:Repo 'tools/real_game/scripts/gen_confirm_live.py') gen_confirm | Out-Null
    $spawned = Py 'print(gc_spawn_sample())' 180
    $log.spawned = $spawned
    "spawned sample guns: $spawned"

    # 3b. Evaluated stats: each gun is readied in one free quick slot, read, and the slot's original weapon restored.
    $log.stats_rows = Py 'print(gc_read_stats())' 300
    "read stats for $($log.stats_rows) guns"

    # 4. Cards (only with -WalkCards): the card trace is on while the inventory is open. The cursor steps right through the backpack; each
    #    step that makes the game print a new card (SetItemCardEx) is recorded with its item. Stops after 12 steps
    #    without a new card, or at -MaxCardSteps.
    $cardItems = [ordered]@{}; $k = 0
    if ($WalkCards) {
        Py "print(card_trace_start('cardtrace.jsonl'))" 20 | Out-Null
        $log.card_hook = Py 'print(gc_hook_card())' 20
        # The player's status menu (the inventory screen) is opened by the game's own ShowStatusMenu. Key I opens the pause
        # menu in this build (checked 2026-10-23), so the key is not used.
        Focus-Game | Out-Null
        $log.status_menu = Py 'get_pc().ShowStatusMenu(); print("opened")' 20
        Start-Sleep 3
        $stale = 0
        for ($k = 0; $k -lt $MaxCardSteps -and $stale -lt 12; $k++) {
            Send-Key 'Right'; Start-Sleep -Milliseconds 300
            $item = Py 'print(gc_last_card())' 10
            if ($item -and -not $cardItems.Contains($item)) { $cardItems[$item] = $k; $stale = 0 } else { $stale++ }
        }
        Save-Shot 'gen_confirm/status_menu_walk.png' | Out-Null
        Send-Key 'Esc'; Start-Sleep 1
        Py 'print(card_trace_stop())' 20 | Out-Null
    }
    $log.card_steps = $k
    $log.cards_seen = $cardItems.Count
    "cards printed for $($cardItems.Count) items in $k cursor steps"

    # 5. Frequency rolls: two common balances at stage 30, in batches; each roll is recorded and destroyed.
    foreach ($subject in $(if ($SkipFrequency) { @() } else { $freqSubjects })) {
        $done = 0
        while ($done -lt $FrequencyRolls) {
            $n = [Math]::Min($FrequencyBatch, $FrequencyRolls - $done)
            $done += [int](Py "print(gc_freq_batch('$($subject.balance)', 30, $n))" 120)
        }
        "rolled $done from $($subject.balance)"
    }
    $log.frequency_rolls = $FrequencyRolls

    $log.finished = (Get-Date).ToString('o')
    $log | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $Out 'session.json')
}
catch {
    $log.error = $_.Exception.Message
    "ERROR: $($_.Exception.Message)"
}
finally {
    # Only what this run started: the quit command, the driver and the game are touched only when we started them
    # (another lane's game and its installed driver are never reached from here).
    if ($proc) {
        try { Invoke-GamePy 'get_pc().ConsoleCommand("quit", False)' 10 | Out-Null } catch { "quit sent (no reply)" }
        for ($i = 0; $i -lt 30 -and -not $proc.HasExited; $i++) { Start-Sleep 2 }
        # Stop only the process this script started, after checking its command line.
        $cmd = (Get-CimInstance Win32_Process -Filter "ProcessId=$($proc.Id)" -ErrorAction SilentlyContinue).CommandLine
        if (-not $proc.HasExited -and $cmd -match 'Borderlands2\.exe' -and $cmd -match '-windowed') { Stop-Process -Id $proc.Id -Force; "stopped our game pid $($proc.Id)" }
    }
    if ($installed) { Remove-Driver }
    if ($runLock) { Exit-RunLock }
    if ($hub) { Remove-Item $HubLock -Force -ErrorAction SilentlyContinue }
    if ($bak) {
        # Compare the save folder with the backup by relative path and hash; differences are listed, not restored.
        $live = Join-Path $env:USERPROFILE 'Documents/My Games/Borderlands 2/WillowGame/SaveData'
        $hashes = { param($root) Get-ChildItem $root -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object { [pscustomobject]@{ rel = $_.FullName.Substring($root.Length); hash = (Get-FileHash $_.FullName).Hash } } }
        $diff = Compare-Object (& $hashes (Join-Path $bak 'SaveData')) (& $hashes $live) -Property rel, hash
        $log.save_compare = @($diff | ForEach-Object { "$($_.SideIndicator) $($_.rel)" })
        "save folder vs backup: $(if ($diff) { "$(@($diff).Count) differences (listed in session.json)" } else { 'identical' })"
    }
    $log | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $Out 'session.json')
}
