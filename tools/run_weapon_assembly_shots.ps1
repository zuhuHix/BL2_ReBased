[CmdletBinding()]
param(
    # Recipe folder the host loads as its inventory (-owitems), e.g. tools/weapon_assembly_sample.py's local/wpa/sample.
    [Parameter(Mandatory = $true)][string]$Items,
    # Recipe ids to capture, in order (default: every <id>.json in -Items, in load_order.txt order when present).
    [string[]]$Ids = @(),
    [string]$Out = 'local/wpa/host_shots',
    # Quick: first-person idle and the side view only (-owgunquick); otherwise also ADS, burst and reload frames.
    [switch]$Quick,
    [string[]]$Extra = @(),
    [int]$TimeoutSeconds = 3600,
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$HubLock = 'C:\Users\yorad\Documents\BL2_ReBased\local\hub-ue.lock',
    [string]$Lane = 'weaponparts'
)
# AI-assisted. One windowed host run in Sanctuary that equips each recipe and captures OWGun_<id>_idle.png (first person) and
# OWGun_<id>_side.png (inventory preview capture), plus the assembly's per-gun log lines (OpenWillow weapon assembly ...), which
# tools/weapon_assembly_oracle.py compares with the game's triangle counts. Frames and the log go to -Out (ignored local/).
# Waits for the machine-wide hub lock and for any running Unreal editor (never touches it); holds local/ue_run.lock.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$Items = (Resolve-Path $Items).Path
if (-not $Ids.Count) {
    $order = Join-Path $Items 'load_order.txt'
    $Ids = if (Test-Path $order) { Get-Content $order | ForEach-Object { $_.Trim() } | Where-Object { $_ } }
           else { Get-ChildItem $Items -Filter *.json | Where-Object { $_.BaseName -match '^[A-Za-z0-9_]+$' } | ForEach-Object BaseName }
}
$outDir = Join-Path $repo $Out
New-Item -ItemType Directory -Force $outDir | Out-Null
$shots = Join-Path $repo 'host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor'
$lock = Join-Path $repo 'local/ue_run.lock'
$log = Join-Path $outDir 'run.log'
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
    $stream = [IO.File]::Open($lock, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    $bytes = [Text.Encoding]::UTF8.GetBytes("run_weapon_assembly_shots $((Get-Date).ToString('o'))"); $stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
    try {
        Get-ChildItem $shots -Filter 'OWGun_*.png' -ErrorAction SilentlyContinue | Remove-Item
        if ($env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = (Resolve-Path -LiteralPath $env:OPENWILLOW_BL2).Path }
        $env:OPENWILLOW_SCENE = Join-Path $repo 'local/sanctuary'
        $argsList = @("`"$repo\host\ue5\OpenWillow\OpenWillow.uproject`"", '/Game/OpenWillow/Sanctuary_P/Sanctuary_P', '-owwalk', '-owmaya',
            "-owitems=`"$Items`"", '-owlevel=40', '-owgunshots', ('-owgunids="' + ($Ids -join ',') + '"'),
            '-game', '-windowed', '-ResX=1280', '-ResY=720', '-nosplash', '-ddc=InstalledNoZenLocalFallback', '-d3d11', "`"-abslog=$log`"") + $Extra
        if ($Quick) { $argsList += '-owgunquick' }
        $p = Start-Process -PassThru -FilePath (Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor.exe') -ArgumentList $argsList
        if (!$p.WaitForExit($TimeoutSeconds * 1000)) { Stop-Process -Id $p.Id -Force; "TIMEOUT: stopped our pid $($p.Id)" }
    } finally {
        Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue
    }
    Get-ChildItem $shots -Filter 'OWGun_*.png' -ErrorAction SilentlyContinue | Copy-Item -Destination $outDir -Force
    "frames: $((Get-ChildItem $outDir -Filter '*.png').Count) in $outDir"
    "assembly reports: $((Select-String -LiteralPath $log -Pattern 'OpenWillow weapon assembly ').Count)"
    Select-String -LiteralPath $log -Pattern 'gunshots skipped|Error:|Fatal' | Select-Object -First 20 | ForEach-Object { $_.Line.Substring(0, [Math]::Min(220, $_.Line.Length)) }
} finally {
    if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force }
}
