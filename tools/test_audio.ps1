param(
    [string]$Engine = 'C:\Program Files\Epic Games\UE_5.8',
    [string]$PlayMap,
    [int]$TimeoutSeconds = 180,
    [int]$WaitForEditorSeconds = 600
)
# Runs the host's sound path on a synthetic tone (-owaudiotest). Module build and map preparation are separate.
# Owns one editor process and the shared lock; waits while another editor or the lock exists and never touches them.
# tools/check_audio.py grades this run's own log against the exact expected rows and decides the exit code.
# PASS means the engine pulled every queued byte. It does not mean anyone heard the tone or that any sound is right.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$PlayMap) { $PlayMap = Join-Path $repo 'local/slice/audio_play.json' }
if (!(Test-Path -LiteralPath $PlayMap)) { throw 'Prepare a play map first: python tools/prepare_slice_audio.py --tone-only' }
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
$process = $null
New-Item -ItemType Directory -Force (Join-Path $repo 'local/audio') | Out-Null
# A name no earlier run can have, so an old log can never be graded as this run's.
$log = Join-Path $repo ("local/audio/run-$(Get-Date -Format 'yyyyMMdd-HHmmss')-$PID-$([guid]::NewGuid().ToString('N').Substring(0, 8)).log")
$code = 2
Enter-OwUeLock -Owner 'test_audio' -TimeoutSeconds $WaitForEditorSeconds
try {
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', "-owaudioplay=`"$PlayMap`"", '-owaudiotest',
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended', '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Output "Launched audio test pid $($process.Id); log: $log"
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $summary = ''
    while ((Get-Date) -lt $deadline) {
        $process.Refresh()
        $text = if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log -Raw -ErrorAction SilentlyContinue } else { '' }
        $match = [regex]::Match($text, 'OWAUDIOTEST SUMMARY (.*)')
        if ($match.Success) { $summary = $match.Groups[1].Value.Trim(); break }
        if ($process.HasExited) { break }
        Start-Sleep -Seconds 2
    }
    if (Test-Path -LiteralPath $log) {
        Select-String -LiteralPath $log -Pattern 'OWAUDIO' | ForEach-Object { $_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '' }
    }
    if (!$summary) { Write-Output 'OWAUDIOTEST SUMMARY result=FAIL reason=no summary' }
    else {
        & python (Join-Path $PSScriptRoot 'check_audio.py') $log
        $code = if ($LASTEXITCODE -eq 0) { 0 } else { 1 }
    }
    Write-Output "Log: $log"
} finally {
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    Exit-OwUeLock
}
exit $code
