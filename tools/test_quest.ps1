param(
    [string]$Engine = 'C:\Program Files\Epic Games\UE_5.8',
    [string]$Game = $env:OPENWILLOW_BL2,
    [string]$Manifest,
    [int]$TimeoutSeconds = 240
)
# Sanctuary slice loop in the host: stock Fire mission + dummy provider + door Kismet, death/respawn, save.
# Two editor launches: the first plays the loop and writes the save, the second resumes from it
# (progress retained across a restart). Owns one editor process at a time and the shared lock.
# Reports host behaviour only; original-game parity is UNVERIFIED (see docs/verification/SANCTUARY_RPG_MISSION.md).
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Manifest) { $Manifest = Join-Path $repo 'local/doors/mover.json' }
if (!(Test-Path -LiteralPath $Manifest)) { throw 'Prepare a mover manifest first with tools/prepare_mover.py' }
if (Get-Process UnrealEditor -ErrorAction SilentlyContinue) { throw 'An Unreal editor is already running; leave it untouched.' }
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$lock = Join-Path $repo 'local/ue_run.lock'
$save = Join-Path $repo 'local/quest/save.json'
New-Item -ItemType Directory -Force (Split-Path $save) | Out-Null
if (Test-Path -LiteralPath $save) { Remove-Item -LiteralPath $save -Force }
$stream = [System.IO.File]::Open($lock, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read)
$bytes = [System.Text.Encoding]::UTF8.GetBytes("test_quest $((Get-Date).ToString('o'))")
$stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
$code = 2
function Invoke-Run([string]$Mode, [string[]]$Extra) {
    $log = Join-Path $repo ('local/quest/run-' + $Mode + '-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya',
        "-owmover=`"$Manifest`"", '-owquest', '-owquesttest', "-owquestsave=`"$save`"") + $Extra + @(
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Host "Launched quest test ($Mode) pid $($process.Id); log: $log"
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $summary = ''
    try {
        while ((Get-Date) -lt $deadline) {
            $process.Refresh()
            $text = if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log -Raw -ErrorAction SilentlyContinue } else { '' }
            $match = [regex]::Match($text, 'OWQUESTTEST SUMMARY (.*)')
            if ($match.Success) { $summary = $match.Groups[1].Value.Trim(); break }
            if ($process.HasExited) { break }
            Start-Sleep -Seconds 2
        }
    } finally {
        if (Test-Path -LiteralPath $log) {
            Select-String -LiteralPath $log -Pattern 'OWQUEST|OWMOVER ' | ForEach-Object { Write-Host ($_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '') }
        }
        if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
        Start-Sleep -Seconds 3
    }
    Write-Host "Log: $log"
    return $summary
}
try {
    $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
    $first = Invoke-Run 'first' @()
    if (!$first) { Write-Output 'OWQUESTTEST SUMMARY result=FAIL reason=no summary (first run)' }
    elseif ($first -notmatch '^result=PASS ') { Write-Output "FIRST RUN: $first" }
    else {
        $second = Invoke-Run 'resume' @('-owquestresume')
        if (!$second) { Write-Output 'OWQUESTTEST SUMMARY result=FAIL reason=no summary (resume run)' }
        elseif ($second -match '^result=PASS ') { $code = 0; Write-Output "FIRST RUN: $first"; Write-Output "RESUME RUN: $second" }
        else { $code = 1; Write-Output "RESUME RUN: $second" }
    }
} finally {
    Remove-Item -LiteralPath $lock -Force
}
exit $code
