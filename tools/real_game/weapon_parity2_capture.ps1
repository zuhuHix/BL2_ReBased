# Weapons parity round 2 ground truth (Maya, Save0008): one real-game session that reads the world FOV through the HUD
# canvas (Sanctuary), the elemental explosion collection and the firing definition classes, then in Three Horns Divide
# (Ice_P) per gun: the muzzle-flash / casing particle systems, hip / ADS / reload / muzzle frames for the slice guns, and a
# fire series at a held bullymong with the status roll, emitter, HurtRadius and damage hooks (scripts/weapon_parity2_probe.py,
# scripts/weapon_firing_probe.py fp3). Takes the screen, keyboard and mouse. Everything written is game data under ignored
# local/realgame/wp2/. Locks: the shared hub lock and the run lock; saves are backed up, saving is blocked in game, and
# afterwards the save folder is compared with the backup (changed files are restored from it). Stops only the game it started.
param(
    [string]$HubLock = 'C:\Users\yorad\Documents\BL2_ReBased\local\hub-ue.lock',
    [string]$Lane = 'weapons-rg',
    [string]$Character = 'Save0008.sav',
    [string[]]$Guns = @('slice_pistol', 'slice_smg', 'slice_assault_rifle', 'slice_shotgun', 'slice_mission_pistol_fire',
                        'pistol_maliwan_shock', 'pistol_maliwan_corrosive', 'pistol_maliwan_slag', 'ar_bandit_fire',
                        'sg_torgue', 'ar_torgue_gyrojet'),
    [int]$StatusPresses = 80,
    [switch]$SkipVisuals
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'wp2'
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

$extraRoots = [ordered]@{ sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings'); SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') }
$hub = $null; $bak = $null; $proc = $null; $procStart = $null; $target = $null; $runLock = $false; $driverOwned = $false
# A token on the game's command line that identifies the process this run started (Stop-OwnGame checks it with the image
# path and the start time before stopping a PID). Unknown switches are ignored by the game.
$marker = "-owrun=$Lane-$PID"
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
    $bak = Backup-Saves; $log.backup = $bak; "backup $bak"
    foreach ($d in $extraRoots.Keys) { if (Test-Path $extraRoots[$d]) { Copy-Item -Recurse $extraRoots[$d] (Join-Path $bak $d) } }
    # -Exclusive re-checks that the folder is absent and removes its own partial install on failure: the folder is this
    # run's only once the call has returned.
    Install-Driver -Exclusive
    $driverOwned = $true
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $proc = Start-Game @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies', "-Character=$Character", $marker); $procStart = $proc.StartTime
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'wp2/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Py 'print(block_saves())'
    Start-Sleep 5; Focus-Game | Out-Null; Send-Key Enter; Start-Sleep 45
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
                Level-View; Focus-Game | Out-Null; Start-Sleep 2
                Save-Shot "wp2/vis/$gun-hip.png" | Out-Null
                Right-Hold 900; Save-Shot "wp2/vis/$gun-ads.png" | Out-Null; Right-Release; Start-Sleep 1
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
    $log.cleanup = PyJson 'fp_cleanup()' 60
    Py 'wp2_remove_hooks(); fp3_remove_hooks(); protect_player(False); get_pc().ConsoleCommand("quit", False)' | Out-Null
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
    if (!$gameGone) {
        "MANUAL RECOVERY: Borderlands 2 (pid $($proc.Id)) did not confirm its exit ($($cleanup.game)). Nothing was removed or restored;"
        "  close it, remove '$script:DriverDir', run Restore-Tree from each folder of '$bak' (realgame.ps1), then remove '$HubLock' and '$script:Lock'."
        $bak = $null; $driverOwned = $false; $runLock = $false; $hub = $null   # skip the steps below; locks stay held on purpose
    }
    try {
        if ($driverOwned) {
            Remove-Driver
            $cleanup.driver = if (Test-Path -LiteralPath $script:DriverDir) { "STILL PRESENT: $script:DriverDir" } else { 'removed' }
        } elseif (!$cleanup.Contains('driver')) { $cleanup.driver = 'not installed by this run; untouched' }
    } catch { $cleanup.driver = "error: $_" }
    $rows = @()
    if ($bak) {
        try {
            $roots = [ordered]@{ SaveData = (Join-Path $script:WillowDir 'SaveData'); Config = (Join-Path $script:WillowDir 'Config') }
            foreach ($d in $extraRoots.Keys) { $roots[$d] = $extraRoots[$d] }
            foreach ($sub in $roots.Keys) {
                $b = Join-Path $bak $sub; $live = $roots[$sub]
                try {
                    # Restore-Tree removes added files, restores missing and changed ones, then compares every hash again.
                    if (Test-Path -LiteralPath $b) { $rows += @(Restore-Tree $b $live $sub) }
                    elseif (Test-Path -LiteralPath $live) { $rows += [pscustomobject]@{ file = $sub; status = 'created during the run; left for the maintainer' } }
                } catch { $rows += [pscustomobject]@{ file = $sub; status = "RESTORE FAILED: $_" } }
            }
            $bad = @($rows | Where-Object { $_.status -like 'MISMATCH*' -or $_.status -like 'RESTORE FAILED*' })
            $cleanup.saves = if ($bad.Count) { "NOT IDENTICAL to $bak ($($bad.Count) problems)" } else { "identical to $bak after $($rows.Count) restores" }
        } catch { $cleanup.saves = "RESTORE FAILED: $_ (backup kept at $bak)" }
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
