param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    [string[]]$Guns = @('slice_pistol', 'slice_shotgun', 'slice_smg', 'infinity_1', 'rl_bandit', 'ar_torgue_gyrojet',
        'sg_torgue', 'pistol_maliwan_shock', 'pistol_maliwan_corrosive', 'pistol_maliwan_slag', 'ar_bandit_fire', 'sg_jakobs',
        'pistol_tediore', 'ar_dahl_burst3'),
    [int]$TimeoutSeconds = 1500,
    [int]$WaitForEditorSeconds = 3600,
    [switch]$SkipBuild,
    # Extra arguments for the game command line, e.g. -Extra '-owaimmodel' (the accuracy pool and kick on during the test).
    [string[]]$Extra = @()
)
# Unattended firing test in the host (Sanctuary_P, -owfiringtest): each named gun is equipped, fired at the stand-in dummy
# (five pulls), at a wall (three) and, when its firing block has a status block, at the dummy again (twenty); a Tediore gun
# is then reloaded with 5 rounds left (the throw and the refill are checked). The host logs
# one OWFIRING result line per gun; tools/check_weapon_firing.py grades them (PASS/FAIL table, exit code).
# Builds the module first (Build.bat, like tools/run_ue_level.ps1; -SkipBuild reuses the binary). Owns one editor at a time
# and the machine-wide UE run lock (tools/ue_lock.ps1): it waits while another UnrealEditor or the lock exists and never touches them.
# Host behaviour only: the firing rules are UNVERIFIED in game. Log: local/firing/run-<stamp>.log.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Game) { throw 'Set OPENWILLOW_BL2 or pass -Game (the installed Borderlands 2 folder).' }
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$buildScript = Join-Path $Engine 'Engine/Build/BatchFiles/Build.bat'
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
$checker = Join-Path $repo 'tools/check_weapon_firing.py'
# The host's recipe folders in load order: -owitems first, then -owitemdir (the first recipe with a given id wins).
$itemDirs = @((Join-Path $repo 'local/items'), (Join-Path $repo 'local/items/slice'), (Join-Path $repo 'local/items/firing/coverage'))
foreach ($dir in $itemDirs) { if (!(Test-Path -LiteralPath $dir)) { throw "Missing recipe folder $dir (tools/weapon_stats.py, weapon_slice_gear.py, weapon_firing.py)" } }
# Stale local data: a listed gun whose recipe has no firing block (made before tools/weapon_firing.py ran on it, or kept from
# an older checkout) is fired as a plain trace and fails on momentum. Regenerate the firing blocks first (light: Python
# only, no rerolls), then require every listed recipe that exists to have one. ar_dahl_burst3 is the fixture made below.
function Get-RecipesWithoutFiring {
    $missing = @()
    foreach ($id in $Guns) {
        $file = $itemDirs | ForEach-Object { Join-Path $_ "$id.json" } | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
        if (!$file) { continue }
        $hasFiring = & python -c "import json,sys; print(1 if json.load(open(sys.argv[1], encoding='utf-8')).get('firing') else 0)" $file
        if ($hasFiring -ne '1') { $missing += $id }
    }
    return $missing
}
$stale = @(Get-RecipesWithoutFiring)
if ($stale.Count) {
    Write-Host "Recipes without a firing block: $($stale -join ', '); regenerating the firing blocks (tools/weapon_firing_coverage.py --only-firing)"
    & python (Join-Path $repo 'tools/weapon_firing_coverage.py') --game $env:OPENWILLOW_BL2 --only-firing
    if ($LASTEXITCODE -ne 0) { throw "tools/weapon_firing_coverage.py --only-firing failed ($LASTEXITCODE)" }
    $stale = @(Get-RecipesWithoutFiring)
    if ($stale.Count) { throw "Still no firing block after regeneration: $($stale -join ', ')" }
}
# The grenade check at the end of the run needs the stock grenade mod's block (ignored local/; Python only).
$grenade = Join-Path $repo 'local/items/grenades/grenade_mod_standard_l8.json'
if (!(Test-Path -LiteralPath $grenade)) {
    & python (Join-Path $repo 'tools/grenade_mod.py') --game $env:OPENWILLOW_BL2 --level 8
    if ($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath $grenade)) { throw "tools/grenade_mod.py did not write $grenade" }
}
# Host fixture (ignored local/; built from the coverage Dahl AR): the same gun with burst_count 3, because no weapon type in
# the data has an AutomaticBurstCount above 1 (Dahl's bursts come from zoom effects the host does not apply yet). It tests
# the host's burst rule, not game data.
$fixtures = Join-Path $repo 'local/items/firing/fixtures'
New-Item -ItemType Directory -Force $fixtures | Out-Null
& python -c "import json,sys; r=json.load(open(sys.argv[1])); r['firing']['burst_count']=3; json.dump(r, open(sys.argv[2],'w'), indent=1)" `
    (Join-Path $itemDirs[2] 'ar_dahl.json') (Join-Path $fixtures 'ar_dahl_burst3.json')
if ($LASTEXITCODE -ne 0) { throw 'Could not write the burst fixture' }
$itemDirs += $fixtures
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
$code = 2
Enter-OwUeLock -Owner 'test_weapon_firing' -TimeoutSeconds $WaitForEditorSeconds
try {
    if (!$SkipBuild) {
        & $buildScript OpenWillowEditor Win64 Development "-Project=$project" -WaitMutex -NoHotReloadFromIDE
        if ($LASTEXITCODE -ne 0) { throw "UE5 build failed: $LASTEXITCODE" }
    }
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $logDir = Join-Path $repo 'local/firing'
    New-Item -ItemType Directory -Force $logDir | Out-Null
    $log = Join-Path $logDir ("run-$stamp.log")
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', '-owcombattest', '-owfiringtest',
        "-owgunids=$($Guns -join ',')", "-owitemids=$($Guns -join ',')", '-owlevel=36', "-owitems=`"$($itemDirs[0])`"",
        "-owitemdir=`"$($itemDirs[1]);$($itemDirs[2]);$($itemDirs[3])`"",
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"") + $Extra
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Host "Launched firing test pid $($process.Id) for $($Guns.Count) guns; log: $log"
    if (!$process.WaitForExit($TimeoutSeconds * 1000)) {
        # Stop only the editor this run started, and only when its command line names this run's log.
        $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($process.Id)" -ErrorAction SilentlyContinue).CommandLine
        if ($commandLine -and $commandLine.Contains($log)) {
            Stop-Process -Id $process.Id -Force
            Write-Host "Timed out after $TimeoutSeconds s; stopped pid $($process.Id) (this run's editor)."
        }
    }
    Start-Sleep -Seconds 3
    if (!(Test-Path -LiteralPath $log)) { throw "No log written: $log" }
    Select-String -LiteralPath $log -Pattern 'OWFIRING (result|skipped|warning|no wall|pull without)' |
        ForEach-Object { Write-Host ($_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '') }
    $checkArgs = @($checker, $log, '--guns', ($Guns -join ','))
    foreach ($dir in $itemDirs) { $checkArgs += @('--items', $dir) }
    & python @checkArgs
    $code = $LASTEXITCODE
    Write-Host "Log: $log"
} finally {
    Exit-OwUeLock
}
exit $code
