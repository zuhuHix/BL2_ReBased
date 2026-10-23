[CmdletBinding()]
param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    # tools/weapon_library.py output (meshes, fragments.json, skins.json, library.json); ignored local data.
    [string]$Library = '',
    # 'meshes', 'skins' or '' for both (host/ue5/import_weapon_library.py, OPENWILLOW_WEAPON_LIBRARY_ONLY).
    [ValidateSet('', 'meshes', 'skins', 'none')][string]$Only = '',
    # Optional '<Type>=<prepared reload JSON>' pairs (tools/weapon_reload_anims.py output): the same weapon reload clips
    # imported onto the library gestalt skeletons (host/ue5/import_weapon_reload_anims.py, library target).
    [string[]]$Reloads = @(),
    [string]$HubLock = 'C:\Users\yorad\Documents\BL2_ReBased\local\hub-ue.lock',
    [string]$Lane = 'weaponparts'
)
# AI-assisted. Imports the weapon part library (each gestalt mesh once, every skin as an instance of one master paint
# material) into the ignored /Game/OpenWillow/Weapons/Library. Game-derived inputs stay under local/; nothing is added to
# Git. Waits for the machine-wide hub lock and for any running Unreal editor (never touches it), then holds the worktree's
# local/ue_run.lock while the commandlet runs. Prints the importer's OW_LIBRARY lines; the full report is
# <Library>/import_report.json.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (-not $Library) { $Library = Join-Path $repo 'local/weapon_library' }
$Library = (Resolve-Path $Library).Path
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$cmd = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor-Cmd.exe'
$lock = Join-Path $repo 'local/ue_run.lock'
$logs = Join-Path $Library 'logs'
New-Item -ItemType Directory -Force -Path $logs | Out-Null

$hub = $null
try {
    while (!$hub) {
        try { $hub = New-Item -ItemType File -Path $HubLock -Value "$Lane $((Get-Date).ToString('o'))" -ErrorAction Stop }
        catch { "hub lock held by '$(Get-Content $HubLock -Raw -ErrorAction SilentlyContinue)'; retry in 60 s"; Start-Sleep 60 }
    }
    for ($i = 0; (Get-Process UnrealEditor, UnrealEditor-Cmd -ErrorAction SilentlyContinue) -and $i -lt 360; $i++) {
        if ($i % 30 -eq 0) { 'an Unreal editor is running (not ours); waiting' }; Start-Sleep 10
    }
    if (Get-Process UnrealEditor, UnrealEditor-Cmd -ErrorAction SilentlyContinue) { throw 'an Unreal editor is still running; leaving it untouched' }
    $Reloads = @($Reloads | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim(" '") } | Where-Object { $_ })  # -File passes a list as one string
    # A meshes import deletes each type's folder, reload clips included: re-import the clips recorded by the last reload import.
    $record = Join-Path $Library 'reloads.json'
    if ($Only -in @('', 'meshes') -and -not $Reloads.Count -and (Test-Path $record)) {
        # Paths are repository-relative (an absolute one from an older record is used as is).
        $Reloads = @((Get-Content -Raw $record | ConvertFrom-Json).PSObject.Properties | ForEach-Object {
            $path = if ([IO.Path]::IsPathRooted($_.Value)) { $_.Value } else { Join-Path $repo $_.Value }
            "$($_.Name)=$path" })
        "restoring reload clips after the mesh import: $($Reloads -join ', ')"
    }
    $env:OPENWILLOW_WEAPON_RELOAD_TARGET = 'library'
    $env:OPENWILLOW_WEAPON_RELOAD_ANIMS = ($Reloads | ForEach-Object { $k, $v = $_ -split '=', 2; "$k=$((Resolve-Path $v).Path)" }) -join ';'
    $stream = [System.IO.File]::Open($lock, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read)
    $bytes = [System.Text.Encoding]::UTF8.GetBytes("import_weapon_library $((Get-Date).ToString('o'))")
    $stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
    $env:OPENWILLOW_WEAPON_LIBRARY = $Library
    $env:OPENWILLOW_WEAPON_LIBRARY_ONLY = $Only
    $jobs = @()
    if ($Only -ne 'none') { $jobs += 'import_weapon_library.py' }
    if ($Reloads.Count) { $jobs += 'import_weapon_reload_anims.py' }
    try {
        foreach ($job in $jobs) {
            $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
            $log = Join-Path $logs "$([IO.Path]::GetFileNameWithoutExtension($job))-$stamp.log"
            $process = Start-Process -FilePath $cmd -PassThru -NoNewWindow -RedirectStandardOutput "$log.stdout.txt" -ArgumentList @(
                "`"$project`"", '-run=pythonscript', "`"-script=$(Join-Path $repo "host/ue5/$job")`"", '-unattended',
                '-nullrhi', '-nosplash', "`"-abslog=$log`"")
            $null = $process.Handle   # without this, Start-Process -PassThru does not populate ExitCode
            if (!$process.WaitForExit(5400000)) { Stop-Process -Id $process.Id -Force; throw "$job timed out" }
            Select-String -LiteralPath $log -Pattern 'LogPython: (Error: )?OW_(LIBRARY|WEAPON_ANIM)' | ForEach-Object { $_.Line -replace '.*LogPython: ', '' }
            if ($process.ExitCode -ne 0) { throw "$job failed (exit $($process.ExitCode)); see $log" }
        }
    } finally {
        Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue
    }
} finally {
    if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force }
}
