# Slice-session watch helpers (dot-source after realgame.ps1; lane capture-prep, 2026-11-11). A capture's -WatchSpec switch uses them to run the
# read-only recorder scripts/slice_watch.py beside its own steps, and, before the game is closed, to dump it and check the dump with
# tools/real_game/slice_watch_check.py. Reads only: nothing here sends input or changes the game; the capture's own steps do what they always did.
# The spec file (function names to hook, what a usable dump must hold) is the runner's, kept under the ignored local/. Contains no game data.

# A label or dump name the recorder accepts: a plain file name.
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
# The watch never stops a capture: a watch that does not start leaves the capture's own steps to run, its snapshots do nothing, and only the
# capture's 'watch' stage fails (Complete-SliceWatch throws inside that stage's try).
$script:SliceWatchOn = $false
$script:SliceWatchError = 'not started'
# Load the recorder into the capture's output folder (the same RG_OUT as the capture's own probes) and start it. Returns its answer (how many hooks
# resolved, which names did not), or an object with only 'error' when it did not start.
function Start-SliceWatch([string]$Spec, [string]$OutFolder, [string]$Label) {
    Assert-WatchName $Label
    try {
        $full = Resolve-WatchSpec $Spec
        Test-WatchReply "$(Invoke-GamePyFile (Join-Path $PSScriptRoot 'scripts/slice_watch.py') $OutFolder)" 'load' | Out-Null
        $answer = Test-WatchReply (Invoke-GamePy "import json; print(json.dumps(sw_start('$Label', r'$full'), default=str))" 120) 'start' | ConvertFrom-Json
        $script:SliceWatchOn = $true; $script:SliceWatchError = ''
        $answer
    } catch {
        $script:SliceWatchOn = $false; $script:SliceWatchError = "$_"
        Write-Warning "slice watch not started: $_"
        [pscustomobject]@{ error = "$_" }
    }
}
# One labelled state snapshot, or one read of every AI pawn. Nothing when the watch is off; a failure is a warning (and the snapshot's absence fails
# the usability check), never an error that would end the capture.
function Save-SliceWatchSnap([string]$Label) {
    Assert-WatchName $Label
    if (!$script:SliceWatchOn) { return }
    try { Test-WatchReply (Invoke-GamePy "print(len(sw_snap('$Label')))" 30) "snap $Label" | Out-Null } catch { Write-Warning "slice watch snapshot ${Label}: $_" }
}
function Save-SliceWatchPawns([string]$Label) {
    Assert-WatchName $Label
    if (!$script:SliceWatchOn) { return }
    try { Test-WatchReply (Invoke-GamePy "import json; print(json.dumps(sw_pawns('$Label')))" 30) "pawns $Label" | ConvertFrom-Json }
    catch { Write-Warning "slice watch pawns ${Label}: $_"; [pscustomobject]@{ error = "$_" } }
}
# Before the game closes: an end snapshot, the dump, the hooks off, then the usability check on the file. (On an error before this, the
# observers stay until the capture's finalizer stops the game; they never block or write, and the finalizer is left word for word as reviewed.) Returns the counts, the check's lines
# (counts and pass/fail only) and usable (true only when the check exited 0).
function Complete-SliceWatch([string]$Spec, [string]$OutDir, [string]$Name = 'watch') {
    Assert-WatchName $Name
    if (!$script:SliceWatchOn) { throw "the watch did not start: $script:SliceWatchError" }
    $full = Resolve-WatchSpec $Spec
    Save-SliceWatchSnap 'end'
    $counts = Test-WatchReply (Invoke-GamePy "import json; print(json.dumps(sw_dump('$Name')))" 120) 'dump' | ConvertFrom-Json
    Test-WatchReply (Invoke-GamePy 'print(sw_stop())' 30) 'stop' | Out-Null
    $script:SliceWatchOn = $false
    $file = Join-Path $OutDir "$Name.json"
    if (!(Test-Path -LiteralPath $file)) { throw "slice watch: $file was not written" }
    $lines = @(& python (Join-Path $PSScriptRoot 'slice_watch_check.py') $file --spec $full 2>&1 | ForEach-Object { "$_" })
    [pscustomobject]@{ counts = $counts; check = $lines; usable = ($LASTEXITCODE -eq 0); file = $file }
}
# The mods-off step of a capture's -SdkModsOff switch, after its backup record and before the driver and the launch: Set-SdkModsOff (pending.ps1)
# rewrites each enabled setting to disabled, records it first, and the capture's restore puts the settings back from the verified backup.
function Invoke-SdkModsOffStep($Log) {
    $off = @(Set-SdkModsOff)
    $Log.sdk_mods_off = $off
    "SDK mods off for this run: $($off -join ', ')"
}
