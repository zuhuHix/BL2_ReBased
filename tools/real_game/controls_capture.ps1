# Controls lane ground truth (docs/verification/CONTROLS.md): one real-game session in Sanctuary with saves blocked from the first
# frame. Reads the stock bindings and the melee definitions, presses the melee key with the chain hooked, reads the HUD's use prompt
# at nearby usable things, reads what M and L do on each open status page and while Maya is down, quits. Takes the screen and keyboard; run it only with a granted heavy slot. Everything written is game
# data under ignored local/realgame/controls/.
#
# Data safety, on every exit:
# - A root is usable only after its copy compares identical; its hashes are stored and re-checked before any restore, and cleanup
#   restores from no other root.
# - The game must be confirmed gone before anything is removed or restored; otherwise nothing is touched.
# - A restore that fails or still differs, or a backup that changed, keeps both locks and the backup, and prints the recovery steps.
# - A root created during the run is left in place, named, and the summary says NOT IDENTICAL.
# - session.json has a real status: completed, incomplete, failed or "recovery needed". The exit code is 0 only for completed.
# Only the game PID this run started is stopped (Stop-OwnGame checks its path, start time and marker).
param(
    # The machine-wide hub lock lives in the main checkout's local/ (shared by every worktree on this machine).
    [string]$HubLock = (Join-Path (Split-Path -Parent (git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir)) 'local\hub-ue.lock'),
    [string]$Lane = 'controls',
    [string]$Character = 'Save0008.sav',    # the level-8 Maya in Sanctuary
    [int]$PromptTargets = 6,                # the nearest usable things read per class
    # What to dot-source for the real-game functions. Only tests/controls_capture_test.py passes another file.
    [string]$Helpers = (Join-Path $PSScriptRoot 'realgame.ps1')
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. $Helpers
$Out = Join-Path $script:RG 'controls'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o'); status = 'running' }
function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }

$extraRoots = [ordered]@{ sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$snapshots = @{}
$stagesRequested = @('bindings', 'melee_facts', 'melee_air', 'prompts', 'melee_npc', 'page_keys', 'page_down')
$log.requested_stages = $stagesRequested
$stagesDone = New-Object Collections.Generic.List[string]; $stageProblems = New-Object Collections.Generic.List[string]; $runError = ''
function Set-Stage([string]$Stage, [bool]$Passed, [string]$Why) { if ($Passed) { $stagesDone.Add($Stage) } else { $stageProblems.Add("${Stage}: $Why") } }
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $runLock = $false; $driverOwned = $false
$marker = "-owrun=$Lane-$PID"   # an argument the game ignores; Stop-OwnGame looks for it on the command line
try {
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
    # -Exclusive re-checks the folder is absent and removes its own partial install on failure, so it is ours only once this returns.
    Install-Driver -BlockSavesAtStart -Exclusive
    $driverOwned = $true
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $proc = Start-Game @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', "-Character=$Character", $marker); $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) {
        if ($proc.HasExited) { break }
        try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 }
    }
    if (-not $ok) { Save-Shot 'controls/boot.png' | Out-Null; throw 'driver did not answer (check Steam console_log for AppError_5)' }
    $log.block_saves = Py 'print(block_saves())'
    if ($log.block_saves -notmatch 'True|block') { throw "saves not blocked: $($log.block_saves)" }
    Start-Sleep 5; Focus-Game | Out-Null; Send-Key Enter; Start-Sleep 45
    $map = ''
    for ($i = 0; $i -lt 6 -and $map -notmatch 'WillowPlayerPawn'; $i++) {   # the level load can hold the command channel
        try { $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)' 30 }
        catch { Start-Sleep 10 }
    }
    "map: $map"; $log.map = $map
    if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'controls/load_failed.png' | Out-Null; throw "no player pawn: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 10   # let the streamed levels spawn
    Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/controls_probe.py') controls | Out-Null

    $bindings = Py 'print(json.dumps(cp_bindings()))' 60
    $log.bindings = $bindings; "bindings: $bindings"
    Set-Stage 'bindings' ($bindings -match '"key"') 'no binding read'
    $facts = Py 'print(json.dumps(cp_melee_facts(), default=str))' 60
    $log.facts = $facts; "facts: $facts"
    Set-Stage 'melee_facts' ($facts -match '"definition"') 'no definition read'
    $log.hooks = Py 'print(json.dumps(cp_melee_hooks()))' 60

    # The melee press: empty air first (the swing's own timing), then three presses 0.3 s apart (does a press in a swing count).
    Focus-Game | Out-Null
    Send-Key V; Start-Sleep 3
    Save-Shot 'controls/after_melee.png' | Out-Null
    Send-Key V; Start-Sleep -Milliseconds 300; Send-Key V; Start-Sleep -Milliseconds 300; Send-Key V; Start-Sleep 3
    Py "print(cp_rows('melee_air'))" | Out-Null
    $count = Py 'print(sum(1 for r in cp_rows() if r["fn"].endswith("MeleeAttack")))'
    Set-Stage 'melee_air' ([int]$count -gt 0) 'no melee call seen for the V presses'

    # Each candidate: stand before it, read the prompt with a screenshot.
    $targets = @(Py 'print(json.dumps(cp_targets()))' 60 | ConvertFrom-Json)
    $picked = @($targets | Group-Object class | ForEach-Object { $_.Group | Select-Object -First $PromptTargets })
    $n = 0; $reads = @()
    foreach ($t in $picked) {
        $n++
        Py "print(cp_prompt(r'$($t.path)'))" | Out-Null
        Start-Sleep 1
        $reads += (Py "print(json.dumps(cp_prompt_read(r'$($t.path)'), default=str))")
        Save-Shot "controls/prompt_$n.png" | Out-Null
    }
    $reads | Set-Content (Join-Path $Out 'prompts.jsonl')
    "prompts: $($reads.Count)"; $log.prompts = $reads.Count
    Set-Stage 'prompts' ($reads.Count -gt 0) 'no usable thing to read'

    # A melee press with the nearest pawn close ahead: whether a stock swing reaches something friendly.
    $npc = @($targets | Where-Object { $_.class -eq 'WillowAIPawn' } | Select-Object -First 1)
    if ($npc) {
        Py "print(cp_prompt(r'$($npc[0].path)', 150.0))" | Out-Null
        Start-Sleep 1; Focus-Game | Out-Null; Send-Key V; Start-Sleep 3
        Py "print(cp_rows('melee_npc'))" | Out-Null
    }
    Set-Stage 'melee_npc' ($npc.Count -gt 0) 'no pawn to stand before'

    # M and L on each open page: open it with its stock key, press one, read and capture, close. Tab and Esc are the page's own business.
    $pageReads = @()
    foreach ($page in 'M', 'L', 'I', 'K') {
        foreach ($key in 'M', 'L') {
            Focus-Game | Out-Null
            Send-Key $page; Start-Sleep 2
            $opened = Py 'print(json.dumps(cp_page(), default=str))' 15
            Send-Key $key; Start-Sleep 2
            $after = Py 'print(json.dumps(cp_page(), default=str))' 15
            Save-Shot "controls/page_${page}_then_${key}.png" | Out-Null
            $pageReads += "{`"page`": `"$page`", `"key`": `"$key`", `"opened`": $opened, `"after`": $after}"
            Send-Key Esc; Start-Sleep 2
            Send-Key Esc; Start-Sleep 1   # if the key left a page open under the first close
            Save-Shot "controls/page_${page}_then_${key}_closed.png" | Out-Null
        }
    }
    $pageReads | Set-Content (Join-Path $Out 'page_keys.jsonl')
    Set-Stage 'page_keys' ($pageReads.Count -eq 8) 'not every page and key was read'
    Py "print(cp_rows('page_keys'))" | Out-Null

    # Last: Maya then bleeds out. Whether the map, the missions and a swing work while she is down.
    $injured = ''
    for ($i = 0; $i -lt 8 -and $injured -notmatch '"injured": true'; $i++) { $injured = Py 'print(json.dumps(cp_injure(), default=str))' 15; Start-Sleep 1 }
    $log.injure = $injured; "injure: $injured"
    if ($injured -match '"injured": true') {
        $downReads = @()
        foreach ($key in 'M', 'L') {
            Focus-Game | Out-Null
            Send-Key $key; Start-Sleep 1
            $downReads += "{`"key`": `"$key`", `"after`": $(Py 'print(json.dumps(cp_page(), default=str))' 15)}"
            Save-Shot "controls/down_$key.png" | Out-Null
            Send-Key Esc; Start-Sleep 1
        }
        Send-Key V; Start-Sleep 1
        Save-Shot 'controls/down_V.png' | Out-Null
        $downReads | Set-Content (Join-Path $Out 'page_down.jsonl')
        Py "print(cp_rows('page_down'))" | Out-Null
    }
    Set-Stage 'page_down' ($injured -match '"injured": true') "she did not go down: $injured"
    Py 'print(cp_unhook())' | Out-Null
    try { Invoke-GamePy 'get_pc().ConsoleCommand("quit", False)' 10 | Out-Null } catch { "quit: no reply ($_); waiting for the exit" }
    for ($i = 0; $i -lt 30 -and -not $proc.HasExited; $i++) { Start-Sleep 2 }
} catch {
    $runError = "$_"
    "run error: $runError"
} finally {
    # Each step is isolated so a failure in one (a log write) cannot skip the restore. Order: stop our game, and only once it has
    # exited remove our driver and restore and verify the roots; then write the logs and release the locks.
    $cleanup = [ordered]@{}
    $gameGone = $true
    if ($proc) {
        try {
            for ($i = 0; $i -lt 10 -and !$proc.HasExited; $i++) { Start-Sleep 1 }
            $cleanup.game = if ($proc.HasExited) { 'exited' } else { Stop-OwnGame $proc $procStart $marker }
            if ($cleanup.game -eq 'stopped' -and !$proc.WaitForExit(15000)) { $cleanup.game = 'stopped, but still running after 15 s' }
            $proc.Refresh(); $gameGone = $proc.HasExited
        } catch { $cleanup.game = "error: $_"; $gameGone = $false }
    }
    $roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
    foreach ($d in $extraRoots.Keys) { $roots[$d] = $extraRoots[$d] }
    # The recovery commands name only roots whose backup is still good.
    $makeRestoreLines = { param($Names) (@($Names) | ForEach-Object { "     Restore-Tree '$(Join-Path $bak $_)' '$($roots[$_])' '$_'" }) -join "`n" }
    $verifiedNames = @($roots.Keys | Where-Object { $snapshots[$_] -is [hashtable] })
    $restoreLines = & $makeRestoreLines $verifiedNames
    $openShell = "In a new PowerShell window: `$env:OPENWILLOW_BL2 = '$env:OPENWILLOW_BL2'; . '$PSScriptRoot\realgame.ps1'"
    $rowRule = "Every row must read 'added, removed', 'missing, restored' or 'changed, restored'; a 'MISMATCH after restore' row means it failed."
    $recovery = ''
    $holdLocks = $false
    $restoring = [bool]$bak
    if (!$gameGone) {
        # A live game could overwrite restored files: nothing is removed or restored, and the locks stay held.
        $cleanup.driver = "NOT REMOVED (game exit not confirmed): $script:DriverDir"
        $cleanup.saves = "NOT RESTORED (game exit not confirmed); backup kept at $bak"
        $recovery = "game exit not confirmed ($($cleanup.game))"
        $holdLocks = $true; $restoring = $false; $driverOwned = $false
        @"
MANUAL RECOVERY: Borderlands 2 (pid $($proc.Id)) did not confirm its exit ($($cleanup.game)). Nothing was removed or restored.
  1. Close Borderlands 2 yourself.
  2. $openShell
     Remove-Item -Recurse -Force '$script:DriverDir'
$restoreLines
     $rowRule
  3. Only when no row reports a mismatch, remove the locks this run left: '$HubLock' (if it names '$Lane') and '$script:Lock'.
"@
    }
    try {
        if ($driverOwned) {
            Remove-Driver
            $cleanup.driver = if (Test-Path -LiteralPath $script:DriverDir) { "STILL PRESENT: $script:DriverDir" } else { 'removed' }
        } elseif (!$cleanup.Contains('driver')) { $cleanup.driver = 'not installed by this run; untouched' }
    } catch { $cleanup.driver = "error: $_" }
    $rows = @(); $changedRoots = @()
    if ($restoring) {
        try {
            $restored = Restore-VerifiedRoots $bak $roots $snapshots
            $rows = @($restored.rows); $cleanup.saves = $restored.summary; $changedRoots = @($restored.changed | Where-Object { $_ })
            if (!$restored.clean) { $holdLocks = $true; $recovery = "restore did not compare clean ($($restored.summary))" }
        } catch {
            $cleanup.saves = "RESTORE FAILED: $_ (backup kept at $bak)"
            $holdLocks = $true; $recovery = "restore failed ($_)"
        }
    }
    if (!$recovery -and $cleanup.driver -match '^(STILL PRESENT|error)') { $recovery = "driver not removed ($($cleanup.driver))" }
    try {
        $log.completed_stages = @($stagesDone); $log.stage_problems = @($stageProblems)
        # The run's own result, and the status: recovery of the maintainer's files outranks the result.
        $log.run_status = Get-RunStatus $stagesRequested @($stagesDone) $runError
        $log.status = Get-RunStatus $stagesRequested @($stagesDone) $runError $recovery
        $log.locks_held = $holdLocks
        $log.finished = (Get-Date).ToString('o'); $log.cleanup = $cleanup
        $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')
        if ($bak) { @{ backup = $bak; compared = (Get-Date).ToString('o'); changes = @($rows) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Out 'save_compare.json') }
    } catch { "log write failed: $_" }
    # Held locks stop another run starting on files that still need recovery.
    if (!$holdLocks) {
        try { if ($runLock) { Exit-RunLock } } catch { "run lock release failed: $_" }
        try { if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force } } catch { "hub lock release failed: $_" }
    }
    "status: $($log.status)"
    $cleanup.GetEnumerator() | ForEach-Object { "cleanup $($_.Key): $($_.Value)" }
    Format-RestoreRows $rows   # as text, so the rows and the steps below reach the screen before `exit`
    if ($holdLocks -and $gameGone) {
        if ($changedRoots.Count) { "Do NOT restore $($changedRoots -join ', '): the backup changed after it was verified. Compare it with the live files by hand." }
        $restoreLines = & $makeRestoreLines @($verifiedNames | Where-Object { $_ -notin $changedRoots })
        @"
MANUAL RECOVERY: $recovery. Both locks are held and the backup is kept at $bak.
  1. Make sure Borderlands 2 is closed. $openShell
$restoreLines
     $rowRule
  2. Only when no row reports a mismatch, remove the locks this run left: '$HubLock' (if it names '$Lane') and '$script:Lock'.
"@
    }
}
exit $(if ($log.status -eq 'completed') { 0 } else { 1 })
