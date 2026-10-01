param(
    [string]$Engine = 'C:\Program Files\Epic Games\UE_5.8',
    [string]$Game = $env:OPENWILLOW_BL2,
    [string]$Manifest,
    [int]$TimeoutSeconds = 180
)
# Builds/preparation are separate. Owns one editor process and the shared lock;
# reports component collision/script checks, not original-game parity.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Manifest) { $Manifest = Join-Path $repo 'local/doors/mover.json' }
if (!(Test-Path -LiteralPath $Manifest)) { throw 'Prepare a mover manifest first with tools/prepare_mover.py' }
if (Get-Process UnrealEditor -ErrorAction SilentlyContinue) { throw 'An Unreal editor is already running; leave it untouched.' }
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$lock = Join-Path $repo 'local/ue_run.lock'
$stream = [System.IO.File]::Open($lock, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read)
$bytes = [System.Text.Encoding]::UTF8.GetBytes("test_mover $((Get-Date).ToString('o'))")
$stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
$process = $null
$log = Join-Path $repo ('local/doors/run-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
$code = 2
try {
    $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya',
        "-owmover=`"$Manifest`"", '-owmovertest', '-game', '-windowed', '-ResX=1280', '-ResY=720',
        '-nosplash', '-unattended', '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Output "Launched mover test pid $($process.Id); log: $log"
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $summary = ''
    while ((Get-Date) -lt $deadline) {
        $process.Refresh()
        $text = if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log -Raw -ErrorAction SilentlyContinue } else { '' }
        $match = [regex]::Match($text, 'OWMOVERTEST SUMMARY (.*)')
        if ($match.Success) { $summary = $match.Groups[1].Value.Trim(); break }
        if ($process.HasExited) { break }
        Start-Sleep -Seconds 2
    }
    if (Test-Path -LiteralPath $log) {
        Select-String -LiteralPath $log -Pattern 'OWMOVER' | ForEach-Object { $_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '' }
    }
    if (!$summary) { Write-Output 'OWMOVERTEST SUMMARY result=FAIL reason=no summary' }
    else { $code = if ($summary -match '^result=PASS ') { 0 } else { 1 } }
    Write-Output "Log: $log"
} finally {
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $lock -Force
}
exit $code
