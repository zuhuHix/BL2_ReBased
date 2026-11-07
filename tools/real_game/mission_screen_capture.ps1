# Marcus's mission screen and the respawn message in the real game, with Better UI Controls off (the maintainer's decision, for one read of E and WASD as stock; the setting is put back byte for byte)
# (mission UI lane): one session in Sanctuary with the level-8 Maya. It answers what the
# host's page must do: which keys the screen takes, what closes it, whether the world runs behind it, what the reward page shows, and what the
# respawn dialog does. Rows from scripts/mission_screen_probe.py and named frames, under ignored local/realgame/mission_screen/.
# Takes the screen and keyboard. Locks: the shared hub lock and the run lock; saves are backed up, saving is blocked in game from the first frame, and
# afterwards all four roots are compared with the backup (changed files are restored from it).
param(
    # The machine-wide hub lock lives in the main checkout's local/ (shared by every worktree on this machine).
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'mission-screen',
    [string]$Character = 'Save0008.sav',   # quick_startup's -Character slot: the level-8 Maya in Sanctuary
    [switch]$SkipRespawn,
    # Give up waiting for the hub lock after this long and report its contents; a stale lock is removed by its owner or the
    # maintainer, never by this script.
    [int]$HubWaitMinutes = 120
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'mission_screen'
New-Item -ItemType Directory -Force $Out | Out-Null
# Stale evidence: a final write that fails must not leave an earlier run's completed behind. Kept under a new name, not deleted: after a run that
# needs recovery, its record is what names the backup and says what went wrong, and a rejected retry must not lose that.
$keptAs = "$(Get-Date -Format 'yyyyMMdd-HHmmss')-$PID"
foreach ($stale in 'session', 'save_compare') { $file = Join-Path $Out "$stale.json"; if (Test-Path -LiteralPath $file) { Move-Item -LiteralPath $file (Join-Path $Out "$stale.$keptAs.json") } }
# End of stale evidence.
$log = [ordered]@{ started = (Get-Date).ToString('o') }

# Mod setting start. mods_base enables a mod at load only if its settings file says "enabled": true; that one value is all this run changes.
# The bytes with only that value set to false. Refuses a file it cannot change that way: then the mod would stay on and the run would read nothing new.
function Get-ModOffBytes([byte[]]$Prior) {
    $text = [Text.UTF8Encoding]::new($false, $true).GetString($Prior)
    if ((ConvertFrom-Json $text).enabled -ne $true) { throw 'the mod setting does not say enabled: true' }
    $found = [regex]::Matches($text, '"enabled"\s*:\s*true')
    if ($found.Count -ne 1) { throw "the mod setting has $($found.Count) enabled values" }
    $off = $text.Substring(0, $found[0].Index) + ($found[0].Value -replace 'true$', 'false') + $text.Substring($found[0].Index + $found[0].Length)
    [Text.UTF8Encoding]::new($false).GetBytes($off)
}
function Test-SameBytes([byte[]]$A, [byte[]]$B) { [Convert]::ToBase64String($A) -ceq [Convert]::ToBase64String($B) }
# Puts the prior bytes back if the file differs, and verifies it. Returns what it did; throws if the file does not end up identical.
function Restore-ModSetting([string]$File, [byte[]]$Prior) {
    if ((Test-Path -LiteralPath $File) -and (Test-SameBytes ([IO.File]::ReadAllBytes($File)) $Prior)) { return 'identical' }
    [IO.File]::WriteAllBytes($File, $Prior)
    if (!(Test-SameBytes ([IO.File]::ReadAllBytes($File)) $Prior)) { throw "$File still differs after writing the prior bytes" }
    'restored, identical'
}
# A durable record, written before the setting is, so that a run killed before its finally leaves the way back. Outside the install and any worktree, next to the machine's run lock.
function Get-ModPendingFile { Join-Path $env:LOCALAPPDATA 'OpenWillow\mission_screen_mod_pending.json' }
function Save-ModPending([string]$File, [byte[]]$Prior, [string]$Backup, [string]$Lane, [string]$HubLock, [string]$Realgame) {
    $pending = Get-ModPendingFile
    New-Item -ItemType Directory -Force (Split-Path -Parent $pending) | Out-Null
    [ordered]@{ file = $File; prior_base64 = [Convert]::ToBase64String($Prior); backup = $Backup; lane = $Lane; hub_lock = $HubLock; run_lock = $script:Lock; driver = $script:DriverDir; game = $env:OPENWILLOW_BL2; realgame = $Realgame; written = (Get-Date).ToString('o') } | ConvertTo-Json | Set-Content -LiteralPath $pending -Encoding UTF8
    if ((Get-Content -Raw -LiteralPath $pending | ConvertFrom-Json).prior_base64 -cne [Convert]::ToBase64String($Prior)) { throw "the pending record $pending did not read back" }
}
function Format-PsQuoted([string]$Text) { "'" + $Text.Replace("'", "''") + "'" }
# Refuses while a record is there, and prints the whole way back. It restores nothing itself: the setting or the saves may have been changed on purpose since. The lines with four spaces are commands, and a test runs them.
function Assert-NoModPending {
    $pending = Get-ModPendingFile
    if (!(Test-Path -LiteralPath $pending)) { return }
    $text = Get-Content -Raw -LiteralPath $pending
    $record = try { $text | ConvertFrom-Json } catch { $null }
    $nl = [Environment]::NewLine
    if (!$record -or !$record.file) { throw "A mission screen run left the record $pending and it cannot be read, so no recovery can be printed. This run changed nothing. Its contents:$nl$text" }
    $hub = Format-PsQuoted $record.hub_lock; $lock = Format-PsQuoted $record.run_lock; $driver = Format-PsQuoted $record.driver; $q = Format-PsQuoted $pending
    throw (@(
        "A mission screen run changed $($record.file) and did not finish putting it back: it was killed, or it held for recovery. This run changed nothing. If that run is still going, wait for it and do nothing here. Record: $pending",
        "Recovery, in this order. 1. Make sure Borderlands 2 is closed. The record is from $($record.written): the restore below takes the saves, config and SDK settings back to the backup, so if anything was played or changed on purpose since, compare first.",
        "2. In a new PowerShell window, one line at a time. Stop at any problem: every restore row must read 'added, removed', 'missing, restored' or 'changed, restored'.",
        "    `$env:OPENWILLOW_BL2 = $(Format-PsQuoted $record.game); . $(Format-PsQuoted $record.realgame)",
        "    if (Test-Path -LiteralPath $driver) { Remove-Item -Recurse -Force -LiteralPath $driver }   # first check it is this driver's copy, not someone's",
        "    Restore-VerifiedBackup $(Format-PsQuoted $record.backup)",
        "    [IO.File]::WriteAllBytes($(Format-PsQuoted $record.file), [Convert]::FromBase64String($(Format-PsQuoted $record.prior_base64)))   # the setting's prior bytes",
        "    if ((Get-Content -Raw -LiteralPath $hub -ErrorAction SilentlyContinue) -like $(Format-PsQuoted "$($record.lane) *")) { Remove-Item -Force -LiteralPath $hub }   # the hub lock, only if it names that run's lane",
        "    if (Test-OwUeLockStale $lock) { Remove-Item -Force -LiteralPath $lock }   # the run lock, only if its owner is gone",
        "    Remove-Item -LiteralPath $q   # last: this record",
        "3. Run again. A lock still there after this is not that run's: its owner or the maintainer removes it."
    ) -join $nl)
}
# Mod setting end.
# The game whose command line holds this run's whole marker, or nothing. Reading only.
function Find-GameByMarker([string]$Marker) {
    $found = @(Get-CimInstance Win32_Process -Filter "Name='Borderlands2.exe'" -ErrorAction Stop | Where-Object { $_.CommandLine -match "(^|\s)$([regex]::Escape($Marker))(\s|$)" })
    if ($found.Count) { Get-Process -Id $found[0].ProcessId -ErrorAction Stop }
}
function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }
function PyJson([string]$Expr, [int]$Timeout = 30) { Py "import json; print(json.dumps($Expr, default=str))" $Timeout | ConvertFrom-Json }
function Shot([string]$Name) { Save-Shot "mission_screen/$Name.png" | Out-Null }
# One read and one frame after a key: what the key changed, and how it looks.
function Read-After([string]$Label, [double]$Wait = 0.6) {
    Start-Sleep -Milliseconds ([int]($Wait * 1000))
    $row = PyJson "ms_read('$Label')"; Shot $Label; $log["read_$Label"] = $row; $row
}
# Press a key on an open screen (every send refuses without game focus); W is held for a second, Mouse moves the view. Esc goes only to a screen that is open: otherwise it opens the pause menu.
# Returns the one read row, whatever the sends print. The mark proves the key reached the screen's own handler: a key that never arrived would read as "this key does nothing".
function Test-Key([string]$Stage, [string]$Key) {
    $mark = "key_${Stage}_$Key"
    Py "print(ms_mark('$mark'))" | Out-Null
    Assert-GameForeground
    $null = switch ($Key) {
        'W' { Send-KeyDown W; Start-Sleep -Milliseconds 1000; Send-KeyUp W }
        'Mouse' { 1..10 | ForEach-Object { Send-Mouse 30 0; Start-Sleep -Milliseconds 30 } }
        default { Send-Key $Key }
    }
    $row = Read-After "${Stage}_after_$Key"
    if ($Key -ne 'Mouse') { $log["delivered_${Stage}_$Key"] = PyJson "ms_key_seen('$mark')" }
    $row
}
# The key tests on a screen that must stay open for the next step; returns the last read. A key that changes the mission's status ends them (2026-11-07: E accepted the offer, and there
# was nothing left to reopen): that is logged, and the caller carries on from the new status. A key that only closes the screen is logged too, and the screen is opened again.
function Test-Keys([string]$Stage, [string[]]$Keys, [int]$Status) {
    $row = $null
    foreach ($key in $Keys) {
        $row = Test-Key $Stage $key
        if ($row.status -ne $Status) { $log["${Stage}_status_changed_by_$key"] = $row.status; return $row }
        if (!$row.open) {
            $log["${Stage}_closed_by_$key"] = $true
            $row = Open-Screen "${Stage}_reopen_after_$key"
            if (!$row.open) { throw "the screen did not open again after $key closed it ($Stage)" }
        }
    }
    $row
}
# Stand in front of Marcus and press E until his screen opens.
# The screen must be closed first, and must open after this press: a screen already open would read as this E having opened it.
function Open-Screen([string]$Label) {
    $row = $null; $mark = $null
    for ($try = 1; $try -le 4; $try++) {
        # Before the stand, which moves the player. Open here is either the last press, slower than the wait below, which is a valid read, or something this run did not do.
        $base = PyJson "ms_read('${Label}_baseline_$try')"
        if ($base.open -ne $false) {
            if ($mark -and (PyJson "ms_opened('$mark')").opened -eq $true) { $log["${Label}_opened_late"] = $try - 1; return (Read-After "${Label}_late_$($try - 1)") }
            throw "the screen is already open before the E for $Label (open $($base.open))"
        }
        $log["stand_$Label"] = PyJson 'fm_stand_in_front()'
        $mark = "open_${Label}_$try"
        Py "print(ms_mark('$mark'))" | Out-Null
        Start-Sleep -Milliseconds 800; Assert-GameForeground; $null = Send-Key E
        $row = Read-After $Label 1.0
        # Up to 3 s more for this press to open it, before another try moves anything.
        for ($poll = 0; $poll -lt 10; $poll++) {
            $opened = PyJson "ms_opened('$mark')"
            if ($opened.opened -eq $true) { break }
            Start-Sleep -Milliseconds 300
        }
        $log["opened_${Label}_$try"] = $opened
        if ($opened.opened -eq $true) {
            if (!$row.open) { $log["${Label}_opened_late"] = $try; $row = Read-After "${Label}_late_$try" }
            if ($row.open) { return $row }
        }
    }
    $row | Add-Member -Force -NotePropertyName open -NotePropertyValue $false
    $row
}
# Which of these keys never reached the screen's handler. A key that was never pressed, because the page closed or a key changed the status first, counts as not reached.
function Get-KeysNotDelivered([string]$Stage, [string[]]$Keys) {
    @($Keys | Where-Object { $log["delivered_${Stage}_$_"].delivered -ne $true })
}

$extraRoots = [ordered]@{ sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$snapshots = @{}
$roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
foreach ($d in $extraRoots.Keys) { $roots[$d] = $extraRoots[$d] }
$stagesRequested = @('hooks', 'mod_off', 'start', 'offer', 'offer_keys', 'offer_esc', 'accept', 'ready', 'turnin', 'turnin_keys', 'reward', 'reward_keys', 'reward_taken', 'closed') + $(if ($SkipRespawn) { @() } else { @('respawn') }) + @('hook_errors')
$stagesDone = New-Object Collections.Generic.List[string]; $stageProblems = New-Object Collections.Generic.List[string]; $runError = ''
$log.status = 'running'; $log.requested_stages = $stagesRequested
# Said per key, so that a key with no delivery read is not taken for one that had it.
$log.keys_without_delivery_read = [ordered]@{ Mouse = 'movement is not a key, so the key handler has nothing to show; only the view and position reads say anything about it' }
function Set-Stage([string]$Stage, [bool]$Passed, [string]$Why) { if ($Passed) { $stagesDone.Add($Stage) } else { $stageProblems.Add("${Stage}: $Why") } }
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $runLock = $false; $driverOwned = $false; $modWritten = $false; $modPrior = $null; $driverAttempted = $false; $launchAttempted = $false
# A token on the game's command line that identifies the process this run started (Stop-OwnGame checks it with the
# image path and the start time before stopping a PID). Unknown switches are ignored by the game.
$marker = "-owrun=$Lane-$PID"
try {
    # Preflight before any lock or backup: first a run that did not finish, whose driver folder would otherwise be the refusal and hide the way back; then bl-sdk present, our driver folder not already there.
    Assert-NoModPending
    if (!(Test-Path -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods'))) { throw "bl-sdk is not installed (no sdk_mods in $env:OPENWILLOW_BL2); not installing it" }
    if (Test-Path -LiteralPath $script:DriverDir) { throw "$script:DriverDir already exists; this run did not create it and leaves it alone" }
    # What was loaded: mods can change what a run sees (2026-11-07: Better UI Controls turned E into Enter on this screen). Reading only; a run without this record is not one to trust.
    $modsRoot = Join-Path $env:OPENWILLOW_BL2 'sdk_mods'
    $log.sdk_mods = @(Get-ChildItem -LiteralPath $modsRoot -Force | ForEach-Object { $_.Name } | Sort-Object)
    $log.sdk_mods_settings = [ordered]@{}
    foreach ($file in Get-ChildItem -LiteralPath (Join-Path $modsRoot 'settings') -Filter *.json) {
        try { $log.sdk_mods_settings[$file.BaseName] = (Get-Content -Raw -LiteralPath $file.FullName | ConvertFrom-Json) } catch { $log.sdk_mods_settings[$file.BaseName] = "unreadable: $_" }
    }
    # Refuse before any lock or backup if the setting cannot be changed: reading only.
    $modFile = Join-Path $modsRoot 'settings\BetterUIControls.json'
    if (!(Test-Path -LiteralPath $modFile)) { throw "no ${modFile}: not changing anything" }
    $null = Get-ModOffBytes ([IO.File]::ReadAllBytes($modFile))
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
    # A root is marked usable only after its copy compares identical; cleanup never restores from any other.
    foreach ($d in 'SaveData', 'Config') { $snapshots[$d] = Assert-SnapshotMatches (Join-Path $script:WillowDir $d) (Join-Path $bak $d) }
    foreach ($d in $extraRoots.Keys) {
        if (Test-Path -LiteralPath $extraRoots[$d]) { $snapshots[$d] = New-VerifiedSnapshot $extraRoots[$d] (Join-Path $bak $d) } else { $snapshots[$d] = 'absent' }
    }
    Save-VerifiedBackupRecord $bak $roots $snapshots
    # -Exclusive re-checks that the folder is absent (it may have appeared while this run waited for the locks) and removes
    # its own partial install on failure, so the folder is this run's only once the call has returned.
    $driverAttempted = $true   # before the call: a failed install can leave the folder behind
    Install-Driver -BlockSavesAtStart -Exclusive
    $driverOwned = $true
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $gameArgs = @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', $marker)
    if ($Character) { $gameArgs += "-Character=$Character" }
    $foreign = Get-ForeignGamesSeen $null
    if ($foreign) { throw "Borderlands2 (pid $foreign) started during the setup; not launching a second one" }
    # The last change before the launch. Its copy in the verified backup must be the same bytes. The launch flag stays right at the launch: a refusal here launched nothing, so the
    # finalizer must not restore the roots over newer files; its own block puts this one file back.
    $modPrior = [IO.File]::ReadAllBytes($modFile)
    if (!(Test-SameBytes ([IO.File]::ReadAllBytes((Join-Path $bak 'sdk_settings\BetterUIControls.json'))) $modPrior)) { throw 'the mod setting differs from its verified backup; not changing it' }
    $modOff = Get-ModOffBytes $modPrior
    $log.mod_setting_prior_base64 = [Convert]::ToBase64String($modPrior)
    Save-ModPending $modFile $modPrior $bak $Lane $HubLock (Join-Path $PSScriptRoot 'realgame.ps1')
    $modWritten = $true   # before the write: a failure inside still restores
    [IO.File]::WriteAllBytes($modFile, $modOff)
    if (!(Test-SameBytes ([IO.File]::ReadAllBytes($modFile)) $modOff)) { throw 'the mod setting did not take the change' }
    $launchAttempted = $true
    # If the launch throws, what counts is whether a game exists: one found by the marker is stopped by the finalizer like any other; with none, nothing launched and the roots stay as they are.
    # A search that fails leaves the flag set; a game it missed is still seen as foreign by the finalizer, which then holds.
    try { $proc = Start-Game $gameArgs }
    catch {
        $startError = $_
        try {
            $found = Find-GameByMarker $marker
            if ($found) { $proc = $found; $procStart = $proc.StartTime } else { $launchAttempted = $false }
        } catch { $log.launch_search_error = "$_" }
        throw $startError
    }
    $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'mission_screen/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    # Proof from the running game, before any key: otherwise E would still be the mod's, and read as stock.
    foreach ($f in 'fire_mission_probe.py', 'mission_screen_probe.py') { Invoke-GamePyFile (Join-Path $PSScriptRoot "scripts/$f") mission_screen | Out-Null }
    $log.mod_off = PyJson "ms_mod_state('BetterUIControls.json')"
    if ($log.mod_off.ok -ne $true) { throw "Better UI Controls is not off in the running game: $($log.mod_off | ConvertTo-Json -Compress)" }
    Start-Sleep 5; Assert-GameForeground; Send-Key Enter; Start-Sleep 45
    $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
    "map: $map"; if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'mission_screen/load_failed.png' | Out-Null; throw "no player pawn: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 1
    $log.hooks = PyJson 'ms_start()'
    Set-Stage 'mod_off' ($true) ''   # the throw above is the check
    Set-Stage 'hooks' (@($log.hooks.missing).Count -eq 0) "functions not found, so their rows can never come: $(@($log.hooks.missing) -join '; ')"
    $log.before = Read-After 'before'
    if ($map -notmatch 'Sanctuary') { throw "not in Sanctuary, so Marcus is not here: $map" }
    Set-Stage 'start' ($log.before.status -eq 0) "the Fire mission is not at status 0 in this save (it is $($log.before.status))"
    if ($log.before.status -ne 0) { throw "the Fire mission is at status $($log.before.status) in this save: there is no offer to read" }

    # 1. The offer: does E open a page first, and which keys does the page take? E again is the first test; Enter comes last, since it accepts.
    $row = Open-Screen 'offer'
    $log.settings = PyJson 'ms_settings()'
    Set-Stage 'offer' ($row.open -eq $true -and $row.status -eq 0) "E at Marcus did not open the screen: open $($row.open), status $($row.status)"
    if (!$row.open) { throw 'the screen did not open at the offer' }
    # With the mod off, E and WASD are the game's own. Space is left out (an earlier run: one press accepted, through the movie). E comes last among the tests; a key that accepts is a finding, and
    # the run goes on from there. Mod still acting would show as an Enter after E's mark.
    $row = Test-Keys 'offer' @('Mouse', 'W', 'A', 'S', 'D', 'F', 'E') 0
    # The mouse has no delivery read (the handler gets no mouse movement): only the view and position say anything about it.
    $missing = Get-KeysNotDelivered 'offer' @('W', 'A', 'S', 'D', 'F', 'E')
    Set-Stage 'offer_keys' ($missing.Count -eq 0 -and @($log.delivered_offer_E.seen) -notcontains 'Enter') "not delivered: $($missing -join ', '); E's rows: $($log.delivered_offer_E | ConvertTo-Json -Compress)"
    $accept = $row
    if ($row.status -eq 0) {
        $esc = Test-Key 'offer' 'Esc'
        Set-Stage 'offer_esc' ($esc.open -eq $false -and $esc.status -eq 0 -and $log.delivered_offer_Esc.delivered -eq $true) "Esc did not close the offer: open $($esc.open), status $($esc.status)"
        if ($esc.open) { throw 'the offer stayed open after Esc' }
        $row = Open-Screen 'offer_again'
        if (!$row.open) { throw 'the screen did not open again at the offer' }
        $accept = $row
        if ($row.status -eq 0) { $accept = Test-Key 'offer' 'Enter' }
    } else {
        Set-Stage 'offer_esc' $false "not read: a key test accepted first ($($row.status))"
    }
    Read-After 'accept_after' 1.0 | Out-Null
    Set-Stage 'accept' ($accept.status -eq 1 -and $log.delivered_offer_Enter.delivered -eq $true) "Enter did not accept: status $($accept.status)"
    if ($accept.status -ne 1) { throw 'the offer was not accepted' }

    # 2. To the turn-in by the shortcut (ms_advance). Marcus walks to the range after the accept, so the wait lets him arrive.
    $ready = $null
    for ($i = 0; $i -lt 30 -and !$ready.ready; $i++) { $ready = PyJson 'ms_advance()'; Start-Sleep 1 }
    if (!$ready.ready) { $ready = PyJson 'ms_force_ready()'; $log.forced_ready = $true }
    $log.ready = $ready
    Set-Stage 'ready' ($ready.ready -eq $true) "status $($ready.status)"
    if (!$ready.ready) { throw 'the mission did not reach ready to turn in' }
    Start-Sleep 15

    # 3. The turn-in list, then the reward page, with the same key tests. Enter on the page is last: it takes the reward.
    $row = Open-Screen 'turnin'
    Set-Stage 'turnin' ($row.open -eq $true -and $row.status -eq 3) "E at Marcus did not open the turn-in screen: open $($row.open), status $($row.status)"
    if (!$row.open) { throw 'the screen did not open at the turn-in' }
    # Enter, as at the offer: E here would be the mod's. The mark is before it.
    Py "print(ms_mark('turnin_enter'))" | Out-Null
    $row = Test-Keys 'turnin' @('Mouse', 'W', 'S', 'E') 3
    $missing = Get-KeysNotDelivered 'turnin' @('W', 'S', 'E')
    Set-Stage 'turnin_keys' ($missing.Count -eq 0 -and @($log.delivered_turnin_E.seen) -notcontains 'Enter') "not delivered: $($missing -join ', '); E's rows: $($log.delivered_turnin_E | ConvertTo-Json -Compress)"
    if ($row.status -eq 3) { $turned = Test-Key 'turnin' 'Enter' }
    foreach ($at in '0.5', '1.0', '1.5') { Read-After "reward_$at" 0.4 | Out-Null }
    Start-Sleep 2; $page = Read-After 'reward_page'
    $log.reward_shown = PyJson 'ms_reward_shown()'
    Set-Stage 'reward' ($log.reward_shown.ok -eq $true) "no reward page after the Enter: $($log.reward_shown | ConvertTo-Json -Compress)"
    if ($page.status -ne 4) { throw 'the turn-in did not complete' }
    # Each press is marked, so the verdict can say which one took the reward, if a key test did.
    Py "print(ms_mark('reward_shown'))" | Out-Null
    $last = $page
    # With the mod off, E here is the game's. F is left out: on this page the script makes it inspect the selected item, and there is none.
    foreach ($key in 'Mouse', 'W', 'S', 'E', 'Esc') {
        if ($last.open) { Py "print(ms_mark('reward_press_$key'))" | Out-Null; $last = Test-Key 'reward' $key }
    }
    $log.reward_closed_by_key_tests = !$last.open
    $missing = Get-KeysNotDelivered 'reward' @('W', 'S', 'E', 'Esc')
    Set-Stage 'reward_keys' ($missing.Count -eq 0) "not delivered: $($missing -join ', '); closed by a key test $($log.reward_closed_by_key_tests)"
    $after = $last
    $log.reward_after_keys = PyJson 'ms_reward_taken()'
    if ($after.open -and $log.reward_after_keys.count -eq 0) {
        Py "print(ms_mark('reward_press_Enter'))" | Out-Null
        $after = Test-Key 'reward' 'Enter'
        foreach ($at in '0.5', '1.0') { $after = Read-After "reward_taken_$at" 0.4 }
    }
    $log.reward_taken = PyJson 'ms_reward_taken()'
    Set-Stage 'reward_taken' ($log.reward_taken.ok -eq $true) "the reward was not taken once by a marked press: $($log.reward_taken | ConvertTo-Json -Compress)"
    # What is left: a list with the next mission, or nothing. Esc only, on a screen that is still open.
    if ($after.open) { $after = Test-Key 'after' 'Esc' }
    Set-Stage 'closed' ($after.open -eq $false) 'the screen is still open after Esc'
    $closed = Read-After 'closed_world' 1.0
    Py "print(ms_dump('screen'))" | Out-Null
    $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')

    # 4. The respawn: down, bleed-out, the station, the message. A frame about each second. The script shows it with AddTrainingText for 7 s, not a dialog box, so no key is pressed:
    # the frames say whether it goes by itself. Ten more frames after the call.
    if (!$SkipRespawn) {
        # Downing Maya with the screen open is the crash risk: stop here unless the screen is confirmed closed.
        if ($closed.open -ne $false -or $closed.movie_playing -eq $true -or 'closed' -notin $stagesDone) { throw "the screen is not confirmed closed (open $($closed.open), movie playing $($closed.movie_playing)); not going on to the respawn" }
        $down = PyJson 'ms_down()'
        if ($down.refused) { throw "ms_down refused: $($down.refused)" }
        $log.cash_before = $down.read.cash
        $deadline = (Get-Date).AddSeconds(60); $n = 0; $dialogAt = $null; $row = $null
        while ((Get-Date) -lt $deadline) {
            $row = Read-After ('respawn_{0:D2}' -f $n) 0.8; $n++
            if (!$dialogAt -and (PyJson 'ms_respawn()').dialog) { $dialogAt = $n }
            if ($dialogAt -and $n -gt $dialogAt + 10) { break }
        }
        $log.respawn_frames = $n; $log.dialog_frame = $dialogAt; $log.cash_after = $row.cash
        $log.respawn = PyJson 'ms_respawn()'
        Py "print(ms_dump('screen'))" | Out-Null
        Set-Stage 'respawn' ($log.respawn.ok -eq $true) "no down, respawn, dialog and matching charge in order: $($log.respawn | ConvertTo-Json -Compress)"
    }
    $log.hook_errors = PyJson 'ms_errors()'
    Set-Stage 'hook_errors' ($log.hook_errors.hook_errors -eq 0) "errors in the hooks, so rows may be missing: $($log.hook_errors | ConvertTo-Json -Compress)"
    Py 'print(ms_stop())' | Out-Null
    # The game may close before it answers this one.
    try { Invoke-GamePy 'get_pc().ConsoleCommand("quit", False)' 15 | Out-Null } catch { "quit: no reply ($_); waiting for the exit" }
    for ($i = 0; $i -lt 20 -and (Get-GameProcess); $i++) { Start-Sleep 2 }
} catch {
    $runError = "$_"
    throw
} finally {
    # Each step is isolated so a failure in one (a log write) cannot skip the restore. Order: stop our game, and only once it
    # has exited remove our driver and restore and verify the saves and config; then write the logs and release the locks.
    # If the game's exit is not confirmed, a live game could overwrite restored files: nothing is removed or restored, the
    # backup is kept and manual-recovery steps are printed.
    $cleanup = [ordered]@{}
    # A throw between ms_start and ms_stop leaves the hooks and the per-frame down job running into shutdown: stop them while the game can still answer.
    if ($proc -and !$proc.HasExited) {
        # Py returns a traceback as text, so only the exact answer counts.
        try { $reply = Py 'ms_stop(); print("stopped")' 5; $cleanup.hooks = if ($reply -ceq 'stopped') { 'stopped' } else { "not stopped: $reply" } }
        catch { $cleanup.hooks = "not stopped: $_" }
    }
    $gameGone = $true
    if ($proc) {
        try {
            for ($i = 0; $i -lt 10 -and !$proc.HasExited; $i++) { Start-Sleep 1 }
            $cleanup.game = if ($proc.HasExited) { 'exited' } else { Stop-OwnGame $proc $procStart $marker }
            if ($cleanup.game -eq 'stopped' -and !$proc.WaitForExit(15000)) { $cleanup.game = 'stopped, but still running after 15 s' }
            $proc.Refresh(); $gameGone = $proc.HasExited
        } catch { $cleanup.game = "error: $_"; $gameGone = $false }
    }
    $restored = $null; $restoreError = ''
    # Those flags are set before their calls, so a failure inside still counts. An install can only need its folder cleaned up; only a launched
    # game can have changed the saves, so only that calls for a restore, which could otherwise erase another writer's changes.
    $launched = $launchAttempted -or $proc
    $touched = $driverAttempted -or $launched
    # Reading only: a game this run did not start is never stopped, and nothing is removed or restored under or after it.
    $untracked = if ($touched) { Get-ForeignGamesSeen $proc } else { '' }
    $safe = $gameGone -and !$untracked
    if (!$safe) {
        $cleanup.driver = "NOT REMOVED (a game may still run): $script:DriverDir"
        $cleanup.saves = "NOT RESTORED (a game may still run); backup kept at $bak"
    }
    try {
        if ($safe -and $driverOwned) {
            Remove-Driver
            $cleanup.driver = if (Test-Path -LiteralPath $script:DriverDir) { "STILL PRESENT: $script:DriverDir" } else { 'removed' }
        } elseif ($safe -and $driverAttempted) {
            # The install failed. Its own rollback is best effort, and the folder may not be this run's, so it is reported, not removed.
            $cleanup.driver = if (Test-Path -LiteralPath $script:DriverDir) { "STILL PRESENT after a failed install: $script:DriverDir" } else { 'install failed; nothing left' }
        } elseif (!$cleanup.Contains('driver')) { $cleanup.driver = 'not installed by this run; untouched' }
    } catch { $cleanup.driver = "error: $_" }
    $rows = @()
    if ($bak -and $safe -and !$launched) { $cleanup.saves = 'not touched: this run launched no game' }
    elseif ($bak -and $safe) {
        # Once more, immediately before the restore: a game launched during the cleanup could overwrite it.
        $untracked = Get-ForeignGamesSeen $proc
        if ($untracked) { $safe = $false; $cleanup.saves = "NOT RESTORED (a game may still run); backup kept at $bak" }
    }
    if ($bak -and $launched -and $safe) {
        try {
            $restored = Restore-VerifiedRoots $bak $roots $snapshots
            $rows = @($restored.rows); $cleanup.saves = $restored.summary
        } catch { $cleanup.saves = "RESTORE FAILED: $_ (backup kept at $bak)"; $restoreError = "$_" }
    }
    # Mod setting: the root restore above should already have brought it back. This checks the exact bytes, whatever that did. Not under a game that may still run: Restore-VerifiedBackup covers it then.
    $modProblem = ''
    if ($modWritten) {
        if ($safe) {
            try {
                $cleanup.mod_setting = Restore-ModSetting $modFile $modPrior
                Remove-Item -LiteralPath (Get-ModPendingFile) -ErrorAction SilentlyContinue
                if (Test-Path -LiteralPath (Get-ModPendingFile)) {
                    $modProblem = "restored, but the pending record could not be removed, and the next run refuses until it is: $(Get-ModPendingFile)"
                    $cleanup.mod_setting += "; $modProblem"
                }
            }
            catch { $modProblem = "not restored: $_"; $cleanup.mod_setting = "RESTORE FAILED: $_ (the record stays: $(Get-ModPendingFile))" }
        } else { $cleanup.mod_setting = "NOT RESTORED (a game may still run; the record stays: $(Get-ModPendingFile))" }
    }
    $recovery = Get-RecoveryReason $gameGone $cleanup.game $restored $restoreError $cleanup.driver $untracked
    if ($modProblem) { $recovery = (@($recovery, "mod setting $modProblem") | Where-Object { $_ }) -join '; ' }
    $holdLocks = [bool]$recovery
    # The evidence first and session.json last, so the status it holds accounts for the rest. Missing evidence cannot read as completed; the
    # restore has already run, and recovery keeps its status.
    $evidenceError = ''
    try {
        if ($restored) { @{ backup = $bak; compared = (Get-Date).ToString('o'); changes = @($rows) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Out 'save_compare.json') }
    } catch { $evidenceError = "$_" }
    $log.completed_stages = @($stagesDone); $log.stage_problems = @($stageProblems)
    $log.run_error = $runError
    $log.status = Get-RunStatus $stagesRequested @($stagesDone) $runError $recovery
    if ($stageProblems.Count -and $log.status -ne 'completed') { $log.status = "$($log.status) ($($stageProblems -join '; '))" }
    if ($evidenceError -and !$recovery) { $log.status = "failed: evidence not written ($evidenceError)" }
    $log.finished = (Get-Date).ToString('o'); $log.cleanup = $cleanup
    try { $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json') }
    catch {
        "log write failed: $_"
        if (!$recovery) { $log.status = "failed: evidence not written ($_)" }
    }
    # Anything left for a person keeps both locks, so no other run starts on it.
    if ($holdLocks) { $runLock = $false; $hub = $null }
    try { if ($runLock) { Exit-RunLock } } catch { "run lock release failed: $_" }
    try { if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force } } catch { "hub lock release failed: $_" }
    "status: $($log.status)"
    if ($runError) { "run error: $runError" }
    $cleanup.GetEnumerator() | ForEach-Object { "cleanup $($_.Key): $($_.Value)" }
    Format-RestoreRows $rows
    if ($holdLocks) { Format-ManualRecovery $recovery $bak $HubLock $Lane }
    # Exit 0 only for a completed run with nothing left to recover. Here in the finally, so an early return passes through it too.
    if ($log.status -ne 'completed') { exit 1 }
}
