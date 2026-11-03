# Slice projectiles lane ground truth (Maya, Save0008): one real-game session that samples, per frame, the player's
# accuracy pool, the view kick and the view rotation around single shots, held fire and zoomed fire of the slice guns, logs
# every AddSpread / GetAdjustedAimFor / AddWeaponKick call (scripts/projectiles_probe.py), and in Three Horns Divide (Ice_P)
# fires the Maliwan fire pistol at a bullymong set to Maya's level for the equal-level status chance (fp3 hooks). Takes the
# screen, keyboard and mouse. Everything written is game data under ignored local/realgame/pj/. Locks, save backup, restore
# and the own-game-only stop are the weapon_parity2_capture.ps1 ones, unchanged.
param(
    # Machine-wide hub lock: OPENWILLOW_HUB_LOCK, else local\hub-ue.lock in the main checkout (resolved below).
    [string]$HubLock = $env:OPENWILLOW_HUB_LOCK,
    [string]$Lane = 'projectiles-rg',
    [string]$Character = 'Save0008.sav',
    [string[]]$Guns = @('slice_pistol', 'slice_smg', 'slice_assault_rifle', 'slice_shotgun'),
    [string]$StatusGun = 'status_maliwan_fire',
    [int]$StatusPresses = 120,
    # Only the status part (Ice_P); the guns' pool and kick runs are skipped.
    [switch]$StatusOnly
)
$ErrorActionPreference = 'Stop'
if (!$HubLock) {
    # The main checkout (the parent of git's common dir), so every worktree of this repository finds the same lock file.
    $common = git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir 2>$null
    $main = if ($LASTEXITCODE -eq 0 -and $common) { Split-Path -Parent $common.Trim() } else { Split-Path -Parent (Split-Path -Parent $PSScriptRoot) }
    $HubLock = Join-Path $main 'local\hub-ue.lock'
}
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'pj'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o'); guns = [ordered]@{} }

function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }
function PyJson([string]$Expr, [int]$Timeout = 30) {
    $r = Py "import json; print(json.dumps($Expr, default=str))" $Timeout
    try { $r | ConvertFrom-Json } catch { $r }
}
# A string as a JSON string literal, which Python reads as the same str: arguments never become source code.
function Q([string]$Text) { ConvertTo-Json -InputObject $Text -Compress }
# Gun ids name local recipe files and output folders: letters, digits and underscores only.
foreach ($id in $Guns) { if ($id -notmatch '^[A-Za-z0-9_]+$') { throw "gun id '$id' is not of the form [A-Za-z0-9_]+" } }
function Right-Hold([int]$Ms) { [OWRG.Native]::mouse_event(0x8, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds $Ms }
function Right-Release { [OWRG.Native]::mouse_event(0x10, 0, 0, 0, [UIntPtr]::Zero) }
function Level-View { Py 'pc = get_pc(); r = pc.Rotation; pc.Rotation = unrealsdk.make_struct("Rotator", Pitch=0, Yaw=(r.Yaw + 16384) & 0xFFFF, Roll=0); print("ok")' | Out-Null }

# Everything the game or the SDK could rewrite: the saves, the config and two SDK stores (an earlier session saw Sanity Saver
# rewrite one). $rootState records, per root, whether it existed at the start and whether its backup was verified.
$roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config')
    sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$rootState = [ordered]@{}
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $target = $null; $runLock = $false
# One id per run: the game's command-line marker (Stop-OwnGame checks it with the image path and the start time before
# stopping a PID; unknown switches are ignored by the game), the driver folder's owner file and the backup folder name.
$runId = "$Lane-$PID-$((Get-Date).ToString('yyyyMMddHHmmssfff'))"
$marker = "-owrun=$runId"
# The driver folder is this run's only while its owner file holds $runId (New-Item without -Force refuses an existing file, so of two runs racing
# for the folder only one can own it); the cleanup removes no other folder.
$ownerFile = Join-Path $script:DriverDir '.owrun'
function Test-DriverOwned { try { (Test-Path -LiteralPath $ownerFile) -and ([IO.File]::ReadAllText($ownerFile) -eq $runId) } catch { $false } }
try {
    # Preflight before any lock or backup: bl-sdk present, our driver folder not already there.
    if (!(Test-Path -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods'))) { throw "bl-sdk is not installed (no sdk_mods in $env:OPENWILLOW_BL2)" }
    if (Test-Path -LiteralPath $script:DriverDir) { throw "$script:DriverDir already exists; this run did not create it and leaves it alone" }
    while (!$hub) {
        try { $hub = New-Item -ItemType File -Path $HubLock -Value "$Lane $((Get-Date).ToString('o'))" -ErrorAction Stop }
        catch { "hub lock held by '$(Get-Content $HubLock -Raw -ErrorAction SilentlyContinue)'; retry in 60 s"; Start-Sleep 60 }
    }
    # Another lane may drive the game without the hub lock: wait for its game to exit (never touch it), up to 90 min.
    for ($i = 0; (Get-GameProcess) -and $i -lt 540; $i++) { if ($i % 30 -eq 0) { "Borderlands2 is running (not ours); waiting $(Get-Date -Format HH:mm)" }; Start-Sleep 10 }
    if (Get-GameProcess) { throw 'Borderlands2 still running after 90 min; not touching it' }
    Start-Sleep 20   # let the other lane's restore and driver removal finish
    Enter-RunLock 600; $runLock = $true
    # Backup: every root is copied to a staging folder, compared file by file (SHA-256) with the live tree, and only then
    # promoted and marked restorable. The cleanup restores only verified roots (Restore-Tree deletes live files that are
    # missing from the backup, so a partial backup must never reach it).
    $bak = Join-Path $script:RG "save-backup-$runId"; $log.backup = $bak
    New-Item -ItemType Directory -Force $bak | Out-Null
    foreach ($sub in $roots.Keys) {
        $live = $roots[$sub]; $st = [ordered]@{ present = (Test-Path -LiteralPath $live); verified = $false }; $rootState[$sub] = $st
        if (!$st.present) { continue }
        $staging = Join-Path $bak "$sub.staging"
        try {
            Copy-Item -Recurse -LiteralPath $live $staging
            $diff = @(Compare-TreeHashes $live $staging)
            if ($diff.Count) { throw "backup of $sub differs from the live tree in $($diff.Count) files ($($diff[0]), ...); not starting" }
            Rename-Item -LiteralPath $staging $sub
        } catch {
            # A failed copy, compare or promotion leaves no staging output behind (this run's folder only).
            Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
            throw
        }
        $st.verified = $true; $st.files = @(Get-ChildItem -LiteralPath (Join-Path $bak $sub) -Recurse -File -Force).Count
    }
    $rootState | ConvertTo-Json -Depth 3 | Set-Content (Join-Path $bak 'backup_complete.json')
    "backup $bak ($(@($rootState.Keys | Where-Object { $rootState[$_].verified }).Count) roots verified)"
    # Claim the driver folder: it must not exist (New-Item without -Force fails then), nor may its owner file (same rule, as
    # for the hub lock). Install-Driver then fills the folder this run owns (not -Exclusive: the folder exists by now).
    New-Item -ItemType Directory -Path $script:DriverDir -ErrorAction Stop | Out-Null
    New-Item -ItemType File -Path $ownerFile -Value $runId -ErrorAction Stop | Out-Null
    Install-Driver
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $proc = Start-Game @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', "-Character=$Character", $marker); $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'pj/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Py 'print(block_saves())'
    Start-Sleep 5; Focus-Game | Out-Null; Send-Key Enter; Start-Sleep 45
    $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
    "map: $map"; if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'pj/load_failed.png' | Out-Null; throw "no player pawn: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 1
    foreach ($f in 'weapon_firing_probe.py', 'phaselock.py', 'phaselock_matched.py', 'weapon_parity2_probe.py', 'projectiles_probe.py') {
        Invoke-GamePyFile (Join-Path $PSScriptRoot "scripts/$f") pj | Out-Null
    }
    $log.protect = Py 'print(len(protect_player(True)))'
    function Wait-Auto([double]$Seconds) {
        $deadline = (Get-Date).AddSeconds($Seconds + 20)
        while ((Get-Date) -lt $deadline) { Start-Sleep 1; $st = PyJson 'wp2_autofire_state()'; if ($st.done) { break } }
        PyJson 'wp2_stop_autofire()' | Out-Null
    }
    # Part 1 (Sanctuary, a wall in front): pool, kick and spread per slice gun.
    foreach ($gun in $(if ($StatusOnly) { @() } else { $Guns })) {
        $g = [ordered]@{}; $log.guns[$gun] = $g
        try {
            $g.spawn = PyJson "fp_spawn($(Q $gun))" 60
            $g.ready = PyJson "fp_ready($(Q $gun), 1)" 60
            Focus-Game | Out-Null; Send-Key '1'; Start-Sleep 2   # draw quick slot 1 (fp_ready only fills it)
            $g.held = Py 'w = get_pc().Pawn.Weapon; print(w.DefinitionData.WeaponTypeDefinition._path_name() if w else None)'
            $g.fields = PyJson 'pj_fields()' 60
            PyJson 'wp2_infinite_ammo(True)' | Out-Null
            $g.wall = PyJson 'fp_face_wall()' 60
            Focus-Game | Out-Null; Start-Sleep 2
            # (a) single shots 1.5 s apart: the pool's floor, the per-shot impulse, hold and decay; kick and its return.
            PyJson "pj_start($(Q "${gun}_single"))" | Out-Null
            PyJson 'wp2_autofire(8, 0.06, 1.5)' | Out-Null; Wait-Auto 16; Start-Sleep 2
            $g.single = PyJson 'pj_stop()' 60
            # (b) held fire for 3 s, then 3 s of rest: bloom toward the maximum and the recovery.
            PyJson "pj_start($(Q "${gun}_hold"))" | Out-Null
            PyJson 'wp2_autofire(1, 3.0, 0.1)' | Out-Null; Wait-Auto 4; Start-Sleep 3
            $g.hold = PyJson 'pj_stop()' 60
            # (c) the same, zoomed: the zoom effect on the cone and the kick.
            PyJson "pj_start($(Q "${gun}_zoom"))" | Out-Null
            [OWRG.Native]::mouse_event(0x8, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 800
            PyJson 'wp2_autofire(1, 2.0, 0.1)' | Out-Null; Wait-Auto 3
            Right-Release; Start-Sleep 3
            $g.zoom = PyJson 'pj_stop()' 60
            # (d) many separate presses for the AddSpread density (pellets x presses draws).
            $n = if ($gun -eq 'slice_shotgun') { 40 } else { 120 }
            $off = if ($gun -eq 'slice_shotgun') { 1.1 } else { 0.3 }
            PyJson "pj_start($(Q "${gun}_spread"))" | Out-Null
            PyJson "wp2_autofire($n, 0.06, $off)" | Out-Null; Wait-Auto ($n * ($off + 0.1)); Start-Sleep 1
            $g.spread = PyJson 'pj_stop()' 60
            PyJson 'wp2_infinite_ammo(False)' | Out-Null
        } catch { $g.error = "$_"; "gun $gun failed: $_"; try { Py 'wp2_stop_autofire(); pj_stop(); print("ok")' | Out-Null } catch {} }
        "gun $gun done: $($g | ConvertTo-Json -Compress -Depth 3)"
        $log | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $Out 'session.json')
    }
    # Part 2 (Ice_P): the equal-level status chance.
    Py 'get_pc().ConsoleCommand("open Ice_P", False); print("sent")' | Out-Null
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep 3
        try { $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn is not None)' 8 } catch { continue }
        if ($map -match 'Ice_P True') { break }
    }
    "map: $map"; if ($map -notmatch 'Ice_P True') { throw "travel to Ice_P failed: $map" }
    Start-Sleep 8; Send-Key Enter; Start-Sleep 1
    for ($i = 0; $i -lt 8 -and !$target; $i++) {
        $pick = Py @'
import json
rows = []
for d, n, c, dead in list_ai():
    p = ai_pawn(n)
    cls = p.AIClass._path_name() if p.AIClass else ""
    rows.append({"name": n, "dist": d, "class": cls.split(".")[-1], "dead": dead, "ctrl": p.Controller is not None})
ok = [r for r in rows if not r["dead"] and r["ctrl"] and "PrimalBeast" in r["class"]]
ok.sort(key=lambda r: (not r["class"].endswith("CharClass_PrimalBeast"), r["dist"]))
print(json.dumps({"pick": ok[0]["name"] if ok else None, "rows": rows[:30]}))
'@
        $res = $pick | ConvertFrom-Json; $log.candidates = $res.rows; $target = $res.pick
        if (!$target) { Start-Sleep 5 }
    }
    if (!$target) { throw "no bullymong: $pick" }
    $log.target = $target
    $log.protect2 = Py 'print(len(protect_player(True)))'
    $log.target_setup = PyJson "fp3_target($(Q $target), 500.0)" 60
    $mayaLevel = Py 'print(get_pc().Pawn.GetExpLevel())'
    $log.maya_level = $mayaLevel
    $log.level_set = PyJson "wp2_try_level($(Q $target), $([int]$mayaLevel))" 60
    $log.hooks_fp3 = PyJson 'fp3_install_hooks()'
    "target $target maya $mayaLevel level set $($log.level_set | ConvertTo-Json -Compress)"
    $s = [ordered]@{}; $log.status = $s
    try {
        $s.spawn = PyJson "fp_spawn($(Q $StatusGun))" 60
        $s.ready = PyJson "fp_ready($(Q $StatusGun), 1)" 60
        Focus-Game | Out-Null; Send-Key '1'; Start-Sleep 2   # draw quick slot 1 (fp_ready only fills it)
        $s.held = Py 'w = get_pc().Pawn.Weapon; print(w.DefinitionData.WeaponTypeDefinition._path_name() if w else None)'
        "status gun held: $($s.held)"
        if ($s.held -notmatch 'Maliwan') { throw "status gun not in hand: $($s.held)" }
        PyJson 'wp2_infinite_ammo(True)' | Out-Null
        $s.run = PyJson "fp3_run_begin($(Q "status_$StatusGun"), {'presses': $StatusPresses})"
        PyJson 'fp3_aim_target(0.0, True)' | Out-Null
        Start-Sleep 1
        PyJson "wp2_autofire($StatusPresses, 0.08, 0.35)" | Out-Null
        Wait-Auto ($StatusPresses * 0.55); Start-Sleep 4
        $s.run_end = PyJson 'fp3_run_end()'
        $s.level_after = PyJson "wp2_try_level($(Q $target), $([int]$mayaLevel))" 60
        PyJson 'wp2_infinite_ammo(False)' | Out-Null
    } catch { $s.error = "$_"; "status run failed: $_"; try { Py 'wp2_stop_autofire(); fp3_run_end(); print("ok")' | Out-Null } catch {} }
    $log | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $Out 'session.json')
    $log.cleanup = PyJson 'fp_cleanup()' 60
    Py 'pj_stop(write=False); wp2_remove_hooks(); fp3_remove_hooks(); protect_player(False); get_pc().ConsoleCommand("quit", False)' | Out-Null
    for ($i = 0; $i -lt 20 -and -not $proc.HasExited; $i++) { Start-Sleep 2 }
} finally {
    # Order: stop our game; only once it has exited, remove our driver and restore and verify the saves, config and SDK stores
    # while both locks are still held; then write the logs; release the locks last, each in its own step. If the game's
    # exit is not confirmed nothing is removed or restored and the locks stay held (manual recovery printed).
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
    # Our PID having exited proves nothing about another game (a lane without the hub lock): any running Borderlands2 stops
    # every destructive step below (restore, removal of created roots, driver removal) and the locks stay held.
    $otherGame = $null
    try { $otherGame = Get-GameProcess } catch { $otherGame = 'unknown (Get-GameProcess failed)' }
    if (!$gameGone -or $otherGame) {
        $why = if (!$gameGone) { "Borderlands 2 (pid $($proc.Id)) did not confirm its exit ($($cleanup.game))" } else { "ANOTHER GAME RUNNING (pid $($otherGame.Id))" }
        $cleanup.saves = "$why`: manual recovery needed, backup at $bak"
        "MANUAL RECOVERY: $why. Nothing was removed or restored;"
        "  close it, remove '$script:DriverDir' if its .owrun file reads '$runId', run Restore-Tree from each verified folder of '$bak' (realgame.ps1), then remove '$HubLock' and '$script:Lock'."
        $bak = $null; $ownerFile = $null; $runLock = $false; $hub = $null   # skip the steps below; locks stay held on purpose
    }
    try {
        if ($ownerFile -and (Test-DriverOwned)) {
            Remove-Driver
            $cleanup.driver = if (Test-Path -LiteralPath $script:DriverDir) { "STILL PRESENT: $script:DriverDir" } else { 'removed' }
        } elseif (!$cleanup.Contains('driver')) { $cleanup.driver = 'not installed by this run; untouched' }
    } catch { $cleanup.driver = "error: $_" }
    $rows = @()
    if ($bak) {
        foreach ($sub in $rootState.Keys) {
            $st = $rootState[$sub]; $b = Join-Path $bak $sub; $live = $roots[$sub]
            try {
                if (!$st.present) {
                    # Absent at the start (an SDK store this run's game created): removed again; still there is a problem.
                    if (Test-Path -LiteralPath $live) {
                        Remove-Item -LiteralPath $live -Recurse -Force -ErrorAction SilentlyContinue
                        $rows += [pscustomobject]@{ file = $sub; status = $(if (Test-Path -LiteralPath $live) { 'CREATED DURING THE RUN, STILL PRESENT' } else { 'created during the run, removed' }) }
                    }
                } elseif ($st.verified) {
                    # Restore-Tree removes added files, restores missing and changed ones, then compares every hash again.
                    $rows += @(Restore-Tree $b $live $sub)
                } else {
                    $rows += [pscustomobject]@{ file = $sub; status = 'NO VERIFIED BACKUP: not restored' }
                }
            } catch { $rows += [pscustomobject]@{ file = $sub; status = "RESTORE FAILED: $_" } }
        }
        # A root the backup never reached (the run stopped before it) has no state: report it, never call it identical.
        foreach ($sub in $roots.Keys) { if (!$rootState.Contains($sub)) { $rows += [pscustomobject]@{ file = $sub; status = 'NO VERIFIED BACKUP: backup did not reach it' } } }
        $problem = 'MISMATCH*', 'RESTORE FAILED*', 'NO VERIFIED BACKUP*', 'CREATED DURING THE RUN, STILL PRESENT'
        $bad = @($rows | Where-Object { $status = $_.status; @($problem | Where-Object { $status -clike $_ }).Count })
        $cleanup.saves = if ($bad.Count) { "NOT IDENTICAL to $bak ($($bad.Count) problems)" } else { "identical to $bak after $($rows.Count) restores" }
    }
    try {
        $log.finished = (Get-Date).ToString('o'); $log.cleanup = $cleanup
        $log | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $Out 'session.json')
        if ($bak) { @{ backup = $bak; compared = (Get-Date).ToString('o'); changes = @($rows) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Out 'save_compare.json') }
    } catch { "log write failed: $_" }
    try { if ($runLock) { Exit-RunLock } } catch { "run lock release failed: $_" }
    try { if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force } } catch { "hub lock release failed: $_" }
    $cleanup.GetEnumerator() | ForEach-Object { "cleanup $($_.Key): $($_.Value)" }
    $rows
}
