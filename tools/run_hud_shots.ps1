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
    # -Down: the down-state sequence instead (-owhuddownshots with the -owcombattest dummy: down, bleed-out, respawn, second
    # wind), checked by tools/check_hud_down.py. -Tracker: the quest's unattended loop (-owquest -owquesttest) with the movie
    # HUD and a capture after each tracker change (-owhudtrackshots), checked by check_hud_down.py --mode tracker.
    [switch]$Down,
    [switch]$Tracker,
    # -Feedback: hits from the four sides of the view and a level-up (-owhudfxshots), checked by check_hud_down.py --mode feedback.
    [switch]$Feedback,
    # -StandInShield: equip a stat-less shield, take a hit, recharge (-owshieldcheck), checked by check_shield_check.py. Needs
    # local/slice/stand_in_shield.json for a pass.
    [switch]$StandInShield,
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
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
$shots = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
$gear = Join-Path $repo 'local/items/slice/slice_manifest.json'
foreach ($file in @($gear, (Join-Path $repo 'local/ui/run/UI_HUD/harness.swf'))) {
    if (!(Test-Path -LiteralPath $file)) { throw "Missing $file (tools/weapon_slice_gear.py, gfx_to_swf.py/hud_harness_swf.py)" }
}
# The localized HUD texts (down title and hint, second wind, turn-in) are optional: without them the movie keeps its own
# texts, and the -Down check's title comparison reports local data missing.
if (!(Test-Path -LiteralPath (Join-Path $repo 'local/ui/run/UI_HUD/hud_strings.json'))) {
    Write-Warning 'local/ui/run/UI_HUD/hud_strings.json is missing (python tools/prepare_hud_strings.py --game <BL2>): the movie keeps its own texts'
}
$level = if ($Level -gt 0) { $Level } else { [int](Get-Content -LiteralPath $gear -Raw | ConvertFrom-Json).level }
if ($Shield) { $Extra += '-owhudshield' }
if (@($Down, $Tracker, $Feedback, $StandInShield | Where-Object { $_ }).Count -gt 1) { throw 'Pick one of -Down, -Tracker, -Feedback, -StandInShield' }
$mode = if ($Down) { @('-owhuddownshots', '-owcombattest') } elseif ($Tracker) {
    $sliceFiles = @((Join-Path $repo 'local/doors/mover.json'), (Join-Path $repo 'local/slice/world.json'),
        (Join-Path $repo 'local/slice/npc_assets.json'), (Join-Path $repo 'local/slice/audio.json'))
    foreach ($file in $sliceFiles) { if (!(Test-Path -LiteralPath $file)) { throw "Missing $file (see tools/test_quest.ps1)" } }
    $trackSave = Join-Path $repo 'local/hudshots/track_save.json'
    if (Test-Path -LiteralPath $trackSave) { Remove-Item -LiteralPath $trackSave -Force }
    @("-owmover=`"$($sliceFiles[0])`"", "-owslice=`"$($sliceFiles[1])`"", "-ownpcs=`"$($sliceFiles[2])`"", "-owaudio=`"$($sliceFiles[3])`"",
      '-owquest', '-owquesttest', "-owquestsave=`"$trackSave`"", '-owhudtrackshots')
} elseif ($Feedback) { @('-owhudfxshots') } elseif ($StandInShield) { @('-owshieldcheck') } else { @('-owhudshots') }
$shotFilter = if ($Down) { 'OWHudDown_*.png' } elseif ($Tracker) { 'OWHudTrack_*.png' } elseif ($Feedback) { 'OWHudFx_*.png' } elseif ($StandInShield) { 'OWShield_*.png' } else { 'OWHud_*.png' }
if ($Tracker -and $TimeoutSeconds -lt 600) { $TimeoutSeconds = 600 }
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$outDir = Join-Path $repo "local/hudshots/$stamp"
New-Item -ItemType Directory -Force $outDir | Out-Null
$log = Join-Path $outDir 'run.log'
$server = $null; $process = $null; $code = 1
Enter-OwUeLock -Owner 'run_hud_shots' -TimeoutSeconds $WaitForEditorSeconds -PollSeconds 30
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
        "-owitems=`"$(Split-Path -Parent $gear)`"", "-owlevel=$level") + $mode + $Extra + @(
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Host "Launched HUD shots pid $($process.Id); log: $log"
    $finished = $process.WaitForExit($TimeoutSeconds * 1000)
    if (Test-Path -LiteralPath $shots) {
        Get-ChildItem -LiteralPath $shots -Filter $shotFilter | Where-Object { $_.LastWriteTime -ge $started } |
            ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $outDir -Force }
    }
    # A run that timed out fails here without the checker; a finished one is checked (its exit code is ours).
    $code = 1
    if ($finished) {
        $checkArgs = if ($Down) { @((Join-Path $PSScriptRoot 'check_hud_down.py'), $log, '--mode', 'down', '--shots', $outDir) }
            elseif ($Tracker) { @((Join-Path $PSScriptRoot 'check_hud_down.py'), $log, '--mode', 'tracker') }
            elseif ($Feedback) { @((Join-Path $PSScriptRoot 'check_hud_down.py'), $log, '--mode', 'feedback', '--shots', $outDir) }
            elseif ($StandInShield) { @((Join-Path $PSScriptRoot 'check_shield_check.py'), $log, '--shots', $outDir) }
            else { @((Join-Path $PSScriptRoot 'check_hud_shots.py'), $log, '--shots', $outDir) }
        if ($Shield -and -not ($Down -or $Tracker -or $Feedback)) { $checkArgs += '--expect-shield' }
        & python @checkArgs
        $code = $LASTEXITCODE
    }
    Write-Host "Captures and log: $outDir (exit $code)"
} finally {
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    if ($server -and !$server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Exit-OwUeLock
}
exit $code
