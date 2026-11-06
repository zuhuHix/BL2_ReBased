# The time limit for a launcher's scripted run, and how it stops what it started. Dot-sourced by run_ue_flash_hud.ps1. Tests:
# tests/run_limit_test.py (the rules) and tests/run_ue_flash_hud_test.py (the real launcher with stubs). Nothing here stops a process by image name.

# An estimate, not measured: a scripted capture should take well under this, and a first run with shader work longer than a normal one.
$script:OwScriptedRunSeconds = 1200
# -owgunshots walks a list of any length (-owgunids), so it gets more. Also an estimate: for a long list pass -TimeoutSeconds.
$script:OwGunShotsSeconds = 3600

# The switches that run by themselves and quit, one for each driver that reaches a quit in the host (search for "quit" and RequestExit). This is a list
# on purpose: a new driver has to be added here. A pattern on the name caught -owcombattest (it only spawns a target for play), -owinventoryselftest (it only
# runs checks) and -owrangeshots (it only adds frames to a range run); none of those quits.
$script:OwScriptedSwitches = @(
    'owinvshots', 'owcombatshots', 'owgunshots', 'owfiringshots', 'owphaselockshots', 'owpickupcardshots', 'owmapshots', 'owstatusshots', 'owminimapshots',
    'owhudshots', 'owhuddownshots', 'owhudfxshots', 'owskillhudshots', 'owrangesurvey', 'owambientshots', 'owinvopenbench', 'owfiringtest', 'owreloadphaselocktest',
    'owmissionlogtest', 'owambienttest', 'owinteractivetest', 'owmovertest', 'owquesttest', 'owrangetest', 'owvendortest', 'owinventoryactions', 'owslicegate')

# The -Extra switches, one each. Launched with powershell -File, -Extra arrives as one comma-joined string.
function Get-OwSwitchNames {
    param([string[]]$Extra = @())
    foreach ($switch in ($Extra -split ',')) { ($switch.Trim() -replace '^-', '') -replace '=.*$', '' }
}

function Test-OwScriptedRun {
    param([string[]]$Extra = @())
    $names = @(Get-OwSwitchNames -Extra $Extra)
    if ($names -contains 'owslicegatestay') { $names = @($names | Where-Object { $_ -ne 'owslicegate' }) } # the gate's run does not quit with this
    foreach ($name in $names) {
        if ($script:OwScriptedSwitches -contains $name) { return $true }
    }
    return $false
}

# 0 means no limit. Without -GameWindow there is never one: that is the maintainer's editor. With it, an explicit -TimeoutSeconds wins;
# otherwise a scripted run gets the default and a person playing gets none.
function Get-OwRunLimitSeconds {
    param([int]$TimeoutSeconds = -1, [string[]]$Extra = @(), [switch]$GameWindow)
    if (!$GameWindow) { return 0 }
    if ($TimeoutSeconds -ge 0) { return $TimeoutSeconds }
    if ((Get-OwSwitchNames -Extra $Extra) -contains 'owgunshots') { return $script:OwGunShotsSeconds }
    if (Test-OwScriptedRun -Extra $Extra) { return $script:OwScriptedRunSeconds }
    return 0
}

# Waits for the process. True when it exited, false when the limit passed with it still running. 0: no limit.
function Wait-OwStartedProcess {
    param([Parameter(Mandatory)][System.Diagnostics.Process]$Process, [int]$LimitSeconds = 0)
    [void]$Process.Handle # without an open handle, ExitCode reads empty after the process has gone
    if ($LimitSeconds -le 0) { $Process.WaitForExit(); return $true }
    return $Process.WaitForExit($LimitSeconds * 1000)
}

# Warns about what a stopped or crashed process left running, by name and id, and stops none of it. Evidence for now: whether the editor's workers exit by
# themselves when it dies is not known, and -Parent still finds them after it has gone.
function Write-OwLeftChildren {
    param([Parameter(Mandatory)][int]$Parent)
    foreach ($child in @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$Parent")) {
        Write-Warning "Still running after the game stopped: $($child.Name) pid $($child.ProcessId) (not stopped)."
    }
}

# Stops one process this run started, through its own Process object: that holds an open handle (the caller opens it right after the start), so it
# cannot reach another process that was given the same id. The command line must also still hold every expected piece. True when stopped or already gone.
function Stop-OwStartedProcess {
    param([Parameter(Mandatory)][System.Diagnostics.Process]$Process, [Parameter(Mandatory)][string[]]$Expected)
    if ($Process.HasExited) { return $true }
    $found = Get-CimInstance Win32_Process -Filter "ProcessId=$($Process.Id)"
    $commandLine = if ($found) { "$($found.CommandLine)" } else { '' }
    foreach ($piece in $Expected) {
        if ($commandLine.IndexOf($piece, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            Write-Warning "Not stopping pid $($Process.Id): its command line does not contain '$piece'."
            return $false
        }
    }
    $Process.Kill()
    [void]$Process.WaitForExit(10000)
    return $Process.HasExited
}
