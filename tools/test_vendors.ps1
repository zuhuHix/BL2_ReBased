param(
    # Unreal Engine install (the folder that contains Engine/).
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    # The machines file from tools/prepare_vendors.py (ignored local/ data). Refused when missing.
    [string]$Vendors = (Join-Path (Split-Path -Parent $PSScriptRoot) 'local/slice/vendors.json'),
    [int]$Money = 5000,
    # The recipe folder the walker loads (-owitems). The default is the inventory suite's six-weapon set when it exists, so
    # the backpack has room for the gun the suite buys; a full backpack makes the buy fail.
    [string]$Items = (Join-Path (Split-Path -Parent $PSScriptRoot) 'local/inventory-actions/bounded-recipes'),
    # The weapon generator manifest: the shops roll their guns with it. Refused when missing.
    [string]$WeaponGen = (Join-Path (Split-Path -Parent $PSScriptRoot) 'local/weapons/gen/weapon_gen_manifest.txt'),
    [int]$Port = 8767,
    [int]$TimeoutSeconds = 600,
    [int]$LockWaitSeconds = 900,
    # Optional machine-wide hub lock file (OPENWILLOW_HUB_LOCK): the suite waits while it exists and never deletes it.
    [string]$HubLock = $env:OPENWILLOW_HUB_LOCK,
    [switch]$SkipBuild,
    [switch]$SkipAutomation,
    # Skips the second launch that checks the currency save carried over (the same -owcurrencysave file, no -owmoney).
    [switch]$SkipPersist
)
# In-engine check of the vending machines (OpenWillowVendor.cpp, the vendors slice). One windowed host run in Sanctuary_P
# with Maya, -owvendors=<vendors.json>, -owmoney=<Money> and -owvendortest: once the walker exists the host opens the
# first weapons machine, reports, buys shelf item 0, reports, sells that gun back (ow.Vendor.SellLast), reports, restocks,
# reports, takes OWVendor_open.png and quits (UOpenWillowVendorSubsystem::TickVendorTest). That first launch saves its money and
# shelf to local/vendors/currency.json (-owcurrencysave). A second launch (unless -SkipPersist) loads that file, with no
# -owmoney, and runs the same sequence; tools/check_vendors.py compares its first report with the first launch's last one.
# Then the UE automation tests OpenWillow.Vendor.* (Stock and Persist) run in their own -nullrhi launch. tools/check_vendors.py
# grades the logs (CHECK lines and SUMMARY, exit code 1 on any FAIL).
# Refuses to start when vendors.json is missing: run tools/prepare_vendors.py first.
# Builds the module first (Build.bat, like tools/test_weapon_firing.ps1; -SkipBuild reuses the binary). Owns one editor at a
# time and the machine-wide UE run lock (tools/ue_lock.ps1); it waits while another UnrealEditor or the lock exists and never
# touches them. The page is served by tools/hud_overlay/serve.py on -Port (started here only when nothing answers there);
# only the server and the editor this run started are stopped, each by its own PID after its command line is checked.
# Logs and screenshots go to local/vendors/ (ignored).
# Exit codes: the checker's (0 all PASS or MISSING only, 1 a FAIL), 2 no run / no logs.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (!$Game) { throw 'Set OPENWILLOW_BL2 or pass -Game (the installed Borderlands 2 folder).' }
$env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
if (!(Test-Path -LiteralPath $Vendors)) {
    throw "Missing ${Vendors}: run tools/prepare_vendors.py first (it writes local/slice/vendors.json from the installed game)."
}
if (!(Test-Path -LiteralPath $WeaponGen)) {
    throw "Missing ${WeaponGen}: run python tools/weapon_gen_manifest.py --game <BL2 folder> first (the shops roll their guns with it)."
}
if ($Items -and !(Test-Path -LiteralPath $Items)) {
    Write-Output "No recipe set at $Items; the walker loads its default items (the backpack may be full and the buy then fails)."
    $Items = ''
}
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$buildScript = Join-Path $Engine 'Engine/Build/BatchFiles/Build.bat'
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
$checker = Join-Path $repo 'tools/check_vendors.py'

# The machine to open: the first supported weapons machine in file order (the host spawns the supported machines in the
# same order, so this is also its spawn index; machine 0 is not a weapons machine in every map).
$document = Get-Content -LiteralPath $Vendors -Raw | ConvertFrom-Json
$supported = @($document.machines | Where-Object { $_.supported -ne $false })
$openIndex = -1
for ($i = 0; $i -lt $supported.Count; $i++) { if ($supported[$i].kind -eq 'weapons') { $openIndex = $i; break } }
if ($openIndex -lt 0) { throw "$Vendors has no supported weapons machine; the suite opens one." }
Write-Output "Opening spawn index $openIndex ($($supported[$openIndex].id)) of $($supported.Count) supported machines."

$outDir = Join-Path $repo 'local/vendors'
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
$server = $null
Enter-OwUeLock -Owner 'test_vendors' -TimeoutSeconds $LockWaitSeconds -PollSeconds 5
try {
    if (!$SkipBuild) {
        & $buildScript OpenWillowEditor Win64 Development "-Project=$project" -WaitMutex -NoHotReloadFromIDE
        if ($LASTEXITCODE -ne 0) { throw "UE5 build failed: $LASTEXITCODE" }
    }
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null

    # --- page server: reuse one that serves these pages, else start one and stop it at the end ---
    $url = "http://127.0.0.1:$Port/"
    $pages = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'hud_overlay')).Path
    try { $running = Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 $url } catch { $running = $null }
    if ($running) {
        if ("$($running.Headers['X-OpenWillow-Pages'])" -ne $pages) { throw "Port $Port serves other pages; stop that server or pass -Port." }
    } else {
        $server = Start-Process -WindowStyle Hidden -PassThru -FilePath python -ArgumentList @(
            "`"$(Join-Path $PSScriptRoot 'hud_overlay/serve.py')`"", '--port', $Port)
        Start-Sleep 2
        Invoke-WebRequest -UseBasicParsing -TimeoutSec 5 $url | Out-Null
    }

    # --- the game runs: Sanctuary_P with Maya, the machines file, the currency save and the console script. Run 1 starts
    # with no save file and the purse -owmoney=<Money>; run 2 (unless -SkipPersist) has no -owmoney and must start where run 1
    # stopped. $log is the run in progress; $firstLog keeps run 1's log for the checker.
    $env:OPENWILLOW_SCENE = Join-Path $repo 'local/sanctuary'
    $currency = Join-Path $outDir 'currency.json'
    Remove-Item -LiteralPath $currency -Force -ErrorAction SilentlyContinue
    $firstLog = $log
    $persistLog = Join-Path $outDir "persist-$stamp.log"
    $passes = if ($SkipPersist) { @(1) } else { @(1, 2) }
    foreach ($pass in $passes) {
    $log = if ($pass -eq 1) { $firstLog } else { $persistLog }
    $shotsOut = if ($pass -eq 1) { Join-Path $outDir "shots-$stamp" } else { Join-Path $outDir "shots-$stamp-persist" }
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya',
        "-owvendors=`"$((Resolve-Path -LiteralPath $Vendors).Path)`"", "-owcurrencysave=`"$currency`"",
        "-owvendingpage=${url}vending.html",
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended',
        '-ddc=InstalledNoZenLocalFallback', '-d3d11', '-owvendortest', "-abslog=`"$log`"")
    $arguments += "-owweapongen=`"$((Resolve-Path -LiteralPath $WeaponGen).Path)`""
    if ($Items) { $arguments += "-owitems=`"$((Resolve-Path -LiteralPath $Items).Path)`"" }
    if ($pass -eq 1) { $arguments += "-owmoney=$Money" }
    $started = Get-Date
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Output "Launched game pid $($process.Id) (run $pass of $($passes.Count), -owvendortest); log: $log"
    # The host quits by itself after the sequence; if it hangs past the deadline, or after four state reports and the
    # screenshot, this run's own game process is stopped.
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while (!$process.HasExited -and (Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 2
        if (Test-Path -LiteralPath $log) {
            $text = Get-Content -LiteralPath $log -Raw -ErrorAction SilentlyContinue
            $reports = ([regex]::Matches("$text", 'OWVENDSTATE \{')).Count
            $shot = $false
            if (Test-Path -LiteralPath $shotsSource) {
                $shot = [bool](Get-ChildItem -LiteralPath $shotsSource -Filter '*.png' -ErrorAction SilentlyContinue |
                    Where-Object { $_.LastWriteTime -ge $started })
            }
            if ($reports -ge 4 -and $shot) { Start-Sleep -Seconds 8; break }
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

    }

    # --- the automation tests, in their own -nullrhi launch (no page, no machines) ---
    if (!$SkipAutomation) {
        $autoArguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-game', '-nullrhi', '-unattended',
            '-nosplash', '-ddc=InstalledNoZenLocalFallback', '-ExecCmds="Automation RunTests OpenWillow.Vendor; Quit"',
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
    if (!(Test-Path -LiteralPath $firstLog)) { throw "No game log written: $firstLog" }
    $checkArgs = @($checker, '--log', $firstLog, '--vendors', $Vendors, '--open-index', "$openIndex", '--open-kind', 'weapons')
    if ($passes.Count -gt 1) {
        if (!(Test-Path -LiteralPath $persistLog)) { throw "No second-launch log written: $persistLog" }
        $checkArgs += @('--persist-log', $persistLog)
    }
    if (!$SkipAutomation -and (Test-Path -LiteralPath $autoLog)) { $checkArgs += @('--automation-log', $autoLog) }
    & python @checkArgs
    $exitCode = $LASTEXITCODE
    Write-Output "Log: $firstLog"
    if ($passes.Count -gt 1) { Write-Output "Second launch log: $persistLog" }
} finally {
    if ($process -and !$process.HasExited) {
        $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($process.Id)" -ErrorAction SilentlyContinue).CommandLine
        if ($commandLine -and $commandLine.Contains($log)) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    }
    if ($autoProcess -and !$autoProcess.HasExited) {
        $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($autoProcess.Id)" -ErrorAction SilentlyContinue).CommandLine
        if ($commandLine -and $commandLine.Contains($autoLog)) { Stop-Process -Id $autoProcess.Id -Force -ErrorAction SilentlyContinue }
    }
    if ($server -and !$server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Exit-OwUeLock
}
exit $exitCode
