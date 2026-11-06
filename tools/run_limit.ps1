# The time limit for a launcher's scripted run, and how it stops what it started. Dot-sourced by run_ue_flash_hud.ps1; no editor
# is needed to test it (tests/run_limit_test.py). Nothing here stops a process by image name.

$script:OwScriptedRunSeconds = 1200 # 20 minutes: a scripted capture takes about 5, a first run with shader work longer

# The switches that drive a run and quit by themselves (-owinvshots, -owfiringtest, -owinventoryactions, -owinvopenbench=3, -owslicegate).
# Launched with powershell -File, -Extra arrives as one comma-joined string.
function Test-OwScriptedRun {
    param([string[]]$Extra = @())
    foreach ($switch in ($Extra -split ',')) {
        if ($switch.Trim() -match '^-ow[a-z0-9]*(shots|tests?|actions|bench|gate)(=|$)') { return $true }
    }
    return $false
}

# 0 means no limit. Without -GameWindow there is never one: that is the maintainer's editor. With it, an explicit -TimeoutSeconds wins;
# otherwise a scripted run gets the default and a person playing gets none.
function Get-OwRunLimitSeconds {
    param([int]$TimeoutSeconds = -1, [string[]]$Extra = @(), [switch]$GameWindow)
    if (!$GameWindow) { return 0 }
    if ($TimeoutSeconds -ge 0) { return $TimeoutSeconds }
    if (Test-OwScriptedRun -Extra $Extra) { return $script:OwScriptedRunSeconds }
    return 0
}

# Waits for the process. True when it exited, false when the limit passed with it still running. 0: no limit.
function Wait-OwStartedProcess {
    param([Parameter(Mandatory)][System.Diagnostics.Process]$Process, [int]$LimitSeconds = 0)
    if ($LimitSeconds -le 0) { $Process.WaitForExit(); return $true }
    return $Process.WaitForExit($LimitSeconds * 1000)
}

# Stops one process this run started: by its id, and only when its command line still holds every expected piece, so a reused id or
# someone else's process is left alone. True when it was stopped or had already gone.
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
    Stop-Process -Id $Process.Id -Force
    [void]$Process.WaitForExit(10000)
    return $Process.HasExited
}
