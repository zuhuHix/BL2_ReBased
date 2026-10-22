param(
    [string]$Engine = 'C:\Program Files\Epic Games\UE_5.8',
    [string]$Game = $env:OPENWILLOW_BL2,
    # Optional machine-wide lock shared by several checkouts (taken before local/ue_run.lock, released after).
    [string]$SharedLock,
    [int]$Port = 8794,
    [int]$TimeoutSeconds = 240,
    [int]$WaitForEditorSeconds = 1800,
    # Run without the Sanctuary mission (no Marcus, so no mission icon); the default adds it when its data exists.
    [switch]$NoQuest
)
# HUD minimap in the host: one -game run with BL2's HUD movie (-owflashhud), the minimap manifest and -owminimapshots
# (OpenWillowMinimap puts Maya at the real-game capture spots and view yaws, captures the screen at each and quits),
# then tools/check_hud_minimap.py checks the logged state and the movie's read-backs against the real game's rules
# (docs/verification/HUD_MINIMAP.md). Screenshots go to local/minimap/runs/<stamp>/. Needs the converted movies under
# local/ui/run and local/slice/minimap.json (tools/prepare_minimap.py).
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$lock = Join-Path $repo 'local/ue_run.lock'
$shots = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
$manifest = Join-Path $repo 'local/slice/minimap.json'
foreach ($file in @($manifest, (Join-Path $repo 'local/ui/run/UI_HUD/harness.swf'),
        (Join-Path $repo 'local/ui/run/UI_TacticalMap_Sanctuary/Sanctuary_P.swf'))) {
    if (!(Test-Path -LiteralPath $file)) { throw "Missing $file (gfx_to_swf.py/hud_harness_swf.py, prepare_minimap.py)" }
}
$quest = @{ mover = 'local/doors/mover.json'; slice = 'local/slice/world.json'; npcs = 'local/slice/npc_assets.json'
    audio = 'local/slice/audio.json'; items = 'local/items/slice/slice_manifest.json' }
$useQuest = !$NoQuest
foreach ($key in @($quest.Keys)) {
    $quest[$key] = Join-Path $repo $quest[$key]
    if (!(Test-Path -LiteralPath $quest[$key])) { $useQuest = $false }
}
if (!$NoQuest -and !$useQuest) { Write-Host 'Mission data missing: running without the mission (no mission icon).' }
function Wait-Lock([string]$Path, [string]$Owner) {
    $deadline = (Get-Date).AddSeconds($WaitForEditorSeconds)
    while ($true) {
        try { New-Item -ItemType File -Path $Path -Value "$Owner $((Get-Date).ToString('o'))" -ErrorAction Stop | Out-Null; return }
        catch { if ((Get-Date) -gt $deadline) { throw "$Path is still held" }; Start-Sleep -Seconds 60 }
    }
}
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$outDir = Join-Path $repo "local/minimap/runs/$stamp"
New-Item -ItemType Directory -Force $outDir | Out-Null
$log = Join-Path $outDir 'run.log'
$server = $null; $process = $null; $haveShared = $false; $haveLocal = $false
$code = 2
try {
    if ($SharedLock) { Wait-Lock $SharedLock 'minimap'; $haveShared = $true }
    Wait-Lock $lock 'test_hud_minimap'; $haveLocal = $true
    $server = Start-Process -PassThru -WindowStyle Hidden -FilePath python -ArgumentList @(
        "`"$(Join-Path $PSScriptRoot 'hud_overlay/serve.py')`"", '--port', $Port)
    Start-Sleep 2
    $url = "http://127.0.0.1:$Port/"
    Invoke-WebRequest -UseBasicParsing -TimeoutSec 5 $url | Out-Null
    $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
    $env:OPENWILLOW_SCENE = Join-Path $repo 'local/sanctuary'
    $started = Get-Date
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', "-owflashhud=$url",
        "-owminimap=`"$manifest`"", '-owminimapshots')
    if ($useQuest) {
        # A fresh mission save in the run folder: the mission is not started, so Marcus carries the "eligible" mark.
        $arguments += @("-owmover=`"$($quest.mover)`"", "-owslice=`"$($quest.slice)`"", "-ownpcs=`"$($quest.npcs)`"",
            "-owaudio=`"$($quest.audio)`"", '-owquest', "-owquestsave=`"$(Join-Path $outDir 'quest_save.json')`"",
            "-owitems=`"$(Split-Path -Parent $quest.items)`"")
    }
    $arguments += @('-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Host "Launched minimap HUD test pid $($process.Id); log: $log"
    $finished = $process.WaitForExit($TimeoutSeconds * 1000)
    # Only this run's captures (written after the launch) are copied; the checker requires every scheduled one.
    if (Test-Path -LiteralPath $shots) {
        Get-ChildItem -LiteralPath $shots -Filter 'OWMinimap_*.png' | Where-Object { $_.LastWriteTime -ge $started } |
            ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $outDir -Force }
    }
    if ($finished) {
        & python (Join-Path $PSScriptRoot 'check_hud_minimap.py') $log --shots $outDir
        $code = $LASTEXITCODE
    } else {
        Write-Output "OWMINIMAPTEST FAIL run_finished editor still running after $TimeoutSeconds s"
        Write-Output 'OWMINIMAPTEST SUMMARY result=FAIL reason=timeout'
        $code = 1
    }
    Write-Host "Screenshots and log: $outDir"
} finally {
    # Only the processes this script started are stopped.
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    if ($server -and !$server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    if ($haveLocal) { Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue }
    if ($haveShared) { Remove-Item -LiteralPath $SharedLock -Force -ErrorAction SilentlyContinue }
}
exit $code
