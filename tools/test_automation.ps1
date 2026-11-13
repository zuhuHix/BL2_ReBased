param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    # The ue_automation suite's test names and how many tests they select. This is the one place both live:
    # tests/automation_paths_test.py reads these two defaults and checks them against the test paths declared under
    # host/ue5/OpenWillow/Source and against the suite's #pass line in tools/suites.txt. Change them together when a test is added.
    [string[]]$Tests = @('OpenWillow.CombatTarget', 'OpenWillow.FocusCam', 'OpenWillow.MayaMovement', 'OpenWillow.MayaState',
        'OpenWillow.Phaselock', 'OpenWillow.Skills', 'OpenWillow.SkillEffects', 'OpenWillow.WeaponAssembly'),
    [int]$Expected = 13,
    [int]$TimeoutSeconds = 1200,
    [int]$LockWaitSeconds = 3600,
    # Print the command line this run would start and exit 0: no lock, no editor, no folder or log is created.
    [switch]$DryRun
)
# AI-assisted (Claude Code), 2026-11-13. The ue_automation suite (tools/suites.txt) as a tracked launcher; it replaces the inline
# command the row held until then. One windowed game process (FocusCam writes frames) runs "Automation RunTests" on the names above
# and quits. The summary counts the framework's "Test Completed" lines; exit 0 only when success equals -Expected and none failed.
# A list of the completed test paths follows, so a lost or shadowed test shows in the suite log. Host tests only: nothing here is
# compared with the running game.
$ErrorActionPreference = 'Stop'
if ($PSBoundParameters.ContainsKey('Tests') -and !$PSBoundParameters.ContainsKey('Expected')) {
    throw '-Tests changes which tests run: pass -Expected with the number of tests the new names select'
}
$repo = Split-Path -Parent $PSScriptRoot
$editor = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe'
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$logDir = Join-Path $repo 'local/automation'
$log = Join-Path $logDir ('run-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
$arguments = @("`"$project`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-game', '-windowed', '-ResX=1280', '-ResY=720', '-d3d11',
    '-unattended', '-nosplash', '-ddc=InstalledNoZenLocalFallback',
    ('-ExecCmds="Automation RunTests ' + ($Tests -join '+') + '; Quit"'), "-abslog=`"$log`"")
if ($DryRun) {
    Write-Output "DRY RUN (nothing started): `"$editor`" $($arguments -join ' ')"
    Write-Output "DRY RUN tests=$($Tests -join ',') expected=$Expected timeout=$TimeoutSeconds editor_found=$(Test-Path -LiteralPath $editor)"
    exit 0
}
if (!(Test-Path -LiteralPath $editor)) { throw "UnrealEditor.exe not found under $Engine (set OPENWILLOW_UE or pass -Engine)" }
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$process = $null
$code = 2
Enter-OwUeLock -Owner 'test_automation' -TimeoutSeconds $LockWaitSeconds
try {
    $process = Start-Process -FilePath $editor -ArgumentList $arguments -PassThru
    Write-Output "Launched automation run pid $($process.Id); log: $log"
    if (!$process.WaitForExit($TimeoutSeconds * 1000)) {
        # Only the process this run started is stopped, by its own id.
        Write-Output "Automation run timed out after $TimeoutSeconds s; stopping pid $($process.Id)"
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
    Start-Sleep -Seconds 3
    $ok = @(); $bad = @()
    if (Test-Path -LiteralPath $log) {
        $ok = @(Select-String -LiteralPath $log -Pattern 'Test Completed\. Result=\{Success\}')
        $bad = @(Select-String -LiteralPath $log -Pattern 'Test Completed\. Result=\{(?!Success)')
    } else {
        Write-Output "No log at $log (the editor did not start or wrote nowhere)"
    }
    foreach ($m in $ok + $bad) { $m.Line }
    $paths = @(foreach ($m in $ok + $bad) { if ($m.Line -match 'Path=\{([^}]*)\}') { $Matches[1] } })
    Write-Output ('AUTOMATION PATHS ' + $paths.Count + ': ' + ($paths -join ', '))
    Write-Output ('AUTOMATION SUMMARY success=' + $ok.Count + ' failed=' + $bad.Count + ' expected=' + $Expected)
    if ($ok.Count -eq $Expected -and $bad.Count -eq 0) { $code = 0 } else { $code = 1 }
    Write-Output "Log: $log"
} finally {
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    Exit-OwUeLock
}
exit $code
