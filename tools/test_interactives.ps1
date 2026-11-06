param(
    # Unreal Engine install (the folder that contains Engine/).
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    # The file from tools/prepare_interactives.py (ignored local/ data). Refused when missing.
    [string]$Interactives = (Join-Path (Split-Path -Parent $PSScriptRoot) 'local/slice/interactives.json'),
    [int]$Money = 5000,
    [int]$Port = 8767,
    [int]$TimeoutSeconds = 600,
    [int]$LockWaitSeconds = 900,
    # Optional machine-wide hub lock file (OPENWILLOW_HUB_LOCK): the suite waits while it exists and never deletes it.
    [string]$HubLock = $env:OPENWILLOW_HUB_LOCK,
    [switch]$SkipBuild,
    [switch]$SkipAutomation
)
# In-engine check of the lootables, chests and loot pickups (OpenWillowInteractive*.cpp, PR 3 of the vendors lane). One
# windowed host run in Sanctuary_P with Maya, -owinteractives=<interactives.json> and -owinteractivetest: once the walker
# exists the host opens three lootables of different balances, halves the pistol pool and health, collects every loot
# pickup through the walker's collect path, reports, takes OWInteractive_open.png and quits. Then the UE automation test
# OpenWillow.Interactive.Loot runs in its own -nullrhi launch. tools/check_interactives.py grades the logs (CHECK lines and
# SUMMARY, exit code 1 on any FAIL).
# Refuses to start when interactives.json is missing: run tools/prepare_interactives.py first.
# Builds the module first (Build.bat, like tools/test_vendors.ps1; -SkipBuild reuses the binary). Owns one editor at a time
# and the machine-wide UE run lock (tools/ue_lock.ps1); it waits while another UnrealEditor or the lock exists and never
# touches them. The page server is not needed here. Only the game and automation processes this run started are stopped,
# each by its own PID after its command line is checked. Logs and screenshots go to local/interactives/ (ignored).
# Exit codes: the checker's (0 nothing failed, missing or incomplete, else 1), 2 no run / no logs.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Game) { throw 'Set OPENWILLOW_BL2 or pass -Game (the installed Borderlands 2 folder).' }
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
if (!(Test-Path -LiteralPath $Interactives)) {
    throw "Missing ${Interactives}: run tools/prepare_interactives.py first (it writes local/slice/interactives.json from the installed game)."
}
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$buildScript = Join-Path $Engine 'Engine/Build/BatchFiles/Build.bat'
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
$checker = Join-Path $repo 'tools/check_interactives.py'

$outDir = Join-Path $repo 'local/interactives'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$log = Join-Path $outDir "run-$stamp.log"
$autoLog = Join-Path $outDir "automation-$stamp.log"
$shotsSource = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
$shotsOut = Join-Path $outDir "shots-$stamp"

. (Join-Path $PSScriptRoot 'ue_lock.ps1')
if ($HubLock -and (Test-Path -LiteralPath $HubLock)) {
    $hubDeadline = (Get-Date).AddSeconds($LockWaitSeconds)
    Write-Output "Waiting for the hub lock $HubLock to clear..."
    while (Test-Path -LiteralPath $HubLock) {
        if ((Get-Date) -gt $hubDeadline) { throw "The hub lock $HubLock is still present; leave it untouched." }
        Start-Sleep -Seconds 10
    }
}
$exitCode = 2
$process = $null
$autoProcess = $null
Enter-OwUeLock -Owner 'test_interactives' -TimeoutSeconds $LockWaitSeconds -PollSeconds 5
try {
    if (!$SkipBuild) {
        & $buildScript OpenWillowEditor Win64 Development "-Project=$project" -WaitMutex -NoHotReloadFromIDE
        if ($LASTEXITCODE -ne 0) { throw "UE5 build failed: $LASTEXITCODE" }
    }

    # --- the game run: Sanctuary_P with Maya, the interactives file and the test sequence ---
    $env:OPENWILLOW_SCENE = Join-Path $repo 'local/sanctuary'
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya',
        "-owinteractives=`"$((Resolve-Path -LiteralPath $Interactives).Path)`"", "-owmoney=$Money",
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', '-owinteractivetest', "-abslog=`"$log`"")
    $started = Get-Date
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Output "Launched game pid $($process.Id) (-owinteractivetest); log: $log"
    # The host quits by itself after the sequence; past the deadline this run's own game process is stopped.
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while (!$process.HasExited -and (Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 2
        if (Test-Path -LiteralPath $log) {
            $text = Get-Content -LiteralPath $log -Raw -ErrorAction SilentlyContinue
            $shot = $false
            if (Test-Path -LiteralPath $shotsSource) {
                $shot = [bool](Get-ChildItem -LiteralPath $shotsSource -Filter '*.png' -ErrorAction SilentlyContinue |
                    Where-Object { $_.LastWriteTime -ge $started })
            }
            if ("$text" -match 'OWINTERACTIVETEST done' -and $shot) { Start-Sleep -Seconds 8; break }
        }
    }
    if (!$process.HasExited) {
        # Stop only the game this run started, and only when its command line names this run's log.
        Start-Sleep -Seconds 3
        $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($process.Id)" -ErrorAction SilentlyContinue).CommandLine
        if ($commandLine -and $commandLine.Contains($log)) {
            Stop-Process -Id $process.Id -Force
            Write-Output "Stopped pid $($process.Id) (this run's game) after the console script."
        }
    }
    if (Test-Path -LiteralPath $shotsSource) {
        $shots = @(Get-ChildItem -LiteralPath $shotsSource -Filter '*.png' -ErrorAction SilentlyContinue |
            Where-Object { $_.LastWriteTime -ge $started })
        if ($shots.Count -gt 0) {
            New-Item -ItemType Directory -Force -Path $shotsOut | Out-Null
            $shots | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $shotsOut -Force }
            Write-Output "Screenshots copied to $shotsOut"
        }
    }

    # --- the automation test, in its own -nullrhi launch (no machines, no manifest) ---
    if (!$SkipAutomation) {
        $autoArguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-game', '-nullrhi', '-unattended',
            '-nosplash', '-ddc=InstalledNoZenLocalFallback', '-ExecCmds="Automation RunTests OpenWillow.Interactive; Quit"',
            "-abslog=`"$autoLog`"")
        $autoProcess = Start-Process -FilePath $editor -ArgumentList $autoArguments -PassThru
        Write-Output "Launched automation pid $($autoProcess.Id); log: $autoLog"
        if (!$autoProcess.WaitForExit($TimeoutSeconds * 1000)) {
            $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($autoProcess.Id)" -ErrorAction SilentlyContinue).CommandLine
            if ($commandLine -and $commandLine.Contains($autoLog)) { Stop-Process -Id $autoProcess.Id -Force }
        }
    }

    # --- grade ---
    Start-Sleep -Seconds 3
    if (!(Test-Path -LiteralPath $log)) { throw "No game log written: $log" }
    $checkArgs = @($checker, '--log', $log, '--interactives', (Resolve-Path -LiteralPath $Interactives).Path)
    # The assets on disk now, not an earlier import's report: this run's content is what the host had to load.
    $checkArgs += @('--content-root', (Join-Path $repo 'host/ue5/OpenWillow/Content'))
    $checkArgs += @('--materials-plan', (Join-Path $repo 'local/lootables/materials_plan.json'))
    if ($SkipAutomation) { $checkArgs += '--skip-automation' }
    elseif (Test-Path -LiteralPath $autoLog) { $checkArgs += @('--automation-log', $autoLog) }
    & python @checkArgs
    $exitCode = $LASTEXITCODE
    Write-Output "Log: $log"
} finally {
    if ($process -and !$process.HasExited) {
        $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($process.Id)" -ErrorAction SilentlyContinue).CommandLine
        if ($commandLine -and $commandLine.Contains($log)) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    }
    if ($autoProcess -and !$autoProcess.HasExited) {
        $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($autoProcess.Id)" -ErrorAction SilentlyContinue).CommandLine
        if ($commandLine -and $commandLine.Contains($autoLog)) { Stop-Process -Id $autoProcess.Id -Force -ErrorAction SilentlyContinue }
    }
    Exit-OwUeLock
}
exit $exitCode
