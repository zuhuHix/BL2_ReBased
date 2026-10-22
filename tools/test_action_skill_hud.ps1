param(
    [string]$Engine = 'C:\Program Files\Epic Games\UE_5.8',
    [string]$Game = $env:OPENWILLOW_BL2,
    # Optional machine-wide lock shared by several checkouts (taken before local/ue_run.lock, released after).
    [string]$SharedLock,
    [int]$Port = 8793,
    [int]$TimeoutSeconds = 240,
    [int]$WaitForEditorSeconds = 1800
)
# Action-skill HUD indicator in the host: one -game run with BL2's HUD movie (-owflashhud), the combat dummy and
# -owskillhudshots (OpenWillowMayaHUD casts Phaselock at the dummy, captures the HUD every 0.5 s through the cooldown
# and quits), then tools/check_action_skill_hud.py checks the logged indicator state against the real game's timings
# (docs/verification/ACTION_SKILL_HUD.md). Screenshots go to local/skillhud/<stamp>/. Needs the converted movies under
# local/ui/run (tools/prepare_hud_icons.py for the icon) and the action-skill manifest.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$lock = Join-Path $repo 'local/ue_run.lock'
$shots = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
$actionSkill = Join-Path $repo 'local/character/action_skill_siren.json'
foreach ($file in @($actionSkill, (Join-Path $repo 'local/ui/run/UI_HUD/harness.swf'), (Join-Path $repo 'local/ui/run/UI_HUD_AAIcons/AA-Siren.swf'))) {
    if (!(Test-Path -LiteralPath $file)) { throw "Missing $file (tools/prepare_action_skill.py, gfx_to_swf.py/hud_harness_swf.py, prepare_hud_icons.py)" }
}
function Wait-Lock([string]$Path, [string]$Owner) {
    $deadline = (Get-Date).AddSeconds($WaitForEditorSeconds)
    while ($true) {
        try { New-Item -ItemType File -Path $Path -Value "$Owner $((Get-Date).ToString('o'))" -ErrorAction Stop | Out-Null; return }
        catch { if ((Get-Date) -gt $deadline) { throw "$Path is still held" }; Start-Sleep -Seconds 60 }
    }
}
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$outDir = Join-Path $repo "local/skillhud/$stamp"
New-Item -ItemType Directory -Force $outDir | Out-Null
$log = Join-Path $outDir 'run.log'
$server = $null; $process = $null; $haveShared = $false; $haveLocal = $false
$code = 2
try {
    if ($SharedLock) { Wait-Lock $SharedLock 'laneC'; $haveShared = $true }
    Wait-Lock $lock 'test_action_skill_hud'; $haveLocal = $true
    $server = Start-Process -PassThru -WindowStyle Hidden -FilePath python -ArgumentList @(
        "`"$(Join-Path $PSScriptRoot 'hud_overlay/serve.py')`"", '--port', $Port)
    Start-Sleep 2
    $url = "http://127.0.0.1:$Port/"
    Invoke-WebRequest -UseBasicParsing -TimeoutSec 5 $url | Out-Null
    $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
    $env:OPENWILLOW_SCENE = Join-Path $repo 'local/sanctuary'
    $started = Get-Date
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', "-owflashhud=$url",
        "-owactionskill=`"$actionSkill`"", '-owcombattest', '-owskillhudshots',
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Host "Launched action-skill HUD test pid $($process.Id); log: $log"
    if (!$process.WaitForExit($TimeoutSeconds * 1000)) { Write-Host 'Timed out; stopping the run.' }
    if (Test-Path -LiteralPath $shots) {
        Get-ChildItem -LiteralPath $shots -Filter 'OWSkillHud_*.png' | Where-Object { $_.LastWriteTime -ge $started } |
            ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $outDir -Force }
    }
    & python (Join-Path $PSScriptRoot 'check_action_skill_hud.py') $log
    $code = $LASTEXITCODE
    Write-Host "Screenshots and log: $outDir"
} finally {
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    if ($server -and !$server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    if ($haveLocal) { Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue }
    if ($haveShared) { Remove-Item -LiteralPath $SharedLock -Force -ErrorAction SilentlyContinue }
}
exit $code
