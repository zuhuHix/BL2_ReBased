[CmdletBinding(PositionalBinding = $false)]
param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    # UModel build 1590 (external, never inside the repository).
    [string]$UModel = $env:OPENWILLOW_UMODEL,
    [string]$Python = 'python',
    # prepare: identity, UModel exports, editor job; mesh: editor import of mesh/materials + reference pose;
    # anims: MD5 clips -> bone tracks, then the editor creates the AnimSequences; zone: the zone-colour material
    # (tools/phaselock_target_editor.py); attach: the worn static meshes with that material (tools/phaselock_target_attach_editor.py),
    # then their placement in host_target.json. 'all' runs them in order. Comma list: -Steps prepare,attach, also under powershell -File.
    [string[]]$Steps = @('all')
)
# AI-assisted. Imports a stock Phaselock target (the bullymong, tools/prepare_phaselock_target.py) into the ignored UE
# content folder /Game/OpenWillow/Characters/PhaselockTarget, reusing the slice NPC editor script
# (tools/slice_npc_editor.py) with this tool's own job file. Game-derived data stays under local/ and the ignored
# Content folder; nothing is added to Git. The host uses it with -owcombattest -owcombatpawn=Bullymong.
#
# The editor runs under the machine-wide UE run lock (tools/ue_lock.ps1), as tools/seed_slice_npc_assets.ps1 does. On a machine where
# several lanes share UE, take the machine-wide lock first; this script does not manage it.
$ErrorActionPreference = 'Stop'
# powershell -File passes a comma list as one string, which a ValidateSet would reject, so the list is split and checked here.
$Steps = @($Steps | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$unknownSteps = @($Steps | Where-Object { $_ -notin 'all', 'prepare', 'mesh', 'anims', 'zone', 'attach' })
if ($unknownSteps.Count -or !$Steps.Count) { throw "Unknown or empty -Steps: '$($unknownSteps -join ',')'. Use any of all, prepare, mesh, anims, zone, attach." }
# Prints the root and refuses when the current directory is in another checkout (tools/checkout_guard.ps1).
. (Join-Path $PSScriptRoot 'checkout_guard.ps1')
$repo = Assert-OwCheckout $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$cmd = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor-Cmd.exe'
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
$out = Join-Path $repo 'local/phaselock/target'
$logs = Join-Path $out 'logs'
$tool = Join-Path $PSScriptRoot 'prepare_phaselock_target.py'
$editorScript = Join-Path $PSScriptRoot 'slice_npc_editor.py'
New-Item -ItemType Directory -Force -Path $out, $logs | Out-Null
$run = { param($name) $Steps -contains 'all' -or $Steps -contains $name }

function Invoke-Tool([string]$Step) {
    $extra = @()
    if ($Game) { $extra += @('--game', $Game) }
    if ($UModel) { $extra += @('--umodel', $UModel) }
    & $Python $tool $Step @extra
    if ($LASTEXITCODE -ne 0) { throw "prepare_phaselock_target.py $Step failed: $LASTEXITCODE" }
}

function Invoke-Editor([string]$Mode, [string]$Script = $editorScript) {
    if (Get-Process UnrealEditor, UnrealEditor-Cmd -ErrorAction SilentlyContinue) {
        throw 'An Unreal editor process is already running; leaving it untouched.'
    }
    # The stored job holds repo-relative paths; the editor reads a copy resolved against this checkout (tools/repo_paths.py).
    $resolved = Join-Path $out 'editor_job.resolved.json'
    & $Python (Join-Path $PSScriptRoot 'repo_paths.py') resolve-job (Join-Path $out 'editor_job.json') $resolved
    if ($LASTEXITCODE -ne 0) { throw "the editor job does not resolve in $repo (see the message above)" }
    $env:OPENWILLOW_SLICE_JOB = $resolved
    $env:OPENWILLOW_SLICE_MODE = $Mode
    $log = Join-Path $logs "editor-$Mode.log"
    $report = Join-Path $out "editor_report_$Mode.json"
    if (Test-Path -LiteralPath $report) { Remove-Item -LiteralPath $report -Force }
    Enter-OwUeLock -Owner 'import_phaselock_target' -TimeoutSeconds 0   # fails at once when held, as before
    try {
        $process = Start-Process -FilePath $cmd -PassThru -NoNewWindow -RedirectStandardOutput (Join-Path $logs "editor-$Mode.stdout.txt") -ArgumentList @(
            "`"$project`"", '-run=pythonscript', "`"-script=$Script`"", '-unattended', '-nullrhi', '-nosplash',
            '-ddc=InstalledNoZenLocalFallback', "`"-abslog=$log`"")
        $null = $process.Handle   # without this, Start-Process -PassThru does not populate ExitCode
        if (!$process.WaitForExit(1500000)) { Stop-Process -Id $process.Id -Force; throw "editor mode $Mode timed out" }
        $code = $process.ExitCode
    } finally {
        Exit-OwUeLock
    }
    if ($code -ne 0 -or !(Test-Path -LiteralPath $report)) {
        $why = Select-String -LiteralPath $log -Pattern 'RuntimeError: (.*)' -ErrorAction SilentlyContinue | Select-Object -Last 1
        throw "editor mode $Mode failed (exit $code)$(if ($why) { ': ' + $why.Matches[0].Groups[1].Value }); see $log"
    }
    Select-String -LiteralPath $log -Pattern 'LogPython: OW_' | ForEach-Object { $_.Line -replace '.*LogPython: ', '' }
}

if (& $run 'prepare') { Invoke-Tool 'all' }
if (& $run 'mesh') { Invoke-Editor 'npcs' }
if (& $run 'anims') { Invoke-Tool 'anims'; Invoke-Editor 'anims' }
if (& $run 'zone') { Invoke-Editor 'zone' (Join-Path $PSScriptRoot 'phaselock_target_editor.py') }
if (& $run 'attach') { Invoke-Editor 'attach' (Join-Path $PSScriptRoot 'phaselock_target_attach_editor.py'); Invoke-Tool 'attachments' }
