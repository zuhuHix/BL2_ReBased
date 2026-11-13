# Slice-session helpers (dot-source after realgame.ps1; lane capture-prep, 2026-11-11). A capture's -WatchSpec switch uses them to run the
# read-only recorder scripts/slice_watch.py beside its own steps and, before the game is closed, to end its recording and check it with
# tools/real_game/slice_watch_check.py; its -SdkModsOff switch uses Invoke-SdkModsOffStep. Nothing here sends input or changes the game.
# The spec file (function names to hook, what a usable recording must hold) is the runner's, kept under the ignored local/. No game data.

# A label or recording name the recorder accepts: a plain file name.
function Assert-WatchName([string]$Name) { if ($Name -notmatch '^[A-Za-z0-9][A-Za-z0-9_-]*$') { throw "slice watch: '$Name' is not a plain name" } }
# The driver answers a game-side error as text: never take it for a reply.
function Test-WatchReply([string]$Reply, [string]$What) {
    if ($Reply -match 'Traceback \(most recent call last\)') { throw "slice watch ${What}: $Reply" }
    "$Reply".Trim()
}
# The spec's full path, refused when it is missing or would break out of the Python string it is passed in.
function Resolve-WatchSpec([string]$Spec) {
    if (!(Test-Path -LiteralPath $Spec -PathType Leaf)) { throw "slice watch spec $Spec does not exist" }
    $full = (Resolve-Path -LiteralPath $Spec).ProviderPath
    if ($full -match "['\r\n]") { throw "slice watch spec path $full holds a quote or a line break" }
    $full
}
# Before a capture changes anything: the spec is checked by the recorder's own rules (types, finite numbers, fixed bounds, known checks).
function Assert-WatchSpec([string]$Spec) {
    $full = Resolve-WatchSpec $Spec
    $out = @(& python (Join-Path $PSScriptRoot 'slice_watch_check.py') --validate-spec $full 2>&1 | ForEach-Object { "$_" })
    if ($LASTEXITCODE -ne 0) { throw "slice watch spec refused: $($out -join ' ')" }
}
# Before a capture changes anything: enough free disk for the backup of every run root (twice: the backup and a pre-restore copy), the recording's
# byte cap and a reserve, on the drive that holds local/realgame. Returns the free and needed bytes.
function Assert-DiskReserve([long]$RecordingBytes = 300000000, [long]$ReserveBytes = 2000000000) {
    $roots = Get-RunRoots
    $size = 0L
    foreach ($r in $roots.Values) { if (Test-Path -LiteralPath $r) { $size += (Get-ChildItem -LiteralPath $r -Recurse -File -Force | Measure-Object Length -Sum).Sum } }
    $need = 2 * $size + $RecordingBytes + $ReserveBytes
    New-Item -ItemType Directory -Force $script:RG | Out-Null
    $drive = [IO.DriveInfo]::new([IO.Path]::GetPathRoot([IO.Path]::GetFullPath($script:RG)))
    if ($drive.AvailableFreeSpace -lt $need) { throw "not enough free disk on $($drive.Name): $($drive.AvailableFreeSpace) bytes free, $need needed (backups, recording, reserve)" }
    [pscustomobject]@{ free = $drive.AvailableFreeSpace; needed = $need }
}
# The watch never stops a capture: a watch that does not start leaves the capture's own steps to run, its snapshots do nothing, and only the
# capture's 'watch' stage fails (Complete-SliceWatch throws inside that stage's try).
$script:SliceWatchOn = $false
$script:SliceWatchError = 'not started'
# One driver-lost condition for the whole run: the first reply that never came (a timeout) sets it, and from then on no helper here talks to
# the driver again; the capture checks it before every later stage and its usability check (Get-DriverLost in cash_capture).
$script:SliceWatchDriverLost = ''
# The same condition for a capture's own steps and finalizer: why the driver counts as lost, or empty.
function Get-DriverLostReason { $script:SliceWatchDriverLost }
function Set-SliceWatchDriverLost($Err) { if (!$script:SliceWatchDriverLost -and "$Err" -match 'no reply') { $script:SliceWatchDriverLost = "$Err" } }
# Called by the captures before every key, click, mouse move and focus check: the helpers here latch a lost driver without throwing, so an
# input that follows one of them with no Py in between would otherwise still go out (review of #251, A5). Throws once the condition is set.
function Assert-DriverAnswers { if ($script:SliceWatchDriverLost) { throw "not sent: the driver stopped answering ($script:SliceWatchDriverLost)" } }
# Load the recorder into the capture's output folder (the same RG_OUT as the capture's own probes) and start it: it writes <OutFolder>/watch.jsonl.
# Returns its answer (how many hooks resolved, which names did not), or an object with only 'error' when it did not start. After a failed or
# timed-out start a stop is sent anyway (best effort): a start that ran late must not leave a recorder running unseen.
function Start-SliceWatch([string]$Spec, [string]$OutFolder, [string]$Label) {
    Assert-WatchName $Label
    try {
        $full = Resolve-WatchSpec $Spec
        Test-WatchReply "$(Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/slice_watch.py') $OutFolder)" 'load' | Out-Null
        $answer = Test-WatchReply (Invoke-GamePy "import json; print(json.dumps(sw_start('$Label', r'$full', 'watch'), default=str))" 120) 'start' | ConvertFrom-Json
        $script:SliceWatchOn = $true; $script:SliceWatchError = ''
        $answer
    } catch {
        $script:SliceWatchOn = $false; $script:SliceWatchError = "$_"
        Set-SliceWatchDriverLost $_
        Write-Warning "slice watch not started: $_"
        # The recovery stop's own "no reply" latches the condition too (review of #251, A6).
        $stop = if ($script:SliceWatchDriverLost) { 'not sent: the driver stopped answering' } else { try { "$(Invoke-GamePy 'print(sw_stop())' 15)".Trim() } catch { Set-SliceWatchDriverLost $_; "no reply: $_" } }
        [pscustomobject]@{ error = "$_"; stop_after_error = $stop }
    }
}
# One labelled state snapshot, or one read of the AI pawns. Nothing when the watch is off; a failure is a warning (and the snapshot's absence fails
# the usability check), never an error that would end the capture. Returns one line saying what happened, so the runner sees it in the log:
# taken, deferred (start and arrival only, until the map settles), NOT TAKEN (asked before the map settled), or the parts that failed.
function Save-SliceWatchSnap([string]$Label) {
    Assert-WatchName $Label
    if (!$script:SliceWatchOn -or $script:SliceWatchDriverLost) { return }
    try {
        $r = Test-WatchReply (Invoke-GamePy "import json; r = sw_snap('$Label'); print(json.dumps({'deferred': bool(r.get('deferred')), 'not_taken': bool(r.get('not_taken')), 'failed': r.get('failed', [])}))" 30) "snap $Label" | ConvertFrom-Json
        if ($r.not_taken) { "slice watch snapshot ${Label}: NOT TAKEN (asked before the map settled)" }
        elseif ($r.deferred) { "slice watch snapshot ${Label}: deferred until the map settles" }
        elseif (@($r.failed).Count) { "slice watch snapshot ${Label}: taken, failed parts: $(@($r.failed) -join ', ')" }
        else { "slice watch snapshot ${Label}: taken" }
    } catch { Set-SliceWatchDriverLost $_; Write-Warning "slice watch snapshot ${Label}: $_" }
}
function Save-SliceWatchPawns([string]$Label) {
    Assert-WatchName $Label
    if (!$script:SliceWatchOn -or $script:SliceWatchDriverLost) { return }
    try { Test-WatchReply (Invoke-GamePy "import json; print(json.dumps(sw_pawns('$Label')))" 30) "pawns $Label" | ConvertFrom-Json }
    catch { Set-SliceWatchDriverLost $_; Write-Warning "slice watch pawns ${Label}: $_"; [pscustomobject]@{ error = "$_" } }
}
# Install the spec's hook groups for one point of the capture's run, one at a time: each group's marker line is flushed before its first hook
# goes in (sw_install_group), and $PauseSeconds pass between two groups of the same point, so a group whose functions fire constantly is
# exercised before the next goes in. Nothing when the watch is off or the driver was lost; a failure is a warning (the usability check fails
# a group installed with names missing), and a lost reply sets the driver-lost condition.
function Install-SliceWatchGroups([string]$At, [int]$PauseSeconds = 15) {
    if ($At -notmatch '^[A-Za-z0-9][A-Za-z0-9_-]*$') { throw "slice watch: '$At' is not a plain group point" }
    if (!$script:SliceWatchOn -or $script:SliceWatchDriverLost) { return }
    try {
        $names = @(Test-WatchReply (Invoke-GamePy "import json; print(json.dumps(sw_group_names_at('$At')))" 30) "groups $At" | ConvertFrom-Json)
        $done = @()
        for ($i = 0; $i -lt $names.Count; $i++) {
            if ($i -gt 0) { Start-Sleep -Seconds $PauseSeconds }
            $done += Test-WatchReply (Invoke-GamePy "import json; print(json.dumps(sw_install_group('$($names[$i])'), default=str))" 120) "group $($names[$i])" | ConvertFrom-Json
        }
        $done
    } catch { Set-SliceWatchDriverLost $_; Write-Warning "slice watch groups ${At}: $_"; [pscustomobject]@{ error = "$_" } }
}
# A flushed marker line in the recording (sw_mark): a stage boundary, so a crash after it is attributed to that stage.
function Save-SliceWatchMark([string]$Label) {
    if ($Label -notmatch '^[A-Za-z0-9][A-Za-z0-9_.:-]*$') { throw "slice watch: '$Label' is not a plain mark" }
    if (!$script:SliceWatchOn -or $script:SliceWatchDriverLost) { return }
    try { Test-WatchReply (Invoke-GamePy "print(sw_mark('$Label'))" 15) "mark $Label" | Out-Null } catch { Set-SliceWatchDriverLost $_; Write-Warning "slice watch mark ${Label}: $_" }
}
# One read of the recorder's settle state, no wait: True only when the map has settled now. A reply that never came latches the
# driver-lost condition. For a labelled read the capture takes only on a settled map (cash_capture's per-kill 'drop').
function Test-SliceWatchSettled {
    if (!$script:SliceWatchOn -or $script:SliceWatchDriverLost) { return $false }
    try { "$(Invoke-GamePy 'print(sw_settled())' 15)".Trim() -ceq 'True' } catch { Set-SliceWatchDriverLost $_; $false }
}
# After a map load, wait (at most $Seconds) until the recorder says the map has settled: the player has a pawn and the map's clock has passed the
# spec's settle_seconds, so the watch's scans are allowed again. Reading only. True when settled; false on a timeout or with the watch off.
function Wait-SliceWatchSettled([int]$Seconds = 40) {
    if (!$script:SliceWatchOn -or $script:SliceWatchDriverLost) { return $false }
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        try { if ("$(Invoke-GamePy 'print(sw_settled())' 15)".Trim() -ceq 'True') { return $true } } catch { Set-SliceWatchDriverLost $_; if ($script:SliceWatchDriverLost) { return $false } }
        Start-Sleep 1
    }
    $false
}
# Before the game closes: an end snapshot, the end of the recording, the hooks off, then the usability check on the file. (On an error before this,
# the observers stay until the capture's finalizer stops the game; they never block or write game state, the file keeps what was flushed, and
# the finalizer is left word for word as reviewed.) Returns the counts, the check's lines (counts and pass/fail only) and usable.
function Complete-SliceWatch([string]$Spec, [string]$OutDir, [string]$Name = 'watch') {
    Assert-WatchName $Name
    if (!$script:SliceWatchOn) { throw "the watch did not start: $script:SliceWatchError" }
    if ($script:SliceWatchDriverLost) { throw "skipped: the driver stopped answering ($script:SliceWatchDriverLost)" }
    $full = Resolve-WatchSpec $Spec
    $null = Save-SliceWatchSnap 'end'
    # One driver-lost condition: a reply that never came, here or before, latches it, and nothing more is sent to the game.
    if ($script:SliceWatchDriverLost) { throw "the driver stopped answering at the end snapshot ($script:SliceWatchDriverLost): no dump or stop was sent" }
    try { $counts = Test-WatchReply (Invoke-GamePy "import json; print(json.dumps(sw_dump('$Name')))" 120) 'dump' | ConvertFrom-Json }
    catch { Set-SliceWatchDriverLost $_; throw }
    try { Test-WatchReply (Invoke-GamePy 'print(sw_stop())' 30) 'stop' | Out-Null } catch { Set-SliceWatchDriverLost $_; throw }
    $script:SliceWatchOn = $false
    $file = Join-Path $OutDir "$Name.jsonl"
    if (!(Test-Path -LiteralPath $file)) { throw "slice watch: $file was not written" }
    $lines = @(& python (Join-Path $PSScriptRoot 'slice_watch_check.py') $file --spec $full 2>&1 | ForEach-Object { "$_" })
    [pscustomobject]@{ counts = $counts; check = $lines; usable = ($LASTEXITCODE -eq 0); file = $file }
}
# The settings Set-SdkModsOff (pending.ps1) would turn off: every sdk_mods\settings\*.json whose "enabled" is true. Reading only.
function Get-SdkModsEnabled([string]$SettingsDir = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods\settings')) {
    @(foreach ($file in Get-ChildItem -LiteralPath $SettingsDir -Filter *.json -File) {
        $json = Get-Content -Raw -LiteralPath $file.FullName | ConvertFrom-Json
        if ($json.PSObject.Properties.Name -contains 'enabled' -and $json.enabled -eq $true) { $file.BaseName }
    }) | Sort-Object
}
# The mods-off step of a capture's -SdkModsOff switch. The captures call it after setting $launchAttempted, so their unchanged finalizer restores
# every root from the verified backup even when the launch never happens or the write stops half way. -Expected: the exact setting names the run
# may turn off; any other set (one more, one less) is refused before anything is written.
function Invoke-SdkModsOffStep($Log, [string[]]$Expected = @()) {
    # pwsh -File passes 'a,b,c' as one string: split it.
    $Expected = @($Expected | ForEach-Object { "$_" -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    if (@($Expected).Count) {
        $would = @(Get-SdkModsEnabled)
        $want = @($Expected | Sort-Object)
        if (($would -join '|') -cne ($want -join '|')) { throw "refusing SDK mods off: the enabled settings are [$($would -join ', ')], the expected are [$($want -join ', ')]; nothing was changed" }
    }
    $off = @(Set-SdkModsOff)
    $Log.sdk_mods_off = $off
    "SDK mods off for this run: $($off -join ', ')"
}
