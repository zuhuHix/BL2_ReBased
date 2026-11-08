# Controls lane ground truth (docs/verification/CONTROLS.md): one real-game session in Sanctuary with saves blocked from the first
# frame. Reads the stock bindings and the melee definitions, presses the melee key with the chain hooked, reads the HUD's use prompt
# at nearby usable things, reads what M and L do on each open status page and while Maya is down, quits. Takes the screen and keyboard; run it only with a granted heavy slot. Everything written is game
# data under ignored local/realgame/controls/.
#
# Data safety, on every exit:
# - A root is usable only after its copy compares identical; its hashes are stored and re-checked before any restore, and cleanup
#   restores from no other root.
# - The game must be confirmed gone before anything is removed or restored; otherwise nothing is touched.
# - Anything left for a person (a restore that fails or differs, a changed backup, a root created during the run, a driver left behind)
#   keeps both locks and the backup, and prints the recovery steps.
# - A root created during the run is left in place, named, and the summary says NOT IDENTICAL.
# - Keys and mouse refuse unless the game has focus. Nothing changes a file before the driver install and game start: a run with neither
#   skips the restore.
# - session.json has a real status: completed, incomplete, failed or "recovery needed". The exit code is 0 only for completed.
# Only the game PID this run started is stopped (Stop-OwnGame checks its path, start time and marker).
param(
    # The machine-wide hub lock lives in the main checkout's local/ (shared by every worktree on this machine).
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'controls',
    [string]$Character = 'Save0008.sav',    # the level-8 Maya in Sanctuary
    [int]$PromptTargets = 8,                # the nearest usable things read per class
    # What to dot-source for the real-game functions. Only tests/controls_capture_test.py passes another file.
    [string]$Helpers = (Join-Path $PSScriptRoot 'realgame.ps1')
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. $Helpers
$Out = Join-Path $script:RG 'controls'
New-Item -ItemType Directory -Force $Out | Out-Null
# Stale evidence: a final write that fails must not leave an earlier run's completed behind. Kept under a new name, not deleted: after a run that
# needs recovery, its record is what names the backup and says what went wrong, and a rejected retry must not lose that.
$keptAs = "$(Get-Date -Format 'yyyyMMdd-HHmmss')-$PID"
foreach ($stale in 'session', 'save_compare') { $file = Join-Path $Out "$stale.json"; if (Test-Path -LiteralPath $file) { Move-Item -LiteralPath $file (Join-Path $Out "$stale.$keptAs.json") } }
# End of stale evidence.
$log = [ordered]@{ started = (Get-Date).ToString('o'); status = 'running' }
function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }
# Evidence for the stages: a status menu that is really up, and the melee calls the game has made so far.
function Get-Page { Py 'print(json.dumps(cp_page(), default=str))' 15 }
function Test-PageOpen([string]$Json) { try { (ConvertFrom-Json $Json).open -eq $true } catch { $false } }
# What a key can change on a page: whether it is open, and which screen. The rest of the read moves by itself (a screen's clock).
function Get-PageState([string]$Json) { try { $page = ConvertFrom-Json $Json; "$($page.open)/$($page.screen)" } catch { "unread: $Json" } }
# Esc on no page opens the pause menu, which then takes every key: only a page that is really open gets one.
function Close-Page {
    for ($i = 0; $i -lt 4; $i++) {
        if (!(Test-PageOpen (Get-Page))) { return $true }
        Send-Key Esc; Start-Sleep 2
    }
    !(Test-PageOpen (Get-Page))
}
# A dialog over a page takes its keys. Enter is its OK.
function Clear-Dialogs {
    for ($i = 0; $i -lt 5 -and [int](Py 'print(cp_dialog())' 15) -gt 0; $i++) { Send-Key Enter; Start-Sleep 1 }
}
# The first read that is a real use target with its clip showing, in the probe's JSON.
function Test-PromptRead([string]$Json) { try { [bool](ConvertFrom-Json $Json).usable } catch { $false } }
function Get-MeleeCount { [int](Py 'print(cp_count())') }
function Test-Number($Value) { $Value -is [int] -or $Value -is [long] -or $Value -is [double] -or $Value -is [decimal] }
# Why the melee facts are not what the stage exists to capture, or nothing: the definition's numbers and switches, and the melee clip's set and length (what the import
# needs). The first-person move and damage_value are logged but not required: the real run read none and an error for them, by a route the record does not rely on.
function Get-MeleeFactsProblem([string]$Json) {
    try { $facts = ConvertFrom-Json $Json } catch { return "not JSON: $Json" }
    $definition = $facts.definition
    if (!(Test-Number $definition.trace) -or $definition.trace -le 0) { return "no trace distance: $Json" }
    if (!(Test-Number $definition.cone)) { return "no cone angle: $Json" }
    if ($definition.use_cone -isnot [bool] -or $definition.use_radius -isnot [bool]) { return "no melee switches: $Json" }
    $clips = 0
    foreach ($set in @($facts.arm_melee_clips)) {
        foreach ($clip in @($set.clips)) {
            if ($null -eq $clip) { continue }
            if (@($clip).Count -ne 2 -or "$($clip[0])" -notmatch 'melee' -or !(Test-Number $clip[1]) -or $clip[1] -le 0) { return "a bad clip read: $Json" }
            $clips++
        }
    }
    if ($clips -eq 0) { return "no melee clip read: $Json" }
    ''
}
# A page read a key can be judged by. A traceback comes back as text, and is no read at all, so it must not pass for a key that did nothing.
function Test-PageRead([string]$Json) { try { (ConvertFrom-Json $Json).open -is [bool] } catch { $false } }
function Test-DownRead([string]$Json) { try { $page = ConvertFrom-Json $Json; $page.open -is [bool] -and $page.injured -eq $true } catch { $false } }
function Test-Injured([string]$Json) { try { (ConvertFrom-Json $Json).injured -eq $true } catch { $false } }

$extraRoots = [ordered]@{ sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
foreach ($d in $extraRoots.Keys) { $roots[$d] = $extraRoots[$d] }
$snapshots = @{}
$stagesRequested = @('melee_facts', 'melee_air', 'melee_interrupted', 'prompts', 'melee_target', 'page_keys', 'page_down')
$log.requested_stages = $stagesRequested
$stagesDone = New-Object Collections.Generic.List[string]; $stageProblems = New-Object Collections.Generic.List[string]; $runError = ''
function Set-Stage([string]$Stage, [bool]$Passed, [string]$Why) { if ($Passed) { $stagesDone.Add($Stage) } else { $stageProblems.Add("${Stage}: $Why") } }
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $runLock = $false; $driverOwned = $false; $driverAttempted = $false; $launchAttempted = $false
$marker = "-owrun=$Lane-$PID"   # an argument the game ignores; Stop-OwnGame looks for it on the command line
try {
    Assert-NoPendingRecords   # an earlier run's pending record stops this one before it takes a lock or changes anything (pending.ps1)
    # Preflight before any lock or backup: bl-sdk present, our driver folder not already there.
    if (!(Test-Path -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods'))) { throw "bl-sdk is not installed (no sdk_mods in $env:OPENWILLOW_BL2); not installing it" }
    if (Test-Path -LiteralPath $script:DriverDir) { throw "$script:DriverDir already exists; this run did not create it and leaves it alone" }
    # No waiting here: the hub grants the slot first, so a held lock means ask the hub.
    try { $hub = New-Item -ItemType File -Path $HubLock -Value "$Lane $((Get-Date).ToString('o'))" -ErrorAction Stop }
    catch { throw "hub lock $HubLock held by '$(Get-Content $HubLock -Raw -ErrorAction SilentlyContinue)'; not waiting (ask the hub)" }
    if (Get-GameProcess) { throw 'Borderlands2 is running although the hub lock was free; not touching it' }
    Enter-RunLock 600; $runLock = $true
    $bak = Backup-Saves; $log.backup = $bak; "backup $bak"
    foreach ($d in 'SaveData', 'Config') { $snapshots[$d] = Assert-SnapshotMatches (Join-Path $script:WillowDir $d) (Join-Path $bak $d) }
    foreach ($d in $extraRoots.Keys) {
        if (Test-Path -LiteralPath $extraRoots[$d]) { $snapshots[$d] = New-VerifiedSnapshot $extraRoots[$d] (Join-Path $bak $d) } else { $snapshots[$d] = 'absent' }
    }
    # The record outlives this process: the printed recovery command restores from it, and refuses a root that changed since.
    Save-VerifiedBackupRecord $bak $roots $snapshots
    # -Exclusive re-checks the folder is absent and removes its own partial install on failure, so it is ours only once this returns.
    $driverAttempted = $true   # before the call: a failed install can leave the folder behind
    Install-Driver -BlockSavesAtStart -Exclusive
    $driverOwned = $true
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $foreign = Get-ForeignGamesSeen $null
    if ($foreign) { throw "a game this run did not start is running (pid $foreign); not launching" }
    $launchAttempted = $true
    $proc = Start-Game @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', "-Character=$Character", $marker); $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) {
        if ($proc.HasExited) { break }
        try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 }
    }
    if (-not $ok) { Save-Shot 'controls/boot.png' | Out-Null; throw 'driver did not answer (check Steam console_log for AppError_5)' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    Start-Sleep 5; Assert-GameForeground; Send-Key Enter; Start-Sleep 45
    $map = ''
    for ($i = 0; $i -lt 6 -and $map -notmatch 'WillowPlayerPawn'; $i++) {   # the level load can hold the command channel
        try { $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)' 30 }
        catch { Start-Sleep 10 }
    }
    "map: $map"; $log.map = $map
    if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'controls/load_failed.png' | Out-Null; throw "no player pawn: $map" }
    if ($map -notmatch '^Sanctuary') { Save-Shot 'controls/wrong_map.png' | Out-Null; throw "not in Sanctuary: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 10   # let the streamed levels spawn
    Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/controls_probe.py') controls | Out-Null

    $bindings = Py 'print(json.dumps(cp_bindings()))' 60
    # Not a stage: the first run found none of these keys in this list, as the data says. The presses below are the evidence.
    $log.bindings = $bindings
    $facts = Py 'print(json.dumps(cp_melee_facts(), default=str))' 60
    $log.facts = $facts; "facts: $facts"
    $factsProblem = Get-MeleeFactsProblem $facts
    Set-Stage 'melee_facts' (!$factsProblem) $factsProblem
    $log.hooks = Py 'print(json.dumps(cp_melee_hooks()))' 60

    # The melee press: empty air first (the swing's own timing), then three presses 0.3 s apart (does a press in a swing count).
    Assert-GameForeground
    Send-Key V; Start-Sleep 3
    Save-Shot 'controls/after_melee.png' | Out-Null
    Send-Key V; Start-Sleep -Milliseconds 300; Send-Key V; Start-Sleep -Milliseconds 300; Send-Key V; Start-Sleep 3
    Py "print(cp_rows('melee_air'))" | Out-Null
    $count = Get-MeleeCount
    Set-Stage 'melee_air' ($count -gt 0) 'no melee call seen for the V presses'

    # What interrupts a stock swing: the status menu opened inside the hit's delay, and a weapon key inside it. The rows show whether
    # the hit behaviour still ran and whether the menu opened; the shots show what happened to the arm.
    $beforeInterrupted = Get-MeleeCount
    Send-Key V; Start-Sleep -Milliseconds 100; Send-Key Tab; Start-Sleep 2
    $menu = Get-Page
    Save-Shot 'controls/melee_then_menu.png' | Out-Null
    if (!(Close-Page)) { throw 'the status menu did not close after a swing' }
    Send-Key V; Start-Sleep -Milliseconds 100; Send-Key '2'; Start-Sleep 3
    Save-Shot 'controls/melee_then_weapon_key.png' | Out-Null
    Py "print(cp_rows('melee_interrupted'))" | Out-Null
    $log.melee_then_menu = $menu
    $interruptedSwings = (Get-MeleeCount) - $beforeInterrupted
    Set-Stage 'melee_interrupted' ((Test-PageOpen $menu) -and $interruptedSwings -gt 0) "menu open: $(Test-PageOpen $menu), melee calls: $interruptedSwings"

    # The list comes through ForEach-Object: in Windows PowerShell ConvertFrom-Json hands an array over as one item, and the first run's targets
    # all ran together into one path, so Maya never moved.
    $targets = @(Py 'print(json.dumps(cp_targets()))' 60 | ConvertFrom-Json | ForEach-Object { $_ })
    # The nearest of each class, one per kind of object (pawns have no definition, so each counts): the first run's list was mostly cardboard boxes.
    $picked = @($targets | Group-Object class | ForEach-Object { $_.Group | Group-Object { if ($_.class -eq 'WillowAIPawn') { $_.path } else { "$($_.definition)" } } | ForEach-Object { $_.Group[0] } | Select-Object -First $PromptTargets })
    $n = 0; $reads = @(); $usableReads = 0
    foreach ($t in $picked) {
        $n++
        Py "print(cp_prompt(r'$($t.path)'))" | Out-Null
        Start-Sleep -Milliseconds 700
        Py "print(cp_aim(r'$($t.path)'))" | Out-Null
        Start-Sleep -Milliseconds 700
        $read = Py "print(json.dumps(cp_prompt_read(r'$($t.path)'), default=str))"
        $reads += $read
        if (Test-PromptRead $read) { $usableReads++ }
        Save-Shot "controls/prompt_$n.png" | Out-Null
        # The same spot with the view turned away: does the target follow the view or the distance? It decides what the prompt names.
        foreach ($turn in 15, 30) {
            Py "print(cp_turn($turn))" | Out-Null
            Start-Sleep 1
            $reads += (Py "print(json.dumps(cp_prompt_read(r'$($t.path) turned $turn more'), default=str))")
        }
    }
    $reads | Set-Content (Join-Path $Out 'prompts.jsonl')
    "prompts: $($reads.Count) reads, $usableReads of $($picked.Count) targets usable"; $log.prompts = $reads.Count; $log.usable_targets = $usableReads
    Set-Stage 'prompts' ($usableReads -gt 0) "no target was usable, of $($picked.Count)"

    # A swing at something: the nearest pawn, then the nearest interactive object. What counts is a hit on the one she stands before.
    $meleeTargets = @(@($targets | Where-Object { $_.class -eq 'WillowAIPawn' } | Select-Object -First 1) + @($targets | Where-Object { "$($_.definition)" -match 'CardboardBox' } | Select-Object -First 1))
    $hitTargets = 0; $hitReads = @()
    foreach ($t in $meleeTargets) {
        Py "print(cp_prompt(r'$($t.path)'))" | Out-Null
        Start-Sleep -Milliseconds 700
        Py "print(cp_aim(r'$($t.path)'))" | Out-Null
        Start-Sleep -Milliseconds 300; Assert-GameForeground; Send-Key V; Start-Sleep 2
        $hits = Py "print(json.dumps(cp_hits(r'$($t.path)'), default=str))"
        $hitReads += "{`"target`": `"$($t.class)`", `"hits`": $hits}"
        if ((ConvertFrom-Json $hits).count -gt 0) { $hitTargets++ }
    }
    $hitReads | Set-Content (Join-Path $Out 'melee_target.jsonl')
    Py "print(cp_rows('melee_target'))" | Out-Null
    $log.melee_hits = $hitReads
    Set-Stage 'melee_target' ($hitTargets -gt 0) "targets: $($meleeTargets.Count), hit: $hitTargets"

    # M and L on each open page: open it with its stock key, clear any dialog, press one, read and capture, close by what is really open.
    $pageReads = @(); $pagesOpened = 0; $keysWithEffect = 0
    foreach ($page in 'M', 'L', 'I', 'K') {
        foreach ($key in 'M', 'L') {
            Assert-GameForeground
            Send-Key $page; Start-Sleep 2
            Clear-Dialogs
            $opened = Get-Page
            Send-Key $key; Start-Sleep 2
            $after = Get-Page
            if (Test-PageOpen $opened) { $pagesOpened++; if ((Test-PageRead $after) -and (Get-PageState $after) -ne (Get-PageState $opened)) { $keysWithEffect++ } }
            Save-Shot "controls/page_${page}_then_${key}.png" | Out-Null
            $pageReads += "{`"page`": `"$page`", `"key`": `"$key`", `"opened`": $opened, `"after`": $after}"
            if (!(Close-Page)) { throw "the page opened with $page did not close" }
            Save-Shot "controls/page_${page}_then_${key}_closed.png" | Out-Null
        }
    }
    $pageReads | Set-Content (Join-Path $Out 'page_keys.jsonl')
    # A key may do nothing on some page, which is what this reads; but every page must have opened, and some key must have done something.
    Set-Stage 'page_keys' ($pagesOpened -eq 8 -and $keysWithEffect -gt 0) "pages opened: $pagesOpened of 8, keys with an effect: $keysWithEffect"
    Py "print(cp_rows('page_keys'))" | Out-Null

    # Last: Maya then bleeds out. Whether the map, the missions and a swing work while she is down. The reads below take about 8 s against
    # about 20 s of bleed-out, so normally the game quits first. A slow session can see her die and respawn in memory: saves stay blocked
    # and everything is restored from the verified backup all the same, and only the last shots would show the respawn.
    $injured = ''; $downReadsValid = 0
    for ($i = 0; $i -lt 8 -and !(Test-Injured $injured); $i++) { $injured = Py 'print(json.dumps(cp_injure(), default=str))' 15; Start-Sleep 1 }
    $log.injure = $injured; "injure: $injured"
    if (Test-Injured $injured) {
        $downReads = @()
        foreach ($key in 'M', 'L') {
            Assert-GameForeground
            Send-Key $key; Start-Sleep 1
            $after = Get-Page
            if (Test-DownRead $after) { $downReadsValid++ }
            $downReads += "{`"key`": `"$key`", `"after`": $after}"
            Save-Shot "controls/down_$key.png" | Out-Null
            if (!(Close-Page)) { throw "a page opened with $key while down did not close" }
        }
        Send-Key V; Start-Sleep 1
        Save-Shot 'controls/down_V.png' | Out-Null
        $downReads | Set-Content (Join-Path $Out 'page_down.jsonl')
        Py "print(cp_rows('page_down'))" | Out-Null
    }
    # A valid read says what the page was, with her still down. That the key arrived rests on focus, checked before each key, and on the same keys having opened pages
    # earlier in this run: there is no read of the key itself, so a key lost in just this moment would still read as doing nothing.
    $downWhy = if (!(Test-Injured $injured)) { "she did not go down: $injured" } else { "valid reads while down: $downReadsValid of 2, pages opened by key earlier: $pagesOpened of 8" }
    Set-Stage 'page_down' ((Test-Injured $injured) -and $downReadsValid -eq 2 -and $pagesOpened -eq 8) $downWhy
    Py 'print(cp_unhook())' | Out-Null
    try { Invoke-GamePy 'get_pc().ConsoleCommand("quit", False)' 10 | Out-Null } catch { "quit: no reply ($_); waiting for the exit" }
    for ($i = 0; $i -lt 30 -and -not $proc.HasExited; $i++) { Start-Sleep 2 }
} catch {
    $runError = "$_"
    throw
} finally {
    # Each step is isolated so a failure in one (a log write) cannot skip the restore. Order: stop our game, and only once it
    # has exited remove our driver and restore and verify the saves and config; then write the logs and release the locks.
    # If the game's exit is not confirmed, a live game could overwrite restored files: nothing is removed or restored, the
    # backup is kept and manual-recovery steps are printed.
    $cleanup = [ordered]@{}
    $cleanup.inputs = Release-HeldInputs   # every key and button this run still holds is let go before anything else
    # A throw between the hooks and their removal leaves them on into shutdown: take them off while the game can still answer.
    if ($proc -and !$proc.HasExited) {
        # Py returns a traceback as text, so only the exact answer counts.
        try { $reply = Py 'cp_unhook(); print("stopped")' 5; $cleanup.hooks = if ($reply -ceq 'stopped') { 'stopped' } else { "not stopped: $reply" } }
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
    $recovery = Get-RecoveryReason $gameGone $cleanup.game $restored $restoreError $cleanup.driver $untracked
    # This run's pending record (pending.ps1) goes only once every root is the backup again by tree hash, with no game and no driver left.
    $cleanup.pending = Complete-PendingRecord $safe
    if ($cleanup.pending -notlike 'removed*' -and $cleanup.pending -ne 'none written' -and !$recovery) { $recovery = "pending record $($cleanup.pending)" }
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
    # Exit 0 only for a completed run with nothing left to recover.
    if ($log.status -ne 'completed') { exit 1 }
}
