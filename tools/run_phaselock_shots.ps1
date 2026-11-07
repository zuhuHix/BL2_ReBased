param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Out = 'local/phaselock/b2',
    [int]$TimeoutSeconds = 240,
    [int]$WaitForEditorSeconds = 900,
    # -MovieHud (test-only): BL2's HUD movie instead of the canvas HUD: starts tools/hud_overlay/serve.py on -MovieHudPort and passes -owflashhud=<url>; the run fails
    # if its log does not show the movie accepted (tools/movie_hud.ps1). Without it nothing changes.
    [switch]$MovieHud,
    [int]$MovieHudPort = 8796,
    # Extra command-line switches (e.g. '-owfov=62.15' for 77.55 degrees at 16:9, '-owfpfov=45', '-owphaselockpitch=3.5').
    [string[]]$Extra = @()
)
# AI-assisted. Captures the host Phaselock presentation on Sanctuary: Maya (-owmaya) with a slice gun, the host
# training dummy 650 uu ahead (-owcombattest), a cast at it and timed screenshots (-owphaselockshots: hit at 0.12 ...
# 6.2 s, then a miss for the fizzle hand effect). Screenshots and the log go to $Out (ignored local/). Holds
# the UE run lock (tools/ue_lock.ps1) for the one editor launch and never touches another editor. Host presentation only: nothing here is
# an original-game capture.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $repo $Out
New-Item -ItemType Directory -Force $outDir | Out-Null
$gear = Join-Path $repo 'local/items/slice/slice_manifest.json'
$itemDir = Split-Path -Parent $gear
$gearLevel = if (Test-Path -LiteralPath $gear) { [int](Get-Content -LiteralPath $gear -Raw | ConvertFrom-Json).level } else { 30 }
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
. (Join-Path $PSScriptRoot 'movie_hud.ps1')
$shots = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
$movie = $null
$movieOk = $true
Enter-OwUeLock -Owner 'run_phaselock_shots' -TimeoutSeconds $WaitForEditorSeconds -AlsoWaitFor Borderlands2
try {
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $log = Join-Path $outDir "run-$stamp.log"
    $started = Get-Date
    $hudSwitch = '-owcanvashud'
    if ($MovieHud) { $movie = Start-OwMovieHud -Port $MovieHudPort; $hudSwitch = "-owflashhud=$($movie.Url)" }
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', $hudSwitch, '-owcombattest', '-owphaselockshots',
        "-owitems=`"$itemDir`"", "-owlevel=$gearLevel") + $Extra + @(
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Host "Launched pid $($process.Id); log $log"
    if (!$process.WaitForExit($TimeoutSeconds * 1000)) {
        Write-Host 'Timed out; stopping the editor this script started.'
        Stop-Process -Id $process.Id -Force
    }
    Start-Sleep -Seconds 2
    Select-String -LiteralPath $log -Pattern 'Phaselock|OpenWillow FX|phaselock capture' | ForEach-Object { $_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '' }
    if (Test-Path -LiteralPath $shots) {
        Get-ChildItem -LiteralPath $shots -Filter 'OWPhaselock_*.png' | Where-Object { $_.LastWriteTime -ge $started } | ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $outDir ($_.BaseName + '-' + $stamp + '.png')) -Force
            Write-Host "Screenshot: $(Join-Path $outDir ($_.BaseName + '-' + $stamp + '.png'))"
        }
    }
    if ($MovieHud) { $movieOk = Test-OwMovieHudLog -Log $log -Driver 'phaselock' -RequireShot }
} finally {
    Stop-OwMovieHud $movie
    Exit-OwUeLock
}
if (!$movieOk) { exit 1 }
