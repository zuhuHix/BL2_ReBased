param(
    # Unreal Engine install (the folder that contains Engine/).
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    [int]$Port = 8785,
    [int]$TimeoutSeconds = 360,
    # How long to wait for the UE run lock (tools/ue_lock.ps1) or a running editor before giving up.
    [int]$LockWaitSeconds = 900,
    [string[]]$Extra = @('-ddc=InstalledNoZenLocalFallback', '-d3d11'),
    # -MovieHud (test-only): BL2's HUD movie instead of the canvas HUD: -owflashhud=<the page server this suite already uses>; the suite fails if its log does not show
    # the movie accepted (tools/movie_hud.ps1). Without it nothing changes.
    [switch]$MovieHud
)
# In-engine check of the Missions log page (tools/hud_overlay/missions.html) with the -owquest slice mission:
# -owmissionlogtest (AOpenWillowMayaHUD::TickMissionLogTest) opens the page, checks it lists nothing before the mission is
# accepted and the mission under the in-progress category after Accept(), Down on the only mission, the header tabs to Inventory and
# back, and Escape. Prints each OWMISSIONTEST row and the summary; screenshots are copied to local/menus/missionlog/.
# Needs the slice manifests test_quest.ps1 uses and local/ui/run/missionlog.json (tools/prepare_mission_log.py).
# Exit codes: 0 all PASS; 1 a FAIL; 2 no summary (launch/timeout). Owns one editor and the machine-wide UE run lock
# (tools/ue_lock.ps1); never touches another editor; stops only the editor and server it started.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
if (!$Game -or !(Test-Path -LiteralPath $Game)) { throw 'Pass -Game <Borderlands 2 folder> or set OPENWILLOW_BL2' }
$manifests = [ordered]@{ mover = 'local/doors/mover.json'; slice = 'local/slice/world.json'; npcs = 'local/slice/npc_assets.json'
    audio = 'local/slice/audio.json'; gear = 'local/items/slice/slice_manifest.json'; actionskill = 'local/character/action_skill_siren.json'
    missionlog = 'local/ui/run/missionlog.json' }
foreach ($file in $manifests.Values) {
    if (!(Test-Path -LiteralPath (Join-Path $repo $file))) { throw "Missing $file (see tools/test_quest.ps1 and tools/prepare_mission_log.py)" }
}
$path = { param($key) (Resolve-Path -LiteralPath (Join-Path $repo $manifests[$key])).Path }
$gear = & $path 'gear'
$gearLevel = [int](Get-Content -LiteralPath $gear -Raw | ConvertFrom-Json).level
$outDir = Join-Path $repo 'local/menus/missionlog'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$log = Join-Path $outDir ('run-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
$save = Join-Path $outDir 'save.json'
if (Test-Path -LiteralPath $save) { Remove-Item -LiteralPath $save -Force }
$shots = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
# --- lock: the machine-wide UE run lock (tools/ue_lock.ps1; waits while any editor runs), released in finally ---
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
. (Join-Path $PSScriptRoot 'movie_hud.ps1')
$exitCode = 2
$process = $null
$server = $null
Enter-OwUeLock -Owner 'test_mission_log' -TimeoutSeconds $LockWaitSeconds -PollSeconds 5
try {
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
    $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $Game).Path
    $env:OPENWILLOW_SCENE = Join-Path $repo 'local/sanctuary'
    $hudSwitch = if ($MovieHud) { "-owflashhud=$url" } else { '-owcanvashud' }
    $arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya', $hudSwitch,
        "-owmover=`"$(& $path 'mover')`"", "-owslice=`"$(& $path 'slice')`"", "-ownpcs=`"$(& $path 'npcs')`"",
        "-owaudio=`"$(& $path 'audio')`"", '-owquest', "-owquestsave=`"$save`"", "-owitems=`"$(Split-Path -Parent $gear)`"",
        "-owactionskill=`"$(& $path 'actionskill')`"", "-owlevel=$gearLevel",
        "-owflashskills=${url}skills.html", "-owflashinventory=${url}inventory.html", '-owmissionlogtest',
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended', "-abslog=`"$log`"") + $Extra
    $started = Get-Date
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Output "Launched UnrealEditor pid $($process.Id); log: $log"
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $summary = $null
    while (!$process.HasExited -and (Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 2
        if (Test-Path -LiteralPath $log) {
            $text = Get-Content -LiteralPath $log -Raw -ErrorAction SilentlyContinue
            $m = [regex]::Match("$text", 'OWMISSIONTEST SUMMARY (.*)')
            if ($m.Success) { $summary = $m.Groups[1].Value.Trim(); break }
        }
    }
    if ($summary) { [void]$process.WaitForExit(20000) }
    $text = if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log -Raw } else { '' }
    foreach ($m in [regex]::Matches($text, 'OWMISSIONTEST (\S+) (PASS|FAIL) (.*)')) {
        Write-Output ('{0,-5} {1,-14} {2}' -f $m.Groups[2].Value, $m.Groups[1].Value, $m.Groups[3].Value.Trim())
    }
    if (Test-Path -LiteralPath $shots) {
        Get-ChildItem -LiteralPath $shots -Filter 'OWMission_*.png' | Where-Object { $_.LastWriteTime -ge $started } |
            ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $outDir -Force }
    }
    if ($summary) {
        Write-Output "OWMISSIONTEST SUMMARY $summary"
        $exitCode = if ($summary -match 'failed=0') { 0 } else { 1 }
    } else {
        Write-Output "OWMISSIONTEST SUMMARY result=FAIL reason=$(if ($process.HasExited) { "editor exited ($($process.ExitCode))" } else { "no summary within $TimeoutSeconds s" })"
    }
    if ($MovieHud -and !(Test-OwMovieHudLog -Log $log -Driver 'missionlog' -RequireShot) -and $exitCode -eq 0) { $exitCode = 1 }
    Write-Output "Log: $log"
} finally {
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    if ($server -and !$server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Exit-OwUeLock
}
exit $exitCode
