# Weapons parity round 2 ground truth (Maya, Save0008): one real-game session that reads the world FOV through the HUD
# canvas (Sanctuary), the elemental explosion collection and the firing definition classes, then in Three Horns Divide
# (Ice_P) per gun: the muzzle-flash / casing particle systems, hip / ADS / reload / muzzle frames for the slice guns, and a
# fire series at a held bullymong with the status roll, emitter, HurtRadius and damage hooks (scripts/weapon_parity2_probe.py,
# scripts/weapon_firing_probe.py fp3). Takes the screen, keyboard and mouse. Everything written is game data under ignored
# local/realgame/wp2/. Locks: the shared hub lock and the run lock; saves are backed up, saving is blocked in game, and
# afterwards the save folder is compared with the backup (changed files are restored from it). Stops only the game it started.
param(
    # Machine-wide hub lock: OPENWILLOW_HUB_LOCK, else local\hub-ue.lock in the main checkout (resolved below).
    [string]$HubLock = $env:OPENWILLOW_HUB_LOCK,
    [string]$Lane = 'weapons-rg',
    [string]$Character = 'Save0008.sav',
    [string[]]$Guns = @('slice_pistol', 'slice_smg', 'slice_assault_rifle', 'slice_shotgun', 'slice_mission_pistol_fire',
                        'pistol_maliwan_shock', 'pistol_maliwan_corrosive', 'pistol_maliwan_slag', 'ar_bandit_fire',
                        'sg_torgue', 'ar_torgue_gyrojet'),
    [int]$StatusPresses = 80,
    [switch]$SkipVisuals
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
$Out = Join-Path $script:RG 'wp2'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o'); guns = [ordered]@{} }

# An empty reply is refused, never taken (run Z47: the parity2 quit wrote one as the game exited). The transport returns '' for it, so the
# test is null OR empty (ConvertTo-ReplyText, realgame.ps1; review of #263, R3).
function Py([string]$Code, [int]$Timeout = 30) { ConvertTo-ReplyText (Invoke-GamePy $Code $Timeout) $Code }
function PyJson([string]$Expr, [int]$Timeout = 30) {
    $r = Py "import json; print(json.dumps($Expr, default=str))" $Timeout
    try { $r | ConvertFrom-Json } catch { $r }
}
# A string as a JSON string literal, which Python reads as the same str: arguments never become source code.
function Q([string]$Text) { ConvertTo-Json -InputObject $Text -Compress }
# Gun ids name local recipe files and output folders: letters, digits and underscores only.
foreach ($id in $Guns) { if ($id -notmatch '^[A-Za-z0-9_]+$') { throw "gun id '$id' is not of the form [A-Za-z0-9_]+" } }
# Right button held across several steps: only with the game in front, and a caller releases it in a finally.
function Right-Hold([int]$Ms) { Assert-GameForeground; Send-MouseEvent 0x8; Start-Sleep -Milliseconds $Ms }
function Right-Release { Send-MouseEvent 0x10 }
function Level-View { Py 'pc = get_pc(); r = pc.Rotation; pc.Rotation = unrealsdk.make_struct("Rotator", Pitch=0, Yaw=(r.Yaw + 16384) & 0xFFFF, Roll=0); print("ok")' | Out-Null }

# Everything the game or the SDK could rewrite: the saves, the config and two SDK stores (an earlier session saw Sanity Saver
# rewrite one). $rootState records, per root, whether it existed at the start and whether its backup was verified.
$roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config')
    sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$rootState = [ordered]@{}
$backupHashes = @{}   # per root: the backup's file hashes as it verified; the finalizer re-checks the backup against them before any restore
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
    Assert-NoPendingRecords   # an earlier run's pending record stops this one before it takes a lock or changes anything (pending.ps1)
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
            # The copy is hashed once: the same table is compared with the live tree and kept as the baseline, so nothing that changes between
            # the two steps can become the baseline. (The rename keeps every relative path.)
            $staged = Get-TreeHashes $staging
            $diff = @(Compare-Hashes (Get-TreeHashes $live) $staged)
            if ($diff.Count) { throw "backup of $sub differs from the live tree in $($diff.Count) files ($($diff[0]), ...); not starting" }
            Rename-Item -LiteralPath $staging $sub
        } catch {
            # A failed copy, compare or promotion leaves no staging output behind (this run's folder only).
            Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
            throw
        }
        $st.verified = $true; $backupHashes[$sub] = $staged; $st.files = $staged.Count
    }
    $rootState | ConvertTo-Json -Depth 3 | Set-Content (Join-Path $bak 'backup_complete.json')
    # verified_hashes.json and this run's pending record (pending.ps1), before the driver: a kill from here on leaves a record the next run refuses on.
    $snapshots = @{}
    foreach ($sub in $rootState.Keys) { $snapshots[$sub] = if (!$rootState[$sub].present) { 'absent' } elseif ($rootState[$sub].verified) { $backupHashes[$sub] } else { 'unverified' } }
    Save-VerifiedBackupRecord $bak $roots $snapshots
    "backup $bak ($(@($rootState.Keys | Where-Object { $rootState[$_].verified }).Count) roots verified)"
    # Claim the driver folder: it must not exist (New-Item without -Force fails then), nor may its owner file (same rule, as
    # for the hub lock). Install-Driver -ClaimedBy then fills the folder this run owns, and refuses any other.
    New-Item -ItemType Directory -Path $script:DriverDir -ErrorAction Stop | Out-Null
    New-Item -ItemType File -Path $ownerFile -Value $runId -ErrorAction Stop | Out-Null
    Install-Driver -ClaimedBy $runId   # the folder this run claimed above (.owrun), and no other
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $proc = Start-Game @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', "-Character=$Character", $marker); $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'wp2/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')
    Start-Sleep 5; Assert-GameForeground; Send-Key Enter; Start-Sleep 45
    $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
    "map: $map"; if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'wp2/load_failed.png' | Out-Null; throw "no player pawn: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 1
    foreach ($f in 'weapon_firing_probe.py', 'phaselock.py', 'phaselock_matched.py', 'weapon_parity2_probe.py') {
        Invoke-GamePyFile (Join-Path $PSScriptRoot "scripts/$f") wp2 | Out-Null
    }

    # World FOV: project known view directions through the HUD canvas, at level pitch.
    Level-View; Start-Sleep 1
    $log.fov_arm = PyJson 'wp2_fov_arm()'
    Start-Sleep 2
    $log.fov = PyJson 'wp2_fov_read()'
    Save-Shot 'wp2/fov_view.png' | Out-Null
    "fov: pov $($log.fov.pov_fov) clip $($log.fov.clip)"
    $log.status_table = PyJson 'wp2_status_table()'
    $log.explosions = PyJson 'wp2_explosions()' 120
    try { $log.defs = PyJson 'fp_defs()' 300 } catch { $log.defs = "fp_defs failed: $_" }

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
    $log.protect = Py 'print(len(protect_player(True)))'
    $log.target_setup = PyJson "fp3_target($(Q $target), 500.0)" 60
    $log.hooks_fp3 = PyJson 'fp3_install_hooks()'
    $log.hooks_wp2 = PyJson 'wp2_install_hooks()'
    "target $target levels $($log.target_setup.levels | ConvertTo-Json -Compress)"

    foreach ($gun in $Guns) {
        $g = [ordered]@{}; $log.guns[$gun] = $g
        try {
            $g.spawn = PyJson "fp_spawn($(Q $gun))" 60
            $g.ready = PyJson "fp_ready($(Q $gun), 1)" 60
            Start-Sleep 3
            $g.particles = PyJson "wp2_particles($(Q $gun))" 120
            $g.held = PyJson 'fp3_damage()'
            if (!$SkipVisuals -and $gun -like 'slice_*') {
                Py 'fp3_unlock_aim(); print("ok")' | Out-Null
                Level-View; Assert-GameForeground; Start-Sleep 2
                Save-Shot "wp2/vis/$gun-hip.png" | Out-Null
                try { Right-Hold 900; Save-Shot "wp2/vis/$gun-ads.png" | Out-Null } finally { Right-Release }
                Start-Sleep 1
                $g.muzzle_burst = Invoke-Burst "wp2/vis/$gun-muzzle" 1.2 25 { Send-Click } 1.0
                Start-Sleep 1
                $g.reload_burst = Invoke-Burst "wp2/vis/$gun-reload" 4.0 60 { Send-Key R } 1.0
                Start-Sleep 1
            }
            $off = if ($gun -like 'sg_*' -or $gun -eq 'slice_shotgun') { 1.1 } else { 0.35 }
            $n = if ($gun -match 'maliwan|fire') { $StatusPresses } else { 25 }
            PyJson 'wp2_infinite_ammo(True)' | Out-Null
            $g.run = PyJson "fp3_run_begin($(Q $gun), {'presses': $n})"
            PyJson "fp3_aim_target(0.0, True)" | Out-Null
            Start-Sleep 1
            PyJson "wp2_autofire($n, 0.08, $off)" | Out-Null
            $deadline = (Get-Date).AddSeconds($n * ($off + 0.2) + 30)
            while ((Get-Date) -lt $deadline) { Start-Sleep 2; $st = PyJson 'wp2_autofire_state()'; if ($st.done) { break } }
            Start-Sleep 4   # DOT ticks and late explosions
            PyJson 'wp2_stop_autofire()' | Out-Null
            $g.run_end = PyJson 'fp3_run_end()'
            PyJson 'fp3_refill()' | Out-Null
            # A few shots at a wall: impact emitters without the pawn in the way.
            try {
                Py 'fp3_unlock_aim(); print("ok")' | Out-Null
                $g.wall = PyJson 'fp_face_wall()' 60
                PyJson "fp_run_begin($(Q "wall_$gun"))" | Out-Null
                PyJson "wp2_autofire(6, 0.08, $off)" | Out-Null
                Start-Sleep ([int](6 * ($off + 0.1) + 4))
                PyJson 'wp2_stop_autofire()' | Out-Null
                $g.wall_end = PyJson 'fp_run_end()'
            } catch { $g.wall_error = "$_" }
            PyJson 'wp2_infinite_ammo(False)' | Out-Null
        } catch { $g.error = "$_"; "gun $gun failed: $_"; try { Py 'wp2_stop_autofire(); fp_run_end(); print("ok")' | Out-Null } catch {} }
        "gun $gun done: $($g.run_end | ConvertTo-Json -Compress)"
        $log | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $Out 'session.json')
    }
    $log.fp_cleanup = PyJson 'fp_cleanup()' 60   # not 'cleanup': the finalizer writes its own record under that key
    # The quit ends the game before it can reply: an empty or missing reply is the expected result here, not an error (as in projectiles_capture.ps1).
    try { Invoke-GamePy 'wp2_remove_hooks(); fp3_remove_hooks(); protect_player(False); get_pc().ConsoleCommand("quit", False)' 30 | Out-Null } catch { "quit sent; no reply: $_" }
    for ($i = 0; $i -lt 20 -and -not $proc.HasExited; $i++) { Start-Sleep 2 }
} finally {
    # Order: stop our game; only once it has exited, remove our driver and restore and verify the saves, config and SDK stores
    # while both locks are still held; then write the logs; release the locks last, each in its own step. If the game's
    # exit is not confirmed nothing is removed or restored and the locks stay held (manual recovery printed).
    $cleanup = [ordered]@{}
    $cleanup.inputs = Release-HeldInputs   # every key and button this run still holds is let go before anything else
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
        "  close it, remove '$script:DriverDir' if its .owrun file reads '$runId', run Restore-PendingRecord '$script:PendingRecord' (realgame.ps1; it re-checks each backup before restoring), then remove '$HubLock' and '$script:Lock'."
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
                    # Absent at the start: a root that exists now was created during the run, perhaps by another writer. It is left in place and
                    # reported by the shared helper (snapshot 'absent'), never removed here; the outcome is the shared path's.
                    $rows += @((Restore-VerifiedRoots $bak @{ $sub = $live } @{ $sub = 'absent' }).rows)
                } elseif ($st.verified) {
                    # The backup is re-checked against the hashes it was verified with before anything is restored (Restore-VerifiedRoots): a backup
                    # changed since is not restored, and the record and both locks stay. Otherwise: Restore-Tree removes added files, restores missing
                    # and changed ones, then compares every hash again.
                    $rows += @((Restore-VerifiedRoots $bak @{ $sub = $live } @{ $sub = $backupHashes[$sub] }).rows)
                } else {
                    $rows += [pscustomobject]@{ file = $sub; status = 'NO VERIFIED BACKUP: not restored' }
                }
            } catch { $rows += [pscustomobject]@{ file = $sub; status = "RESTORE FAILED: $_" } }
        }
        # A root the backup never reached (the run stopped before it) has no state: report it, never call it identical.
        foreach ($sub in $roots.Keys) { if (!$rootState.Contains($sub)) { $rows += [pscustomobject]@{ file = $sub; status = 'NO VERIFIED BACKUP: backup did not reach it' } } }
        $problem = 'MISMATCH*', 'RESTORE FAILED*', 'NO VERIFIED BACKUP*', 'BACKUP CHANGED*', 'CREATED DURING THE RUN, STILL PRESENT', 'created during the run; left*'
        $bad = @($rows | Where-Object { $status = $_.status; @($problem | Where-Object { $status -clike $_ }).Count })
        $cleanup.saves = if ($bad.Count) { "NOT IDENTICAL to $bak ($($bad.Count) problems)" } else { "identical to $bak after $($rows.Count) restores" }
    }
    # This run's pending record goes only once every root is the backup again by tree hash, with no game and no driver left; otherwise it and both
    # locks stay, and the steps are printed.
    $cleanup.pending = Complete-PendingRecord ([bool]$bak)
    if ($cleanup.pending -notlike 'removed*' -and $cleanup.pending -ne 'none written') {
        $runLock = $false; $hub = $null
        if ($script:PendingRecord) { Format-PendingRecovery $script:PendingRecord (Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json) @() }
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
