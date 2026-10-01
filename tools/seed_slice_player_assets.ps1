[CmdletBinding()]
param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    # Any of: fx, items, paint. 'all' runs them in order.
    [ValidateSet('all', 'fx', 'items', 'paint')][string[]]$Steps = @('all'),
    # Inputs for the 'fx' step (host/ue5/import_infinity_proxy.py): a filtered Infinity glTF and the UModel PNG export
    # of its MIC textures (Weap_Pistols_Comp, Weap_LauncherShotgunPistol_Comp, Weap_Pistols_Nrm, Pattern_Infiniti).
    [string]$InfinityGltf = 'local/items/pistol_vladof_5_infinity_3.gltf',
    [string]$InfinityTextures = 'local/external/umodel/weapon-materials-20260930/Startup/Texture2D',
    # Paint input for the 'paint' step (tools/prepare_weapon_paint.py output).
    [string]$Paint = 'local/items/paint/slice_mission_pistol_fire.json'
)
# AI-assisted. Seeds the UE content the slice's player side needs, with existing importers and one additive one;
# nothing here deletes Weapons/Items, Maya's folder or any slice NPC content:
#   fx     host/ue5/import_infinity_proxy.py, only when /Game/OpenWillow/Weapons/InfinityProxy is absent (it replaces
#          that one folder). Provides M_OW_FxAdditive (tracers, flashes, Phaselock shell) and M_OW_BulletHole.
#   items  host/ue5/import_slice_items.py: local/items/slice pool-rolled guns -> Weapons/SliceItems/SK_<id>.
#   paint  host/ue5/import_weapon_paint.py with the prepared paint JSON (repaints the named mesh only).
# Holds local/ue_run.lock for each editor launch and stops if another Unreal editor runs (never touches it).
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$cmd = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor-Cmd.exe'
$lock = Join-Path $repo 'local/ue_run.lock'
$logs = Join-Path $repo 'local/items/logs'
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$run = { param($name) $Steps -contains 'all' -or $Steps -contains $name }

function Invoke-EditorScript([string]$Name, [string]$Script, [hashtable]$Environment) {
    if (Get-Process UnrealEditor, UnrealEditor-Cmd -ErrorAction SilentlyContinue) {
        throw 'An Unreal editor process is already running; leaving it untouched.'
    }
    $stream = [System.IO.File]::Open($lock, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read)
    $bytes = [System.Text.Encoding]::UTF8.GetBytes("seed_slice_player_assets $Name $((Get-Date).ToString('o'))")
    $stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
    try {
        foreach ($key in $Environment.Keys) { Set-Item -Path "env:$key" -Value $Environment[$key] }
        $log = Join-Path $logs "editor-$Name.log"
        $process = Start-Process -FilePath $cmd -PassThru -NoNewWindow -RedirectStandardOutput (Join-Path $logs "editor-$Name.stdout.txt") -ArgumentList @(
            "`"$project`"", '-run=pythonscript', "`"-script=$Script`"", '-unattended', '-nullrhi', '-nosplash',
            '-ddc=InstalledNoZenLocalFallback', "`"-abslog=$log`"")
        $null = $process.Handle   # without this, Start-Process -PassThru does not populate ExitCode
        try {
            if (!$process.WaitForExit(1500000)) { throw "editor step $Name timed out" }
            $code = $process.ExitCode
        } finally {
            if (!$process.HasExited) { Stop-Process -Id $process.Id -Force }
        }
        Select-String -LiteralPath $log -Pattern 'LogPython: (OW_|Error)|Traceback' | ForEach-Object { $_.Line -replace '.*LogPython: ', '' }
        if ($code -ne 0 -or (Select-String -LiteralPath $log -Pattern 'Traceback|LogPython: Error' -Quiet)) { throw "editor step $Name failed (exit $code); see $log" }
    } finally { Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue }
}

$full = { param($path) (Resolve-Path -LiteralPath (Join-Path $repo $path)).Path }
if (& $run 'fx') {
    if (Test-Path -LiteralPath (Join-Path $repo 'host/ue5/OpenWillow/Content/OpenWillow/Weapons/InfinityProxy')) {
        Write-Output 'fx: Weapons/InfinityProxy already exists; not replaced'
    } else {
        Invoke-EditorScript 'fx' (Join-Path $repo 'host/ue5/import_infinity_proxy.py') @{
            OPENWILLOW_PISTOL_GLTF = (& $full $InfinityGltf); OPENWILLOW_INFINITY_TEXTURES = (& $full $InfinityTextures) }
    }
}
if (& $run 'items') {
    Invoke-EditorScript 'items' (Join-Path $repo 'host/ue5/import_slice_items.py') @{ OPENWILLOW_ITEMS = (& $full 'local/items/slice') }
}
if (& $run 'paint') {
    Invoke-EditorScript 'paint' (Join-Path $repo 'host/ue5/import_weapon_paint.py') @{ OPENWILLOW_WEAPON_PAINT = (& $full $Paint) }
}
