[CmdletBinding()]
param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    # UModel build 1590 (external, never inside the repository).
    [string]$UModel = $env:OPENWILLOW_UMODEL,
    [string]$Python = 'python',
    # tools/prepare_interactives.py output (ignored); default local/slice/interactives.json.
    [string]$Interactives = '',
    # plan: writes local/lootables/plan.json (always runs first); prepare: UModel exports; mesh: editor meshes and pickups;
    # tracks: MD5 clips -> bone tracks (python); anims: editor AnimSequences; report: import_report.json.
    [ValidateSet('all', 'prepare', 'mesh', 'tracks', 'anims', 'report')][string[]]$Steps = @('all'),
    # 0 = refuse at once when the UE run lock is held by another owner (the default).
    [int]$WaitForEditorSeconds = 0,
    # Prints the plan and every command that would run. Launches nothing (no UModel, no UnrealEditor).
    [switch]$DryRun
)
# AI-assisted. Imports the Sanctuary lootables (cardboard boxes, strong boxes, storage lockers, laundry machines,
# dumpsters, the Dahl ammo crate), their pickup meshes (ammo, cash, eridium, health vial) into the ignored UE Content
# folders /Game/OpenWillow/Lootables and /Game/OpenWillow/Pickups. The two unsupported chests are skipped.
#
# Steps (see the parameter comments): plan -> prepare (UModel) -> mesh (editor) -> tracks (python) -> anims (editor)
# -> report. Every game-derived file stays under the ignored local/lootables/ (UModel output in local/lootables/umodel/).
# The editor runs under the machine-wide UE run lock (tools/ue_lock.ps1), taken with -WaitForEditorSeconds 0 by default,
# so the script refuses at once when another owner holds it. Nothing is added to Git.
#
# The -DryRun switch writes only the plan file and prints the commands. Use it to check the planned asset list.
# UNVERIFIED: the imported assets have not been run in the game; the glTF and MD5 paths are first-use paths.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$cmd = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor-Cmd.exe'
. (Join-Path $PSScriptRoot 'ue_lock.ps1')
$out = Join-Path $repo 'local/lootables'
$logs = Join-Path $out 'logs'
$tool = Join-Path $PSScriptRoot 'lootables_editor.py'
$planPath = Join-Path $out 'plan.json'
$jobsPath = Join-Path $out 'umodel_jobs.json'
$reportPath = Join-Path $out 'import_report.json'
if (-not $Interactives) { $Interactives = Join-Path $repo 'local/slice/interactives.json' }
$run = { param($name) $Steps -contains 'all' -or $Steps -contains $name }

function Get-SafeName([string]$Text) { return ($Text -replace '[^A-Za-z0-9_.-]', '_') }

function Invoke-Plan {
    New-Item -ItemType Directory -Force -Path $out, $logs | Out-Null
    if (-not (Test-Path -LiteralPath $Interactives)) { throw "Interactives file not found: $Interactives (run tools/prepare_interactives.py)" }
    # Python's own output goes to the host, so the function returns only the plan object.
    $planArguments = @('plan', '--interactives', $Interactives, '--out', $planPath)
    if ($Game) { $planArguments += @('--cooked', (Join-Path $Game 'WillowGame/CookedPCConsole')) }
    & $Python $tool @planArguments | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "lootables_editor.py plan failed: $LASTEXITCODE" }
    return (Get-Content -LiteralPath $planPath -Raw | ConvertFrom-Json)
}

function Get-UModelArguments($job, [string]$cooked) {
    # One export per job. AnimSet and mesh jobs have their own output folder, so UModel's -groups is not needed.
    return @("-path=$cooked", '-game=border', '-export', "-$($job.format)", '-nooverwrite', '-uncook',
        "-out=$($job.out)", $job.package, $job.object, $job.class)
}

function Show-Plan($plan) {
    Write-Output "Plan: $planPath"
    Write-Output "Skeletal meshes ($(@($plan.skeletal_meshes).Count)):"
    foreach ($m in $plan.skeletal_meshes) { Write-Output "  $($m.path)  ->  $($m.dest)  (objects: $(@($m.objects).Count))" }
    Write-Output "Anim sets ($(@($plan.anim_sets).Count)):"
    foreach ($a in $plan.anim_sets) { Write-Output "  $($a.path)  clips: $(@($a.clips) -join ', ')  missing: $(@($a.missing_clips) -join ', ')" }
    Write-Output "Pickup meshes ($(@($plan.pickup_meshes).Count)):"
    foreach ($p in $plan.pickup_meshes) { Write-Output "  $($p.path)  ->  $($p.dest)" }
    Write-Output "Skipped objects: $(@($plan.skipped_objects).Count)"
    foreach ($s in $plan.skipped_objects) { Write-Output "  $($s.id): $($s.reason)" }
}

function Invoke-Prepare($plan) {
    $cookedText = if ($Game) { Join-Path $Game 'WillowGame/CookedPCConsole' } else { '<OPENWILLOW_BL2>/WillowGame/CookedPCConsole' }
    $umodelText = if ($UModel) { $UModel } else { '<umodel.exe>' }
    if ($DryRun) {
        foreach ($job in $plan.exports) {
            Write-Output ("DRY RUN UModel [{0}]: {1} {2}" -f $job.id, $umodelText, ((Get-UModelArguments $job $cookedText) -join ' '))
        }
        return
    }
    if (-not $UModel) { throw 'Pass -UModel or set OPENWILLOW_UMODEL (UModel build 1590; never inside the repository).' }
    $umodelPath = (Resolve-Path -LiteralPath $UModel).Path
    if ([IO.Path]::GetFileName($umodelPath) -notmatch '^umodel(\.exe)?$') { throw 'UModel must point to umodel.exe' }
    if (-not $Game) { throw 'Pass -Game or set OPENWILLOW_BL2' }
    $cooked = (Resolve-Path -LiteralPath (Join-Path $Game 'WillowGame/CookedPCConsole')).Path
    if (!(Test-Path -LiteralPath (Join-Path $cooked 'Ash_P.upk'))) { throw 'Cooked path does not look like Borderlands 2 CookedPCConsole' }
    $records = @()
    foreach ($job in $plan.exports) {
        New-Item -ItemType Directory -Force -Path $job.out | Out-Null
        $log = Join-Path $logs ("umodel-" + (Get-SafeName $job.id) + '.log')
        $arguments = Get-UModelArguments $job $cooked
        $watch = [Diagnostics.Stopwatch]::StartNew()
        # UModel writes to stderr too; keep PowerShell from turning that into a terminating error.
        $previous = $ErrorActionPreference
        $ErrorActionPreference = 'Continue'
        try { & $umodelPath @arguments *> $log; $code = $LASTEXITCODE } finally { $ErrorActionPreference = $previous }
        $watch.Stop()
        $files = @(Get-ChildItem -LiteralPath $job.out -Recurse -File -ErrorAction SilentlyContinue)
        $bytes = [int64](($files | Measure-Object -Property Length -Sum).Sum)
        $records += [pscustomobject]@{
            id = $job.id; format = $job.format; class = $job.class; object = $job.object; exit_code = $code
            seconds = [math]::Round($watch.Elapsed.TotalSeconds, 2); files = $files.Count; bytes = $bytes; log = $log }
        Write-Output "umodel $($job.id): exit $code, $($files.Count) files, $([math]::Round($watch.Elapsed.TotalSeconds, 1)) s"
    }
    ConvertTo-Json -InputObject @($records) -Depth 4 | Set-Content -LiteralPath $jobsPath -Encoding UTF8
    $failed = @($records | Where-Object { $_.exit_code -ne 0 -or $_.files -eq 0 })
    Write-Output "prepare: $($records.Count) UModel jobs, $($failed.Count) without output (see $jobsPath)"
    # Nothing exported means nothing to import: stop before spending an editor launch on it.
    if ($records.Count -gt 0 -and $failed.Count -eq $records.Count) { throw "All $($records.Count) UModel jobs failed; see $logs" }
}

function Invoke-LootEditor([string]$Mode, [string]$ReportName) {
    $editorArguments = @("`"$project`"", '-run=pythonscript', "`"-script=$tool`"", '-unattended', '-nullrhi', '-nosplash',
        '-ddc=InstalledNoZenLocalFallback')
    if ($DryRun) {
        Write-Output "DRY RUN editor [$Mode]: env OPENWILLOW_LOOT_MODE=$Mode OPENWILLOW_LOOT_PLAN=$planPath"
        Write-Output ("DRY RUN editor [$Mode]: $cmd " + ($editorArguments -join ' ') + " -abslog=$(Join-Path $logs "editor-$Mode.log")")
        Write-Output "DRY RUN editor [$Mode]: would take the UE run lock with a wait of $WaitForEditorSeconds s"
        return
    }
    if (!(Test-Path -LiteralPath $cmd)) { throw "UnrealEditor-Cmd not found: $cmd (pass -Engine)" }
    $log = Join-Path $logs "editor-$Mode.log"
    $stdout = Join-Path $logs "editor-$Mode.stdout.txt"
    $reportFile = Join-Path $out $ReportName
    if (Test-Path -LiteralPath $reportFile) { Remove-Item -LiteralPath $reportFile -Force }
    $env:OPENWILLOW_LOOT_MODE = $Mode
    $env:OPENWILLOW_LOOT_PLAN = $planPath
    # Refuses (throws) when another owner holds the lock or an Unreal editor runs; it never touches that process.
    Enter-OwUeLock -Owner 'import_lootables' -TimeoutSeconds $WaitForEditorSeconds
    try {
        $process = Start-Process -FilePath $cmd -PassThru -NoNewWindow -RedirectStandardOutput $stdout `
            -ArgumentList ($editorArguments + "`"-abslog=$log`"")
        $null = $process.Handle   # without this, Start-Process -PassThru does not populate ExitCode
        if (!$process.WaitForExit(1500000)) {
            # Stop only the editor this run started, and only after checking its command line.
            $own = Get-CimInstance Win32_Process -Filter "ProcessId=$($process.Id)" -ErrorAction SilentlyContinue
            if ($own -and $own.CommandLine -like '*UnrealEditor-Cmd*') { Stop-Process -Id $process.Id -Force }
            throw "editor mode $Mode timed out"
        }
        $code = $process.ExitCode
    } finally {
        Exit-OwUeLock
        Remove-Item Env:OPENWILLOW_LOOT_MODE -ErrorAction SilentlyContinue
        Remove-Item Env:OPENWILLOW_LOOT_PLAN -ErrorAction SilentlyContinue
    }
    if (Test-Path -LiteralPath $log) {
        Select-String -LiteralPath $log -Pattern 'LogPython: (OW_LOOT|Error)|Traceback' | ForEach-Object { $_.Line -replace '.*LogPython: ', '' }
    }
    if ($code -ne 0 -or !(Test-Path -LiteralPath $reportFile)) { throw "editor mode $Mode failed (exit $code); see $log" }
    Write-Output "editor ${Mode}: report $reportFile"
}

# ----------------------------------------------------------------------------------------- main

New-Item -ItemType Directory -Force -Path $out, $logs | Out-Null
$plan = Invoke-Plan
if ($DryRun) {
    Show-Plan $plan
    Write-Output ''
    Write-Output 'DRY RUN: the plan file is written; nothing below is launched.'
}
if (& $run 'prepare') {
    if ($DryRun) { Write-Output 'Commands for prepare (UModel exports):' }
    Invoke-Prepare $plan
}
if (& $run 'mesh') {
    if ($DryRun) { Write-Output 'Commands for mesh (editor, meshes mode):' }
    Invoke-LootEditor 'meshes' 'editor_meshes.json'
}
if (& $run 'tracks') {
    if ($DryRun) {
        Write-Output "DRY RUN tracks: $Python $tool tracks --plan $planPath"
    } else {
        & $Python $tool tracks --plan $planPath
        if ($LASTEXITCODE -ne 0) { throw "lootables_editor.py tracks failed: $LASTEXITCODE" }
    }
}
if (& $run 'anims') {
    if ($DryRun) { Write-Output 'Commands for anims (editor, anims mode):' }
    if (-not $DryRun -and !(Test-Path -LiteralPath (Join-Path $out 'editor_meshes.json'))) {
        throw 'anims needs local/lootables/editor_meshes.json (run the mesh step first)'
    }
    Invoke-LootEditor 'anims' 'editor_anims.json'
}
if (& $run 'report') {
    if ($DryRun) {
        Write-Output "DRY RUN report: $Python $tool report --plan $planPath --umodel-jobs $jobsPath --out $reportPath"
    } else {
        & $Python $tool report --plan $planPath --umodel-jobs $jobsPath --out $reportPath
        if ($LASTEXITCODE -ne 0) { throw "lootables_editor.py report failed: $LASTEXITCODE" }
    }
}
