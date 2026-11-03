param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    [string]$Manifest,
    # X,Y,Z[,Yaw]: put the range here instead of the host's scout (OpenWillowRange.cpp ScoutSpot).
    [string]$At,
    [int]$TimeoutSeconds = 300,
    [int]$WaitForEditorSeconds = 3600,
    [switch]$Shots,
    [switch]$SkipBuild
)
# AI-assisted (Claude). Unattended gun-range test in the host (Sanctuary_P, -owrangetest): the range is built outside the
# town (scout or -At), Maya is teleported there, and the host's driver checks placement, body and critical hits, the shield
# order, the invulnerable and passive toggles, patrol, death, ragdoll, respawn and the damage popups. It logs one
# OWRANGETEST line per step; tools/check_range.py grades them (exit code). -Shots also saves frames to local/range/shots.
# Prepare the manifest first: python tools/prepare_range.py (stock health from the VM; enemy assets from
# tools/import_phaselock_target.ps1 when imported). Builds the module first unless -SkipBuild. Owns one editor at a time and
# the machine-wide UE run lock (tools/ue_lock.ps1); it never touches another process. Host behaviour only: UNVERIFIED in game.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Game) { throw 'Set OPENWILLOW_BL2 or pass -Game (the installed Borderlands 2 folder).' }
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
if (!$Manifest) { $Manifest = Join-Path $repo 'local/range/range_manifest.json' }
if (!(Test-Path -LiteralPath $Manifest)) { throw "No range manifest at $Manifest (run python tools/prepare_range.py)" }
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$buildScript = Join-Path $Engine 'Engine/Build/BatchFiles/Build.bat'
$checker = Join-Path $repo 'tools/check_range.py'
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
$code = 2
Enter-OwUeLock -Owner 'test_range' -TimeoutSeconds $WaitForEditorSeconds
try {
    if (!$SkipBuild) {
        & $buildScript OpenWillowEditor Win64 Development "-Project=$project" -WaitMutex -NoHotReloadFromIDE
        if ($LASTEXITCODE -ne 0) { throw "UE5 build failed: $LASTEXITCODE" }
    }
    $logDir = Join-Path $repo 'local/range'
    New-Item -ItemType Directory -Force $logDir | Out-Null
    $log = Join-Path $logDir ('run-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', '-owcanvashud', '-owrangetest',
        "-owrange=`"$Manifest`"", '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    # The Fire mission's stock dummy (local/slice manifests, as tools/test_quest.ps1 uses them); without them the host
    # stands engine-shape dummies and says so.
    $slice = @{ owslice = 'local/slice/world.json'; ownpcs = 'local/slice/npc_assets.json'; owaudio = 'local/slice/audio.json' }
    $sliceFiles = $slice.Values | ForEach-Object { Join-Path $repo $_ }
    if (@($sliceFiles | Where-Object { !(Test-Path -LiteralPath $_) }).Count -eq 0) {
        foreach ($key in $slice.Keys) { $arguments += "-$key=`"$(Join-Path $repo $slice[$key])`"" }
    } else { Write-Host 'Slice manifests missing under local/slice: the range uses engine-shape dummies.' }
    if ($At) { $arguments += "-owrangeat=$At" }
    if ($Shots) { $arguments += '-owrangeshots' }
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Host "Launched range test pid $($process.Id); log: $log"
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
    Select-String -LiteralPath $log -Pattern 'OWRANGE' |
        ForEach-Object { Write-Host ($_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '') }
    & python $checker $log
    $code = $LASTEXITCODE
    Write-Host "Log: $log"
} finally {
    Exit-OwUeLock
}
exit $code
