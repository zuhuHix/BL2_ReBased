# Batch driver for the heavy suites (AI-assisted (Claude Opus 5.5); hub glue from 2026-11-11, tracked since 2026-11-12).
# The only way the test runner starts a heavy suite. docs/TOOLING.md, "Heavy suites: the suite map", explains it.
#
#   powershell -NoProfile -File tools/run_suites.ps1 -Worktree <test worktree> -Commit <40-char sha> -OutDir <log folder>
#              [-Suites a,b,c] [-Repeat <suite>:<n>] [-Map <suites.txt>]
#
# Reads the map in the worktree under test, <Worktree>/tools/suites.txt (or -Map): the map of -Commit, since the worktree
# must be clean and at -Commit. Asks tools/suites_due.py next to this script what is due and to record passes; the ledger
# is the helper's (HUB_DIR, or local/hub in the main checkout). Each suite line has a
# "#pass <name> | <must match> | <must not match>" line; separators there need a space before the "|" (an alternation
# such as FAIL|ERROR has none).
# Row verdicts: PASS, FAIL-RERUN (failed, one rerun follows), FLAKY-PASS (the rerun passed), FAIL (the rerun failed too).
# Exit code: the number of suites that ended FAIL. 100 = refused before running anything; 101 = stopped because an editor
# from this worktree was still running 120 s after a suite (its pid, start time and command line are printed).
# It starts no build, kills no process, deletes no lock and pushes nothing. The suite scripts take the UE lock themselves.
param(
    [Parameter(Mandatory)][string]$Worktree,
    [Parameter(Mandatory)][string]$Commit,
    [Parameter(Mandatory)][string]$OutDir,
    [string[]]$Suites,
    [string[]]$Repeat,
    [string]$Map
)
$ErrorActionPreference = 'Stop'
# Only for Get-OwUeBusyProcess (the editor names the lock waits for). The driver never takes the lock: the suite scripts do.
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
function Refuse([string]$Why) { Write-Host "run_suites: REFUSED: $Why"; exit 100 }

$dueScript = Join-Path $PSScriptRoot 'suites_due.py'
$python = Get-Command python -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
if (!$python) { Refuse 'no python on PATH (tools/suites_due.py needs it)' }
$python = $python.Source
$Worktree = (Resolve-Path -LiteralPath $Worktree).Path

# 1. The worktree must be clean and at the commit we were told to test.
if ($Commit -notmatch '^[0-9a-f]{40}$') { Refuse "-Commit must be a 40-character sha, got '$Commit'" }
$head = (git -C $Worktree rev-parse HEAD).Trim()
if ($head -ne $Commit) { Refuse "$Worktree is at $head, not $Commit" }
if (git -C $Worktree status --porcelain) { Refuse "$Worktree has uncommitted changes (git status --porcelain is not empty)" }

# Read suites.txt: name | minutes | paths | command, and the #pass lines.
if (!$Map) { $Map = Join-Path $Worktree 'tools\suites.txt' }
if (!(Test-Path -LiteralPath $Map)) { Refuse "no suite map at $Map" }
$map = (Resolve-Path -LiteralPath $Map).Path
$commands = [ordered]@{}; $passRules = @{}
foreach ($raw in Get-Content -LiteralPath $map) {
    $line = $raw.Trim()
    if ($line -match '^#pass\s+(\S+)\s+\|(.*)$') {
        $parts = @("|$($Matches[2])" -split '\s*\|(?=\s|$)' | Select-Object -Skip 1 | ForEach-Object { $_.Trim() })
        $passRules[$Matches[1]] = @{ Need = $parts[0]; Never = $(if ($parts.Count -gt 1) { $parts[1] } else { '' }) }
        continue
    }
    if ($line -eq '' -or $line.StartsWith('#')) { continue }
    $f = $line -split '\|', 4
    $name = $f[0].Trim()
    if ($name -eq '*ignore*') { continue }
    if ($f.Count -lt 4) { Refuse "suites.txt line has fewer than 4 fields: $line" }
    $commands[$name] = $f[3].Trim()
}

# -Repeat suite:n, as a table.
$times = @{}
foreach ($r in @($Repeat | ForEach-Object { $_ -split "," } | Where-Object { $_ })) {
    if ($r -notmatch '^([^:]+):(\d+)$' -or [int]$Matches[2] -lt 1) { Refuse "-Repeat wants <suite>:<n>, got '$r'" }
    $times[$Matches[1]] = [int]$Matches[2]
}

# 2. Which suites: the given names, or what suites_due.py says is due (map order).
Push-Location $Worktree
try {
    if ($Suites) {
        $list = @($Suites | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
    } else {
        $due = @(& $python $dueScript --map $map due $Commit)
        if ($LASTEXITCODE -ne 0) { Refuse "suites_due.py due failed (exit $LASTEXITCODE):`n$($due -join "`n")" }
        $list = @($due | Where-Object { $_ -and $_ -notmatch '^(-- |nothing due)' } | ForEach-Object { ($_ -split '\s+')[0] })
    }
} finally { Pop-Location }
foreach ($n in $times.Keys) { if ($list -notcontains $n) { $list += $n } }
foreach ($n in $list) {
    if (!$commands.Contains($n)) { Refuse "no suite named '$n' in $map" }
    if (!$passRules.ContainsKey($n)) { Refuse "suite '$n' has no '#pass $n | ... | ...' line in $map" }
}
if ($list.Count -eq 0) { Write-Host "run_suites: nothing due on $Commit"; exit 0 }

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OutDir = (Resolve-Path -LiteralPath $OutDir).Path
$tsv = Join-Path $OutDir 'results.tsv'
if (!(Test-Path -LiteralPath $tsv)) { "suite`tattempt`tverdict`texit`tseconds`tlog" | Out-File -Encoding utf8 $tsv }
$rows = @()

# Before each attempt: wait up to 120 s until no Unreal editor started from this worktree (its command line names the
# worktree) is still running, since some suites refuse to start next to one. Only looks; never stops a process.
function Wait-NoEditor {
    $needle = $Worktree.Replace('/', '\').ToLowerInvariant()
    for ($t = 0; $t -le 120; $t += 5) {
        $left = @(Get-OwUeBusyProcess | ForEach-Object { Get-CimInstance Win32_Process -Filter "ProcessId = $($_.Id)" } |
            Where-Object { $_.CommandLine -and $_.CommandLine.Replace('/', '\').ToLowerInvariant().Contains($needle) })
        if (!$left.Count) { return }
        Start-Sleep -Seconds 5
    }
    foreach ($p in $left) {
        Write-Host "run_suites: STOPPED: editor still running after 120 s: pid $($p.ProcessId), started $($p.CreationDate), command line: $($p.CommandLine)"
    }
    Write-Host ""
    Write-Host "suite`tattempt`tverdict`texit`tseconds`tlog"
    $script:rows | ForEach-Object { Write-Host $_ }
    exit 101
}

# The checkout must stay at -Commit with no tracked file changed, or a pass would be recorded for content that did not run.
function Get-Drift {
    $now = (git -C $Worktree rev-parse HEAD).Trim()
    if ($now -ne $Commit -or (git -C $Worktree status --porcelain --untracked-files=no)) { return 'the checkout changed during the run' }
    return $null
}

# One attempt: run the command as its own powershell child from the worktree, log stdout+stderr, judge it.
function Invoke-Attempt([string]$Name, [string]$Attempt, [string]$LogName) {
    Wait-NoEditor
    $drift = Get-Drift
    if ($drift) {
        Write-Host ("  {0,-24} {1,-10} FAILED: {2}" -f $Name, $Attempt, $drift)
        return @{ Ok = $false; Code = '-'; Seconds = 0; Log = '-' }
    }
    $log = Join-Path $OutDir $LogName
    # -EncodedCommand instead of -Command, so quotes in the command survive; the tail passes the real exit code on
    # (powershell -Command alone turns every failure into 1).
    $script = "`$global:LASTEXITCODE = 0`n$($commands[$Name])`n`$ok = `$?`nif (`$LASTEXITCODE) { exit `$LASTEXITCODE }`nif (-not `$ok) { exit 1 }`nexit 0"
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($script))
    Set-Location $Worktree
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $ErrorActionPreference = 'Continue'
    & powershell -NoProfile -EncodedCommand $encoded 2>&1 | ForEach-Object { "$_" } | Out-File -Encoding utf8 $log
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    $seconds = [int]$clock.Elapsed.TotalSeconds
    $text = [IO.File]::ReadAllText($log)
    $rule = $passRules[$Name]; $why = @()
    if ($code -ne 0) { $why += "exit code $code" }
    if ($rule.Need -and ![regex]::IsMatch($text, $rule.Need, 'Multiline')) { $why += "missing /$($rule.Need)/" }
    if ($rule.Never -and [regex]::IsMatch($text, $rule.Never, 'Multiline')) { $why += "found /$($rule.Never)/" }
    $ok = $why.Count -eq 0
    Write-Host ("  {0,-24} {1,-10} {2} {3}s {4}" -f $Name, $Attempt, $(if ($ok) { 'ok' } else { 'FAILED: ' + ($why -join ', ') }), $seconds, $LogName)
    return @{ Ok = $ok; Code = $code; Seconds = $seconds; Log = $LogName }
}
function Add-Row([string]$Name, [string]$Attempt, [string]$Verdict, $r) {
    $row = "$Name`t$Attempt`t$Verdict`t$($r.Code)`t$($r.Seconds)`t$($r.Log)"
    $row | Out-File -Encoding utf8 -Append $tsv
    $script:rows += $row
}

# 3-6. Each suite in turn; a failed attempt gets exactly one rerun; record passes at once.
$failed = 0
foreach ($name in $list) {
    $n = if ($times.ContainsKey($name)) { $times[$name] } else { 1 }
    $allPassed = $true
    for ($i = 1; $i -le $n; $i++) {
        $tag = if ($i -eq 1) { '' } else { ".run$i" }
        $first = Invoke-Attempt $name "run$i" "$name$tag.log"
        if ($first.Ok) { Add-Row $name "run$i" 'PASS' $first; continue }
        Add-Row $name "run$i" 'FAIL-RERUN' $first
        $second = Invoke-Attempt $name "run$i.rerun" "$name$tag.rerun.log"
        if ($second.Ok) { Add-Row $name "run$i.rerun" 'FLAKY-PASS' $second; continue }
        Add-Row $name "run$i.rerun" 'FAIL' $second
        $allPassed = $false
    }
    $drift = Get-Drift
    if ($drift) {
        Write-Host "  ${name}: not recorded: $drift"
        Add-Row $name 'record' 'FAIL' @{ Code = '-'; Seconds = 0; Log = '-' }
        $failed++
    } elseif ($allPassed) {
        Push-Location $Worktree
        & $python $dueScript --map $map record $Commit $name
        if ($LASTEXITCODE -ne 0) { Write-Host "run_suites: WARNING: recording $name failed (exit $LASTEXITCODE)" }
        Pop-Location
    } else { $failed++ }
}

# 7. The table, and the number of FAIL suites as the exit code.
Write-Host ""
Write-Host "suite`tattempt`tverdict`texit`tseconds`tlog"
$rows | ForEach-Object { Write-Host $_ }
Write-Host "-- $($list.Count) suite(s), $failed FAIL, on $Commit; table appended to $tsv"
exit $failed
