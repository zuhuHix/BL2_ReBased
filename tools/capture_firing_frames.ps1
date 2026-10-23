param(
    [string]$Engine = 'C:\Program Files\Epic Games\UE_5.8',
    [string]$Game = $env:OPENWILLOW_BL2,
    [string[]]$Guns = @('pistol_bandit', 'sg_jakobs', 'rl_bandit', 'ar_torgue_gyrojet', 'pistol_maliwan_shock', 'ar_bandit_fire',
        'slice_shotgun', 'slice_smg'),
    [int]$TimeoutSeconds = 900,
    [int]$WaitForEditorSeconds = 3600,
    [switch]$SkipBuild
)
# Host frames for the real-game firing references (local/realgame/firing/r3/frames): one trigger pull per gun in Sanctuary_P
# (-owfiringshots), a flat wall 465.9 uu ahead, pitch 0, yaw 90, FOV 77.55, frames 02-24 at ~46 ms (shot = frame 04).
# Builds the module first (Build.bat, like tools/test_weapon_firing.ps1; -SkipBuild reuses the binary). Owns one editor at a time
# and the shared lock local/ue_run.lock: it waits while another UnrealEditor or the lock exists and never touches them.
# Output: local/firing/frames/host/<gun>_NN.png (copied from the editor's Saved/Screenshots, only files this run wrote), the run
# log local/firing/frames/run-<stamp>.log. Then run tools/firing_contact_sheet.py to build the sheets.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Game) { throw 'Set OPENWILLOW_BL2 or pass -Game (the installed Borderlands 2 folder).' }
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$buildScript = Join-Path $Engine 'Engine/Build/BatchFiles/Build.bat'
$lock = Join-Path $repo 'local/ue_run.lock'
$shots = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
$outDir = Join-Path $repo 'local/firing/frames/host'
# The host's recipe folders in load order: -owitems first, then -owitemdir (the first recipe with a given id wins).
$itemDirs = @((Join-Path $repo 'local/items'), (Join-Path $repo 'local/items/slice'), (Join-Path $repo 'local/items/firing/coverage'))
foreach ($dir in $itemDirs) { if (!(Test-Path -LiteralPath $dir)) { throw "Missing recipe folder $dir (tools/weapon_stats.py, weapon_slice_gear.py, weapon_firing.py)" } }
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
# Take the lock as soon as no editor runs and nobody holds it (CreateNew is the atomic claim).
$deadline = (Get-Date).AddSeconds($WaitForEditorSeconds)
$stream = $null
while (!$stream) {
    if (!(Get-Process UnrealEditor -ErrorAction SilentlyContinue)) {
        try { $stream = [System.IO.File]::Open($lock, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read) } catch { $stream = $null }
    }
    if (!$stream) {
        if ((Get-Date) -gt $deadline) { throw 'Another Unreal editor or local/ue_run.lock is still present; leave it untouched.' }
        Start-Sleep -Seconds 5
    }
}
$bytes = [System.Text.Encoding]::UTF8.GetBytes("capture_firing_frames $((Get-Date).ToString('o'))")
$stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
$code = 2
try {
    if (!$SkipBuild) {
        & $buildScript OpenWillowEditor Win64 Development "-Project=$project" -WaitMutex -NoHotReloadFromIDE
        if ($LASTEXITCODE -ne 0) { throw "UE5 build failed: $LASTEXITCODE" }
    }
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $logDir = Join-Path $repo 'local/firing/frames'
    New-Item -ItemType Directory -Force $logDir | Out-Null
    $log = Join-Path $logDir ("run-$stamp.log")
    $started = Get-Date
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', '-owfiringshots',
        "-owgunids=$($Guns -join ',')", "-owitemids=$($Guns -join ',')", '-owlevel=36', "-owitems=`"$($itemDirs[0])`"",
        "-owitemdir=`"$($itemDirs[1]);$($itemDirs[2])`"",
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', "-abslog=`"$log`"")
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Host "Launched firing shots pid $($process.Id) for $($Guns.Count) guns; log: $log"
    if (!$process.WaitForExit($TimeoutSeconds * 1000)) {
        # Stop only the editor this run started, and only when its command line names this run's log.
        $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($process.Id)" -ErrorAction SilentlyContinue).CommandLine
        if ($commandLine -and $commandLine.Contains($log)) {
            Stop-Process -Id $process.Id -Force
            Write-Host "Timed out after $TimeoutSeconds s; stopped pid $($process.Id) (this run's editor)."
        }
    }
    Start-Sleep -Seconds 3
    if (!(Test-Path -LiteralPath $log)) { throw "No log written: $log" }
    Select-String -LiteralPath $log -Pattern 'OpenWillow firing shots (skipped|lost|done|wall|: no wall|: geometry)|Error:' |
        ForEach-Object { Write-Host ($_.Line -replace '^.*LogTemp: (Display: |Warning: |Error: )?', '') }
    # Copy only the screenshots this run wrote (modified since it started), renamed <gun>_NN.png for the contact sheet.
    New-Item -ItemType Directory -Force $outDir | Out-Null
    $copied = 0
    if (Test-Path -LiteralPath $shots) {
        Get-ChildItem -LiteralPath $shots -Filter 'OWFiring_*.png' | Where-Object { $_.LastWriteTime -ge $started.AddSeconds(-2) } | ForEach-Object {
            if ($_.Name -match '^OWFiring_(.+)_(\d\d)\.png$') {
                Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $outDir ("$($Matches[1])_$($Matches[2]).png")) -Force
                $copied++
            }
        }
    }
    Write-Host "Copied $copied frames to $outDir"
    foreach ($gun in $Guns) {
        $count = @(Get-ChildItem -LiteralPath $outDir -Filter "${gun}_??.png" -ErrorAction SilentlyContinue).Count
        Write-Host ("{0,-22} {1,2} frames" -f $gun, $count)
    }
    Write-Host "Log: $log"
    $code = if ($copied -gt 0) { 0 } else { 1 }
} finally {
    Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue
}
exit $code
