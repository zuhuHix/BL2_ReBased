param(
    [string]$Engine = 'C:\Program Files\Epic Games\UE_5.8',
    [string]$Game = $env:OPENWILLOW_BL2,
    [string]$Ambient,
    # Seconds the pawns run before the self-test summary (-owambienttest).
    [int]$Seconds = 60,
    # Capture run: tour the manifest's viewpoints taking screenshots instead of the timed self-test (-owambientshots).
    [switch]$Shots,
    [string]$Tag = 'run',
    [int]$TimeoutSeconds = 300,
    [int]$WaitForEditorSeconds = 1800
)
# Sanctuary ambient NPCs in the host (docs/verification/SANCTUARY_AMBIENT_NPCS.md). One editor launch of the walking
# map with -owambient=<manifest>; waits for the shared local/ue_run.lock like tools/test_quest.ps1 and never touches an
# editor it did not start. Self-test: prints "OWAMBIENT SUMMARY ..." after -Seconds. Capture: screenshots are copied to
# local/ambient/<Tag>/. Host behaviour only; original-game parity is UNVERIFIED.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Ambient) { $Ambient = Join-Path $repo 'local/slice/ambient_world.json' }
if (!(Test-Path -LiteralPath $Ambient)) { throw "Missing $Ambient (tools/prepare_ambient_world.py)" }
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$lock = Join-Path $repo 'local/ue_run.lock'
$shotDir = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
$out = Join-Path $repo "local/ambient/$Tag"
New-Item -ItemType Directory -Force $out | Out-Null
$deadline = (Get-Date).AddSeconds($WaitForEditorSeconds)
$stream = $null
while (!$stream) {
    if (!(Get-Process UnrealEditor, Borderlands2 -ErrorAction SilentlyContinue)) {
        try { $stream = [System.IO.File]::Open($lock, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read) } catch { $stream = $null }
    }
    if (!$stream) {
        if ((Get-Date) -gt $deadline) { throw 'Another editor/game or local/ue_run.lock is still present; leave it untouched.' }
        Start-Sleep -Seconds 5
    }
}
$bytes = [System.Text.Encoding]::UTF8.GetBytes("B test_ambient $((Get-Date).ToString('o'))")
$stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
$code = 2
try {
    $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $log = Join-Path $out "run-$stamp.log"
    $started = Get-Date
    $mode = if ($Shots) { '-owambientshots' } else { '-owambienttest' }
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', "-owambient=`"$Ambient`"", $mode, "-owambientseconds=$Seconds",
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended', '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Host "Launched ambient run pid $($process.Id); log: $log"
    $limit = (Get-Date).AddSeconds($TimeoutSeconds)
    $summary = ''
    try {
        while ((Get-Date) -lt $limit) {
            $process.Refresh()
            $text = if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log -Raw -ErrorAction SilentlyContinue } else { '' }
            $match = [regex]::Match($text, 'OWAMBIENT SUMMARY (.*)')
            if ($match.Success) { $summary = $match.Groups[1].Value.Trim(); Start-Sleep -Seconds 3; break }
            if ($process.HasExited) { break }
            Start-Sleep -Seconds 2
        }
    } finally {
        if (Test-Path -LiteralPath $log) {
            Select-String -LiteralPath $log -Pattern 'OWAMBIENT' | ForEach-Object { Write-Host ($_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '') }
        }
        if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
        Start-Sleep -Seconds 3
    }
    if (Test-Path -LiteralPath $shotDir) {
        Get-ChildItem -LiteralPath $shotDir -Filter 'OWAmbient_*.png' | Where-Object { $_.LastWriteTime -ge $started } | ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $out $_.Name) -Force
            Write-Host "Screenshot: $(Join-Path $out $_.Name)"
        }
    }
    if ($summary -match '^result=PASS ') { $code = 0 } elseif ($summary) { $code = 1 }
    Write-Output "OWAMBIENT SUMMARY $summary"
} finally {
    Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue
}
exit $code
