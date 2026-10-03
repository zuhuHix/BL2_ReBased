param(
    [string]$Engine = 'C:\Program Files\Epic Games\UE_5.8',
    [string]$Out = 'local/phaselock/b2',
    [int]$TimeoutSeconds = 240,
    [int]$WaitForEditorSeconds = 900,
    # Extra command-line switches (e.g. '-owfov=100').
    [string[]]$Extra = @()
)
# AI-assisted. Captures the host Phaselock presentation on Sanctuary: Maya (-owmaya) with a slice gun, the host
# training dummy 650 uu ahead (-owcombattest), a cast at it and timed screenshots (-owphaselockshots: hit at 0.12 ...
# 6.2 s, then a miss for the fizzle hand effect). Screenshots and the log go to $Out (ignored local/). Holds
# local/ue_run.lock for the one editor launch and never touches another editor. Host presentation only: nothing here is
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
$lock = Join-Path $repo 'local/ue_run.lock'
$shots = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
$deadline = (Get-Date).AddSeconds($WaitForEditorSeconds)
$stream = $null
while (!$stream) {
    if (!(Get-Process UnrealEditor, UnrealEditor-Cmd -ErrorAction SilentlyContinue) -and !(Test-Path -LiteralPath $lock)) {
        try { $stream = [System.IO.File]::Open($lock, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read) } catch { $stream = $null }
    }
    if (!$stream) {
        if ((Get-Date) -gt $deadline) { throw 'Another Unreal editor or local/ue_run.lock is still present; leave it untouched.' }
        Start-Sleep -Seconds 5
    }
}
$bytes = [System.Text.Encoding]::UTF8.GetBytes("run_phaselock_shots $((Get-Date).ToString('o'))")
$stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
try {
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $log = Join-Path $outDir "run-$stamp.log"
    $started = Get-Date
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', '-owcombattest', '-owphaselockshots',
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
} finally {
    Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue
}
