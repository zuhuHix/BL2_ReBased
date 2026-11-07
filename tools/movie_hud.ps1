# The -MovieHud switch of the capture and suite tools, shared (dot-source). Test-only: see docs/verification/HUD_PARITY.md, "Captures with the movie".
# Tests: tests/movie_hud_test.py. Nothing here stops a process by image name.

. (Join-Path $PSScriptRoot 'run_limit.ps1')

# Reuses a server of these pages on the port, else starts tools/hud_overlay/serve.py. Refuses another checkout's server. Returns the page URL and,
# when this call started it, the server process; give the result to Stop-OwMovieHud.
function Start-OwMovieHud {
    param([Parameter(Mandatory)][int]$Port)
    $url = "http://127.0.0.1:$Port/"
    $pages = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'hud_overlay')).Path
    try { $running = Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 $url } catch { $running = $null }
    if ($running) {
        if ("$($running.Headers['X-OpenWillow-Pages'])" -ne $pages) { throw "Port $Port serves other pages; stop that server or pass another port." }
        return [pscustomobject]@{ Url = $url; Server = $null; Port = $Port }
    }
    $server = Start-Process -PassThru -WindowStyle Hidden -FilePath python -ArgumentList @(
        "`"$(Join-Path $PSScriptRoot 'hud_overlay/serve.py')`"", '--port', $Port)
    [void]$server.Handle # open now: stopping it later goes through this object (run_limit.ps1)
    $result = [pscustomobject]@{ Url = $url; Server = $server; Port = $Port }
    try {
        Start-Sleep 2
        Invoke-WebRequest -UseBasicParsing -TimeoutSec 5 $url | Out-Null
    } catch {
        Stop-OwMovieHud $result
        throw "The page server did not answer on port $Port (it exits when the converted movies are missing; run serve.py by hand to see why): $_"
    }
    return $result
}

# Stops the server only if Start-OwMovieHud started it. $null (a run without -MovieHud) does nothing.
function Stop-OwMovieHud {
    param($MovieHud)
    if (!$MovieHud -or !$MovieHud.Server) { return }
    # In a caller's finally: a failed stop is said, and must not keep the caller from releasing its lock.
    try { [void](Stop-OwStartedProcess -Process $MovieHud.Server -Expected @('serve.py', "--port $($MovieHud.Port)")) }
    catch { Write-Warning "Could not stop the page server pid $($MovieHud.Server.Id): $_" }
}

# True when the log proves the run's frames had the movie to show: the host accepted the movie, never fell back to the canvas, no capture gave up on it, and each
# frame's "OWCAPTUREHUD shot" line follows that driver's "OWCAPTUREHUD ready" line. -Driver: this driver must have reached its ready line. -RequireShot: and taken a frame.
# It does not prove the frames show the movie: someone still looks at them. Prints the reason when false.
function Test-OwMovieHudLog {
    param([Parameter(Mandatory)][string]$Log, [string]$Driver, [switch]$RequireShot)
    $text = if (Test-Path -LiteralPath $Log) { "$(Get-Content -LiteralPath $Log -Raw)" } else { '' }
    $ready = @{}
    $shots = @{}
    $early = $null
    foreach ($line in [regex]::Matches($text, 'OWCAPTUREHUD (ready|shot) driver=(\S+)[^\r\n]*')) {
        $name = $line.Groups[2].Value
        if ($line.Groups[1].Value -eq 'ready') { $ready[$name] = $true; continue }
        $shots[$name] = $true
        if (!$ready[$name] -and !$early) { $early = "a frame before its driver's ready line: $($line.Value)" }
    }
    $why = if ($text -match 'OWCAPTUREHUD FAIL[^\r\n]*') { $Matches[0] }
        elseif ($text -match 'OpenWillow HUD path: canvas[^\r\n]*') { $Matches[0] }
        elseif ($text -notmatch 'OpenWillow HUD path: movie') { 'no "OpenWillow HUD path: movie" line' }
        elseif ($early) { $early }
        elseif ($Driver -and !$ready[$Driver]) { "no ""OWCAPTUREHUD ready driver=$Driver"" line" }
        elseif ($Driver -and $RequireShot -and !$shots[$Driver]) { "no ""OWCAPTUREHUD shot driver=$Driver"" line" }
    if ($why) { Write-Host "MOVIE HUD FAIL: $why ($Log)"; return $false }
    Write-Host "MOVIE HUD ok: the host accepted the movie, did not fall back, and every frame logged came after its driver's ready line ($Log)"
    return $true
}
