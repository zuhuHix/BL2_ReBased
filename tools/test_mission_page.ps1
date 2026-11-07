param(
    # Unreal Engine install (the folder that contains Engine/).
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    [int]$Port = 8785,
    [int]$TimeoutSeconds = 360,
    # How long to wait for the UE run lock (tools/ue_lock.ps1) or a running editor before giving up.
    [int]$LockWaitSeconds = 900,
    [string[]]$Extra = @('-ddc=InstalledNoZenLocalFallback', '-d3d11'),
    [switch]$NoMovie,
    # -MovieHud (test-only): BL2's HUD movie instead of the canvas HUD: -owflashhud=<the page server this suite already uses>; the suite fails if its log does not show
    # the movie accepted (tools/movie_hud.ps1). Without it nothing changes.
    [switch]$MovieHud
)
# In-engine check of Marcus's mission page (tools/hud_overlay/mission_screen.html): real keys through Slate at the offer, the turn-in and the reward page. Prints each
# OWMISSIONPAGETEST row and the summary; screenshots go to local/menus/missionpage/. -NoMovie serves a folder without the mission movie: the page must show its plain panel.
# A pass says what the host did, not that it matches the game.
#
# Exit codes: 0 all PASS; 1 a FAIL; 2 no summary (launch/timeout). Owns one editor and the machine-wide UE run lock
# (tools/ue_lock.ps1); never touches another editor; stops only the editor and server it started.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine" }
if (!$Game -or !(Test-Path -LiteralPath $Game)) { throw 'Pass -Game <Borderlands 2 folder> or set OPENWILLOW_BL2' }
$manifests = [ordered]@{ mover = 'local/doors/mover.json'; slice = 'local/slice/world.json'; npcs = 'local/slice/npc_assets.json'
    audio = 'local/slice/audio.json'; gear = 'local/items/slice/slice_manifest.json'; actionskill = 'local/character/action_skill_siren.json' }
foreach ($file in $manifests.Values) {
    if (!(Test-Path -LiteralPath (Join-Path $repo $file))) { throw "Missing $file (see tools/test_quest.ps1)" }
}
$path = { param($key) (Resolve-Path -LiteralPath (Join-Path $repo $manifests[$key])).Path }
$gear = & $path 'gear'
$gearLevel = [int](Get-Content -LiteralPath $gear -Raw | ConvertFrom-Json).level
$outDir = Join-Path $repo 'local/menus/missionpage'
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
Enter-OwUeLock -Owner 'test_mission_page' -TimeoutSeconds $LockWaitSeconds -PollSeconds 5
try {
    $url = "http://127.0.0.1:$Port/"
    $pages = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'hud_overlay')).Path
    try { $running = Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 $url } catch { $running = $null }
    if ($running -and $NoMovie) { throw "Port $Port already serves the full movies, so -NoMovie would test nothing; stop that server or pass -Port." }
    if ($running) {
        if ("$($running.Headers['X-OpenWillow-Pages'])" -ne $pages) { throw "Port $Port serves other pages; stop that server or pass -Port." }
    } else {
        $serveArgs = @("`"$(Join-Path $PSScriptRoot 'hud_overlay/serve.py')`"", '--port', $Port)
        if ($NoMovie) {
            # The server's own requirements and nothing of the mission's.
            $bare = Join-Path $outDir 'nomovie'
            New-Item -ItemType Directory -Force -Path $bare | Out-Null
            foreach ($folder in 'ruffle', 'UI_HUD') { Copy-Item -Recurse -Force -LiteralPath (Join-Path $repo "local/ui/run/$folder") $bare }
            $serveArgs += @('--movies', "`"$bare`"")
        }
        $server = Start-Process -WindowStyle Hidden -PassThru -FilePath python -ArgumentList $serveArgs
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
        "-owflashmissionscreen=${url}mission_screen.html", "-owflashskills=${url}skills.html", '-owmissionpagetest',
        '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-unattended', "-abslog=`"$log`"") + $Extra
    if ($NoMovie) { $arguments += '-owmissionpagenomovie' }
    $started = Get-Date
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Output "Launched UnrealEditor pid $($process.Id); log: $log"
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $summary = $null
    $readyAt = $null
    $staleBuild = $false
    while (!$process.HasExited -and (Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 2
        if (Test-Path -LiteralPath $log) {
            $text = Get-Content -LiteralPath $log -Raw -ErrorAction SilentlyContinue
            $m = [regex]::Match("$text", 'OWMISSIONPAGETEST SUMMARY (.*)')
            if ($m.Success) { $summary = $m.Groups[1].Value.Trim(); break }
            # The suite logs page_ready, pass or fail, within 60 s of its first tick (OWQUEST READY comes first). Without it the build has no page suite: a stale DLL (2026-11-07: 360 s of silence).
            if ("$text" -match 'OWQUEST READY' -and "$text" -notmatch 'OWMISSIONPAGETEST page_ready') {
                if (!$readyAt) { $readyAt = Get-Date }
                if (((Get-Date) - $readyAt).TotalSeconds -gt 120) { $staleBuild = $true; break }
            }
        }
    }
    if ($summary) { [void]$process.WaitForExit(20000) }
    $text = if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log -Raw } else { '' }
    foreach ($m in [regex]::Matches($text, 'OWMISSIONPAGETEST (\S+) (PASS|FAIL) (.*)')) {
        Write-Output ('{0,-5} {1,-14} {2}' -f $m.Groups[2].Value, $m.Groups[1].Value, $m.Groups[3].Value.Trim())
    }
    if (Test-Path -LiteralPath $shots) {
        Get-ChildItem -LiteralPath $shots -Filter 'OWMissionPage_*.png' | Where-Object { $_.LastWriteTime -ge $started } |
            ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $outDir -Force }
    }
    if ($summary) {
        # The checker requires the exact rows, each once: a failed=0 summary alone would pass a run that lost one.
        & python (Join-Path $PSScriptRoot 'check_mission_page.py') $log
        $exitCode = if ($LASTEXITCODE -eq 0) { 0 } else { 1 }
    } else {
        Write-Output "OWMISSIONPAGETEST SUMMARY result=FAIL reason=$(if ($staleBuild) { 'OWQUEST READY but no page_ready row in 120 s: is the UE module built from this branch?' } elseif ($process.HasExited) { "editor exited ($($process.ExitCode))" } else { "no summary within $TimeoutSeconds s" })"
    }
    if ($MovieHud -and !(Test-OwMovieHudLog -Log $log) -and $exitCode -eq 0) { $exitCode = 1 }
    Write-Output "Log: $log"
} finally {
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    if ($server -and !$server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Exit-OwUeLock
}
exit $exitCode
