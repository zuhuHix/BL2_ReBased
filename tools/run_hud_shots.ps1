param(
    [string]$Engine = 'C:\Program Files\Epic Games\UE_5.8',
    [string]$Game = $env:OPENWILLOW_BL2,
    [int]$Port = 8795,
    [int]$TimeoutSeconds = 180,
    # Maya's level (default: the slice manifest's) and -Shield: equip the first gear-manifest shield that level allows.
    [int]$Level = 0,
    [switch]$Shield,
    # -Affine: the page's 2D stand-in for the clusters' 3D turn instead of the perspective warp (A/B captures).
    [switch]$Affine,
    [int]$WaitForEditorSeconds = 1800,
    # Extra command-line switches, e.g. '-owcanvashud' for the fallback HUD.
    [string[]]$Extra = @()
)
# Gameplay-HUD captures for the HUD parity critic (docs/verification/HUD_PARITY.md): one -game run with BL2's HUD movie
# (the default HUD), the slice weapons (-owitems, as tools/run_phaselock_shots.ps1) and -owhudshots (OpenWillowMayaHUD
# captures the held weapon at rest, fires three shots, captures again and quits). Captures and the log go to
# local/hudshots/<stamp>/ (ignored). Host presentation only: nothing here is an original-game capture.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$lock = Join-Path $repo 'local/ue_run.lock'
$shots = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
$gear = Join-Path $repo 'local/items/slice/slice_manifest.json'
foreach ($file in @($gear, (Join-Path $repo 'local/ui/run/UI_HUD/harness.swf'))) {
    if (!(Test-Path -LiteralPath $file)) { throw "Missing $file (tools/weapon_slice_gear.py, gfx_to_swf.py/hud_harness_swf.py)" }
}
$level = if ($Level -gt 0) { $Level } else { [int](Get-Content -LiteralPath $gear -Raw | ConvertFrom-Json).level }
if ($Shield) { $Extra += '-owhudshield' }
$deadline = (Get-Date).AddSeconds($WaitForEditorSeconds)
while ($true) {
    try { New-Item -ItemType File -Path $lock -Value "run_hud_shots $((Get-Date).ToString('o'))" -ErrorAction Stop | Out-Null; break }
    catch { if ((Get-Date) -gt $deadline) { throw "$lock is still held" }; Start-Sleep -Seconds 30 }
}
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$outDir = Join-Path $repo "local/hudshots/$stamp"
New-Item -ItemType Directory -Force $outDir | Out-Null
$log = Join-Path $outDir 'run.log'
$server = $null; $process = $null; $code = 1
try {
    $server = Start-Process -PassThru -WindowStyle Hidden -FilePath python -ArgumentList @(
        "`"$(Join-Path $PSScriptRoot 'hud_overlay/serve.py')`"", '--port', $Port)
    Start-Sleep 2
    $url = "http://127.0.0.1:$Port/"
    Invoke-WebRequest -UseBasicParsing -TimeoutSec 5 $url | Out-Null
    if ($Affine) { $url += '?hud3d=affine' }
    $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
    $env:OPENWILLOW_SCENE = Join-Path $repo 'local/sanctuary'
    $started = Get-Date
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', "-owflashhud=$url",
        "-owitems=`"$(Split-Path -Parent $gear)`"", "-owlevel=$level", '-owhudshots') + $Extra + @(
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Host "Launched HUD shots pid $($process.Id); log: $log"
    $finished = $process.WaitForExit($TimeoutSeconds * 1000)
    if (Test-Path -LiteralPath $shots) {
        Get-ChildItem -LiteralPath $shots -Filter 'OWHud_*.png' | Where-Object { $_.LastWriteTime -ge $started } |
            ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $outDir -Force }
    }
    # A run that timed out fails here without the checker; a finished one is checked (its exit code is ours).
    $code = 1
    if ($finished) {
        $checkArgs = @((Join-Path $PSScriptRoot 'check_hud_shots.py'), $log, '--shots', $outDir)
        if ($Shield) { $checkArgs += '--expect-shield' }
        & python @checkArgs
        $code = $LASTEXITCODE
    }
    Write-Host "Captures and log: $outDir (exit $code)"
} finally {
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    if ($server -and !$server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue
}
exit $code
