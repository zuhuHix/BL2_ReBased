param(
    [string]$Engine = 'C:\Program Files\Epic Games\UE_5.8',
    [string]$Game = $env:OPENWILLOW_BL2,
    [string[]]$Guns = @('slice_pistol', 'slice_shotgun', 'slice_smg', 'infinity_1', 'rl_bandit', 'ar_torgue_gyrojet',
        'sg_torgue', 'pistol_maliwan_shock', 'pistol_maliwan_corrosive', 'pistol_maliwan_slag', 'ar_bandit_fire', 'sg_jakobs',
        'pistol_tediore', 'ar_dahl_burst3'),
    [int]$TimeoutSeconds = 1500,
    [int]$WaitForEditorSeconds = 3600,
    [switch]$SkipBuild
)
# Unattended firing test in the host (Sanctuary_P, -owfiringtest): each named gun is equipped, fired at the stand-in dummy
# (five pulls), at a wall (three) and, when its firing block has a status block, at the dummy again (twenty); a Tediore gun
# is then reloaded with 5 rounds left (the throw and the refill are checked). The host logs
# one OWFIRING result line per gun; tools/check_weapon_firing.py grades them (PASS/FAIL table, exit code).
# Builds the module first (Build.bat, like tools/run_ue_level.ps1; -SkipBuild reuses the binary). Owns one editor at a time
# and the shared lock local/ue_run.lock: it waits while another UnrealEditor or the lock exists and never touches them.
# Host behaviour only: the firing rules are UNVERIFIED in game. Log: local/firing/run-<stamp>.log.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Game) { throw 'Set OPENWILLOW_BL2 or pass -Game (the installed Borderlands 2 folder).' }
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$buildScript = Join-Path $Engine 'Engine/Build/BatchFiles/Build.bat'
$lock = Join-Path $repo 'local/ue_run.lock'
$checker = Join-Path $repo 'tools/check_weapon_firing.py'
# The host's recipe folders in load order: -owitems first, then -owitemdir (the first recipe with a given id wins).
$itemDirs = @((Join-Path $repo 'local/items'), (Join-Path $repo 'local/items/slice'), (Join-Path $repo 'local/items/firing/coverage'))
foreach ($dir in $itemDirs) { if (!(Test-Path -LiteralPath $dir)) { throw "Missing recipe folder $dir (tools/weapon_stats.py, weapon_slice_gear.py, weapon_firing.py)" } }
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
# Take the lock as soon as no editor runs and nobody holds it (CreateNew is the atomic claim).
$deadline = (Get-Date).AddSeconds($WaitForEditorSeconds)
$stream = $null
while (!$stream) {
    if (!(Get-Process UnrealEditor -ErrorAction SilentlyContinue)) {
        try { $stream = [System.IO.File]::Open($lock, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read) } catch { $stream = $null }
    }
    if (!$stream) {
        if ((Get-Date) -gt $deadline) { throw 'Another Unreal editor or local/ue_run.lock is still present; leave it untouched.' }
        Start-Sleep -Seconds 5
    }
}
$bytes = [System.Text.Encoding]::UTF8.GetBytes("test_weapon_firing $((Get-Date).ToString('o'))")
$stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
$code = 2
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
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
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
    Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue
}
exit $code
